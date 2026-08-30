//! # Meta host functions: `lex`, `parse`, `eval`, `run`, `self`, `compile`,
//!   `run_compiled`
//!
//! ADR-0041 Phase 2 — the "meta is gone" step. These are handl-callable
//! host functions that expose the language pipeline stages as ordinary
//! functions:
//!
//! - `lex(lang, text)` → tokens (JSON-serialized [`Tokens`])
//! - `parse(lang, tokens)` → ast (JSON-serialized [`Ast`])
//! - `eval(lang, ast)` → result value (just the eval engine, no lex/parse)
//! - `run(lang, text)` → result value (convenience: `eval(lang, parse(lang, lex(lang, text)))`)
//! - `self` → the currently-running [`Language`] (an ambient global variable,
//!   not a host function call — ADR-0041 Phase 3)
//! - `compile(lang, ast, target)` → a compiled-program handle (ADR-0041 §5,
//!   ADR-0044 §2). `target` is `"bytecode"` or `"native"`. The handle is a
//!   tagged value `{"__compiled__": <id>, "target": <target>}` referencing a
//!   runtime-held registry entry carrying the non-serializable compiled
//!   artifact.
//! - `run_compiled(handle)` → result value. Runs a previously compiled
//!   program; equivalent to `eval(lang, ast)` but executes the pre-compiled
//!   artifact instead of recompiling.
//!
//! The key design: `eval(lang, ast)` takes a **pre-parsed AST**, not source
//! text. This separates the pipeline stages so a handl program can
//! manipulate ASTs and tokens as first-class values:
//! `run(lang, text) == eval(lang, parse(lang, lex(lang, text)))`.
//!
//! `compile` follows the same staged shape: it takes a pre-parsed `Ast` and
//! produces a compiled-program handle. `run_compiled(handle)` runs it. This
//! is **Option B** of the task spec: `compile(lang, ast, target)` returns a
//! compiled program directly (not a curried compiler). The engines compile a
//! specific `Ast` — each node in the arena is lowered to bytecode/native
//! code — so the artifact is tied to a `(lang, ast)` pair. This is the most
//! useful concrete signature for the self-hosting bootstrap (task 3:
//! compile the handl-defining-handl program, run it) without over-building a
//! curried-compiler layer the engines do not yet support. ADR-0041 §5's
//! `compile : Language -> Target` vision (a compiler for a language, callable
//! with any `Ast`) is the Futamura ideal; the engines currently compile a
//! specific `Ast`, so Option B is the honest, faithful-to-the-engines step.
//!
//! A `Language` is a genuine first-class value (ADR-0041): the first argument
//! to `lex`/`parse`/`eval`/`run`/`compile` is a tagged [`Value`] carrying the
//! actual [`handl_language::Language`] data (see
//! [`handl_language::language_value`]). The host functions extract the
//! [`Language`] directly — no string-handle registry lookup is required.
//!
//! [`MetaHostFunctions`] implements [`HostFunctions`] by returning the meta
//! functions for their names and delegating everything else to an optional
//! user-supplied [`HostFunctions`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use handl_language::{
    as_language, language_value, Ast, AstRef, LangError, Language, Source, Tokens, Value,
};
use handl_plugin_engine_eval::{HostFn, HostFunctions};
use handl_plugin_engine_eval_bytecode::{compile as compile_bytecode, BytecodeVm, Instr};
use handl_plugin_engine_eval_cranelift::{compile_all_nodes_jit, CompiledResult, CraneliftEngine};

use crate::{Runtime, RuntimeConfig};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Extract a [`Language`] value from the first argument of a meta host
/// function call.
fn extract_language(arg: Option<&Value>, fn_name: &str) -> Result<Language, LangError> {
    let v = arg.ok_or_else(|| {
        LangError::Engine(format!("{}: expected a Language value as first argument", fn_name))
    })?;
    as_language(v).ok_or_else(|| {
        LangError::Engine(format!(
            "{}: expected a Language value as first argument",
            fn_name
        ))
    })
}

/// Build a [`Runtime`] for `lang` with [`MetaHostFunctions`] attached, so the
/// sub-runtime can also call `lex`/`parse`/`eval`/`run`/`compile`.
///
/// Injects `self` as a global variable bound to a [`Language`] value carrying
/// `lang` (ADR-0041 Phase 3: `self` is an ambient variable, not a function
/// call). The sub-runtime's [`MetaHostFunctions`] carries `lang` as its
/// current language, so a 1-argument `run(text)` call inside the sub-runtime
/// evaluates `text` under `lang`.
///
/// The [`MetaHostFunctions`] is leaked to obtain a `'static` reference — it
/// lives for the process lifetime.
fn build_runtime(
    lang: Language,
    config: &RuntimeConfig,
    delegate: &Option<Rc<dyn HostFunctions>>,
) -> Result<Runtime<'static>, LangError> {
    let self_val = language_value(&lang);
    let sub_meta = MetaHostFunctions::new(lang.clone(), config.clone(), delegate.clone());
    let sub_meta_static: &'static MetaHostFunctions = Box::leak(Box::new(sub_meta));
    let mut globals = HashMap::new();
    globals.insert("self".to_string(), self_val);
    Runtime::from_config_with_globals(lang, config, Some(sub_meta_static), Some(&globals))
}

