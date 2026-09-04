//! # handl-runtime
//!
//! Binds a [`Language`] to engine impls and exposes staged `lex`/`parse`/`eval`
//! plus `compile`/`run_compiled` over a `Language` + stage inputs
//! (ADR-0044 §3, §7; ADR-0047 §2).
//!
//! A [`Runtime`] owns a [`Language`] and optional engine impls for each stage
//! (lex, parse, eval) plus a map of compile engines. Each stage dispatches to
//! its bound engine or returns [`LangError::NotImplemented`] if no engine is
//! attached. This enables **eval-only runtimes** (a JIT/cranelift runtime that
//! compiles a pre-parsed [`Ast`]) as well as full `lex → parse → eval`
//! pipelines.
//!
//! ## Registry-based engine selection (ADR-0047 §2)
//!
//! Engine selection is **registry-based**, not a hardcoded `from_config`
//! match. The host populates an [`EngineRegistry`] — a name→engine map — and
//! [`Runtime::from_registry`] resolves the [`RuntimeConfig`] names against it.
//! `handl-runtime` names **no** engine impl crate; the engine impls
//! (`plugin-engine-lex-pattern`, `plugin-engine-parse-pratt`,
//! `plugin-engine-eval-tree-walk`, `plugin-engine-eval-bytecode`,
//! `plugin-engine-eval-cranelift`) are deps of the **host**, which registers
//! them. Adding an engine = register it; no `handl-runtime` edit.
//!
//! [`RuntimeConfig`] carries engine names as strings (e.g. `"pratt"`,
//! `"tree-walk"`); an empty string leaves that stage unbound. Unknown
//! non-empty names return [`LangError::Engine`].
//!
//! ## `RuntimeFactory` / `RuntimeHandle` (ADR-0047 §2)
//!
//! The [`RuntimeFactory`] / [`RuntimeHandle`] trait pair lets (future)
//! `plugin-lib-lang-*` leaf crates depend on `handl-runtime` without reaching
//! up to engines: a leaf holds a `&dyn RuntimeFactory` injected by the host,
//! calls `factory.build(lang, delegate)` to obtain a `Box<dyn RuntimeHandle>`,
//! and dispatches `lex`/`parse`/`eval`/`compile`/`run_compiled` through it.
//! [`Runtime`] implements both traits; the factory re-injects the meta
//! functions into the sub-runtime itself, breaking the cycle.
//!
//! ## `compile` / `run_compiled` via `CompiledHandle` + `CompiledArtifact` (ADR-0047 §3)
//!
//! `Runtime::compile(ast, target)` asks the registered [`CompileEngine`] for
//! `target` to produce a `Box<dyn CompiledArtifact>`, stores it in the
//! thread-local [`COMPILED_REGISTRY`], and returns a serializable
//! [`CompiledHandle`]. `Runtime::run_compiled(handle)` looks up the artifact
//! and runs it. The non-serializable artifact never crosses the trait
//! boundary; only the serializable handle does. The `CompileEngine` /
//! `CompiledArtifact` impls live in the engine crates
//! (`plugin-engine-eval-bytecode`, `plugin-engine-eval-cranelift`), not here.
//!
//! ## Dyn-compatibility note
//!
//! [`ParserEngine`] is not dyn-compatible (its `expression` method references
//! `Self` via [`ParserContext`]). The [`ParseEngineDyn`] wrapper trait
//! exposes only the dyn-compatible `parse` method, allowing parse engines to
//! be stored as trait objects. [`LexerEngine`] and [`EvalEngine`] are already
//! dyn-compatible and need no wrapper.

#![warn(missing_docs)]

pub mod desugar_pass;
pub mod fp;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use handl_language::{Ast, LangError, Language, Source, Tokens, Value};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_lex::{LexerEngine, LexerRequest};
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
/// Re-export [`TypecheckResult`] so downstream mock [`RuntimeHandle`]
/// implementations can name it without a direct dependency on
/// `handl-plugin-engine-typecheck` (ADR-0054).
pub use handl_plugin_engine_typecheck::TypecheckResult;
use handl_plugin_engine_typecheck::{TypecheckEngine, TypecheckRequest};

// ---------------------------------------------------------------------------
// Compiled-handle / artifact / compile-engine traits (ADR-0047 §3)
// ---------------------------------------------------------------------------

/// A serializable handle to a compiled program.
///
/// `Runtime::compile` produces a `CompiledHandle` referencing a thread-local
/// [`COMPILED_REGISTRY`] entry holding the non-serializable compiled
/// artifact. The handle itself is a serializable value (it crosses the
/// `plugin-lib-compile` / `plugin-lib-run-compiled` trait boundary); the
/// artifact is not serializable (native code, bytecode internal state) and
/// stays in the registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompiledHandle {
    /// The registry id referencing the compiled artifact.
    pub id: u64,
    /// The compile target name (e.g. `"bytecode"`, `"native"`).
    pub target: String,
}

/// A compiled artifact — engine-specific, non-serializable, held in the
/// [`COMPILED_REGISTRY`] behind a trait object.
///
/// Each compile engine (`plugin-engine-eval-bytecode`,
/// `plugin-engine-eval-cranelift`) produces its own impl carrying the
/// compiled artifact plus the `Language` and `Ast` it was compiled from
/// (needed to execute: the bytecode VM lazily compiles child nodes using
/// `lang` + `ast`; the cranelift helpers read `ast` node payloads and `lang`
/// at execution time). `run` executes the artifact using the supplied
/// module registry (for `Foreign` dispatch,
/// re-attached by the runtime so the compiled program can recurse via the
/// meta host functions).
pub trait CompiledArtifact {
    /// Execute this compiled artifact, returning the result value.
    ///
    /// `modules` is the module registry to use for `Foreign`
    /// dispatch (the runtime re-attaches the meta host functions so a
    /// compiled program can call `eval`/`compile`/`run_compiled`).
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on execution failure.
    fn run(
        &self,
        modules: Option<Rc<dyn handl_platform::ModuleLookup>>,
    ) -> Result<Value, LangError>;

    /// Produce an owned boxed clone of this artifact.
    ///
    /// Used by `Runtime::run_compiled` to take an owned copy out of the
    /// [`COMPILED_REGISTRY`] under a short-lived borrow, then release the
    /// borrow before executing — avoiding re-entrancy panics if the
    /// compiled program itself calls `compile`/`run_compiled`.
    fn box_clone(&self) -> Box<dyn CompiledArtifact>;
}

/// A compile engine — produces [`CompiledArtifact`] impls from an [`Ast`].
///
/// Impl live in the engine crates (`plugin-engine-eval-bytecode`,
/// `plugin-engine-eval-cranelift`), not in `handl-runtime`. The runtime
/// looks one up by target name in the [`EngineRegistry`] and calls
/// `compile` to obtain a `Box<dyn CompiledArtifact>`.
pub trait CompileEngine {
    /// Compile `ast` under `lang` to the requested `target`, returning a
    /// boxed [`CompiledArtifact`].
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on compilation failure (unsupported scheme
    /// version, unsupported construct, etc.).
    fn compile(
        &self,
        lang: &Language,
        ast: &Ast,
        target: &str,
    ) -> Result<Box<dyn CompiledArtifact>, LangError>;

    /// Produce an owned boxed clone of this engine.
    ///
    /// Used by `Runtime::from_registry` to clone the compile engines out of
    /// the [`EngineRegistry`] into the `Runtime`.
    fn box_clone(&self) -> Box<dyn CompileEngine>;
}

// ---------------------------------------------------------------------------
// ParseEngineDyn — dyn-compatible wrapper around ParserEngine
// ---------------------------------------------------------------------------

/// A dyn-compatible wrapper around [`ParserEngine`].
///
/// [`ParserEngine`] is not dyn-compatible because its `expression` method
/// takes a [`ParserContext`] parameterized by `Self`. This wrapper exposes
/// only the [`parse`](ParserEngine::parse) method, which is dyn-compatible,
/// allowing parse engines to be stored as `Box<dyn ParseEngineDyn>`.
///
/// Any `Clone` type implementing [`ParserEngine`] automatically implements
/// this trait via a blanket impl. The `Clone` bound is required so the
/// registry can hand out owned clones (ADR-0047 §2).
pub trait ParseEngineDyn {
    /// Parse a token stream into an [`Ast`] using the given language.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on parse failure.
    fn parse_dyn(&self, lang: &Language, req: &ParserRequest) -> Result<Ast, LangError>;

