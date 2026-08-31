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
//!   ADR-0044 §2, ADR-0047 §3). `target` is `"bytecode"` or `"native"`. The
//!   handle is a tagged value `{"__compiled__": <id>, "target": <target>}`
//!   referencing a runtime-held registry entry carrying the non-serializable
//!   compiled artifact.
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
//! [`MetaHostFunctions`] implements [`ModuleLookup`] by returning the meta
//! functions for their names and delegating everything else to an optional
//! user-supplied [`ModuleLookup`].
//!
//! ## Registry-based (ADR-0047 §2)
//!
//! `MetaHostFunctions` carries an `Rc<EngineRegistry>` (plus a
//! [`RuntimeConfig`]); `build_runtime` resolves the config against the
//! registry via [`Runtime::from_registry_with_globals`]. `handl-runtime`
//! names no engine crate — the registry is the indirection. `compile` /
//! `run_compiled` delegate to [`Runtime::compile`] /
//! [`Runtime::run_compiled`], which use the [`CompileEngine`] /
//! [`CompiledArtifact`] traits (ADR-0047 §3); the engine impls live in the
//! engine crates, not here.

use std::collections::HashMap;
use std::rc::Rc;

use handl_language::{
    Ast, LangError, Language, Source, Tokens, Value, as_language, language_value,
};
use handl_platform::{EvalContext, EvalError, Function, ModuleLookup};

