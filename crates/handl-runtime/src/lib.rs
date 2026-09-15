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
//! `plugin-engine-exec-tree-walk`, `plugin-engine-exec-bytecode`,
//! `plugin-engine-exec-cranelift`) are deps of the **host**, which registers
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
//! (`plugin-engine-exec-bytecode`, `plugin-engine-exec-cranelift`), not here.
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
pub mod erase_pass;
pub mod fp;
pub mod module_prep;
pub mod runtime_assert_pass;
pub mod vm_primitives;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use handl_language::{Ast, AstRef, LangError, Language, Source, Tokens, Value};
use handl_kit_arrow_lower::lower_ast;
use plugin_engine::term_interpreter::interpret;
use handl_plugin_engine_exec::{EvalEngine, EvalRequest, ImportResolver};
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
/// Each compile engine (`plugin-engine-exec-bytecode`,
/// `plugin-engine-exec-cranelift`) produces its own impl carrying the
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
/// Impl live in the engine crates (`plugin-engine-exec-bytecode`,
/// `plugin-engine-exec-cranelift`), not in `handl-runtime`. The runtime
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
    /// The optional import resolver for `import "path"` statements
    /// (Phase 9.5). The runtime exposes this via its [`ImportResolver`]
    /// impl (delegating here) and wires it into the eval engine at build
    /// time via [`Runtime::from_registry_with_resolver`].
    import_resolver: Option<Rc<dyn ImportResolver>>,
    /// The base directory for resolving relative import paths (Phase 9.5).
    /// `None` → resolve against the process current working directory.
    module_root: Option<PathBuf>,
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
            import_resolver: None,
            module_root: None,
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

    /// Attach an import resolver for `import "path"` statements (builder
    /// style, Phase 9.5). Note: this stores the resolver but does **not**
    /// rewire an already-attached eval engine. Use
    /// [`Runtime::from_registry_with_resolver`] to wire the resolver into
    /// the eval engine at build time.
    pub fn with_import_resolver(mut self, resolver: Rc<dyn ImportResolver>) -> Self {
        self.import_resolver = Some(resolver);
        self
    }

    /// Set the base directory for resolving relative import paths (builder
    /// style, Phase 9.5). `None` (the default) resolves against the process
    /// current working directory.
    pub fn with_module_root(mut self, root: PathBuf) -> Self {
        self.module_root = Some(root);
        self
    }

    /// The import resolver, if one was provided (Phase 9.5).
    pub fn import_resolver(&self) -> Option<&Rc<dyn ImportResolver>> {
        self.import_resolver.as_ref()
    }

    /// The configured module root for resolving relative imports (Phase 9.5).
    pub fn module_root(&self) -> Option<&Path> {
        self.module_root.as_deref()
    }

    /// The language definition bound to this runtime.
    pub fn lang(&self) -> &Language {
        &self.lang
    }

    /// Mutable access to the language definition bound to this runtime.
    pub fn lang_mut(&mut self) -> &mut Language {
        &mut self.lang
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
    ///
    /// # Desugar pipeline (ADR-0082, Option A)
    ///
    /// The desugar pass runs **at parse time** (here in `Runtime::parse`),
    /// before both typecheck and eval. The typechecker and evaluator always
    /// see the desugared AST — they never observe raw `data`/`while`/`for`/
    /// `nullish`/`opt-chain` nodes. This is Option A from the Phase 1 task
    /// spec: desugar at parse time is the current and preferred behavior.
    /// `data` declarations (ADR-0082) are rewritten into `block` definitions
    /// (predicate + constructors + recursor) by `desugar_data` before any
    /// downstream tool sees them.
    pub fn parse(&self, toks: &Tokens) -> Result<Ast, LangError> {
        let engine = self
            .parse_engine
            .as_ref()
            .ok_or(LangError::NotImplemented)?;
        let req = ParserRequest::new(toks.clone());
        let ast = engine.parse_dyn(&self.lang, &req)?;
        // ADR-0094 §6: run the "desugar" transform pass between parse and
        // eval; a failed transform propagates (no silent fallthrough).
        desugar_pass::apply_desugar(&self.lang, &ast)
    }

    /// Evaluate an [`Ast`] into a [`Value`].
    ///
    /// First tries lowering the canonical AST to a `Term` and running the
    /// small `Term` interpreter. If the lowerer does not support the file,
    /// it logs a warning and falls back to the bound eval engine so existing
    /// surface constructs keep working. If lowering succeeds but the
    /// interpreter cannot handle a `Term` variant, the error is propagated.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::NotImplemented`] if no eval engine is bound, or
    /// propagates the engine's error.
    pub fn eval(&self, ast: &Ast) -> Result<Value, LangError> {
        if !ast.is_empty() {
            let root = AstRef::new(ast.len() - 1);
            match lower_ast(ast, &self.lang, root) {
                Ok(term) => match interpret(&term, Value::Null) {
                    Ok(value) => return Ok(value),
                    Err(e) => {
                        eprintln!("[handl-runtime] Term interpreter failed: {e}");
                        return Err(LangError::engine(e));
                    }
                },
                Err(e) => {
                    eprintln!(
                        "[handl-runtime] lower_ast unsupported: {e}; falling back to AST eval"
                    );
                }
            }
        }

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

    /// Evaluate an [`Ast`] like [`Runtime::eval`], but with `globals`
    /// injected as additional root-scope variables for this call only.
    ///
    /// The eval engine is cloned (via [`EvalEngine::box_clone`] +
    /// [`EvalEngine::with_global`]) so the runtime's engine is unchanged;
    /// this is the per-call "bind arguments into the env" seam used by
    /// hosts that need to pass *values* (e.g. closures, tagged values) into
    /// a nested evaluation rather than serializing them into source text.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::NotImplemented`] if no eval engine is bound, or
    /// propagates the engine's error.
    pub fn eval_with_globals(
        &self,
        ast: &Ast,
        globals: &HashMap<String, Value>,
    ) -> Result<Value, LangError> {
        let engine = self.eval_engine.as_ref().ok_or(LangError::NotImplemented)?;
        let mut engine = engine.box_clone();
        for (name, val) in globals {
            engine = engine.with_global(name, val.clone());
        }
        let req = EvalRequest::new(ast.clone());
        engine.eval(&self.lang, &req)
    }

    /// Like [`eval_with_globals`](Self::eval_with_globals), but takes the
    /// [`Ast`] behind an `Rc` and dispatches to
    /// [`EvalEngine::eval_shared`], so engines that hold the arena by `Rc`
    /// internally (the tree-walk engine) reuse the caller's allocation
    /// instead of deep-cloning it into an [`EvalRequest`].
    ///
    /// This is the per-call seam for hosts that cache a parsed AST and
    /// re-evaluate it many times with different globals — e.g.
    /// `plugin-lib-handl`'s `HandlFunction` bridge.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::NotImplemented`] if no eval engine is bound, or
    /// propagates the engine's error.
    pub fn eval_shared_with_globals(
        &self,
        ast: &Rc<Ast>,
        globals: &HashMap<String, Value>,
    ) -> Result<Value, LangError> {
        let engine = self.eval_engine.as_ref().ok_or(LangError::NotImplemented)?;
        let mut engine = engine.box_clone();
        for (name, val) in globals {
            engine = engine.with_global(name, val.clone());
        }
        engine.eval_shared(&self.lang, ast)
    }

    /// Evaluate an [`Ast`] held behind an `Rc`, binding `args` directly into
    /// the call frame's scope rather than via per-call `with_global` engine
    /// cloning.
    ///
    /// This is the per-call seam for hosts that cache a parsed AST and
    /// re-evaluate it many times with different arguments (e.g.
    /// `plugin-lib-handl`'s `HandlFunction` bridge). The argument names are
    /// expected to match identifiers referenced inside `ast` (typically
    /// `__arg_0`, `__arg_1`, … from a cached applied-form AST).
    ///
    /// Unlike [`eval_shared_with_globals`](Self::eval_shared_with_globals),
    /// which clones the eval engine once per argument via `with_global`, this
    /// method delegates to `EvalEngine::eval_shared_with_call_args`, which the
    /// tree-walk engine overrides to bind args directly on the call frame's
    /// scope stack — **zero engine clones per call**. Engines that do not
    /// override the default fall back to the clone-and-inject path, preserving
    /// existing behavior.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::NotImplemented`] if no eval engine is bound, or
    /// propagates the engine's error.
    pub fn eval_shared_with_call_args(
        &self,
        ast: &Rc<Ast>,
        args: &[(String, Value)],
    ) -> Result<Value, LangError> {
        let engine = self.eval_engine.as_ref().ok_or(LangError::NotImplemented)?;
        engine.eval_shared_with_call_args(&self.lang, ast, args)
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

    /// `lex → parse → typecheck → eval` (ADR-0054, Phase 10.1).
    ///
    /// Like [`Runtime::run`], but inserts a typecheck stage between parse and
    /// eval. If the typecheck engine reports any `Severity::Error` diagnostics
    /// (or returns `Err`), eval is short-circuited and the error is returned
    /// immediately — surfacing type errors as compile-time diagnostics rather
    /// than runtime crashes or silent wrong results. Warnings and info-level
    /// diagnostics do not block eval.
    ///
    /// If no typecheck engine is bound (`self.typecheck_engine` is `None`),
    /// this falls through to plain `lex → parse → eval` (it does **not** fail)
    /// — the engine may simply not be configured.
    ///
    /// # Errors
    ///
    /// Propagates the first stage error. A typecheck error is returned as
    /// [`LangError::Engine`] carrying the first error diagnostic's message.
    pub fn run_checked(&self, src: &Source) -> Result<Value, LangError> {
        let toks = self.lex(src)?;
        let mut ast = self.parse(&toks)?;
        // If a typecheck engine is bound, run it before eval. Fatal type
        // errors are reported via `Err` (per `TypecheckDiagnostic` docs); we
        // also scan the diagnostics vec for any `Severity::Error` entries as a
        // defensive measure. Warnings/info do not block eval.
        if let Some(engine) = self.typecheck_engine.as_ref() {
            let req = TypecheckRequest::new(ast.clone());
            let result = match engine.typecheck(&self.lang, &req) {
                Ok(r) => r,
                Err(LangError::NotImplemented) => {
                    // The bound typecheck engine cannot handle this source yet
                    // (e.g. the maturing Handl model). Degrade to plain eval
                    // rather than surfacing NotImplemented to the caller.
                    return self.eval(&ast);
                }
                Err(e) => return Err(e),
            };
            if let Some(err) = result
                .diagnostics
                .iter()
                .find(|d| d.severity.eq_ignore_ascii_case("error"))
            {
                return Err(LangError::engine(format!(
                    "typecheck error: {}",
                    err.message
                )));
            // ADR-0068 graduated guarantees: when the typechecker emits
            // `runtime_assert` diagnostics (Blocked predicates), rewrite
            // `type-annot` nodes into `runtime-assert` nodes so the eval
            // engine emits an actual `assert T(e)` at runtime.
            } else {
                // ADR-0068 graduated guarantees: rewrite every `type-annot`
                // into a `runtime-assert` node so predicate annotations are
                // checked at runtime. Non-closure predicates pass through
                // unchanged, so this is safe for all type-annot sites.
                runtime_assert_pass::apply_runtime_asserts(&self.lang, &mut ast);
            }
            // ADR-0032: proof erasure. Replace nodes whose inferred type is
            // a singleton proof constructor with `erased` before eval.
            erase_pass::apply_erase(&self.lang, &mut ast, &result.node_types);
        }
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
        Self::from_registry_with_resolver(lang, registry, config, modules, globals, None, None)
    }

    /// Like [`from_registry_with_globals`] but also wires an
    /// [`ImportResolver`] into the eval engine for `import "path"`
    /// statements and records a `module_root` for relative path resolution
    /// (Phase 9.5).
    ///
    /// The resolver is passed to the eval engine via
    /// [`EvalEngine::with_import_resolver`] (engines that support imports
    /// override it; others ignore it). The runtime also stores the resolver
    /// so its own [`ImportResolver`] impl can delegate to it, and stores
    /// `module_root` for path resolution.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Engine`] for any unknown engine name.
    pub fn from_registry_with_resolver(
        lang: Language,
        registry: &Rc<EngineRegistry>,
        config: &RuntimeConfig,
        modules: Option<Rc<dyn handl_platform::ModuleLookup>>,
        globals: Option<&HashMap<String, Value>>,
        import_resolver: Option<Rc<dyn ImportResolver>>,
        module_root: Option<PathBuf>,
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

        // Resolve eval engine (clone + wire modules/globals/resolver via
        // trait methods).
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
            if let Some(resolver) = &import_resolver {
                engine = engine.with_import_resolver(Rc::clone(resolver));
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

        // Resolve typecheck engine (clone + wire modules and globals via
        // trait method).
        let typecheck_engine: Option<Box<dyn TypecheckEngine>> = if config.typecheck.is_empty() {
            None
        } else {
            let reg_engine = registry
                .typecheck_engine(&config.typecheck)
                .ok_or_else(|| {
                    LangError::engine(format!("unknown typecheck engine: {}", config.typecheck))
                })?;
            let mut engine: Box<dyn TypecheckEngine> = match modules.clone() {
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

        Ok(Self {
            lang,
            lex_engine,
            parse_engine,
            eval_engine,
            typecheck_engine,
            compile_engines,
            modules,
            import_resolver,
            module_root,
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
        let rt = Self::from_registry_with_resolver(
            lang,
            registry,
            &self.config,
            modules,
            Some(&globals),
            self.import_resolver.clone(),
            self.module_root.clone(),
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

// ---------------------------------------------------------------------------
// ImportResolver — Runtime delegation + ModuleLoader (Phase 9.5)
// ---------------------------------------------------------------------------

impl ImportResolver for Runtime {
    fn resolve_import(&self, path: &str) -> Result<Value, LangError> {
        // The runtime delegates to its configured import resolver (a
        // shared `ModuleLoader`). This keeps the runtime's own engines
        // free of self-reference: the loader owns independent engine clones
        // and a shared module cache, so loading a module never re-enters the
        // runtime's eval engine directly. If no resolver is configured,
        // imports are unsupported.
        let resolver = self.import_resolver.as_ref().ok_or_else(|| {
            LangError::engine("import: no import resolver configured for this runtime")
        })?;
        resolver.resolve_import(path)
    }
}

/// A self-contained module loader for `import "path"` statements
/// (Phase 9.5).
///
/// A `ModuleLoader` owns independent clones of the lex/parse/eval engines
/// (so evaluating an imported module never re-enters the importer's eval
/// engine), a shared module cache keyed by canonical path, a "currently
/// loading" set for cycle detection, and a `module_root` for resolving
/// relative paths. It implements [`ImportResolver`]; the runtime delegates
/// to it.
///
/// # Construction (two-phase)
///
/// The loader's eval engine needs the loader itself as its import resolver
/// (so nested `import` statements inside a module resolve through the same
/// cache/cycle-detector). Because the engine holds an `Rc` clone of the
/// loader, the engine cannot be set at construction time (the `Rc` would
/// prevent `Rc::get_mut`). Instead, construct via [`ModuleLoader::new`]
/// (engine unset), build the eval engine with
/// `TreeWalkEngine::with_import_resolver(Rc::clone(&loader))`, then call
/// [`ModuleLoader::set_eval_engine`] (which requires a unique `Rc` — call
/// before sharing the loader).
pub struct ModuleLoader {
    /// The language bound to this loader (shared with the runtime).
    lang: Rc<Language>,
    /// The lex engine clone for module source lexing.
    lex_engine: Box<dyn LexerEngine>,
    /// The parse engine clone for module source parsing.
    parse_engine: Box<dyn ParseEngineDyn>,
    /// The eval engine for module evaluation. Stored in an `Rc` so it can
    /// be cheaply cloned out per `load_module` call (the eval engine is
    /// re-entrant: an imported module may itself import, re-entering
    /// `load_module` → `eval`). A `RefCell<Option<...>>` allows late
    /// binding via [`ModuleLoader::set_eval_engine`].
    eval_engine: RefCell<Option<Rc<dyn EvalEngine>>>,
    /// The base directory for resolving relative import paths.
    module_root: Option<PathBuf>,
    /// Project dependencies declared in `.handl.toml`'s `[dependencies]`
    /// table: import-name → absolute package root directory. An import path
    /// whose first `/`-separated segment names a dependency resolves inside
    /// that directory (`import "arrows/fn/map.hnl"` →
    /// `<deps["arrows"]>/fn/map.hnl`).
    dependencies: HashMap<String, PathBuf>,
    /// Directory stack for nested imports: while a module is being
    /// evaluated, its own directory is the resolution base for relative
    /// import paths inside it.
    dir_stack: RefCell<Vec<PathBuf>>,
    /// Cache of evaluated module bindings, keyed by canonical path.
    cache: RefCell<HashMap<String, Value>>,
    /// Paths currently being loaded (cycle detection).
    loading: RefCell<HashSet<String>>,
}

impl ModuleLoader {
    /// Create a new module loader with the given engines and module root.
    /// The eval engine is unset; call [`ModuleLoader::set_eval_engine`]
    /// before using the loader (with a `TreeWalkEngine` wired with
    /// `Rc::clone(&loader) as Rc<dyn ImportResolver>` as its import
    /// resolver).
    pub fn new(
        lang: Rc<Language>,
        lex_engine: Box<dyn LexerEngine>,
        parse_engine: Box<dyn ParseEngineDyn>,
        module_root: Option<PathBuf>,
    ) -> Self {
        Self {
            lang,
            lex_engine,
            parse_engine,
            eval_engine: RefCell::new(None),
            module_root,
            dependencies: HashMap::new(),
            dir_stack: RefCell::new(Vec::new()),
            cache: RefCell::new(HashMap::new()),
            loading: RefCell::new(HashSet::new()),
        }
    }

    /// Declare project dependencies (`.handl.toml` `[dependencies]`):
    /// import-name → absolute package root directory. See
    /// [`ModuleLoader::resolve_path`] for the resolution semantics.
    pub fn with_dependencies(mut self, deps: HashMap<String, PathBuf>) -> Self {
        self.dependencies = deps;
        self
    }

    /// Late-bind the eval engine (two-phase construction). Requires a
    /// unique `Rc` (call before sharing the loader). The engine should be
    /// a `TreeWalkEngine` wired with `Rc::clone(&loader)` as its import
    /// resolver so nested imports resolve through this loader.
    pub fn set_eval_engine(self: &Rc<Self>, engine: Box<dyn EvalEngine>) {
        debug_assert!(
            self.eval_engine.borrow().is_none(),
            "ModuleLoader::set_eval_engine called twice"
        );
        // Re-wire the engine's import resolver to this loader so that
        // `import`/`from` statements inside loaded modules resolve through
        // the same cache and relative-path base.
        let resolver = Rc::clone(self) as Rc<dyn ImportResolver>;
        let engine = engine.with_import_resolver(resolver);
        *self.eval_engine.borrow_mut() = Some(Rc::from(engine));
    }

    /// Resolve an import path to a canonical absolute path string, used as
    /// the cache key.
    ///
    /// # Resolution semantics
    ///
    /// 1. **Dependency specifier.** If the first `/`-separated segment of
    ///    `path` names a declared `[dependencies]` entry, the remainder of
    ///    the path resolves inside that package's root directory:
    ///    `import "arrows/fn/map.hnl"` → `<deps["arrows"]>/fn/map.hnl`.
    ///    Dependency names shadow a same-named directory relative to the
    ///    module root.
    /// 2. **Relative path.** Otherwise `path` resolves against the directory
    ///    of the module currently being loaded (nested imports), falling
    ///    back to `module_root`, then to the process working directory.
    /// 3. **Directory package.** If the resolved path is a directory, the
    ///    loader reads `<dir>/index.hnl` instead (so `from "../time"` loads
    ///    `handl-packages/time/index.hnl`).
    /// 4. **Bare name.** If the path has no `.hnl` extension, the loader
    ///    first tries `<path>.hnl` (so `from "./clock_gettime"` loads
    ///    `clock_gettime.hnl`) before falling through to host-module
    ///    resolution.
    fn resolve_path(&self, path: &str) -> Result<String, LangError> {
        let (first, rest) = match path.split_once('/') {
            Some((f, r)) => (f, Some(r)),
            None => (path, None),
        };
        let joined = if let Some(dep_dir) = self.dependencies.get(first) {
            match rest {
                Some(r) if !r.is_empty() => dep_dir.join(r),
                _ => dep_dir.clone(),
            }
        } else {
            let base = self
                .dir_stack
                .borrow()
                .last()
                .cloned()
                .or_else(|| self.module_root.clone())
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| PathBuf::from("."));
            base.join(path)
        };
        let candidate = if joined.is_dir() {
            let index = joined.join("index.hnl");
            if index.is_file() {
                index
            } else if let Some(name) = joined.file_name().and_then(|s| s.to_str()) {
                let main = joined.join(format!("{}.hnl", name));
                if main.is_file() {
                    main
                } else if !path.ends_with(".hnl") {
                    let with_ext = joined.with_extension("hnl");
                    if with_ext.is_file() { with_ext } else { joined }
                } else {
                    joined
                }
            } else if !path.ends_with(".hnl") {
                let with_ext = joined.with_extension("hnl");
                if with_ext.is_file() { with_ext } else { joined }
            } else {
                joined
            }
        } else if !path.ends_with(".hnl") {
            let with_ext = joined.with_extension("hnl");
            if with_ext.is_file() { with_ext } else { joined }
        } else {
            joined
        };
        // `canonicalize` fails if the file does not exist; in that case use
        // the candidate path so the subsequent read error is clear.
        let resolved = std::fs::canonicalize(&candidate)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| candidate.to_string_lossy().into_owned());
        Ok(resolved)
    }

    /// Load (or return cached) module bindings for `path`.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if the file is missing/unreadable, lex/parse/
    /// eval fails, or a circular import is detected.
    pub fn load_module(&self, path: &str) -> Result<Value, LangError> {
        let resolved = self.resolve_path(path)?;

        // Cache hit — return the previously evaluated bindings.
        if let Some(cached) = self.cache.borrow().get(&resolved).cloned() {
            return Ok(cached);
        }
        // Cycle detection: if this path is already on the active loading
        // stack, we have a circular import.
        if self.loading.borrow().contains(&resolved) {
            return Err(LangError::engine(format!(
                "circular import detected: {}",
                path
            )));
        }
        self.loading.borrow_mut().insert(resolved.clone());

        // Do the fallible load in a helper so the `loading` entry is always
        // removed — including on error paths (a failed load may be retried
        // and must not trip the cycle detector).
        let result = self.load_module_uncached(&resolved, path);
        self.loading.borrow_mut().remove(&resolved);
        let bindings = result?;

        // Cache the result.
        self.cache.borrow_mut().insert(resolved, bindings.clone());
        Ok(bindings)
    }

    /// Read, parse, desugar, and evaluate the module at the already-resolved
    /// path `resolved`, returning its collected top-level bindings.
    /// `display_path` is the path as written in the `import` statement, used
    /// in error messages.
    fn load_module_uncached(&self, resolved: &str, display_path: &str) -> Result<Value, LangError> {
        // Read the module source.
        let src = std::fs::read_to_string(resolved).map_err(|e| {
            LangError::engine(format!(
                "import \"{}\": cannot read module: {}",
                display_path, e
            ))
        })?;
        // Drop module-declaration header lines that name a *host module*
        // rather than a resolvable file (`from "str" import ...`,
        // `import x from "str"`) — those resolve via the host module chain,
        // which the loader's eval engine already shares, so the
        // declarations are decorative (the same convention `hnl run`/
        // `hnl test` apply). Path-specifier declarations are kept: they
        // are the executable import op and resolve through this loader.
        // See ADR-0096 §"Module-name imports".
        let src = self.strip_module_decl_lines(&src, resolved);
        // Canonical `.hnl` modules are prepared the same way as
        // `handl_plugin_lib_handl` does: top-level `export` declarations
        // become an object literal so multi-export modules evaluate to a
        // record and type-only exports like `export Clock<T>: Grade` are
        // safe to run.
        let src = module_prep::prepare_module(&src);
        let source = Source::new(&src);
        let toks = {
            let req = LexerRequest::new(source);
            self.lex_engine.lex(&self.lang, &req).map_err(|e| {
                LangError::engine(format!(
                    "import \"{}\" ({}): lex error: {}",
                    display_path, resolved, e
                ))
            })?
        };
        let ast = {
            let req = ParserRequest::new(toks);
            self.parse_engine.parse_dyn(&self.lang, &req).map_err(|e| {
                LangError::engine(format!(
                    "import \"{}\" ({}): parse error: {}",
                    display_path, resolved, e
                ))
            })?
        };
        // Apply the same desugar pass the runtime applies at parse time
        // (ADR-0082), so imported modules see the same AST shape.
        let ast = desugar_pass::apply_desugar(&self.lang, &ast).map_err(|e| {
            LangError::engine(format!(
                "import \"{}\" ({}): desugar error: {}",
                display_path, resolved, e
            ))
        })?;

        // While this module is being evaluated, its own directory becomes
        // the base for resolving nested relative imports. The guard pops
        // the entry on every exit path (including eval errors).
        struct DirStackGuard<'a>(&'a RefCell<Vec<PathBuf>>);
        impl Drop for DirStackGuard<'_> {
            fn drop(&mut self) {
                self.0.borrow_mut().pop();
            }
        }
        let _dir_guard = Path::new(resolved).parent().map(|dir| {
            self.dir_stack.borrow_mut().push(dir.to_path_buf());
            DirStackGuard(&self.dir_stack)
        });

        // Evaluate the module. Clone the eval engine out of the RefCell
        // (cheap `Rc` clone) so re-entrant `load_module` calls (nested
        // imports) can independently borrow the cell without panic.
        let engine_rc = self.eval_engine.borrow().clone().ok_or_else(|| {
            LangError::engine(format!(
                "import \"{}\" ({}): module loader has no eval engine configured",
                display_path, resolved
            ))
        })?;
        let value = {
            let req = EvalRequest::new(ast.clone());
            engine_rc.eval(&self.lang, &req).map_err(|e| {
                LangError::engine(format!(
                    "import \"{}\" ({}): eval error: {}",
                    display_path, resolved, e
                ))
            })?
        };

        // Collect the module's top-level bindings into a Value::Object.
        let bindings = collect_module_bindings(self.lang.as_ref(), &ast, value);

        // Seal the exported bindings for the cross-boundary handoff: arrow
        // values minted by this module's eval carry `body` indices into
        // *this* AST, so the engine stamps them with a home frame (see
        // `EvalEngine::seal_module_value`; identity for engines whose
        // values carry no ambient-eval references).
        engine_rc
            .seal_module_value(&ast, &self.lang, bindings)
            .map_err(|e| {
                LangError::engine(format!(
                    "import \"{}\" ({}): seal error: {}",
                    display_path, resolved, e
                ))
            })
    }

    /// Remove module-declaration header lines that name a *host module*
    /// rather than a resolvable file — `from "<spec>" import ...` where
    /// `<spec>` does not resolve to an existing file, and `import <name>
    /// ...`/`import <name> from "..."` unconditionally.
    ///
    /// A non-file specifier (a bare host-module name like `str`, or an
    /// extension-less sibling like `./dedent`) denotes a module resolved by
    /// the `ModuleResolver`/module chain, which the loader's eval engine
    /// already shares — so the line is decorative and would otherwise fail
    /// as a missing file. Specifiers that do resolve to a file are left in
    /// place: `import "path"` and `from "path" import ...` are the
    /// executable import op and must resolve (or error honestly) through
    /// this loader.
    ///
    /// `resolved` is the canonical path of the module being loaded; its
    /// parent directory is the first resolution base, matching
    /// [`ModuleLoader::resolve_path`].
    fn strip_module_decl_lines(&self, src: &str, resolved: &str) -> String {
        src.lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("import") {
                    if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                        let inner = rest.trim_start();
                        if inner.starts_with('"') || inner.starts_with('\'') {
                            // `import "<spec>"` — keep: the executable op.
                            return true;
                        }
                        // `import <name> ...` — a declaration the canonical
                        // grammar cannot execute; always decorative here.
                        return false;
                    }
                    return true;
                }
                if let Some(rest) = trimmed.strip_prefix("from") {
                    if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                        // `from "<spec>" import ...` — keep iff <spec>
                        // resolves to a file (the executable import op);
                        // strip when it names a host module.
                        return match first_quoted_spec(rest) {
                            Some(spec) => self.spec_resolves_to_file(spec, resolved),
                            // Unparseable declaration — keep it so the
                            // parser reports the error.
                            None => true,
                        };
                    }
                }
                true
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Whether `spec` resolves to an existing file, using the same bases as
    /// [`ModuleLoader::resolve_path`]: a `[dependencies]` first segment
    /// resolves inside the package directory; otherwise the directory of
    /// the module being loaded (`resolved`'s parent), then the module
    /// root, then the process working directory.
    ///
    /// A directory is considered to resolve to a file when it contains an
    /// `index.hnl`; a bare path with no `.hnl` extension resolves to a file
    /// when `<path>.hnl` exists.
    fn spec_resolves_to_file(&self, spec: &str, resolved: &str) -> bool {
        self.resolve_spec_to_file(spec, resolved).is_some()
    }

    /// Resolve `spec` against the same bases as [`ModuleLoader::resolve_path`]
    /// and return the first matching file path (a file, a directory's
    /// `index.hnl`, or a `<path>.hnl` for a bare name). Returns `None` when
    /// the spec names a host module rather than a file.
    fn resolve_spec_to_file(&self, spec: &str, resolved: &str) -> Option<PathBuf> {
        let spec_path = Path::new(spec);
        if spec_path.is_absolute() {
            return Self::file_candidate(spec_path, spec);
        }
        let (first, rest) = match spec.split_once('/') {
            Some((f, r)) => (f, Some(r)),
            None => (spec, None),
        };
        if let Some(dep_dir) = self.dependencies.get(first) {
            let joined = match rest {
                Some(r) if !r.is_empty() => dep_dir.join(r),
                _ => dep_dir.clone(),
            };
            return Self::file_candidate(&joined, spec);
        }
        let bases: Vec<Option<PathBuf>> = vec![
            Path::new(resolved).parent().map(Path::to_path_buf),
            self.module_root.clone(),
            std::env::current_dir().ok(),
        ];
        for base in bases.into_iter().flatten() {
            if let Some(cand) = Self::file_candidate(&base.join(spec), spec) {
                return Some(cand);
            }
        }
        None
    }

    /// Return a concrete file path for a joined import candidate: a
    /// directory's `index.hnl`, a `<path>.hnl` for a bare name, or the
    /// joined path itself if it is already a file.
    fn file_candidate(joined: &Path, spec: &str) -> Option<PathBuf> {
        if joined.is_dir() {
            let index = joined.join("index.hnl");
            if index.is_file() {
                return Some(index);
            } else if let Some(name) = joined.file_name().and_then(|s| s.to_str()) {
                let main = joined.join(format!("{}.hnl", name));
                if main.is_file() {
                    return Some(main);
                }
            }
        }
        if !spec.ends_with(".hnl") {
            let with_ext = joined.with_extension("hnl");
            if with_ext.is_file() {
                return Some(with_ext);
            }
        }
        if joined.is_file() {
            Some(joined.to_path_buf())
        } else {
            None
        }
    }
}

/// Extract the first `'`- or `"`-quoted string literal's contents from a
/// source fragment (e.g. the specifier in `from "..." import ...`).
/// Returns `None` when no complete quoted string is present. Quotes are not
/// escapable here — module specifiers never contain them.
fn first_quoted_spec(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let start = bytes.iter().position(|&b| b == b'"' || b == b'\'')?;
    let quote = bytes[start];
    let rest = &s[start + 1..];
    let end = rest.as_bytes().iter().position(|&b| b == quote)?;
    Some(&rest[..end])
}

impl ImportResolver for ModuleLoader {
    fn resolve_import(&self, path: &str) -> Result<Value, LangError> {
        self.load_module(path)
    }
}

/// Collect a module's top-level bindings into a [`Value::Object`].
///
/// The Pratt parser currently parses one top-level statement per `parse`
/// call, so a module AST has a single root node. This extracts the binding
/// introduced by that statement using `Language::binding_target`:
///
/// - A node whose shape declares a `target` child role (`assign`,
///   `let`) has that child's name used as the export key.
/// - A node whose shape carries a `name` payload (e.g. `bind`) uses it
///   directly.
/// - Any other root → `{}` (no exported bindings; the module is evaluated
///   for effect only).
///
/// The value is the result of evaluating the whole module AST (the root),
/// which for an `assign` is its right-hand side.
fn collect_module_bindings(lang: &Language, ast: &Ast, value: Value) -> Value {
    use serde_json::Map;
    if ast.is_empty() {
        return Value::Object(Map::new());
    }
    let root_idx = ast.len() - 1;
    let root = match ast.at(AstRef::new(root_idx)) {
        Ok(n) => n,
        Err(_) => return Value::Object(Map::new()),
    };
    if let Some(name) = lang.binding_target(ast, root) {
        let mut map = Map::new();
        map.insert(name, value);
        return Value::Object(map);
    }
    // Object-returning modules (e.g. `do { ...; { a: 1, b: 2 } }`) are their
    // own export table, supporting braced named imports like
    // `from "m" import { a, b }`.
    if let Value::Object(map) = value {
        if !map.is_empty() {
            return Value::Object(map);
        }
    }
    Value::Object(Map::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{
        ArrSource, AstNode, AstShapeRef, EvalSchemeDef, GrammarPosition, GrammarRow, Token,
        TokenKindDef,
    };
    use handl_platform::{EvalContext, Function, Module, ModuleLookup};
    use handl_plugin_engine_lex::Lexer;
    use handl_plugin_engine_parse::{Parser, ParserContext};
    use handl_plugin_module::ModuleRegistry;
    use plugin_engine::TermData;
    use serde_json::{Value, json};

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

    /// A stub typecheck engine that typechecks clean (no diagnostics).
    #[derive(Clone)]
    struct StubTypecheckClean {
        typ: Value,
    }

    impl TypecheckEngine for StubTypecheckClean {
        fn typecheck(
            &self,
            _lang: &Language,
            _req: &TypecheckRequest,
        ) -> Result<TypecheckResult, LangError> {
            Ok(TypecheckResult {
                typ: self.typ.clone(),
                diagnostics: Vec::new(),
                node_types: Default::default(),
            })
        }

        fn box_clone(&self) -> Box<dyn TypecheckEngine> {
            Box::new(self.clone())
        }
    }

    /// A stub typecheck engine that always fails with a type error.
    #[derive(Clone)]
    struct StubTypecheckErr;

    impl TypecheckEngine for StubTypecheckErr {
        fn typecheck(
            &self,
            _lang: &Language,
            _req: &TypecheckRequest,
        ) -> Result<TypecheckResult, LangError> {
            Err(LangError::engine("typecheck error: unbound variable 'x'"))
        }

        fn box_clone(&self) -> Box<dyn TypecheckEngine> {
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
        use handl_plugin_engine_exec_bytecode::BytecodeEngine;
        use handl_plugin_engine_exec_cranelift::CraneliftEngine;
        use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
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
        let add_scheme = {
            let lhs = TermData::op(
                "proj_child",
                vec![],
                &ArrSource::Native,
                Some(Value::from(0u64)),
            );
            let rhs = TermData::op(
                "proj_child",
                vec![],
                &ArrSource::Native,
                Some(Value::from(1u64)),
            );
            TermData::op("add", vec![lhs, rhs], &ArrSource::Native, None)
                .to_term()
                .expect("TermData round-trips to Term")
        };
        lang.eval
            .merge(EvalSchemeDef::new("e.add", "binary.add", add_scheme))
            .unwrap();
        lang
    }

    fn tok(kind: &str, text: &str) -> Token {
        Token {
            kind: kind.into(),
            text: text.into(),
            ..Default::default()
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
            handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new(),
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
        let dbl_scheme = {
            let arg = TermData::op(
                "proj_child",
                vec![],
                &ArrSource::Native,
                Some(Value::from(0u64)),
            );
            TermData::op(
                "double",
                vec![arg],
                &ArrSource::Foreign("double".into()),
                None,
            )
            .to_term()
            .expect("TermData round-trips to Term")
        };
        lang.eval
            .merge(EvalSchemeDef::new("e.dbl", "foreign.double", dbl_scheme))
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
        let _ = handl_plugin_engine_exec::Eval;
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
            ..Default::default()
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

    // -----------------------------------------------------------------------
    // run_checked: typecheck before eval (Phase 10.1)
    // -----------------------------------------------------------------------

    #[test]
    fn run_checked_typechecks_before_eval() {
        // lex + parse + (clean typecheck) + eval → Ok(value).
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: Tokens(vec![tok("token.number", "42")]),
            }))
            .with_parse_engine(StubParse)
            .with_eval_engine(Box::new(StubEval { value: json!(3) }))
            .with_typecheck_engine(Box::new(StubTypecheckClean { typ: json!(null) }));
        let result = rt.run_checked(&Source::new("1 + 2")).unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn run_checked_catches_type_error() {
        // The typecheck engine returns Err → eval is short-circuited.
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: Tokens(vec![tok("token.number", "42")]),
            }))
            .with_parse_engine(StubParse)
            .with_eval_engine(Box::new(StubEval { value: json!(999) }))
            .with_typecheck_engine(Box::new(StubTypecheckErr));
        let err = rt.run_checked(&Source::new("x")).unwrap_err();
        assert!(
            matches!(err, LangError::Engine { .. }),
            "expected LangError::Engine, got {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("typecheck error"),
            "expected typecheck error message, got: {msg}"
        );
    }

    #[test]
    fn run_still_works_without_typecheck() {
        // Backward compat: run() is lex → parse → eval (no typecheck).
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: Tokens(vec![tok("token.number", "42")]),
            }))
            .with_parse_engine(StubParse)
            .with_eval_engine(Box::new(StubEval { value: json!(3) }));
        let result = rt.run(&Source::new("1 + 2")).unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn run_checked_falls_through_without_typecheck_engine() {
        // No typecheck engine bound → falls through to eval (does not fail).
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: Tokens(vec![tok("token.number", "42")]),
            }))
            .with_parse_engine(StubParse)
            .with_eval_engine(Box::new(StubEval { value: json!(3) }));
        let result = rt.run_checked(&Source::new("1 + 2")).unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn run_checked_error_in_diagnostics_vec_short_circuits() {
        // A typecheck engine that returns Ok with an error-severity diagnostic
        // in the diagnostics vec (defensive path).
        #[derive(Clone)]
        struct StubTypecheckDiagErr;
        impl TypecheckEngine for StubTypecheckDiagErr {
            fn typecheck(
                &self,
                _lang: &Language,
                _req: &TypecheckRequest,
            ) -> Result<TypecheckResult, LangError> {
                use handl_plugin_engine_typecheck::TypecheckDiagnostic;
                Ok(TypecheckResult {
                    typ: json!(null),
                    diagnostics: vec![TypecheckDiagnostic {
                        severity: "error".to_string(),
                        message: "predicate failure".to_string(),
                        span: None,
                    }],
                    node_types: Default::default(),
                })
            }
            fn box_clone(&self) -> Box<dyn TypecheckEngine> {
                Box::new(self.clone())
            }
        }
        let rt = Runtime::new(Language::new())
            .with_lex_engine(Box::new(StubLex {
                tokens: Tokens(vec![tok("token.number", "42")]),
            }))
            .with_parse_engine(StubParse)
            .with_eval_engine(Box::new(StubEval { value: json!(999) }))
            .with_typecheck_engine(Box::new(StubTypecheckDiagErr));
        let err = rt.run_checked(&Source::new("1")).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
        assert!(err.to_string().contains("predicate failure"));
    }
}