    /// Produce an owned boxed clone of this engine.
    ///
    /// Used by `Runtime::from_registry` to clone the parse engine out of the
    /// [`EngineRegistry`] into the `Runtime`.
    fn box_clone(&self) -> Box<dyn ParseEngineDyn>;
}

impl<E: ParserEngine + Clone + 'static> ParseEngineDyn for E {
    fn parse_dyn(&self, lang: &Language, req: &ParserRequest) -> Result<Ast, LangError> {
        ParserEngine::parse(self, lang, req)
    }

    fn box_clone(&self) -> Box<dyn ParseEngineDyn> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// EngineRegistry — name→engine map populated by the host (ADR-0047 §2)
// ---------------------------------------------------------------------------

/// A name→engine map, populated by the host. `handl-runtime` looks up by
/// name; it names no engine crate. Multiple engines per stage coexist.
///
/// The host builds an `EngineRegistry`, registers the engines it wants
/// available, and passes it to [`Runtime::from_registry`] /
/// [`Runtime::from_registry_with_globals`] (and to
/// [`RuntimeFactoryImpl::new`]). The registry is the indirection that
/// makes name-based engine selection composable without `handl-runtime`
/// hardcoding any match.
pub struct EngineRegistry {
    /// Lex engines keyed by name.
    lex: HashMap<String, Box<dyn LexerEngine>>,
    /// Parse engines keyed by name (wrapped for dyn compatibility).
    parse: HashMap<String, Box<dyn ParseEngineDyn>>,
    /// Eval engines keyed by name.
    eval: HashMap<String, Box<dyn EvalEngine>>,
    /// Typecheck engines keyed by name (ADR-0054).
    typecheck: HashMap<String, Box<dyn TypecheckEngine>>,
    /// Compile engines keyed by target name.
    compile: HashMap<String, Box<dyn CompileEngine>>,
}

impl std::fmt::Debug for EngineRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineRegistry")
            .field("lex", &self.lex.keys().collect::<Vec<_>>())
            .field("parse", &self.parse.keys().collect::<Vec<_>>())
            .field("eval", &self.eval.keys().collect::<Vec<_>>())
            .field("typecheck", &self.typecheck.keys().collect::<Vec<_>>())
            .field("compile", &self.compile.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Default for EngineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            lex: HashMap::new(),
            parse: HashMap::new(),
            eval: HashMap::new(),
            typecheck: HashMap::new(),
            compile: HashMap::new(),
        }
    }

    /// Register a lex engine under `name`.
    pub fn register_lex(&mut self, name: impl Into<String>, engine: Box<dyn LexerEngine>) {
        self.lex.insert(name.into(), engine);
    }

    /// Register a parse engine under `name`.
    ///
    /// Accepts any `Clone` type implementing [`ParserEngine`]; the engine is
    /// stored via the dyn-compatible [`ParseEngineDyn`] wrapper.
    pub fn register_parse<E: ParserEngine + Clone + 'static>(
        &mut self,
        name: impl Into<String>,
        engine: E,
    ) {
        self.parse.insert(name.into(), Box::new(engine));
    }

    /// Register an eval engine under `name`.
    pub fn register_eval(&mut self, name: impl Into<String>, engine: Box<dyn EvalEngine>) {
        self.eval.insert(name.into(), engine);
    }

    /// Register a typecheck engine under `name` (ADR-0054).
    pub fn register_typecheck(
        &mut self,
        name: impl Into<String>,
        engine: Box<dyn TypecheckEngine>,
    ) {
        self.typecheck.insert(name.into(), engine);
    }

    /// Register a compile engine under `target` name.
    pub fn register_compile(&mut self, name: impl Into<String>, engine: Box<dyn CompileEngine>) {
        self.compile.insert(name.into(), engine);
    }

    /// Look up a lex engine by name.
    pub fn lex_engine(&self, name: &str) -> Option<&dyn LexerEngine> {
        self.lex.get(name).map(|e| e.as_ref())
    }

    /// Look up a parse engine by name.
    pub fn parse_engine(&self, name: &str) -> Option<&dyn ParseEngineDyn> {
        self.parse.get(name).map(|e| e.as_ref())
    }

    /// Look up an eval engine by name.
    pub fn eval_engine(&self, name: &str) -> Option<&dyn EvalEngine> {
        self.eval.get(name).map(|e| e.as_ref())
    }

    /// Look up a typecheck engine by name (ADR-0054).
    pub fn typecheck_engine(&self, name: &str) -> Option<&dyn TypecheckEngine> {
        self.typecheck.get(name).map(|e| e.as_ref())
    }

    /// Look up a compile engine by target name.
    pub fn compile_engine(&self, name: &str) -> Option<&dyn CompileEngine> {
        self.compile.get(name).map(|e| e.as_ref())
    }

    /// Return the target names of all registered compile engines.
    pub fn compile_targets(&self) -> Vec<String> {
        self.compile.keys().cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// RuntimeConfig — name-based engine selection (ADR-0047 §2)
// ---------------------------------------------------------------------------

/// Name-based engine selection for a [`Runtime`], resolved against an
/// [`EngineRegistry`] (ADR-0047 §2).
///
/// Each field is an engine name. An empty string leaves that stage's engine
/// unbound (the stage returns [`LangError::NotImplemented`] at runtime). A
/// non-empty name requests a specific engine, resolved by
/// [`Runtime::from_registry`] against the registry at build time.
///
/// This is serializable so a kit manifest or runtime configuration file can
/// carry it as data — swapping an engine is a config change, not a recompile.
/// Per ADR §2, the fields are required `String`s (not `Option<String>`);
/// `RuntimeConfig::default()` has empty strings (callers check for emptiness
/// → unbound stage).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// The lex engine name (e.g. `"data"` for the data-driven lexer engine).
    pub lex: String,
    /// The parse engine name (e.g. `"pratt"`).
    pub parse: String,
    /// The eval engine name (e.g. `"tree-walk"`, `"bytecode"`, `"cranelift"`).
    pub eval: String,
    /// The typecheck engine name (e.g. `"nbe"`). Empty ⇒ unbound
    /// (ADR-0054).
    pub typecheck: String,
}

impl RuntimeConfig {
    /// Create an empty config with no engines selected (all stages unbound).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the lex engine name (builder style).
    pub fn with_lex(mut self, name: impl Into<String>) -> Self {
        self.lex = name.into();
        self
    }

    /// Set the parse engine name (builder style).
    pub fn with_parse(mut self, name: impl Into<String>) -> Self {
        self.parse = name.into();
        self
    }

    /// Set the eval engine name (builder style).
    pub fn with_eval(mut self, name: impl Into<String>) -> Self {
        self.eval = name.into();
        self
    }

    /// Set the typecheck engine name (builder style, ADR-0054).
    pub fn with_typecheck(mut self, name: impl Into<String>) -> Self {
        self.typecheck = name.into();
        self
    }
}