// ---------------------------------------------------------------------------
// lex(lang, text) → tokens
// ---------------------------------------------------------------------------

/// `lex(lang, text)` host function.
///
/// Extracts the [`Language`] from the first argument, builds a [`Runtime`],
/// and lexes `text` into tokens. Returns tokens as a JSON-serialized
/// [`Tokens`] value.
pub struct LexHostFn {
    config: RuntimeConfig,
    delegate: Option<Rc<dyn HostFunctions>>,
}

impl LexHostFn {
    /// Create a `lex` host function using `config` for sub-runtimes.
    pub fn new(config: RuntimeConfig, delegate: Option<Rc<dyn HostFunctions>>) -> Self {
        Self { config, delegate }
    }
}

impl HostFn for LexHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        let lang = extract_language(args.first(), "lex")?;
        let text = args
            .get(1)
            .and_then(|v| v.as_str())
            .ok_or_else(|| LangError::Engine("lex: expected (lang, text)".into()))?;
        let runtime = build_runtime(lang, &self.config, &self.delegate)?;
        let tokens = runtime.lex(&Source::new(text))?;
        serde_json::to_value(&tokens).map_err(|e| LangError::Engine(format!("lex: {}", e)))
    }
}

// ---------------------------------------------------------------------------
// parse(lang, tokens) → ast
// ---------------------------------------------------------------------------

/// `parse(lang, tokens)` host function.
///
/// Extracts the [`Language`] from the first argument, builds a [`Runtime`],
/// and parses `tokens` (JSON-serialized [`Tokens`]) into an AST. Returns the
/// AST as a JSON-serialized [`Ast`] value.
pub struct ParseHostFn {
    config: RuntimeConfig,
    delegate: Option<Rc<dyn HostFunctions>>,
}

impl ParseHostFn {
    /// Create a `parse` host function using `config` for sub-runtimes.
    pub fn new(config: RuntimeConfig, delegate: Option<Rc<dyn HostFunctions>>) -> Self {
        Self { config, delegate }
    }
}

impl HostFn for ParseHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        let lang = extract_language(args.first(), "parse")?;
        let tokens_val = args
            .get(1)
            .ok_or_else(|| LangError::Engine("parse: expected (lang, tokens)".into()))?;
        let tokens: Tokens = serde_json::from_value(tokens_val.clone())
            .map_err(|e| LangError::Engine(format!("parse: {}", e)))?;
        let runtime = build_runtime(lang, &self.config, &self.delegate)?;
        let ast = runtime.parse(&tokens)?;
        serde_json::to_value(&ast).map_err(|e| LangError::Engine(format!("parse: {}", e)))
    }
}

// ---------------------------------------------------------------------------
// eval(lang, ast) → value
// ---------------------------------------------------------------------------

/// `eval(lang, ast)` host function (ADR-0041 Phase 2).
///
/// Extracts the [`Language`] from the first argument, builds a [`Runtime`],
/// and evaluates `ast` (JSON-serialized [`Ast`]) through the eval engine only
/// — no lex/parse. Returns the result value.
pub struct EvalHostFn {
    config: RuntimeConfig,
    delegate: Option<Rc<dyn HostFunctions>>,
}

impl EvalHostFn {
    /// Create a new `eval` host function using `config` for sub-runtimes.
    pub fn new(config: RuntimeConfig, delegate: Option<Rc<dyn HostFunctions>>) -> Self {
        Self { config, delegate }
    }
}

impl HostFn for EvalHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        let lang = extract_language(args.first(), "eval")?;
        let ast_val = args
            .get(1)
            .ok_or_else(|| LangError::Engine("eval: expected (lang, ast)".into()))?;
        // Accept either a raw serialized `Ast` value or a tagged `__ast__`
        // value (the form bound by `Scheme::Vau` — ADR-0041 §6, ADR-0045).
        let ast: Ast = if let Some(tagged) = handl_language::as_ast(ast_val) {
            tagged
        } else {
            serde_json::from_value(ast_val.clone())
                .map_err(|e| LangError::Engine(format!("eval: {}", e)))?
        };
        let runtime = build_runtime(lang, &self.config, &self.delegate)?;
        runtime.eval(&ast)
    }
}

// ---------------------------------------------------------------------------
// run(lang, text) → value  (convenience: lex → parse → eval)
// ---------------------------------------------------------------------------

/// `run(lang, text)` host function — convenience composite.
///
/// Equivalent to `eval(lang, parse(lang, lex(lang, text)))`. Provided
/// so source programs can evaluate source strings without manually composing
/// the three stages.
///
/// A 1-argument form `run(text)` evaluates `text` under the current language
/// (the [`Language`] carried by this host function, matching the `self`
/// global).
pub struct RunHostFn {
    current_lang: Language,
    config: RuntimeConfig,
    delegate: Option<Rc<dyn HostFunctions>>,
}

impl RunHostFn {
    /// Create a `run` host function that uses `current_lang` for the
    /// 1-argument `run(text)` form and `config` for sub-runtimes.
    pub fn new(
        current_lang: Language,
        config: RuntimeConfig,
        delegate: Option<Rc<dyn HostFunctions>>,
    ) -> Self {
        Self {
            current_lang,
            config,
            delegate,
        }
    }
}

