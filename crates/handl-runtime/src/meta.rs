//! # Meta host functions: `lex`, `parse`, `eval`, `run`, `self`, `compile`
//!
//! ADR-0041 Phase 2 — the "meta is gone" step. These are handl-callable
//! host functions that expose the language pipeline stages as ordinary
//! functions:
//!
//! - `lex(handle, text)` → tokens (JSON-serialized [`Tokens`])
//! - `parse(handle, tokens)` → ast (JSON-serialized [`Ast`])
//! - `eval(handle, ast)` → result value (just the eval engine, no lex/parse)
//! - `run(handle, text)` → result value (convenience: `eval(lang, parse(lex(text)))`)
//! - `self()` → handle of the currently-running language
//! - `compile(handle, target)` → compiled-artifact marker (stub)
//!
//! The key design: `eval(lang, ast)` takes a **pre-parsed AST**, not source
//! text. This separates the pipeline stages so a handl program can
//! manipulate ASTs and tokens as first-class values:
//! `run(lang, text) == eval(lang, parse(lang, lex(lang, text)))`.
//!
//! A [`LanguageRegistry`] maps handle strings to `(Language, RuntimeConfig)`.
//! It is `Rc`-shared so host functions can create sub-runtimes recursively
//! without borrowing conflicts.
//!
//! [`MetaHostFunctions`] implements [`HostFunctions`] by returning the meta
//! functions for their names and delegating everything else to an optional
//! user-supplied [`HostFunctions`].

use std::collections::HashMap;
use std::rc::Rc;

use handl_language::{Ast, LangError, Language, Source, Tokens, Value};
use handl_plugin_engine_eval::{HostFn, HostFunctions};

use crate::{Runtime, RuntimeConfig};

/// A registry of named [`Language`] values, `Rc`-shared for recursive calls.
///
/// Maps a handle string (e.g. `"handl"`) to the [`Language`] data and the
/// [`RuntimeConfig`] that selects its engines. Built once at startup.
#[derive(Debug, Default)]
pub struct LanguageRegistry {
    entries: HashMap<String, (Language, RuntimeConfig)>,
}

impl LanguageRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a [`Language`] under `handle` with the given [`RuntimeConfig`].
    pub fn register(
        &mut self,
        handle: impl Into<String>,
        lang: Language,
        config: RuntimeConfig,
    ) {
        self.entries.insert(handle.into(), (lang, config));
    }

    /// Look up the [`Language`] and [`RuntimeConfig`] for `handle`.
    pub fn get(&self, handle: &str) -> Option<&(Language, RuntimeConfig)> {
        self.entries.get(handle)
    }
}

// ---------------------------------------------------------------------------
// Shared helper: build a sub-runtime with meta functions re-attached
// ---------------------------------------------------------------------------

/// Build a [`Runtime`] for `handle` with [`MetaHostFunctions`] attached, so
/// the sub-runtime can also call `lex`/`parse`/`eval`/`run`/`compile`.
///
/// Injects `self` as a global variable bound to `handle` (ADR-0041 Phase 3:
/// `self` is an ambient variable, not a function call).
///
/// The [`MetaHostFunctions`] is leaked to obtain a `'static` reference — it
/// lives for the process lifetime.
fn build_runtime(
    registry: &Rc<LanguageRegistry>,
    handle: &str,
    delegate: &Option<Rc<dyn HostFunctions>>,
) -> Result<Runtime<'static>, LangError> {
    let (lang, config) = registry
        .get(handle)
        .ok_or_else(|| LangError::Engine(format!("unknown language: {}", handle)))?
        .clone();
    let sub_meta = MetaHostFunctions::new(registry.clone(), handle.to_string(), delegate.clone());
    let sub_meta_static: &'static MetaHostFunctions = Box::leak(Box::new(sub_meta));
    let mut globals = HashMap::new();
    globals.insert("self".to_string(), Value::String(handle.to_string()));
    Runtime::from_config_with_globals(lang, &config, Some(sub_meta_static), Some(&globals))
}

// ---------------------------------------------------------------------------
// lex(handle, text) → tokens
// ---------------------------------------------------------------------------

/// `lex(handle, text)` host function.
///
/// Looks up the [`Language`] under `handle`, builds a [`Runtime`], and lexes
/// `text` into tokens. Returns tokens as a JSON-serialized [`Tokens`] value.
pub struct LexHostFn {
    registry: Rc<LanguageRegistry>,
    delegate: Option<Rc<dyn HostFunctions>>,
}