// ---------------------------------------------------------------------------
// RuntimeFactory / RuntimeHandle traits (ADR-0047 §2)
// ---------------------------------------------------------------------------

/// Builds a [`RuntimeHandle`] for a [`Language`], with meta re-attached so the
/// sub-runtime can recurse.
///
/// The factory carries the registry + config and re-injects them into
/// sub-runtimes. It also injects `self` as a global variable bound to the
/// [`Language`] value (ADR-0041 Phase 3) — this is not a parameter; the
/// factory does it automatically. The (future) `plugin-lib-lang-*` leaf
/// crates hold a `&dyn RuntimeFactory` injected by the host and call
/// `factory.build(...)` per invocation.
pub trait RuntimeFactory {
    /// Build a [`RuntimeHandle`] for `lang`, re-attaching the meta functions
    /// (so the sub-runtime can recurse) and injecting `self` as a global.
    ///
    /// `delegate` is an optional user-supplied [`handl_platform::ModuleLookup`]
    /// for non-meta host functions.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if the runtime cannot be built (unknown engine
    /// name, etc.).
    fn build(
        &self,
        lang: Language,
        delegate: Option<Rc<dyn handl_platform::ModuleLookup>>,
    ) -> Result<Box<dyn RuntimeHandle>, LangError>;
}

/// A handle to a runtime bound to a [`Language`]. All stage calls go through
/// this trait; the impl owns the engines and the [`COMPILED_REGISTRY`]
/// access.
pub trait RuntimeHandle {
    /// Lex source text into a token stream.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on lex failure or if no lex engine is bound.
    fn lex(&self, src: &Source) -> Result<Tokens, LangError>;

    /// Parse a token stream into an [`Ast`].
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on parse failure or if no parse engine is bound.
    fn parse(&self, toks: &Tokens) -> Result<Ast, LangError>;

    /// Evaluate an [`Ast`] into a [`Value`].
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on eval failure or if no eval engine is bound.
    fn eval(&self, ast: &Ast) -> Result<Value, LangError>;

    /// Typecheck an [`Ast`], returning the synthesized type (ADR-0054).
    ///
    /// This is the **pure-eval** path: type expressions are evaluated via
    /// the bound typecheck engine (which delegates value-level computation
    /// to a pure eval engine) without running effects. Returns the
    /// synthesized type (a value — types are values, ADR-0032) plus
    /// diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on typecheck failure or if no typecheck
    /// engine is bound.
    fn typecheck(&self, ast: &Ast) -> Result<TypecheckResult, LangError>;

    /// Compile an [`Ast`] to `target`, returning a serializable
    /// [`CompiledHandle`].
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on compile failure or if no compile engine is
    /// registered for `target`.
    fn compile(&self, ast: &Ast, target: &str) -> Result<CompiledHandle, LangError>;

    /// Run a previously compiled program referenced by `handle`.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on execution failure or if the handle is
    /// unknown.
    fn run_compiled(&self, handle: &CompiledHandle) -> Result<Value, LangError>;
}

// ---------------------------------------------------------------------------
// RuntimeFactoryImpl — breaks the meta-module / Runtime chicken-and-egg
// (ADR-0047 §2)
// ---------------------------------------------------------------------------

/// A [`RuntimeFactory`] that re-attaches a meta chain to sub-runtimes.
///
/// Holds an [`EngineRegistry`] + [`RuntimeConfig`] + a settable meta chain
/// (the `Rc<dyn ModuleLookup>` that provides `lex`/`parse`/`eval`/`compile`/
/// `run_compiled`). The meta chain is set after construction via
/// [`RuntimeFactoryImpl::set_meta_chain`], breaking the chicken-and-egg
/// between the `Runtime` (which needs `modules` at construction) and the
/// meta modules (which need `Rc<dyn RuntimeFactory>` at construction).
///
/// `build` creates a sub-`Runtime` with the meta chain re-attached (so the
/// sub-runtime can recurse via `Foreign` dispatch) and `self` injected as a
/// global bound to the [`Language`] value (ADR-0041 Phase 3).
///
/// The `delegate` parameter to [`RuntimeFactory::build`] is intentionally
/// ignored — the meta chain (set via [`set_meta_chain`](Self::set_meta_chain))
/// already includes the user delegate layered in. This is because the meta
/// modules pass their `delegate` (the user delegate or chain tail) to
/// `factory.build`, but `build` uses the pre-set meta chain (which already
/// has the user delegate). This is correct because the user delegate doesn't
/// change across recursion levels.
pub struct RuntimeFactoryImpl {
    registry: Rc<EngineRegistry>,
    config: RuntimeConfig,
    meta_chain: std::cell::RefCell<Option<Rc<dyn handl_platform::ModuleLookup>>>,
}

impl RuntimeFactoryImpl {
    /// Create a factory backed by `registry` + `config`, with no meta chain
    /// yet. Call [`set_meta_chain`](Self::set_meta_chain) before `build` is
    /// invoked.
    pub fn new(registry: Rc<EngineRegistry>, config: RuntimeConfig) -> Self {
        Self {
            registry,
            config,
            meta_chain: std::cell::RefCell::new(None),
        }
    }

    /// Set the meta chain that `build` re-attaches to each sub-runtime.
    pub fn set_meta_chain(&self, chain: Rc<dyn handl_platform::ModuleLookup>) {
        *self.meta_chain.borrow_mut() = Some(chain);
    }
}

impl RuntimeFactory for RuntimeFactoryImpl {
    fn build(
        &self,
        lang: Language,
        _delegate: Option<Rc<dyn handl_platform::ModuleLookup>>,
    ) -> Result<Box<dyn RuntimeHandle>, LangError> {
        let modules = self.meta_chain.borrow().clone();
        let self_val = handl_language::language_value(&lang);
        let mut globals = HashMap::new();
        globals.insert("self".to_string(), self_val);
        let rt = Runtime::from_registry_with_globals(
            lang,
            &self.registry,
            &self.config,
            modules,
            Some(&globals),
        )?;
        Ok(Box::new(rt))
    }
}

// ---------------------------------------------------------------------------
// COMPILED_REGISTRY — thread-local store of compiled artifacts (ADR-0047 §3)
// ---------------------------------------------------------------------------

/// The sentinel object key marking a [`Value`] as a compiled-program handle.
///
/// A compiled-program handle is a JSON object
/// `{"__compiled__": <id>, "target": <target>}`. The `id` references an entry
/// in the thread-local [`COMPILED_REGISTRY`] carrying the non-serializable
/// compiled artifact. The handle itself is a serializable value; the artifact
/// is not — native code cannot be serialized into `serde_json::Value`.
pub const COMPILED_VALUE_KEY: &str = "__compiled__";

// Thread-local registry of compiled artifacts, keyed by handle id.
//
// The artifacts are non-serializable (bytecode `Instr` vectors are
// serializable in principle, but cranelift `CompiledResult` holds native
// function pointers and a `JITModule` — not `serde`-serializable, not
// `Send`). A thread-local `RefCell` matches the engines' single-threaded
// execution model and avoids `Send` bounds the artifacts cannot meet.
thread_local! {
    static COMPILED_REGISTRY: RefCell<HashMap<u64, Box<dyn CompiledArtifact>>> =
        RefCell::new(HashMap::new());
}

/// Monotonic id generator for compiled-program handles.
static COMPILED_NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Build a compiled-program handle [`Value`] for `id` and `target`.
pub fn compiled_value(id: u64, target: &str) -> Value {
    serde_json::json!({
        COMPILED_VALUE_KEY: id,
        "target": target,
    })
}