impl HostFn for RunHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        // Two calling conventions:
        //   run(lang, text) — 2-arg: explicit Language value
        //   run(text)       — 1-arg: use the current language (self)
        let (lang, text) = if args.len() >= 2 {
            let l = extract_language(args.first(), "run")?;
            let t = args
                .get(1)
                .and_then(|v| v.as_str())
                .ok_or_else(|| LangError::Engine("run: expected (lang, text) or (text)".into()))?;
            (l, t.to_string())
        } else if args.len() == 1 {
            let t = args
                .first()
                .and_then(|v| v.as_str())
                .ok_or_else(|| LangError::Engine("run: expected (lang, text) or (text)".into()))?;
            (self.current_lang.clone(), t.to_string())
        } else {
            return Err(LangError::Engine("run: expected (lang, text) or (text)".into()));
        };
        let runtime = build_runtime(lang, &self.config, &self.delegate)?;
        runtime.run(&Source::new(&text))
    }
}

// ---------------------------------------------------------------------------
// compile(lang, ast, target) → compiled-program handle
// ---------------------------------------------------------------------------

/// The sentinel object key marking a [`Value`] as a compiled-program handle.
///
/// A compiled-program handle is a JSON object
/// `{"__compiled__": <id>, "target": <target>}`. The `id` references an entry
/// in the thread-local [`COMPILED_REGISTRY`] carrying the non-serializable
/// compiled artifact (bytecode instructions or native function pointers +
/// JIT module). The handle itself is a serializable value; the artifact is
/// not — native code cannot be serialized into `serde_json::Value`.
pub const COMPILED_VALUE_KEY: &str = "__compiled__";

/// A native (cranelift) compiled program: the JIT compilation result
/// (per-node function pointers + constant pools + the JIT module kept
/// alive) plus the `Language` and `Ast`. Held in [`Rc`] so the
/// `CompiledResult` (and its JIT module) can be referenced during
/// execution without holding the registry borrow — the `Rc` clone keeps
/// it alive even if the registry entry is removed.
struct NativeProgram {
    /// The cranelift compilation result (owns the JIT module).
    result: CompiledResult,
    /// The language the program was compiled under.
    lang: Language,
    /// The AST the program was compiled from.
    ast: Ast,
}

/// A compiled program held in the runtime registry.
///
/// Carries the compiled artifact plus the `Language` and `Ast` it was
/// compiled from (needed to execute: the bytecode VM lazily compiles child
/// nodes using `lang` + `ast`; the cranelift helpers read `ast` node payloads
/// and `lang` at execution time).
enum CompiledProgram {
    /// A bytecode-compiled program: the root node's compiled instruction
    /// sequence plus the `Language` and `Ast` for lazy child compilation.
    Bytecode {
        /// The root node's compiled bytecode program.
        instrs: Vec<Instr>,
        /// The language the program was compiled under.
        lang: Language,
        /// The AST the program was compiled from.
        ast: Ast,
    },
    /// A native (cranelift) compiled program.
    Native(Rc<NativeProgram>),
}

// Thread-local registry of compiled programs, keyed by handle id.
//
// The artifacts are non-serializable (bytecode `Instr` vectors are
// serializable in principle, but cranelift `CompiledResult` holds native
// function pointers and a `JITModule` — not `serde`-serializable, not
// `Send`). A thread-local `RefCell` matches the engines' single-threaded
// execution model and avoids `Send` bounds the artifacts cannot meet.
thread_local! {
    static COMPILED_REGISTRY: RefCell<HashMap<u64, CompiledProgram>> =
        RefCell::new(HashMap::new());
}

/// Monotonic id generator for compiled-program handles.
static COMPILED_NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Build a compiled-program handle [`Value`] for `id` and `target`.
fn compiled_value(id: u64, target: &str) -> Value {
    serde_json::json!({
        COMPILED_VALUE_KEY: id,
        "target": target,
    })
}

/// Extract a compiled-program handle id from a [`Value`].
///
/// Returns `None` if the value is not a tagged compiled-program handle.
fn as_compiled_id(value: &Value) -> Option<u64> {
    value.get(COMPILED_VALUE_KEY).and_then(|v| v.as_u64())
}

/// Compile the root node's scheme to a bytecode program.
///
/// Mirrors [`BytecodeEngine::eval`](handl_plugin_engine_eval_bytecode::BytecodeEngine::eval):
/// resolves the root node's eval scheme via `lang.eval[node.kind]` and
/// compiles it, falling back to the no-scheme payload-load path.
fn compile_root_bytecode(lang: &Language, ast: &Ast) -> Result<Vec<Instr>, LangError> {
    if ast.is_empty() {
        return Ok(vec![]);
    }
    let root_ref = AstRef::new(ast.len() - 1);
    let root_node = ast.at(root_ref)?;
    let scheme_def = lang.eval.get_by_identity(&root_node.kind);
    match scheme_def {
        Some(def) => compile_bytecode(&def.scheme, root_node),
        None => Ok(vec![Instr::LoadPayload, Instr::Return]),
    }
}