impl LexHostFn {
    /// Create a `lex` host function backed by `registry`.
    pub fn new(registry: Rc<LanguageRegistry>, delegate: Option<Rc<dyn HostFunctions>>) -> Self {
        Self { registry, delegate }
    }
}

impl HostFn for LexHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        let handle = args
            .first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| LangError::Engine("lex: expected (handle, text)".into()))?;
        let text = args
            .get(1)
            .and_then(|v| v.as_str())
            .ok_or_else(|| LangError::Engine("lex: expected (handle, text)".into()))?;
        let runtime = build_runtime(&self.registry, handle, &self.delegate)?;
        let tokens = runtime.lex(&Source::new(text))?;
        serde_json::to_value(&tokens).map_err(|e| LangError::Engine(format!("lex: {}", e)))
    }
}

// ---------------------------------------------------------------------------
// parse(handle, tokens) → ast
// ---------------------------------------------------------------------------

/// `parse(handle, tokens)` host function.
///
/// Looks up the [`Language`] under `handle`, builds a [`Runtime`], and parses
/// `tokens` (JSON-serialized [`Tokens`]) into an AST. Returns the AST as a
/// JSON-serialized [`Ast`] value.
pub struct ParseHostFn {
    registry: Rc<LanguageRegistry>,
    delegate: Option<Rc<dyn HostFunctions>>,
}

impl ParseHostFn {
    /// Create a `parse` host function backed by `registry`.
    pub fn new(registry: Rc<LanguageRegistry>, delegate: Option<Rc<dyn HostFunctions>>) -> Self {
        Self { registry, delegate }
    }
}

impl HostFn for ParseHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        let handle = args
            .first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| LangError::Engine("parse: expected (handle, tokens)".into()))?;
        let tokens_val = args
            .get(1)
            .ok_or_else(|| LangError::Engine("parse: expected (handle, tokens)".into()))?;
        let tokens: Tokens = serde_json::from_value(tokens_val.clone())
            .map_err(|e| LangError::Engine(format!("parse: {}", e)))?;
        let runtime = build_runtime(&self.registry, handle, &self.delegate)?;
        let ast = runtime.parse(&tokens)?;
        serde_json::to_value(&ast).map_err(|e| LangError::Engine(format!("parse: {}", e)))
    }
}

// ---------------------------------------------------------------------------
// eval(handle, ast) → value
// ---------------------------------------------------------------------------

/// `eval(handle, ast)` host function (ADR-0041 Phase 2).
///
/// Looks up the [`Language`] under `handle`, builds a [`Runtime`], and
/// evaluates `ast` (JSON-serialized [`Ast`]) through the eval engine only —
/// no lex/parse. Returns the result value.
pub struct EvalHostFn {
    registry: Rc<LanguageRegistry>,
    delegate: Option<Rc<dyn HostFunctions>>,
}

impl EvalHostFn {
    /// Create a new `eval` host function backed by `registry`.
    pub fn new(registry: Rc<LanguageRegistry>, delegate: Option<Rc<dyn HostFunctions>>) -> Self {
        Self { registry, delegate }
    }
}

impl HostFn for EvalHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        let handle = args
            .first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| LangError::Engine("eval: expected (handle, ast)".into()))?;
        let ast_val = args
            .get(1)
            .ok_or_else(|| LangError::Engine("eval: expected (handle, ast)".into()))?;
        let ast: Ast = serde_json::from_value(ast_val.clone())
            .map_err(|e| LangError::Engine(format!("eval: {}", e)))?;
        let runtime = build_runtime(&self.registry, handle, &self.delegate)?;
        runtime.eval(&ast)
    }
}

// ---------------------------------------------------------------------------
// run(handle, text) → value  (convenience: lex → parse → eval)
// ---------------------------------------------------------------------------

/// `run(handle, text)` host function — convenience composite.
///
/// Equivalent to `eval(handle, parse(handle, lex(handle, text)))`. Provided
/// so source programs can evaluate source strings without manually composing
/// the three stages.
pub struct RunHostFn {
    registry: Rc<LanguageRegistry>,
    delegate: Option<Rc<dyn HostFunctions>>,
}