use crate::{CompiledHandle, EngineRegistry, Runtime, RuntimeConfig};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Extract a [`Language`] value from the first argument of a meta host
/// function call.
fn extract_language(arg: Option<&Value>, fn_name: &str) -> Result<Language, LangError> {
    let v = arg.ok_or_else(|| {
        LangError::Engine(format!(
            "{}: expected a Language value as first argument",
            fn_name
        ))
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
fn build_runtime(
    lang: Language,
    config: &RuntimeConfig,
    registry: &Rc<EngineRegistry>,
    delegate: &Option<Rc<dyn ModuleLookup>>,
) -> Result<Runtime, LangError> {
    let self_val = language_value(&lang);
    let sub_meta = MetaHostFunctions::new(
        lang.clone(),
        config.clone(),
        Rc::clone(registry),
        delegate.clone(),
    );
    let sub_meta_rc: Rc<dyn ModuleLookup> = Rc::new(sub_meta);
    let mut globals = HashMap::new();
    globals.insert("self".to_string(), self_val);
    Runtime::from_registry_with_globals(lang, registry, config, Some(sub_meta_rc), Some(&globals))
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
    registry: Rc<EngineRegistry>,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl LexHostFn {
    /// Create a `lex` host function using `config` + `registry` for
    /// sub-runtimes.
    pub fn new(
        config: RuntimeConfig,
        registry: Rc<EngineRegistry>,
        delegate: Option<Rc<dyn ModuleLookup>>,
    ) -> Self {
        Self {
            config,
            registry,
            delegate,
        }
    }
}

impl Function for LexHostFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let lang =
            extract_language(args.first(), "lex").map_err(|e| EvalError::Other(e.to_string()))?;
        let text = args
            .get(1)
            .and_then(|v| v.as_str())
            .ok_or_else(|| EvalError::Other("lex: expected (lang, text)".into()))?;
        let runtime = build_runtime(lang, &self.config, &self.registry, &self.delegate)
            .map_err(|e| EvalError::Other(e.to_string()))?;
        let tokens = runtime
            .lex(&Source::new(text))
            .map_err(|e| EvalError::Other(e.to_string()))?;
        serde_json::to_value(&tokens).map_err(|e| EvalError::Other(format!("lex: {}", e)))
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
    registry: Rc<EngineRegistry>,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl ParseHostFn {
    /// Create a `parse` host function using `config` + `registry` for
    /// sub-runtimes.
    pub fn new(
        config: RuntimeConfig,
        registry: Rc<EngineRegistry>,
        delegate: Option<Rc<dyn ModuleLookup>>,
    ) -> Self {
        Self {
            config,
            registry,
            delegate,
        }
    }
}

impl Function for ParseHostFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let lang =
            extract_language(args.first(), "parse").map_err(|e| EvalError::Other(e.to_string()))?;
        let tokens_val = args
            .get(1)
            .ok_or_else(|| EvalError::Other("parse: expected (lang, tokens)".into()))?;
        let tokens: Tokens = serde_json::from_value(tokens_val.clone())
            .map_err(|e| EvalError::Other(format!("parse: {}", e)))?;
        let runtime = build_runtime(lang, &self.config, &self.registry, &self.delegate)
            .map_err(|e| EvalError::Other(e.to_string()))?;
        let ast = runtime
            .parse(&tokens)
            .map_err(|e| EvalError::Other(e.to_string()))?;
        serde_json::to_value(&ast).map_err(|e| EvalError::Other(format!("parse: {}", e)))
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
    registry: Rc<EngineRegistry>,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl EvalHostFn {
    /// Create a new `eval` host function using `config` + `registry` for
    /// sub-runtimes.
    pub fn new(
        config: RuntimeConfig,
        registry: Rc<EngineRegistry>,
        delegate: Option<Rc<dyn ModuleLookup>>,
    ) -> Self {
        Self {
            config,
            registry,
            delegate,
        }
    }
}

impl Function for EvalHostFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let lang =
            extract_language(args.first(), "eval").map_err(|e| EvalError::Other(e.to_string()))?;
        let ast_val = args
            .get(1)
            .ok_or_else(|| EvalError::Other("eval: expected (lang, ast)".into()))?;
        // Accept either a raw serialized `Ast` value or a tagged `__ast__`
        // value (the form bound by `Scheme::Vau` — ADR-0041 §6, ADR-0045).
        let ast: Ast = if let Some(tagged) = handl_language::as_ast(ast_val) {
            tagged
        } else {
            serde_json::from_value(ast_val.clone())
                .map_err(|e| EvalError::Other(format!("eval: {}", e)))?
        };
        let runtime = build_runtime(lang, &self.config, &self.registry, &self.delegate)
            .map_err(|e| EvalError::Other(e.to_string()))?;
        runtime
            .eval(&ast)
            .map_err(|e| EvalError::Other(e.to_string()))
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
    registry: Rc<EngineRegistry>,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl RunHostFn {
    /// Create a `run` host function that uses `current_lang` for the
    /// 1-argument `run(text)` form and `config` + `registry` for
    /// sub-runtimes.
    pub fn new(
        current_lang: Language,
        config: RuntimeConfig,
        registry: Rc<EngineRegistry>,
        delegate: Option<Rc<dyn ModuleLookup>>,
    ) -> Self {
        Self {
            current_lang,
            config,
            registry,
            delegate,
        }
    }
}

impl Function for RunHostFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        // Two calling conventions:
        //   run(lang, text) — 2-arg: explicit Language value
        //   run(text)       — 1-arg: use the current language (self)
        let (lang, text) =
            if args.len() >= 2 {
                let l = extract_language(args.first(), "run")
                    .map_err(|e| EvalError::Other(e.to_string()))?;
                let t = args.get(1).and_then(|v| v.as_str()).ok_or_else(|| {
                    EvalError::Other("run: expected (lang, text) or (text)".into())
                })?;
                (l, t.to_string())
            } else if args.len() == 1 {
                let t = args.first().and_then(|v| v.as_str()).ok_or_else(|| {
                    EvalError::Other("run: expected (lang, text) or (text)".into())
                })?;
                (self.current_lang.clone(), t.to_string())
            } else {
                return Err(EvalError::Other(
                    "run: expected (lang, text) or (text)".into(),
                ));
            };
        let runtime = build_runtime(lang, &self.config, &self.registry, &self.delegate)
            .map_err(|e| EvalError::Other(e.to_string()))?;
        runtime
            .run(&Source::new(&text))
            .map_err(|e| EvalError::Other(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// compile(lang, ast, target) → compiled-program handle
// ---------------------------------------------------------------------------

/// `compile(lang, ast, target)` host function (ADR-0041 §5, ADR-0044 §2,
/// ADR-0047 §3).
///
/// Compiles a pre-parsed `Ast` under `lang` to the requested `target` and
/// returns a compiled-program handle. The handle references a thread-local
/// registry entry holding the non-serializable compiled artifact.
///
/// `target = "bytecode"` compiles via the bytecode compile engine.
/// `target = "native"` (or `"cranelift"`) compiles via the cranelift compile
/// engine.
///
/// Delegates to [`Runtime::compile`], which looks up the [`CompileEngine`]
/// for `target` in the [`EngineRegistry`] (ADR-0047 §3). The compile engine
/// impls live in the engine crates, not in `handl-runtime`.
///
/// Use [`RunCompiledHostFn`] (`run_compiled(handle)`) to execute the result.
pub struct CompileHostFn {
    config: RuntimeConfig,
    registry: Rc<EngineRegistry>,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl CompileHostFn {
    /// Create a `compile` host function using `config` + `registry` for
    /// sub-runtimes.
    pub fn new(
        config: RuntimeConfig,
        registry: Rc<EngineRegistry>,
        delegate: Option<Rc<dyn ModuleLookup>>,
    ) -> Self {
        Self {
            config,
            registry,
            delegate,
        }
    }
}

impl Function for CompileHostFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let lang = extract_language(args.first(), "compile")
            .map_err(|e| EvalError::Other(e.to_string()))?;
        let ast_val = args
            .get(1)
            .ok_or_else(|| EvalError::Other("compile: expected (lang, ast, target)".into()))?;
        let ast: Ast = serde_json::from_value(ast_val.clone())
            .map_err(|e| EvalError::Other(format!("compile: {}", e)))?;
        let target = args.get(2).and_then(|v| v.as_str()).unwrap_or("native");

        let runtime = build_runtime(lang, &self.config, &self.registry, &self.delegate)
            .map_err(|e| EvalError::Other(e.to_string()))?;
        let handle = runtime
            .compile(&ast, target)
            .map_err(|e| EvalError::Other(e.to_string()))?;
        // Return the handle as a tagged JSON value (same shape as before:
        // {"__compiled__": <id>, "target": <target>}).
        Ok(crate::compiled_value(handle.id, &handle.target))
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
/// Delegates to [`Runtime::run_compiled`], which looks up the
/// [`CompiledArtifact`] in the thread-local [`COMPILED_REGISTRY`] and
/// executes it with the runtime's host-functions registry attached
/// (ADR-0047 §3).
pub struct RunCompiledHostFn {
    config: RuntimeConfig,
    registry: Rc<EngineRegistry>,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl RunCompiledHostFn {
    /// Create a `run_compiled` host function using `config` + `registry`
    /// for sub-runtimes and `delegate` for user host functions.
    pub fn new(
        config: RuntimeConfig,
        registry: Rc<EngineRegistry>,
        delegate: Option<Rc<dyn ModuleLookup>>,
    ) -> Self {
        Self {
            config,
            registry,
            delegate,
        }
    }
}

impl Function for RunCompiledHostFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let handle = args
            .first()
            .ok_or_else(|| EvalError::Other("run_compiled: expected (handle)".into()))?;
        let id = crate::as_compiled_id(handle).ok_or_else(|| {
            EvalError::Other("run_compiled: expected a compiled-program handle".into())
        })?;
        let target = crate::as_compiled_target(handle).unwrap_or("");

        let handle = CompiledHandle {
            id,
            target: target.to_string(),
        };

        // Build a runtime to execute the compiled artifact. The runtime
        // re-attaches the meta host functions so the compiled program can
        // recurse via `Foreign` dispatch.
        //
        // We don't have the original `lang` / `ast` here — the artifact
        // carries them (ADR-0047 §3). We build a runtime with a dummy
        // language (the artifact's `run` method uses its own `lang`/`ast`).
        let dummy_lang = Language::new();
        let runtime = build_runtime(dummy_lang, &self.config, &self.registry, &self.delegate)
            .map_err(|e| EvalError::Other(e.to_string()))?;
        runtime
            .run_compiled(&handle)
            .map_err(|e| EvalError::Other(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// MetaHostFunctions registry
// ---------------------------------------------------------------------------

/// A [`ModuleLookup`] registry that includes the meta functions
/// (`lex`/`parse`/`eval`/`run`/`compile`/`run_compiled`) and delegates
/// unknown names to an optional user-supplied [`ModuleLookup`].
///
/// `self` is not a host function — it is injected as a global variable by
/// [`build_runtime`] / `Kit::new` (ADR-0041 Phase 3).
///
/// Carries an `Rc<EngineRegistry>` (ADR-0047 §2) so sub-runtimes can resolve
/// engine names against the same registry the host populated.
pub struct MetaHostFunctions {
    lex_fn: LexHostFn,
    parse_fn: ParseHostFn,
    eval_fn: EvalHostFn,
    run_fn: RunHostFn,
    compile_fn: CompileHostFn,
    run_compiled_fn: RunCompiledHostFn,
    delegate: Option<Rc<dyn ModuleLookup>>,
}

impl Clone for MetaHostFunctions {
    fn clone(&self) -> Self {
        Self {
            lex_fn: LexHostFn {
                config: self.lex_fn.config.clone(),
                registry: Rc::clone(&self.lex_fn.registry),
                delegate: self.delegate.clone(),
            },
            parse_fn: ParseHostFn {
                config: self.parse_fn.config.clone(),
                registry: Rc::clone(&self.parse_fn.registry),
                delegate: self.delegate.clone(),
            },
            eval_fn: EvalHostFn {
                config: self.eval_fn.config.clone(),
                registry: Rc::clone(&self.eval_fn.registry),
                delegate: self.delegate.clone(),
            },
            run_fn: RunHostFn {
                current_lang: self.run_fn.current_lang.clone(),
                config: self.run_fn.config.clone(),
                registry: Rc::clone(&self.run_fn.registry),
                delegate: self.delegate.clone(),
            },
            compile_fn: CompileHostFn {
                config: self.compile_fn.config.clone(),
                registry: Rc::clone(&self.compile_fn.registry),
                delegate: self.delegate.clone(),
            },
            run_compiled_fn: RunCompiledHostFn {
                config: self.run_compiled_fn.config.clone(),
                registry: Rc::clone(&self.run_compiled_fn.registry),
                delegate: self.delegate.clone(),
            },
            delegate: self.delegate.clone(),
        }
    }
}

impl MetaHostFunctions {
    /// Create a `MetaHostFunctions` with `current_lang` as the running
    /// language, `config` for sub-runtimes, `registry` for engine name
    /// resolution (ADR-0047 §2), and an optional `delegate` for user
    /// functions.
    pub fn new(
        current_lang: Language,
        config: RuntimeConfig,
        registry: Rc<EngineRegistry>,
        delegate: Option<Rc<dyn ModuleLookup>>,
    ) -> Self {
        Self {
            lex_fn: LexHostFn::new(config.clone(), Rc::clone(&registry), delegate.clone()),
            parse_fn: ParseHostFn::new(config.clone(), Rc::clone(&registry), delegate.clone()),
            eval_fn: EvalHostFn::new(config.clone(), Rc::clone(&registry), delegate.clone()),
            run_fn: RunHostFn::new(
                current_lang,
                config.clone(),
                Rc::clone(&registry),
                delegate.clone(),
            ),
            compile_fn: CompileHostFn::new(config.clone(), Rc::clone(&registry), delegate.clone()),
            run_compiled_fn: RunCompiledHostFn::new(config, registry, delegate.clone()),
            delegate,
        }
    }
}

impl ModuleLookup for MetaHostFunctions {
    fn lookup(&self, name: &str) -> Option<&dyn Function> {
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
/// `config` to select engines and `registry` for engine resolution
/// (ADR-0047 §2).
pub fn canonical_meta_host_functions(
    lang: Language,
    config: RuntimeConfig,
    registry: Rc<EngineRegistry>,
) -> Rc<MetaHostFunctions> {
    Rc::new(MetaHostFunctions::new(lang, config, registry, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{Ast, AstNode, language_value};
    use handl_plugin_engine_eval_cranelift::CraneliftEngine;
    use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
    use handl_plugin_engine_lex_pattern::DataLexerEngine;
    use handl_plugin_engine_parse_pratt::PrattEngine;
    use serde_json::json;

    fn canonical_lang() -> Language {
        handl_kit_data_canonical::assemble()
            .expect("canonical kit must assemble")
            .lang()
            .clone()
    }

    /// Build the default engine registry for tests: data lexer, pratt parser,
    /// tree-walk/bytecode/cranelift eval. Compile engines are NOT registered
    /// here to avoid a dev-dependency cycle (engine crates depend on
    /// `handl-runtime` for the `CompileEngine` trait).
    fn test_registry() -> Rc<EngineRegistry> {
        let mut reg = EngineRegistry::new();
        reg.register_lex("data", Box::new(DataLexerEngine::new()));
        reg.register_parse("pratt", PrattEngine::new());
        reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
        reg.register_eval(
            "bytecode",
            Box::new(handl_plugin_engine_eval_bytecode::BytecodeEngine::new()),
        );
        reg.register_eval("cranelift", Box::new(CraneliftEngine::new()));
        Rc::new(reg)
    }

    fn make_runtime() -> Runtime {
        make_runtime_from_lang(canonical_lang())
    }

    /// Build a runtime from an explicit [`Language`] (for tests that
    /// override eval schemes). Injects `self` and wires meta host fns.
    fn make_runtime_from_lang(canonical: Language) -> Runtime {
        let config = RuntimeConfig::new()
            .with_lex("data")
            .with_parse("pratt")
            .with_eval("tree-walk");
        let registry = test_registry();
        let meta = canonical_meta_host_functions(canonical.clone(), config.clone(), registry);
        // Inject `self` as a global variable bound to the current Language
        // value (ADR-0041 Phase 3).
        let meta_rc: Rc<dyn ModuleLookup> = meta;
        let mut globals = HashMap::new();
        globals.insert("self".to_string(), language_value(&canonical));
        Runtime::from_registry_with_globals(
            canonical,
            &test_registry(),
            &config,
            Some(meta_rc),
            Some(&globals),
        )
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
        assert!(
            result.is_array(),
            "lex should return an array, got: {}",
            result
        );
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
    fn parse_ast_value(runtime: &Runtime, source: &str) -> Value {
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
            err.to_string().contains("unknown compile target"),
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
        assert!(err.to_string().contains("Language value"), "got: {}", err);
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
            .merge(
                handl_language::EvalSchemeDef::new(
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
                .with_replaces(vec!["eval.vau".to_string()]),
            )
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