/// `compile(lang, ast, target)` host function (ADR-0041 §5, ADR-0044 §2).
///
/// Compiles a pre-parsed `Ast` under `lang` to the requested `target` and
/// returns a compiled-program handle. The handle references a thread-local
/// registry entry holding the non-serializable compiled artifact.
///
/// `target = "bytecode"` compiles via the bytecode engine (root node's
/// scheme → `Vec<Instr>`; child nodes compile lazily at run time).
/// `target = "native"` (or `"cranelift"`) compiles via the cranelift engine
/// (all nodes → native function pointers via JIT).
///
/// Use [`RunCompiledHostFn`] (`run_compiled(handle)`) to execute the result.
pub struct CompileHostFn {
    config: RuntimeConfig,
}

impl CompileHostFn {
    /// Create a `compile` host function using `config` for sub-runtimes.
    pub fn new(config: RuntimeConfig) -> Self {
        Self { config }
    }
}

impl HostFn for CompileHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        let lang = extract_language(args.first(), "compile")?;
        let ast_val = args
            .get(1)
            .ok_or_else(|| LangError::Engine("compile: expected (lang, ast, target)".into()))?;
        let ast: Ast = serde_json::from_value(ast_val.clone())
            .map_err(|e| LangError::Engine(format!("compile: {}", e)))?;
        let target = args
            .get(2)
            .and_then(|v| v.as_str())
            .unwrap_or("native");

        let program = match target {
            "bytecode" => {
                let instrs = compile_root_bytecode(&lang, &ast)?;
                CompiledProgram::Bytecode { instrs, lang, ast }
            }
            "native" | "cranelift" => {
                if ast.is_empty() {
                    return Ok(compiled_value(0, target));
                }
                if !handl_language::SchemeVersion::v1().supports(&lang.scheme_version) {
                    return Err(LangError::Engine(format!(
                        "compile: unsupported scheme version: {} (engine supports v1)",
                        lang.scheme_version
                    )));
                }
                let result = compile_all_nodes_jit(&lang, &ast)?;
                CompiledProgram::Native(Rc::new(NativeProgram { result, lang, ast }))
            }
            other => {
                return Err(LangError::Engine(format!(
                    "compile: unknown target '{}' (expected \"bytecode\" or \"native\")",
                    other
                )));
            }
        };

        let id = COMPILED_NEXT_ID.fetch_add(1, Ordering::SeqCst) + 1;
        COMPILED_REGISTRY.with(|r| {
            r.borrow_mut().insert(id, program);
        });
        Ok(compiled_value(id, target))
    }
}

// ---------------------------------------------------------------------------
// run_compiled(handle) → value
// ---------------------------------------------------------------------------

/// `run_compiled(handle)` host function.
///
/// Executes a compiled program produced by [`CompileHostFn`]. The handle is
/// a tagged value `{"__compiled__": <id>, "target": ...}`. The program runs
/// with the meta host functions attached (so `Foreign` schemes resolve to
/// `lex`/`parse`/`eval`/`run`/`compile`/`run_compiled`), matching `eval`.
///
/// For `"bytecode"` programs, the root instruction program executes via the
/// bytecode VM (child nodes compile lazily on first access, as in
/// [`BytecodeEngine::eval`]). For `"native"` programs, the pre-compiled
/// cranelift function pointers execute via
/// [`CraneliftEngine::run_compiled`].
pub struct RunCompiledHostFn {
    config: RuntimeConfig,
    delegate: Option<Rc<dyn HostFunctions>>,
}

impl RunCompiledHostFn {
    /// Create a `run_compiled` host function using `config` for sub-runtimes
    /// and `delegate` for user host functions.
    pub fn new(
        config: RuntimeConfig,
        delegate: Option<Rc<dyn HostFunctions>>,
    ) -> Self {
        Self { config, delegate }
    }
}