impl RunHostFn {
    /// Create a `run` host function backed by `registry`.
    pub fn new(registry: Rc<LanguageRegistry>, delegate: Option<Rc<dyn HostFunctions>>) -> Self {
        Self { registry, delegate }
    }
}

impl HostFn for RunHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        // Two calling conventions:
        //   run(handle, text) — 2-arg: explicit language handle
        //   run(text)         — 1-arg: use the current language (self)
        let (handle, text) = if args.len() >= 2 {
            let h = args
                .first()
                .and_then(|v| v.as_str())
                .ok_or_else(|| LangError::Engine("run: expected (handle, text) or (text)".into()))?;
            let t = args
                .get(1)
                .and_then(|v| v.as_str())
                .ok_or_else(|| LangError::Engine("run: expected (handle, text) or (text)".into()))?;
            (h.to_string(), t.to_string())
        } else if args.len() == 1 {
            let t = args
                .first()
                .and_then(|v| v.as_str())
                .ok_or_else(|| LangError::Engine("run: expected (handle, text) or (text)".into()))?;
            ("handl".to_string(), t.to_string())
        } else {
            return Err(LangError::Engine("run: expected (handle, text) or (text)".into()));
        };
        let runtime = build_runtime(&self.registry, &handle, &self.delegate)?;
        runtime.run(&Source::new(&text))
    }
}

// ---------------------------------------------------------------------------
// self() → handle
// ---------------------------------------------------------------------------

/// `self()` host function — returns the handle of the currently-running
/// language (ADR-0041 Phase 3).
pub struct SelfHostFn {
    handle: String,
}

impl SelfHostFn {
    /// Create a `self` host function that returns `handle`.
    pub fn new(handle: impl Into<String>) -> Self {
        Self {
            handle: handle.into(),
        }
    }
}

impl HostFn for SelfHostFn {
    fn call(&self, _args: &[Value]) -> Result<Value, LangError> {
        Ok(Value::String(self.handle.clone()))
    }
}

// ---------------------------------------------------------------------------
// compile(handle, target) → marker (stub)
// ---------------------------------------------------------------------------

/// `compile(handle, target)` host function (stub).
///
/// Returns the handle as a marker value. Full native compilation via the
/// cranelift engine is a follow-on.
pub struct CompileHostFn {
    registry: Rc<LanguageRegistry>,
}

impl CompileHostFn {
    /// Create a `compile` host function backed by `registry`.
    pub fn new(registry: Rc<LanguageRegistry>) -> Self {
        Self { registry }
    }
}

impl HostFn for CompileHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        let handle = args
            .first()
            .and_then(|v| v.as_str())
            .ok_or_else(|| LangError::Engine("compile: expected (handle, target)".into()))?;
        let target = args
            .get(1)
            .and_then(|v| v.as_str())
            .unwrap_or("native");
        if self.registry.get(handle).is_none() {
            return Err(LangError::Engine(format!(
                "compile: unknown language: {}",
                handle
            )));
        }
        Ok(Value::String(format!("compiled:{}:{}", handle, target)))
    }
}

// ---------------------------------------------------------------------------
// MetaHostFunctions registry
// ---------------------------------------------------------------------------

/// A [`HostFunctions`] registry that includes the meta functions
/// (`lex`/`parse`/`eval`/`run`/`self`/`compile`) and delegates unknown names
/// to an optional user-supplied [`HostFunctions`].
pub struct MetaHostFunctions {
    lex_fn: LexHostFn,
    parse_fn: ParseHostFn,
    eval_fn: EvalHostFn,
    run_fn: RunHostFn,
    self_fn: SelfHostFn,
    compile_fn: CompileHostFn,
    delegate: Option<Rc<dyn HostFunctions>>,
}

impl Clone for MetaHostFunctions {
    fn clone(&self) -> Self {
        Self {
            lex_fn: LexHostFn {
                registry: Rc::clone(&self.lex_fn.registry),
                delegate: self.delegate.clone(),
            },
            parse_fn: ParseHostFn {
                registry: Rc::clone(&self.parse_fn.registry),
                delegate: self.delegate.clone(),
            },
            eval_fn: EvalHostFn {
                registry: Rc::clone(&self.eval_fn.registry),
                delegate: self.delegate.clone(),
            },
            run_fn: RunHostFn {
                registry: Rc::clone(&self.run_fn.registry),
                delegate: self.delegate.clone(),
            },
            self_fn: SelfHostFn {
                handle: self.self_fn.handle.clone(),
            },
            compile_fn: CompileHostFn {
                registry: Rc::clone(&self.compile_fn.registry),
            },
            delegate: self.delegate.clone(),
        }
    }
}