/// Extract a compiled-program handle id from a [`Value`].
///
/// Returns `None` if the value is not a tagged compiled-program handle.
pub fn as_compiled_id(value: &Value) -> Option<u64> {
    value.get(COMPILED_VALUE_KEY).and_then(|v| v.as_u64())
}

/// Extract the target string from a compiled-program handle [`Value`].
pub fn as_compiled_target(value: &Value) -> Option<&str> {
    value.get("target").and_then(|v| v.as_str())
}

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

/// Binds a [`Language`] to engine impls and exposes staged `lex`/`parse`/`eval`
/// plus `compile`/`run_compiled` (ADR-0044 §3, ADR-0047 §2).
///
/// A `Runtime` owns the [`Language`], optional engine trait objects for each
/// stage, a map of compile engines, and (for re-building sub-runtimes via
/// [`RuntimeFactory`]) the [`EngineRegistry`] and [`RuntimeConfig`] it was
/// built from. Each stage method dispatches to its bound engine or returns
/// [`LangError::NotImplemented`] if no engine is attached.
///
/// An eval-only runtime — no lex or parse engine — is fully supported.
///
/// # Construction
///
/// Build via [`Runtime::from_registry`] / [`Runtime::from_registry_with_globals`]
/// (registry-based, ADR-0047 §2) or via [`Runtime::new`] + the `with_*`
/// builders (manual engine attachment, for tests / eval-only runtimes).
pub struct Runtime {
    /// The language definition bound to this runtime.
    lang: Language,
    /// The optional lex engine.
    lex_engine: Option<Box<dyn LexerEngine>>,
    /// The optional parse engine (wrapped for dyn compatibility).
    parse_engine: Option<Box<dyn ParseEngineDyn>>,
    /// The optional eval engine.
    eval_engine: Option<Box<dyn EvalEngine>>,
    /// The optional typecheck engine (ADR-0054).
    typecheck_engine: Option<Box<dyn TypecheckEngine>>,
    /// The compile engines keyed by target name (for `compile`/`run_compiled`).
    compile_engines: HashMap<String, Box<dyn CompileEngine>>,
    /// The optional module registry, retained for inspection and for
    /// `run_compiled` to pass to the artifact.
    modules: Option<Rc<dyn handl_platform::ModuleLookup>>,
    /// The registry this runtime was built from (for `RuntimeFactory::build`
    /// to re-build sub-runtimes). `None` if built via the `with_*` builders.
    registry: Option<Rc<EngineRegistry>>,
    /// The config this runtime was built from (for `RuntimeFactory::build`).
    config: RuntimeConfig,
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("lang", &self.lang)
            .field("has_lex_engine", &self.lex_engine.is_some())
            .field("has_parse_engine", &self.parse_engine.is_some())
            .field("has_eval_engine", &self.eval_engine.is_some())
            .field("has_typecheck_engine", &self.typecheck_engine.is_some())
            .field(
                "compile_targets",
                &self.compile_engines.keys().collect::<Vec<_>>(),
            )
            .field("has_modules", &self.modules.is_some())
            .finish()
    }
}

impl Runtime {
    /// Create a new runtime bound to `lang` with no engines attached.
    ///
    /// All stage methods (`lex`, `parse`, `eval`) will return
    /// [`LangError::NotImplemented`] until engines are attached via the
    /// `with_*` builders or [`Runtime::from_registry`].
    pub fn new(lang: Language) -> Self {
        Self {
            lang,
            lex_engine: None,
            parse_engine: None,
            eval_engine: None,
            typecheck_engine: None,
            compile_engines: HashMap::new(),
            modules: None,
            registry: None,
            config: RuntimeConfig::default(),
        }
    }

    /// Attach a lex engine (builder style).
    pub fn with_lex_engine(mut self, engine: Box<dyn LexerEngine>) -> Self {
        self.lex_engine = Some(engine);
        self
    }