impl HostFn for RunCompiledHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        let handle = args
            .first()
            .ok_or_else(|| LangError::Engine("run_compiled: expected (handle)".into()))?;
        let id = as_compiled_id(handle).ok_or_else(|| {
            LangError::Engine("run_compiled: expected a compiled-program handle".into())
        })?;

        // Clone the execution-relevant data out of the registry under a
        // short-lived borrow, then release the borrow before running. This
        // avoids re-entrancy panics if the compiled program itself calls
        // `compile`/`run_compiled` (which would re-borrow the registry).
        // For native programs, the `Rc<NativeProgram>` clone keeps the
        // `CompiledResult` (and its JIT module) alive during execution
        // without holding the registry borrow.
        enum RunData {
            Bytecode {
                instrs: Vec<Instr>,
                lang: Language,
                ast: Ast,
            },
            Native {
                native: Rc<NativeProgram>,
            },
            Empty,
        }
        let run_data = if id == 0 {
            RunData::Empty
        } else {
            COMPILED_REGISTRY.with(|r| {
                let reg = r.borrow();
                let program = reg.get(&id).ok_or_else(|| {
                    LangError::Engine(format!(
                        "run_compiled: no compiled program for handle {}",
                        id
                    ))
                })?;
                Ok::<RunData, LangError>(match program {
                    CompiledProgram::Bytecode { instrs, lang, ast } => RunData::Bytecode {
                        instrs: instrs.clone(),
                        lang: lang.clone(),
                        ast: ast.clone(),
                    },
                    CompiledProgram::Native(native) => RunData::Native {
                        native: Rc::clone(native),
                    },
                })
            })?
        };

        match run_data {
            RunData::Empty => Ok(Value::Null),
            RunData::Bytecode { instrs, lang, ast } => {
                if ast.is_empty() {
                    return Ok(Value::Null);
                }
                let sub_meta =
                    MetaHostFunctions::new(lang.clone(), self.config.clone(), self.delegate.clone());
                let sub_meta_static: &'static MetaHostFunctions =
                    Box::leak(Box::new(sub_meta));
                let vm = BytecodeVm::with_host(sub_meta_static);
                vm.execute(&lang, &ast, &instrs)
            }
            RunData::Native { native } => {
                if native.ast.is_empty() {
                    return Ok(Value::Null);
                }
                let sub_meta = MetaHostFunctions::new(
                    native.lang.clone(),
                    self.config.clone(),
                    self.delegate.clone(),
                );
                let sub_meta_static: &'static MetaHostFunctions =
                    Box::leak(Box::new(sub_meta));
                let engine = CraneliftEngine::with_host(sub_meta_static);
                engine.run_compiled(&native.lang, &native.ast, &native.result)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MetaHostFunctions registry
// ---------------------------------------------------------------------------

/// A [`HostFunctions`] registry that includes the meta functions
/// (`lex`/`parse`/`eval`/`run`/`compile`/`run_compiled`) and delegates
/// unknown names to an optional user-supplied [`HostFunctions`].
///
/// `self` is not a host function — it is injected as a global variable by
/// [`build_runtime`] / `Kit::new` (ADR-0041 Phase 3).
pub struct MetaHostFunctions {
    lex_fn: LexHostFn,
    parse_fn: ParseHostFn,
    eval_fn: EvalHostFn,
    run_fn: RunHostFn,
    compile_fn: CompileHostFn,
    run_compiled_fn: RunCompiledHostFn,
    delegate: Option<Rc<dyn HostFunctions>>,
}

impl Clone for MetaHostFunctions {
    fn clone(&self) -> Self {
        Self {
            lex_fn: LexHostFn {
                config: self.lex_fn.config.clone(),
                delegate: self.delegate.clone(),
            },
            parse_fn: ParseHostFn {
                config: self.parse_fn.config.clone(),
                delegate: self.delegate.clone(),
            },
            eval_fn: EvalHostFn {
                config: self.eval_fn.config.clone(),
                delegate: self.delegate.clone(),
            },
            run_fn: RunHostFn {
                current_lang: self.run_fn.current_lang.clone(),
                config: self.run_fn.config.clone(),
                delegate: self.delegate.clone(),
            },
            compile_fn: CompileHostFn {
                config: self.compile_fn.config.clone(),
            },
            run_compiled_fn: RunCompiledHostFn {
                config: self.run_compiled_fn.config.clone(),
                delegate: self.delegate.clone(),
            },
            delegate: self.delegate.clone(),
        }
    }
}

impl MetaHostFunctions {
    /// Create a `MetaHostFunctions` with `current_lang` as the running
    /// language, `config` for sub-runtimes, and an optional `delegate` for
    /// user functions.
    pub fn new(
        current_lang: Language,
        config: RuntimeConfig,
        delegate: Option<Rc<dyn HostFunctions>>,
    ) -> Self {
        Self {
            lex_fn: LexHostFn::new(config.clone(), delegate.clone()),
            parse_fn: ParseHostFn::new(config.clone(), delegate.clone()),
            eval_fn: EvalHostFn::new(config.clone(), delegate.clone()),
            run_fn: RunHostFn::new(current_lang, config.clone(), delegate.clone()),
            compile_fn: CompileHostFn::new(config.clone()),
            run_compiled_fn: RunCompiledHostFn::new(config, delegate.clone()),
            delegate,
        }
    }
}

impl HostFunctions for MetaHostFunctions {
    fn lookup(&self, name: &str) -> Option<&dyn HostFn> {
        match name {
            "lex" => Some(&self.lex_fn),
            "parse" => Some(&self.parse_fn),
            "eval" => Some(&self.eval_fn),
            "run" => Some(&self.run_fn),
            "compile" => Some(&self.compile_fn),
            "run_compiled" => Some(&self.run_compiled_fn),
            // Note: `self` is not a host function — it's injected as a
            // global variable by `build_runtime` / `Kit::new`. ADR-0041
            // Phase 3.
            _ => self.delegate.as_ref().and_then(|d| d.lookup(name)),
        }
    }
}

/// Build a [`MetaHostFunctions`] for the canonical handl language, using
/// `config` to select engines for sub-runtimes.
pub fn canonical_meta_host_functions(
    lang: Language,
    config: RuntimeConfig,
) -> Rc<MetaHostFunctions> {
    Rc::new(MetaHostFunctions::new(lang, config, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{language_value, Ast, AstNode};
    use serde_json::json;

    fn canonical_lang() -> Language {
        handl_kit_data_canonical::assemble()
            .expect("canonical kit must assemble")
            .lang()
            .clone()
    }

    fn make_runtime() -> Runtime<'static> {
        make_runtime_from_lang(canonical_lang())
    }

    /// Build a runtime from an explicit [`Language`] (for tests that
    /// override eval schemes). Injects `self` and wires meta host fns.
    fn make_runtime_from_lang(canonical: Language) -> Runtime<'static> {
        let config = RuntimeConfig::new()
            .with_lex("data")
            .with_parse("pratt")
            .with_eval("tree-walk");
        let meta = canonical_meta_host_functions(canonical.clone(), config.clone());
        // Leak meta to get &'static for the runtime.
        let meta_static: &'static MetaHostFunctions =
            Box::leak(Box::new(Rc::try_unwrap(meta).unwrap_or_else(|rc| (*rc).clone())));
        // Inject `self` as a global variable bound to the current Language
        // value (ADR-0041 Phase 3).
        let mut globals = HashMap::new();
        globals.insert("self".to_string(), language_value(&canonical));
        Runtime::from_config_with_globals(canonical, &config, Some(meta_static), Some(&globals))
            .expect("runtime must build")
    }

    /// `lex(lang, "1 + 2")` returns a JSON-serialized Tokens value.
    #[test]
    fn lex_host_function_returns_tokens() {
        let runtime = make_runtime();
        let lang_val = language_value(&canonical_lang());
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(lang_val));
        ast.push(AstNode::new("value").with_payload(json!("1 + 2")));
        ast.push(AstNode::new("foreign.lex").with_payload(json!({"children": [0, 1]})));
        let result = runtime.eval(&ast).expect("lex must succeed");
        // Tokens serialize as a JSON array of {kind, text} objects.
        assert!(result.is_array(), "lex should return an array, got: {}", result);
    }

    /// `parse(lang, tokens)` returns a JSON-serialized Ast value.
    #[test]
    fn parse_host_function_returns_ast() {
        let runtime = make_runtime();
        let lang_val = language_value(&canonical_lang());
        // First lex "1 + 2" to get tokens, then parse them.
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(lang_val.clone()));
        ast.push(AstNode::new("value").with_payload(json!("1 + 2")));
        ast.push(AstNode::new("foreign.lex").with_payload(json!({"children": [0, 1]})));
        let tokens = runtime.eval(&ast).expect("lex must succeed");

        // Now parse: 0: lang, 1: <tokens>, 2: foreign.parse {children:[0,1]}
        let mut ast2 = Ast::new();
        ast2.push(AstNode::new("value").with_payload(lang_val));
        ast2.push(AstNode::new("value").with_payload(tokens));
        ast2.push(AstNode::new("foreign.parse").with_payload(json!({"children": [0, 1]})));
        let result = runtime.eval(&ast2).expect("parse must succeed");
        // Ast serializes as {"nodes": [...]}
        assert!(
            result.get("nodes").is_some(),
            "parse should return an Ast with 'nodes', got: {}",
            result
        );
    }

    /// `eval(lang, ast)` evaluates a pre-parsed AST and returns the result.
    #[test]
    fn eval_host_function_evals_ast() {
        let runtime = make_runtime();
        let lang_val = language_value(&canonical_lang());

        // Build a simple AST: 0: value 42
        let mut sub_ast = Ast::new();
        sub_ast.push(AstNode::new("value").with_payload(json!(42)));
        let sub_ast_val = serde_json::to_value(&sub_ast).unwrap();

        // Build the calling AST: 0: lang, 1: <ast>, 2: foreign.eval
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(lang_val));
        ast.push(AstNode::new("value").with_payload(sub_ast_val));
        ast.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
        let result = runtime.eval(&ast).expect("eval must succeed");
        assert_eq!(result, json!(42), "eval(lang, ast[42]) should return 42");
    }

    /// Full pipeline: `eval(lang, parse(lang, lex(lang, text)))` == `run(lang, text)`.
    #[test]
    fn pipeline_eval_parse_lex_equals_run() {
        let runtime = make_runtime();
        let lang_val = language_value(&canonical_lang());

        // Build: lex(lang, "1 + 2")
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(lang_val.clone()));
        ast.push(AstNode::new("value").with_payload(json!("1 + 2")));
        ast.push(AstNode::new("foreign.lex").with_payload(json!({"children": [0, 1]})));
        let tokens = runtime.eval(&ast).expect("lex must succeed");

        // parse(lang, tokens)
        let mut ast2 = Ast::new();
        ast2.push(AstNode::new("value").with_payload(lang_val.clone()));
        ast2.push(AstNode::new("value").with_payload(tokens));
        ast2.push(AstNode::new("foreign.parse").with_payload(json!({"children": [0, 1]})));
        let parsed_ast = runtime.eval(&ast2).expect("parse must succeed");

        // eval(lang, parsed_ast)
        let mut ast3 = Ast::new();
        ast3.push(AstNode::new("value").with_payload(lang_val.clone()));
        ast3.push(AstNode::new("value").with_payload(parsed_ast));
        ast3.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
        let pipeline_result = runtime.eval(&ast3).expect("eval must succeed");

        // run(lang, "1 + 2")
        let mut ast4 = Ast::new();
        ast4.push(AstNode::new("value").with_payload(lang_val));
        ast4.push(AstNode::new("value").with_payload(json!("1 + 2")));
        ast4.push(AstNode::new("foreign.run").with_payload(json!({"children": [0, 1]})));
        let run_result = runtime.eval(&ast4).expect("run must succeed");

        assert_eq!(
            pipeline_result, run_result,
            "eval(parse(lex(text))) should equal run(text)"
        );
        assert_eq!(pipeline_result, json!(3));
    }

    /// `self` is injected as a global variable bound to the current
    /// [`Language`] value (ADR-0041 Phase 3: `self` is an ambient variable,
    /// not a fn call).
    #[test]
    fn self_global_returns_current_language() {
        let runtime = make_runtime();
        // `self` is a var node — the tree-walk looks it up in the root scope
        // where `build_runtime` injected `self = language_value(lang)`.
        let mut ast = Ast::new();
        ast.push(AstNode::new("var").with_payload(json!("self")));
        let result = runtime.eval(&ast).expect("self must succeed");
        assert!(
            handl_language::is_language_value(&result),
            "self should return a Language value, got: {}",
            result
        );
        assert_eq!(
            handl_language::as_language(&result),
            Some(canonical_lang()),
            "self should carry the canonical Language"
        );
    }

    /// `run(self, "1 + 2")` works end-to-end: `self` is a Language value
    /// passed as the first argument to `run`.
    #[test]
    fn run_with_self_global() {
        let runtime = make_runtime();
        // 0: var "self", 1: value "1 + 2", 2: foreign.run {children:[0,1]}
        let mut ast = Ast::new();
        ast.push(AstNode::new("var").with_payload(json!("self")));
        ast.push(AstNode::new("value").with_payload(json!("1 + 2")));
        ast.push(AstNode::new("foreign.run").with_payload(json!({"children": [0, 1]})));
        let result = runtime.eval(&ast).expect("run(self, ...) must succeed");
        assert_eq!(result, json!(3));
    }

    /// Parse `source` under the canonical language into an [`Ast`] value
    /// (JSON-serialized), for use as the `ast` argument to `compile`.
    fn parse_ast_value(runtime: &Runtime<'static>, source: &str) -> Value {
        let lang_val = language_value(&canonical_lang());
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(lang_val));
        ast.push(AstNode::new("value").with_payload(json!(source)));
        ast.push(AstNode::new("foreign.lex").with_payload(json!({"children": [0, 1]})));
        let tokens = runtime.eval(&ast).expect("lex must succeed");
        let lang_val = language_value(&canonical_lang());
        let mut ast2 = Ast::new();
        ast2.push(AstNode::new("value").with_payload(lang_val));
        ast2.push(AstNode::new("value").with_payload(tokens));
        ast2.push(AstNode::new("foreign.parse").with_payload(json!({"children": [0, 1]})));
        runtime.eval(&ast2).expect("parse must succeed")
    }

    /// `compile(lang, ast, "bytecode")` returns a compiled-program handle
    /// (not the old stub string), and `run_compiled(handle)` produces the
    /// same result as `eval(lang, ast)`.
    #[test]
    fn compile_bytecode_run_compiled_matches_eval() {
        let runtime = make_runtime();
        let lang_val = language_value(&canonical_lang());
        let ast_val = parse_ast_value(&runtime, "1 + 2");

        // compile(lang, ast, "bytecode")
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(lang_val.clone()));
        ast.push(AstNode::new("value").with_payload(ast_val.clone()));
        ast.push(AstNode::new("value").with_payload(json!("bytecode")));
        ast.push(AstNode::new("foreign.compile").with_payload(json!({"children": [0, 1, 2]})));
        let handle = runtime.eval(&ast).expect("compile must succeed");

        // The result is a tagged handle, not the old stub string.
        assert!(
            !handle.is_string(),
            "compile should return a handle, not a stub string, got: {}",
            handle
        );
        assert!(
            as_compiled_id(&handle).is_some(),
            "compile should return a compiled-program handle, got: {}",
            handle
        );

        // run_compiled(handle)
        let mut ast2 = Ast::new();
        ast2.push(AstNode::new("value").with_payload(handle));
        ast2.push(AstNode::new("foreign.run_compiled").with_payload(json!({"children": [0]})));
        let compiled_result = runtime.eval(&ast2).expect("run_compiled must succeed");

        // eval(lang, ast) for comparison
        let mut ast3 = Ast::new();
        ast3.push(AstNode::new("value").with_payload(lang_val));
        ast3.push(AstNode::new("value").with_payload(ast_val));
        ast3.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
        let eval_result = runtime.eval(&ast3).expect("eval must succeed");

        assert_eq!(
            compiled_result, eval_result,
            "run_compiled should match eval for the same ast"
        );
        assert_eq!(compiled_result, json!(3));
    }

    /// `compile(lang, ast, "native")` returns a compiled-program handle and
    /// `run_compiled(handle)` produces the same result as `eval(lang, ast)`.
    #[test]
    fn compile_native_run_compiled_matches_eval() {
        let runtime = make_runtime();
        let lang_val = language_value(&canonical_lang());
        let ast_val = parse_ast_value(&runtime, "1 + 2");

        // compile(lang, ast, "native")
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(lang_val.clone()));
        ast.push(AstNode::new("value").with_payload(ast_val.clone()));
        ast.push(AstNode::new("value").with_payload(json!("native")));
        ast.push(AstNode::new("foreign.compile").with_payload(json!({"children": [0, 1, 2]})));
        let handle = runtime.eval(&ast).expect("compile must succeed");

        // The result is a tagged handle, not the old stub string.
        assert!(
            !handle.is_string(),
            "compile should return a handle, not a stub string, got: {}",
            handle
        );
        assert!(
            as_compiled_id(&handle).is_some(),
            "compile should return a compiled-program handle, got: {}",
            handle
        );

        // run_compiled(handle)
        let mut ast2 = Ast::new();
        ast2.push(AstNode::new("value").with_payload(handle));
        ast2.push(AstNode::new("foreign.run_compiled").with_payload(json!({"children": [0]})));
        let compiled_result = runtime.eval(&ast2).expect("run_compiled must succeed");

        // eval(lang, ast) for comparison
        let mut ast3 = Ast::new();
        ast3.push(AstNode::new("value").with_payload(lang_val));
        ast3.push(AstNode::new("value").with_payload(ast_val));
        ast3.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
        let eval_result = runtime.eval(&ast3).expect("eval must succeed");

        assert_eq!(
            compiled_result, eval_result,
            "run_compiled (native) should match eval for the same ast"
        );
        assert_eq!(compiled_result, json!(3));
    }

    /// `compile` with an unknown target returns a clear error.
    #[test]
    fn compile_unknown_target_returns_error() {
        let runtime = make_runtime();
        let lang_val = language_value(&canonical_lang());
        let ast_val = parse_ast_value(&runtime, "1");
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(lang_val));
        ast.push(AstNode::new("value").with_payload(ast_val));
        ast.push(AstNode::new("value").with_payload(json!("wasm")));
        ast.push(AstNode::new("foreign.compile").with_payload(json!({"children": [0, 1, 2]})));
        let err = runtime.eval(&ast).unwrap_err();
        assert!(
            err.to_string().contains("unknown target"),
            "expected unknown-target error, got: {}",
            err
        );
    }

    /// `run_compiled` with a non-handle value returns a clear error.
    #[test]
    fn run_compiled_non_handle_returns_error() {
        let runtime = make_runtime();
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!("not-a-handle")));
        ast.push(AstNode::new("foreign.run_compiled").with_payload(json!({"children": [0]})));
        let err = runtime.eval(&ast).unwrap_err();
        assert!(
            err.to_string().contains("compiled-program handle"),
            "expected handle error, got: {}",
            err
        );
    }

    /// `eval` with a non-Language first argument returns an error.
    #[test]
    fn eval_non_language_first_arg_returns_error() {
        let runtime = make_runtime();
        let mut sub_ast = Ast::new();
        sub_ast.push(AstNode::new("value").with_payload(json!(1)));
        let sub_ast_val = serde_json::to_value(&sub_ast).unwrap();
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!("not-a-language")));
        ast.push(AstNode::new("value").with_payload(sub_ast_val));
        ast.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
        let err = runtime.eval(&ast).unwrap_err();
        assert!(
            err.to_string().contains("Language value"),
            "got: {}",
            err
        );
    }

    // --- Vau (fexpr) tests (ADR-0041 §6, ADR-0045) ---

    /// The canonical `vau` node kind binds its child unevaluated and
    /// returns the `__ast__` value. The child is never evaluated.
    #[test]
    fn vau_canonical_returns_unevaluated_ast() {
        let runtime = make_runtime();
        // 0: value 42, 1: vau {children:[0]}
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!(42)));
        ast.push(AstNode::new("vau").with_payload(json!({"children": [0]})));
        let result = runtime.eval(&ast).expect("vau must succeed");
        // The result is a tagged __ast__ value wrapping the subtree.
        assert!(
            handl_language::is_ast_value(&result),
            "vau should return an __ast__ value, got: {}",
            result
        );
        let inner = handl_language::as_ast(&result).expect("must deserialize");
        // The subtree root is the `value` node.
        let root = &inner.nodes()[inner.len() - 1];
        assert_eq!(root.kind, "value");
        assert_eq!(root.payload, json!(42));
    }

    /// The ArrowApply escape: a `vau` node binds its child unevaluated,
    /// then the body calls `eval(self, expr)` to evaluate it on demand.
    /// This composes `Scheme::Vau` with the meta `eval` host function.
    #[test]
    fn vau_eval_escape_evaluates_child_on_demand() {
        // Build a language where vau's body is
        //   eval(self, expr)  i.e. App{var eval, [var self, var expr]}
        let mut lang = canonical_lang();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "eval.vau-escape",
                "vau",
                handl_language::Scheme::vau(
                    vec!["expr".into()],
                    handl_language::Scheme::App {
                        callee: Box::new(handl_language::Scheme::var("eval")),
                        args: vec![
                            handl_language::Scheme::var("self"),
                            handl_language::Scheme::var("expr"),
                        ],
                    },
                ),
            )
            .with_replaces(vec!["eval.vau".to_string()]))
            .unwrap();
        let runtime = make_runtime_from_lang(lang);
        // 0: value 1, 1: value 2, 2: binary.add {children:[0,1]}
        // 3: vau {children:[2]}
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!(1)));
        ast.push(AstNode::new("value").with_payload(json!(2)));
        ast.push(AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})));
        ast.push(AstNode::new("vau").with_payload(json!({"children": [2]})));
        let result = runtime.eval(&ast).expect("vau eval escape must succeed");
        assert_eq!(result, json!(3));
    }
}