impl MetaHostFunctions {
    /// Create a `MetaHostFunctions` with `registry`, `current` as the
    /// running language handle, and an optional `delegate` for user functions.
    pub fn new(
        registry: Rc<LanguageRegistry>,
        current: String,
        delegate: Option<Rc<dyn HostFunctions>>,
    ) -> Self {
        Self {
            lex_fn: LexHostFn::new(Rc::clone(&registry), delegate.clone()),
            parse_fn: ParseHostFn::new(Rc::clone(&registry), delegate.clone()),
            eval_fn: EvalHostFn::new(Rc::clone(&registry), delegate.clone()),
            run_fn: RunHostFn::new(Rc::clone(&registry), delegate.clone()),
            self_fn: SelfHostFn::new(current),
            compile_fn: CompileHostFn::new(registry),
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
            // Note: `self` is no longer a host function — it's injected as a
            // global variable by `build_runtime` / `Kit::new`. ADR-0041 Phase 3.
            _ => self.delegate.as_ref().and_then(|d| d.lookup(name)),
        }
    }
}

/// Build a [`MetaHostFunctions`] for the canonical handl language, registering
/// it under the handle `"handl"` with the standard engine config.
pub fn canonical_meta_host_functions(
    lang: Language,
) -> (Rc<LanguageRegistry>, Rc<MetaHostFunctions>) {
    let mut registry = LanguageRegistry::new();
    registry.register(
        "handl",
        lang,
        RuntimeConfig::new()
            .with_lex("data")
            .with_parse("pratt")
            .with_eval("tree-walk"),
    );
    let registry = Rc::new(registry);
    let meta = Rc::new(MetaHostFunctions::new(
        registry.clone(),
        "handl".to_string(),
        None,
    ));
    (registry, meta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{Ast, AstNode};
    use serde_json::json;

    fn make_runtime() -> Runtime<'static> {
        let canonical = handl_kit_data_canonical::assemble()
            .expect("canonical kit must assemble")
            .lang()
            .clone();
        let (_registry, meta) = canonical_meta_host_functions(canonical.clone());
        // Leak meta to get &'static for the runtime.
        let meta_static: &'static MetaHostFunctions =
            Box::leak(Box::new(Rc::try_unwrap(meta).unwrap_or_else(|rc| (*rc).clone())));
        let config = RuntimeConfig::new()
            .with_lex("data")
            .with_parse("pratt")
            .with_eval("tree-walk");
        // Inject `self` as a global variable (ADR-0041 Phase 3).
        let mut globals = HashMap::new();
        globals.insert("self".to_string(), Value::String("handl".to_string()));
        Runtime::from_config_with_globals(canonical, &config, Some(meta_static), Some(&globals))
            .expect("runtime must build")
    }

    /// `lex("handl", "1 + 2")` returns a JSON-serialized Tokens value.
    #[test]
    fn lex_host_function_returns_tokens() {
        let runtime = make_runtime();
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!("handl")));
        ast.push(AstNode::new("value").with_payload(json!("1 + 2")));
        ast.push(AstNode::new("foreign.lex").with_payload(json!({"children": [0, 1]})));
        let result = runtime.eval(&ast).expect("lex must succeed");
        // Tokens serialize as a JSON array of {kind, text} objects.
        assert!(result.is_array(), "lex should return an array, got: {}", result);
    }

    /// `parse("handl", tokens)` returns a JSON-serialized Ast value.
    #[test]
    fn parse_host_function_returns_ast() {
        let runtime = make_runtime();
        // First lex "1 + 2" to get tokens, then parse them.
        // Build: 0: value "handl", 1: value "1 + 2", 2: foreign.lex {children:[0,1]}
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!("handl")));
        ast.push(AstNode::new("value").with_payload(json!("1 + 2")));
        ast.push(AstNode::new("foreign.lex").with_payload(json!({"children": [0, 1]})));
        let tokens = runtime.eval(&ast).expect("lex must succeed");

        // Now parse: 0: value "handl", 1: <tokens>, 2: foreign.parse {children:[0,1]}
        let mut ast2 = Ast::new();
        ast2.push(AstNode::new("value").with_payload(json!("handl")));
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

    /// `eval("handl", ast)` evaluates a pre-parsed AST and returns the result.
    #[test]
    fn eval_host_function_evals_ast() {
        let runtime = make_runtime();

        // Build a simple AST: 0: value 42
        let mut sub_ast = Ast::new();
        sub_ast.push(AstNode::new("value").with_payload(json!(42)));
        let sub_ast_val = serde_json::to_value(&sub_ast).unwrap();

        // Build the calling AST: 0: value "handl", 1: <ast>, 2: foreign.eval
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!("handl")));
        ast.push(AstNode::new("value").with_payload(sub_ast_val));
        ast.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
        let result = runtime.eval(&ast).expect("eval must succeed");
        assert_eq!(result, json!(42), "eval(handl, ast[42]) should return 42");
    }

    /// Full pipeline: `eval(lang, parse(lang, lex(lang, text)))` == `run(lang, text)`.
    #[test]
    fn pipeline_eval_parse_lex_equals_run() {
        let runtime = make_runtime();

        // Build: lex("handl", "1 + 2")
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!("handl")));
        ast.push(AstNode::new("value").with_payload(json!("1 + 2")));
        ast.push(AstNode::new("foreign.lex").with_payload(json!({"children": [0, 1]})));
        let tokens = runtime.eval(&ast).expect("lex must succeed");

        // parse("handl", tokens)
        let mut ast2 = Ast::new();
        ast2.push(AstNode::new("value").with_payload(json!("handl")));
        ast2.push(AstNode::new("value").with_payload(tokens));
        ast2.push(AstNode::new("foreign.parse").with_payload(json!({"children": [0, 1]})));
        let parsed_ast = runtime.eval(&ast2).expect("parse must succeed");

        // eval("handl", parsed_ast)
        let mut ast3 = Ast::new();
        ast3.push(AstNode::new("value").with_payload(json!("handl")));
        ast3.push(AstNode::new("value").with_payload(parsed_ast));
        ast3.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
        let pipeline_result = runtime.eval(&ast3).expect("eval must succeed");

        // run("handl", "1 + 2")
        let mut ast4 = Ast::new();
        ast4.push(AstNode::new("value").with_payload(json!("handl")));
        ast4.push(AstNode::new("value").with_payload(json!("1 + 2")));
        ast4.push(AstNode::new("foreign.run").with_payload(json!({"children": [0, 1]})));
        let run_result = runtime.eval(&ast4).expect("run must succeed");

        assert_eq!(
            pipeline_result, run_result,
            "eval(parse(lex(text))) should equal run(text)"
        );
        assert_eq!(pipeline_result, json!(3));
    }

    /// `self` is injected as a global variable bound to the current language
    /// handle (ADR-0041 Phase 3: `self` is an ambient variable, not a fn call).
    #[test]
    fn self_global_returns_current_handle() {
        let runtime = make_runtime();
        // `self` is a var node — the tree-walk looks it up in the root scope
        // where `build_runtime` injected `self = "handl"`.
        let mut ast = Ast::new();
        ast.push(AstNode::new("var").with_payload(json!("self")));
        let result = runtime.eval(&ast).expect("self must succeed");
        assert_eq!(result, json!("handl"), "self should return 'handl'");
    }

    /// `compile("handl", "native")` returns a compiled marker.
    #[test]
    fn compile_host_function_returns_marker() {
        let runtime = make_runtime();
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!("handl")));
        ast.push(AstNode::new("value").with_payload(json!("native")));
        ast.push(AstNode::new("foreign.compile").with_payload(json!({"children": [0, 1]})));
        let result = runtime.eval(&ast).expect("compile must succeed");
        assert_eq!(result, json!("compiled:handl:native"));
    }

    /// `eval` with an unknown language handle returns an error.
    #[test]
    fn eval_unknown_language_returns_error() {
        let runtime = make_runtime();
        let mut sub_ast = Ast::new();
        sub_ast.push(AstNode::new("value").with_payload(json!(1)));
        let sub_ast_val = serde_json::to_value(&sub_ast).unwrap();
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!("nonexistent")));
        ast.push(AstNode::new("value").with_payload(sub_ast_val));
        ast.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
        let err = runtime.eval(&ast).unwrap_err();
        assert!(err.to_string().contains("unknown language"), "got: {}", err);
    }
}