    /// Attach a parse engine (builder style).
    ///
    /// Accepts any `Clone` type implementing [`ParserEngine`]; the engine is
    /// stored via the dyn-compatible [`ParseEngineDyn`] wrapper.
    pub fn with_parse_engine<E: ParserEngine + Clone + 'static>(mut self, engine: E) -> Self {
        self.parse_engine = Some(Box::new(engine));
        self
    }

    /// Attach an eval engine (builder style).
    ///
    /// The caller is responsible for wiring host functions into this engine
    /// directly (e.g. calling `TreeWalkEngine::with_modules` before boxing).
    /// The `Runtime` does not rewire a manually-set engine.
    pub fn with_eval_engine(mut self, engine: Box<dyn EvalEngine>) -> Self {
        self.eval_engine = Some(engine);
        self
    }

    /// Attach a typecheck engine (builder style, ADR-0054).
    pub fn with_typecheck_engine(mut self, engine: Box<dyn TypecheckEngine>) -> Self {
        self.typecheck_engine = Some(engine);
        self
    }

    /// Record a module registry (builder style).
    ///
    /// This stores the registry for inspection and for `run_compiled` but
    /// does **not** rewire an already-attached eval engine. Use
    /// [`Runtime::from_registry_with_globals`] to have the runtime wire
    /// the module registry into the eval engine automatically.
    pub fn with_modules(mut self, modules: Rc<dyn handl_platform::ModuleLookup>) -> Self {
        self.modules = Some(modules);
        self
    }

    /// The language definition bound to this runtime.
    pub fn lang(&self) -> &Language {
        &self.lang
    }

    /// The module registry, if one was provided.
    pub fn modules(&self) -> Option<&Rc<dyn handl_platform::ModuleLookup>> {
        self.modules.as_ref()
    }

    /// Lex source text into a token stream using the bound lex engine.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::NotImplemented`] if no lex engine is bound, or
    /// propagates the engine's error.
    pub fn lex(&self, src: &Source) -> Result<Tokens, LangError> {
        let engine = self.lex_engine.as_ref().ok_or(LangError::NotImplemented)?;
        let req = LexerRequest::new(src.clone());
        engine.lex(&self.lang, &req)
    }

    /// Parse a token stream into an [`Ast`] using the bound parse engine.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::NotImplemented`] if no parse engine is bound, or
    /// propagates the engine's error.
    pub fn parse(&self, toks: &Tokens) -> Result<Ast, LangError> {
        let engine = self
            .parse_engine
            .as_ref()
            .ok_or(LangError::NotImplemented)?;
        let req = ParserRequest::new(toks.clone());
        let ast = engine.parse_dyn(&self.lang, &req)?;
        // ADR-0053 §7: apply data-driven desugar pass between parse and eval.
        Ok(desugar_pass::apply_desugar(&self.lang, &ast))
    }

    /// Evaluate an [`Ast`] into a [`Value`] using the bound eval engine.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::NotImplemented`] if no eval engine is bound, or
    /// propagates the engine's error.
    pub fn eval(&self, ast: &Ast) -> Result<Value, LangError> {
        let engine = self.eval_engine.as_ref().ok_or(LangError::NotImplemented)?;
        let req = EvalRequest::new(ast.clone());
        engine.eval(&self.lang, &req)
    }

    /// Typecheck an [`Ast`], returning the synthesized type (ADR-0054).
    ///
    /// This is the **pure-eval** path: the bound typecheck engine evaluates
    /// type expressions without running effects. Returns the synthesized
    /// type (a value — types are values, ADR-0032) plus diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::NotImplemented`] if no typecheck engine is bound,
    /// or propagates the engine's error.
    pub fn typecheck(&self, ast: &Ast) -> Result<TypecheckResult, LangError> {
        let engine = self
            .typecheck_engine
            .as_ref()
            .ok_or(LangError::NotImplemented)?;
        let req = TypecheckRequest::new(ast.clone());
        engine.typecheck(&self.lang, &req)
    }

    /// Convenience: `eval(parse(lex(src)?)?)`.
    ///
    /// Composes all three stages. If any stage's engine is missing, returns
    /// [`LangError::NotImplemented`] from that stage.
    ///
    /// # Errors
    ///
    /// Propagates the first stage error.
    pub fn run(&self, src: &Source) -> Result<Value, LangError> {
        let toks = self.lex(src)?;
        let ast = self.parse(&toks)?;
        self.eval(&ast)
    }

    /// Compile an [`Ast`] to `target`, returning a serializable
    /// [`CompiledHandle`] (ADR-0047 §3).
    ///
    /// Looks up the compile engine for `target` in this runtime's compile
    /// map, asks it to produce a `Box<dyn CompiledArtifact>`, stores the
    /// artifact in the thread-local [`COMPILED_REGISTRY`], and returns a
    /// handle. Use [`Runtime::run_compiled`] to execute the result.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Engine`] if no compile engine is registered for
    /// `target`, or propagates the engine's compilation error.
    pub fn compile(&self, ast: &Ast, target: &str) -> Result<CompiledHandle, LangError> {
        let engine = self
            .compile_engines
            .get(target)
            .ok_or_else(|| LangError::engine(format!("unknown compile target: {}", target)))?;
        let artifact = engine.compile(&self.lang, ast, target)?;
        let id = COMPILED_NEXT_ID.fetch_add(1, Ordering::SeqCst) + 1;
        COMPILED_REGISTRY.with(|r| {
            r.borrow_mut().insert(id, artifact);
        });
        Ok(CompiledHandle {
            id,
            target: target.to_string(),
        })
    }

    /// Run a previously compiled program referenced by `handle`
    /// (ADR-0047 §3).
    ///
    /// Looks up the [`CompiledArtifact`] in the [`COMPILED_REGISTRY`], clones
    /// it out under a short-lived borrow (so re-entrant `compile` /
    /// `run_compiled` calls don't panic), then executes it with this
    /// runtime's module registry attached (so `Foreign` dispatch —
    /// including the meta host functions — resolves).
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Engine`] if the handle is unknown, or propagates
    /// the artifact's execution error.
    pub fn run_compiled(&self, handle: &CompiledHandle) -> Result<Value, LangError> {
        if handle.id == 0 {
            return Ok(Value::Null);
        }
        let artifact = COMPILED_REGISTRY.with(|r| {
            let reg = r.borrow();
            reg.get(&handle.id)
                .ok_or_else(|| {
                    LangError::engine(format!(
                        "run_compiled: no compiled program for handle {}",
                        handle.id
                    ))
                })
                .map(|a| a.box_clone())
        })?;
        artifact.run(self.modules.clone())
    }

    /// Build a [`Runtime`] from a [`Language`], an [`EngineRegistry`], and a
    /// name-based [`RuntimeConfig`] (ADR-0047 §2).
    ///
    /// Resolves `config.lex` / `config.parse` / `config.eval` (strings)
    /// against `registry`. An empty string leaves that stage unbound (`None`).
    /// A non-empty unknown name returns [`LangError::Engine`]. All compile
    /// engines in the registry are cloned into the runtime (compile is
    /// multi-target, selected per `compile` call).
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Engine`] for any unknown engine name.
    pub fn from_registry(
        lang: Language,
        registry: &Rc<EngineRegistry>,
        config: &RuntimeConfig,
    ) -> Result<Self, LangError> {
        Self::from_registry_with_globals(lang, registry, config, None, None)
    }

    /// Like [`from_registry`] but injects `globals` as root-scope variables
    /// in the eval engine (e.g. `self` → language value). ADR-0041 Phase 3.
    ///
    /// If `modules` is `Some`, it is passed to the eval engine (via
    /// `EvalEngine::with_modules`) for `Foreign`
    /// dispatch; if `None`, no module registry is wired.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Engine`] for any unknown engine name.
    pub fn from_registry_with_globals(
        lang: Language,
        registry: &Rc<EngineRegistry>,
        config: &RuntimeConfig,
        modules: Option<Rc<dyn handl_platform::ModuleLookup>>,
        globals: Option<&HashMap<String, Value>>,
    ) -> Result<Self, LangError> {
        // Resolve lex engine (clone out of the registry).
        let lex_engine: Option<Box<dyn LexerEngine>> = if config.lex.is_empty() {
            None
        } else {
            Some(
                registry
                    .lex_engine(&config.lex)
                    .ok_or_else(|| {
                        LangError::engine(format!("unknown lex engine: {}", config.lex))
                    })?
                    .box_clone(),
            )
        };

        // Resolve parse engine (clone out of the registry).
        let parse_engine: Option<Box<dyn ParseEngineDyn>> = if config.parse.is_empty() {
            None
        } else {
            Some(
                registry
                    .parse_engine(&config.parse)
                    .ok_or_else(|| {
                        LangError::engine(format!("unknown parse engine: {}", config.parse))
                    })?
                    .box_clone(),
            )
        };

        // Resolve eval engine (clone + wire modules/globals via trait methods).
        let eval_engine: Option<Box<dyn EvalEngine>> = if config.eval.is_empty() {
            None
        } else {
            let reg_engine = registry.eval_engine(&config.eval).ok_or_else(|| {
                LangError::engine(format!("unknown eval engine: {}", config.eval))
            })?;
            let mut engine = match modules.clone() {
                Some(m) => reg_engine.with_modules(m),
                None => reg_engine.box_clone(),
            };
            if let Some(globals) = globals {
                for (name, val) in globals {
                    engine = engine.with_global(name, val.clone());
                }
            }
            Some(engine)
        };

        // Clone all compile engines into the runtime (multi-target).
        let mut compile_engines: HashMap<String, Box<dyn CompileEngine>> = HashMap::new();
        for target in registry.compile_targets() {
            if let Some(eng) = registry.compile_engine(&target) {
                compile_engines.insert(target, eng.box_clone());
            }
        }

        // Resolve typecheck engine (clone + wire modules via trait method).
        let typecheck_engine: Option<Box<dyn TypecheckEngine>> = if config.typecheck.is_empty() {
            None
        } else {
            let reg_engine = registry
                .typecheck_engine(&config.typecheck)
                .ok_or_else(|| {
                    LangError::engine(format!("unknown typecheck engine: {}", config.typecheck))
                })?;
            let engine = match modules.clone() {
                Some(m) => reg_engine.with_modules(m),
                None => reg_engine.box_clone(),
            };
            Some(engine)
        };

        Ok(Self {
            lang,
            lex_engine,
            parse_engine,
            eval_engine,
            typecheck_engine,
            compile_engines,
            modules,
            registry: Some(Rc::clone(registry)),
            config: config.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// RuntimeFactory / RuntimeHandle impls for Runtime
// ---------------------------------------------------------------------------

impl RuntimeFactory for Runtime {
    fn build(
        &self,
        lang: Language,
        _delegate: Option<Rc<dyn handl_platform::ModuleLookup>>,
    ) -> Result<Box<dyn RuntimeHandle>, LangError> {
        let registry = self
            .registry
            .as_ref()
            .ok_or_else(|| LangError::engine("RuntimeFactory::build: runtime has no registry"))?;
        // Re-attach this runtime's module chain (which includes the meta
        // functions) so the sub-runtime can recurse via Foreign dispatch.
        let modules = self.modules.clone();
        // Inject `self` as a global variable bound to the Language value
        // (ADR-0041 Phase 3).
        let self_val = handl_language::language_value(&lang);
        let mut globals = HashMap::new();
        globals.insert("self".to_string(), self_val);
        let rt = Self::from_registry_with_globals(
            lang,
            registry,
            &self.config,
            modules,
            Some(&globals),
        )?;
        Ok(Box::new(rt))
    }
}

impl RuntimeHandle for Runtime {
    fn lex(&self, src: &Source) -> Result<Tokens, LangError> {
        Runtime::lex(self, src)
    }

    fn parse(&self, toks: &Tokens) -> Result<Ast, LangError> {
        Runtime::parse(self, toks)
    }

    fn eval(&self, ast: &Ast) -> Result<Value, LangError> {
        Runtime::eval(self, ast)
    }

    fn typecheck(&self, ast: &Ast) -> Result<TypecheckResult, LangError> {
        Runtime::typecheck(self, ast)
    }

    fn compile(&self, ast: &Ast, target: &str) -> Result<CompiledHandle, LangError> {
        Runtime::compile(self, ast, target)
    }

    fn run_compiled(&self, handle: &CompiledHandle) -> Result<Value, LangError> {
        Runtime::run_compiled(self, handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{
        AstNode, AstShapeRef, EvalSchemeDef, GrammarPosition, GrammarRow, Scheme, Token,
        TokenKindDef,
    };
    use handl_platform::{EvalContext, Function, Module, ModuleLookup};
    use handl_plugin_engine_lex::Lexer;
    use handl_plugin_engine_parse::{Parser, ParserContext};
    use handl_plugin_module::ModuleRegistry;
    use serde_json::{json, Value};

    // -----------------------------------------------------------------------
    // Stub engines for independent dispatch testing
    // -----------------------------------------------------------------------

    /// A stub lex engine that returns a fixed token stream.
    #[derive(Clone)]
    struct StubLex {
        tokens: Tokens,
    }

    impl LexerEngine for StubLex {
        fn lex(&self, _lang: &Language, _req: &LexerRequest) -> Result<Tokens, LangError> {
            Ok(self.tokens.clone())
        }

        fn box_clone(&self) -> Box<dyn LexerEngine> {
            Box::new(self.clone())
        }
    }

    /// A stub parse engine that returns a single-node Ast.
    #[derive(Clone)]
    struct StubParse;

    impl ParserEngine for StubParse {
        fn expression(
            &self,
            _ctx: &mut ParserContext<'_, '_, Self>,
            _rbp: u8,
        ) -> Result<handl_language::AstRef, LangError> {
            Ok(handl_language::AstRef::new(0))
        }

        fn parse(&self, _lang: &Language, req: &ParserRequest) -> Result<Ast, LangError> {
            let mut ast = Ast::new();
            if !req.tokens.as_slice().is_empty() {
                ast.push(
                    AstNode::new("stub").with_payload(json!(req.tokens.as_slice()[0].text.clone())),
                );
            }
            Ok(ast)
        }
    }

    /// A stub eval engine that returns a fixed value.
    #[derive(Clone)]
    struct StubEval {
        value: Value,
    }

    impl EvalEngine for StubEval {
        fn eval(&self, _lang: &Language, _req: &EvalRequest) -> Result<Value, LangError> {
            Ok(self.value.clone())
        }

        fn box_clone(&self) -> Box<dyn EvalEngine> {
            Box::new(self.clone())
        }
    }

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    /// A module exposing a single `double` function for tests.
    struct DoubleModule;

    impl Module for DoubleModule {
        fn name(&self) -> &str {
            "test"
        }
        fn function(&self, name: &str) -> Option<&dyn Function> {
            if name == "double" {
                Some(&DoubleFn)
            } else {
                None
            }
        }
    }

    /// A host function that doubles its integer argument.
    struct DoubleFn;

    impl Function for DoubleFn {
        fn call(
            &self,
            args: &[Value],
            _ctx: &mut EvalContext,
        ) -> Result<Value, handl_platform::EvalError> {
            let n = args[0].as_i64().unwrap();
            Ok(Value::from(n * 2))
        }
    }

    /// A module registry with "double" registered.
    fn double_modules() -> Rc<dyn ModuleLookup> {
        Rc::new(ModuleRegistry::new().with(DoubleModule))
    }

    /// Build a registry pre-populated with the lex/parse/eval engine impls
    /// (data lexer, pratt parser, tree-walk/bytecode/cranelift eval).
    /// Compile engines are NOT registered here to avoid a dev-dependency
    /// cycle (the engine crates depend on `handl-runtime` for the
    /// `CompileEngine` trait). Compile-engine registration is tested in
    /// `kit-canonical` instead.
    fn default_registry() -> Rc<EngineRegistry> {
        use handl_plugin_engine_eval_bytecode::BytecodeEngine;
        use handl_plugin_engine_eval_cranelift::CraneliftEngine;
        use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
        use handl_plugin_engine_lex_pattern::DataLexerEngine;
        use handl_plugin_engine_parse_pratt::PrattEngine;
        let mut reg = EngineRegistry::new();
        reg.register_lex("data", Box::new(DataLexerEngine::new()));
        reg.register_parse("pratt", PrattEngine::new());
        reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
        reg.register_eval("bytecode", Box::new(BytecodeEngine::new()));
        reg.register_eval("cranelift", Box::new(CraneliftEngine::new()));
        Rc::new(reg)
    }

    /// Build a small arithmetic language: numbers + `+` infix, with eval
    /// schemes for `lit` (no scheme → payload) and `binary.add` (Prim add).
    fn arith_lang() -> Language {
        let mut lang = Language::new();
        // Token kinds
        lang.tokens
            .merge(TokenKindDef::new("token.number"))
            .unwrap();
        lang.tokens.merge(TokenKindDef::new("token.plus")).unwrap();
        // Grammar: number prefix (atom, arity 0)
        lang.grammar
            .merge(GrammarRow::new(
                "g.num",
                "token.number",
                GrammarPosition::Prefix,
                AstShapeRef::new("lit", 0),
            ))
            .unwrap();
        // Grammar: + infix (lbp 10, rbp 11)
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.plus",
                    "token.plus",
                    GrammarPosition::Infix,
                    AstShapeRef::new("binary.add", 2),
                )
                .with_binding_power(10, 11),
            )
            .unwrap();
        // Eval scheme: binary.add → arr(add, [proj_child 0, proj_child 1])
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::arr_native(
                    "add",
                    vec![
                        Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                        Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                    ],
                ),
            ))
            .unwrap();
        lang
    }

    fn tok(kind: &str, text: &str) -> Token {
        Token {
            kind: kind.into(),
            text: text.into(),
        }
    }

    // -----------------------------------------------------------------------
    // Runtime construction with engine selection (each independently)
    // -----------------------------------------------------------------------

    #[test]
    fn new_runtime_has_no_engines() {
        let lang = Language::new();
        let rt = Runtime::new(lang);
        assert_eq!(
            rt.lex(&Source::new("x")).unwrap_err(),
            LangError::NotImplemented
        );
        assert_eq!(
            rt.parse(&Tokens::new()).unwrap_err(),
            LangError::NotImplemented
        );
        assert_eq!(rt.eval(&Ast::new()).unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn with_lex_engine_attaches_lex_only() {
        let rt = Runtime::new(Language::new()).with_lex_engine(Box::new(StubLex {
            tokens: Tokens(vec![tok("token.number", "42")]),
        }));
        // lex works, parse/eval still NotImplemented.
        let toks = rt.lex(&Source::new("42")).unwrap();
        assert_eq!(toks.as_slice().len(), 1);
        assert_eq!(rt.parse(&toks).unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn with_parse_engine_attaches_parse_only() {
        let rt = Runtime::new(Language::new()).with_parse_engine(StubParse);
        let toks = Tokens(vec![tok("token.number", "42")]);
        let ast = rt.parse(&toks).unwrap();
        assert_eq!(ast.len(), 1);
        assert_eq!(ast.nodes()[0].kind, "stub");
        // lex/eval still NotImplemented.
        assert_eq!(
            rt.lex(&Source::new("x")).unwrap_err(),
            LangError::NotImplemented
        );
        assert_eq!(rt.eval(&ast).unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn with_eval_engine_attaches_eval_only() {
        let rt =
            Runtime::new(Language::new()).with_eval_engine(Box::new(StubEval { value: json!(99) }));
        let result = rt.eval(&Ast::new()).unwrap();
        assert_eq!(result, json!(99));
        // lex/parse still NotImplemented.
        assert_eq!(
            rt.lex(&Source::new("x")).unwrap_err(),
            LangError::NotImplemented
        );
        assert_eq!(
            rt.parse(&Tokens::new()).unwrap_err(),
            LangError::NotImplemented
        );
    }

    // -----------------------------------------------------------------------
    // Runtime::lex/parse/eval dispatch to bound engines
    // -----------------------------------------------------------------------

    #[test]
    fn lex_dispatches_to_bound_engine() {
        let expected = Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]);
        let rt = Runtime::new(Language::new()).with_lex_engine(Box::new(StubLex {
            tokens: expected.clone(),
        }));
        let got = rt.lex(&Source::new("1 + 2")).unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn parse_dispatches_to_bound_engine() {
        let rt = Runtime::new(Language::new()).with_parse_engine(StubParse);
        let toks = Tokens(vec![tok("token.number", "7")]);
        let ast = rt.parse(&toks).unwrap();
        assert_eq!(ast.nodes()[0].payload, json!("7"));
    }

    #[test]
    fn eval_dispatches_to_bound_engine() {
        let rt = Runtime::new(Language::new()).with_eval_engine(Box::new(StubEval {
            value: json!("hello"),
        }));
        let val = rt.eval(&Ast::new()).unwrap();
        assert_eq!(val, json!("hello"));
    }

    // -----------------------------------------------------------------------
    // Runtime with a missing engine (eval-only runtime)
    // -----------------------------------------------------------------------

    #[test]
    fn eval_only_runtime_lex_returns_error_eval_works() {
        let rt = Runtime::new(arith_lang()).with_eval_engine(Box::new(
            handl_plugin_engine_eval_tree_walk::TreeWalkEngine::new(),
        ));
        // lex returns NotImplemented (no lex engine).
        assert_eq!(
            rt.lex(&Source::new("1 + 2")).unwrap_err(),
            LangError::NotImplemented
        );
        // parse returns NotImplemented (no parse engine).
        assert_eq!(
            rt.parse(&Tokens::new()).unwrap_err(),
            LangError::NotImplemented
        );
        // eval works on a pre-parsed Ast.
        // Ast: 0: lit(2), 1: lit(3), 2: binary.add {children: [0, 1]}
        let mut ast = Ast::new();
        ast.push(AstNode::new("lit").with_payload(json!(2)));
        ast.push(AstNode::new("lit").with_payload(json!(3)));
        ast.push(AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})));
        let result = rt.eval(&ast).unwrap();
        assert_eq!(result, json!(5));
    }

    // -----------------------------------------------------------------------
    // Engine selection from data (RuntimeConfig + EngineRegistry)
    // -----------------------------------------------------------------------

    #[test]
    fn from_registry_eval_tree_walk_binds_tree_walk() {
        let lang = arith_lang();
        let reg = default_registry();
        let config = RuntimeConfig::new().with_eval("tree-walk");
        let rt = Runtime::from_registry(lang, &reg, &config).unwrap();
        // eval should work (not NotImplemented).
        let mut ast = Ast::new();
        ast.push(AstNode::new("lit").with_payload(json!(42)));
        let result = rt.eval(&ast).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn from_registry_eval_bytecode_binds_bytecode() {
        let lang = Language::new();
        let reg = default_registry();
        let config = RuntimeConfig::new().with_eval("bytecode");
        let rt = Runtime::from_registry(lang, &reg, &config).unwrap();
        // eval should work (not NotImplemented); an empty Ast returns Null.
        let result = rt.eval(&Ast::new()).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn from_registry_eval_cranelift_binds_cranelift() {
        let lang = Language::new();
        let reg = default_registry();
        let config = RuntimeConfig::new().with_eval("cranelift");
        let rt = Runtime::from_registry(lang, &reg, &config).unwrap();
        // eval should work (not NotImplemented); an empty Ast returns Null.
        let result = rt.eval(&Ast::new()).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn from_registry_unknown_eval_name_returns_config_error() {
        let lang = Language::new();
        let reg = default_registry();
        let config = RuntimeConfig::new().with_eval("nonsense");
        let err = Runtime::from_registry(lang, &reg, &config).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
        assert_eq!(err, LangError::engine("unknown eval engine: nonsense"));
    }

    #[test]
    fn from_registry_parse_pratt_binds_pratt() {
        let lang = arith_lang();
        let reg = default_registry();
        let config = RuntimeConfig::new().with_parse("pratt");
        let rt = Runtime::from_registry(lang, &reg, &config).unwrap();
        // parse should work (not NotImplemented).
        let toks = Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]);
        let ast = rt.parse(&toks).unwrap();
        assert_eq!(ast.len(), 3);
        assert_eq!(ast.nodes()[2].kind, "binary.add");
    }

    #[test]
    fn from_registry_unknown_parse_name_returns_config_error() {
        let lang = Language::new();
        let reg = default_registry();
        let config = RuntimeConfig::new().with_parse("chumsky");
        let err = Runtime::from_registry(lang, &reg, &config).unwrap_err();
        assert_eq!(err, LangError::engine("unknown parse engine: chumsky"));
    }

    #[test]
    fn from_registry_unknown_lex_name_returns_config_error() {
        let lang = Language::new();
        let reg = default_registry();
        let config = RuntimeConfig::new().with_lex("pratt");
        let err = Runtime::from_registry(lang, &reg, &config).unwrap_err();
        assert_eq!(err, LangError::engine("unknown lex engine: pratt"));
    }

    #[test]
    fn from_registry_lex_pattern_binds_pattern_lexer() {
        // The "data" lex engine name binds the data-driven DataLexerEngine.
        let lang = Language::new();
        let reg = default_registry();
        let config = RuntimeConfig::new().with_lex("data");
        let rt = Runtime::from_registry(lang, &reg, &config).unwrap();
        // lex is bound (no NotImplemented); an empty language has no rules so
        // lexing any non-empty source yields a LexError, not NotImplemented.
        let err = rt.lex(&Source::new("x")).unwrap_err();
        assert!(matches!(err, LangError::LexError { .. }));
    }

    #[test]
    fn from_registry_empty_config_leaves_all_unbound() {
        let lang = Language::new();
        let reg = default_registry();
        let config = RuntimeConfig::new();
        let rt = Runtime::from_registry(lang, &reg, &config).unwrap();
        assert_eq!(
            rt.lex(&Source::new("x")).unwrap_err(),
            LangError::NotImplemented
        );
        assert_eq!(
            rt.parse(&Tokens::new()).unwrap_err(),
            LangError::NotImplemented
        );
        assert_eq!(rt.eval(&Ast::new()).unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn from_registry_with_host_passes_to_eval_engine() {
        // Build a language with a "foreign" node kind that calls "double".
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.dbl",
                "foreign.double",
                Scheme::arr_foreign(
                    "double",
                    vec![Scheme::arr_native_lit(
                        "proj_child",
                        vec![],
                        Value::from(0u64),
                    )],
                ),
            ))
            .unwrap();

        let reg = default_registry();
        let config = RuntimeConfig::new().with_eval("tree-walk");
        let rt =
            Runtime::from_registry_with_globals(lang, &reg, &config, Some(double_modules()), None)
                .unwrap();
        assert!(rt.modules().is_some());

        // Ast: 0: lit(21), 1: foreign.double {children: [0]}
        let mut ast = Ast::new();
        ast.push(AstNode::new("lit").with_payload(json!(21)));
        ast.push(AstNode::new("foreign.double").with_payload(json!({"children": [0]})));
        let result = rt.eval(&ast).unwrap();
        assert_eq!(result, json!(42));
    }

    // -----------------------------------------------------------------------
    // Runtime::run convenience composes the three stages
    // -----------------------------------------------------------------------

    #[test]
    fn run_composes_three_stages_with_stub_engines() {
        let stub_tokens = Tokens(vec![tok("token.number", "42")]);
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: stub_tokens,
            }))
            .with_parse_engine(StubParse)
            .with_eval_engine(Box::new(StubEval { value: json!(123) }));
        let result = rt.run(&Source::new("42")).unwrap();
        assert_eq!(result, json!(123));
    }

    #[test]
    fn run_without_lex_engine_returns_not_implemented() {
        let rt = Runtime::new(Language::new())
            .with_parse_engine(StubParse)
            .with_eval_engine(Box::new(StubEval { value: json!(0) }));
        assert_eq!(
            rt.run(&Source::new("x")).unwrap_err(),
            LangError::NotImplemented
        );
    }

    #[test]
    fn run_without_parse_engine_returns_not_implemented() {
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: Tokens(vec![tok("token.number", "1")]),
            }))
            .with_eval_engine(Box::new(StubEval { value: json!(0) }));
        // lex succeeds, parse returns NotImplemented.
        assert_eq!(
            rt.run(&Source::new("1")).unwrap_err(),
            LangError::NotImplemented
        );
    }

    #[test]
    fn run_without_eval_engine_returns_not_implemented() {
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: Tokens(vec![tok("token.number", "1")]),
            }))
            .with_parse_engine(StubParse);
        // lex and parse succeed, eval returns NotImplemented.
        assert_eq!(
            rt.run(&Source::new("1")).unwrap_err(),
            LangError::NotImplemented
        );
    }

    #[test]
    fn run_with_no_engines_returns_not_implemented() {
        let rt = Runtime::new(Language::new());
        assert_eq!(
            rt.run(&Source::new("x")).unwrap_err(),
            LangError::NotImplemented
        );
    }

    // -----------------------------------------------------------------------
    // RuntimeConfig serde round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn runtime_config_serde_roundtrip() {
        let config = RuntimeConfig::new()
            .with_lex("logos")
            .with_parse("pratt")
            .with_eval("tree-walk");
        let json_str = serde_json::to_string(&config).unwrap();
        let back: RuntimeConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back, config);
    }

    #[test]
    fn runtime_config_empty_serde_roundtrip() {
        let config = RuntimeConfig::new();
        let json_str = serde_json::to_string(&config).unwrap();
        let back: RuntimeConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back, config);
        assert!(back.lex.is_empty());
        assert!(back.parse.is_empty());
        assert!(back.eval.is_empty());
    }

    #[test]
    fn runtime_config_partial_serde_roundtrip() {
        let config = RuntimeConfig::new().with_eval("tree-walk");
        let json_str = serde_json::to_string(&config).unwrap();
        let back: RuntimeConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back, config);
        assert!(back.lex.is_empty());
        assert!(back.parse.is_empty());
        assert_eq!(back.eval, "tree-walk");
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    #[test]
    fn lang_accessor_returns_bound_language() {
        let lang = arith_lang();
        let rt = Runtime::new(lang.clone());
        assert_eq!(rt.lang(), &lang);
    }

    #[test]
    fn host_accessor_returns_none_by_default() {
        let rt = Runtime::new(Language::new());
        assert!(rt.modules().is_none());
    }

    #[test]
    fn host_accessor_returns_some_when_set() {
        let rt = Runtime::new(Language::new()).with_modules(double_modules());
        assert!(rt.modules().is_some());
    }

    // -----------------------------------------------------------------------
    // Builder chaining
    // -----------------------------------------------------------------------

    #[test]
    fn builder_chains_all_engines() {
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: Tokens(vec![tok("token.number", "1")]),
            }))
            .with_parse_engine(StubParse)
            .with_eval_engine(Box::new(StubEval { value: json!(42) }));
        let result = rt.run(&Source::new("1")).unwrap();
        assert_eq!(result, json!(42));
    }

    // -----------------------------------------------------------------------
    // Operation markers (verify re-exports compile)
    // -----------------------------------------------------------------------

    #[test]
    fn operation_markers_are_accessible() {
        let _ = Lexer;
        let _ = Parser;
        let _ = handl_plugin_engine_eval::Eval;
    }

    // -----------------------------------------------------------------------
    // Debug impl
    // -----------------------------------------------------------------------

    #[test]
    fn runtime_debug_impl_works() {
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: Tokens::new(),
            }))
            .with_parse_engine(StubParse)
            .with_eval_engine(Box::new(StubEval { value: json!(0) }));
        let debug_str = format!("{:?}", rt);
        assert!(debug_str.contains("Runtime"));
        assert!(debug_str.contains("has_lex_engine: true"));
        assert!(debug_str.contains("has_parse_engine: true"));
        assert!(debug_str.contains("has_eval_engine: true"));
    }

    #[test]
    fn runtime_debug_no_engines() {
        let rt = Runtime::new(Language::new());
        let debug_str = format!("{:?}", rt);
        assert!(debug_str.contains("has_lex_engine: false"));
        assert!(debug_str.contains("has_parse_engine: false"));
        assert!(debug_str.contains("has_eval_engine: false"));
    }

    // -----------------------------------------------------------------------
    // StubParse::expression coverage (required by trait, not called via parse)
    // -----------------------------------------------------------------------

    #[test]
    fn stub_parse_expression_is_callable() {
        // StubParse overrides parse(), so its expression() method is never
        // called through the normal dispatch path. Call it directly to
        // verify the trait impl is correct and cover those lines.
        use handl_plugin_engine_parse::ParserEngine as _;
        let engine = StubParse;
        let lang = Language::new();
        let toks = vec![Token {
            kind: "token.x".into(),
            text: "x".into(),
        }];
        let mut ctx = ParserContext::new(&engine, &lang, &toks);
        let r = engine.expression(&mut ctx, 0).unwrap();
        assert_eq!(r.index(), 0);
    }

    // -----------------------------------------------------------------------
    // EngineRegistry
    // -----------------------------------------------------------------------

    #[test]
    fn engine_registry_lookups_return_registered_engines() {
        let reg = default_registry();
        assert!(reg.lex_engine("data").is_some());
        assert!(reg.parse_engine("pratt").is_some());
        assert!(reg.eval_engine("tree-walk").is_some());
        assert!(reg.eval_engine("bytecode").is_some());
        assert!(reg.eval_engine("cranelift").is_some());
        // Compile engines are not registered in default_registry() to avoid
        // a dev-dependency cycle; see kit-canonical for compile-engine tests.
        assert!(reg.compile_engine("bytecode").is_none());
        assert!(reg.compile_engine("native").is_none());
    }

    #[test]
    fn engine_registry_lookups_return_none_for_unregistered() {
        let reg = EngineRegistry::new();
        assert!(reg.lex_engine("data").is_none());
        assert!(reg.parse_engine("pratt").is_none());
        assert!(reg.eval_engine("tree-walk").is_none());
        assert!(reg.compile_engine("native").is_none());
    }

    #[test]
    fn engine_registry_debug_lists_keys() {
        let reg = default_registry();
        let s = format!("{:?}", reg);
        assert!(s.contains("EngineRegistry"));
    }
}
