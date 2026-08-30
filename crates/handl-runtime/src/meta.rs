//! # Meta host functions: `eval`, `self`, `compile`
//!
//! ADR-0041 Phase 2 — the "meta is gone" step. These are handl-callable
//! host functions, dispatched via [`Scheme::Foreign`] when a handl program
//! evaluates a `foreign.eval` / `foreign.self` / `foreign.compile` node.
//!
//! - `eval(handle, src)` — look up the [`Language`] registered under `handle`,
//!   build a fresh [`Runtime`], and run `src` under it. Returns the result
//!   value.
//! - `self()` — return the handle of the currently-running language.
//! - `compile(handle, target)` — return a compiled-artifact marker (the
//!   handle). Full native compilation via cranelift is a follow-on; this
//!   proves the call surface.
//!
//! A [`LanguageRegistry`] maps handle strings to `(Language, RuntimeConfig)`.
//! It is `Rc`-shared so `eval` can create sub-runtimes recursively without
//! borrowing conflicts.
//!
//! [`MetaHostFunctions`] implements [`HostFunctions`] by returning the meta
//! functions for `"eval"` / `"self"` / `"compile"` and delegating everything
//! else to an optional user-supplied [`HostFunctions`] (so meta functions
//! coexist with user host functions like `"double"`).

use std::collections::HashMap;
use std::rc::Rc;

use handl_language::{LangError, Language, Source, Value};
use handl_plugin_engine_eval::{HostFn, HostFunctions};

use crate::{Runtime, RuntimeConfig};

/// A registry of named [`Language`] values, `Rc`-shared for recursive `eval`.
///
/// Maps a handle string (e.g. `"handl"`) to the [`Language`] data and the
/// [`RuntimeConfig`] that selects its engines. Built once at startup; the
/// `eval` host function clones the `Rc` to build sub-runtimes.
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

/// `eval(handle, src)` host function (ADR-0041 Phase 2).
///
/// Looks up the [`Language`] registered under `handle` in the
/// [`LanguageRegistry`], builds a fresh [`Runtime`] (with meta host functions
/// re-attached so the sub-program can also call `eval`), and runs `src`
/// through `lex → parse → eval`. Returns the result value.
pub struct EvalHostFn {
    /// Shared registry of named languages.
    registry: Rc<LanguageRegistry>,
    /// Optional user host functions to pass through to sub-runtimes.
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
            .ok_or_else(|| LangError::Engine("eval: expected (handle, src)".into()))?;
        let src = args
            .get(1)
            .and_then(|v| v.as_str())
            .ok_or_else(|| LangError::Engine("eval: expected (handle, src)".into()))?;

        let (lang, config) = self
            .registry
            .get(handle)
            .ok_or_else(|| LangError::Engine(format!("eval: unknown language: {}", handle)))?
            .clone();

        // Build a MetaHostFunctions for the sub-runtime so the sub-program
        // can also call eval/self/compile. The sub-meta uses the same
        // registry and the same delegate.
        let sub_meta = MetaHostFunctions::new(
            self.registry.clone(),
            handle.to_string(),
            self.delegate.clone(),
        );
        let runtime = Runtime::from_config(lang, &config, Some(&sub_meta))?;
        runtime.run(&Source::new(src))
    }
}

/// `self()` host function — returns the handle of the currently-running
/// language (ADR-0041 Phase 3).
pub struct SelfHostFn {
    /// The handle of the language currently running.
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

/// `compile(handle, target)` host function (ADR-0041 Phase 2 stub).
///
/// Returns the handle as a marker value. Full native compilation via the
/// cranelift engine is a follow-on; this proves the call surface is
/// handl-callable.
pub struct CompileHostFn {
    /// Shared registry of named languages.
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

        // Verify the language exists.
        if self.registry.get(handle).is_none() {
            return Err(LangError::Engine(format!(
                "compile: unknown language: {}",
                handle
            )));
        }

        // Stub: return a marker indicating compilation succeeded.
        Ok(Value::String(format!("compiled:{}:{}", handle, target)))
    }
}

/// A [`HostFunctions`] registry that includes the meta functions
/// (`eval`/`self`/`compile`) and delegates unknown names to an optional
/// user-supplied [`HostFunctions`].
///
/// This is the bridge that makes `eval`/`self`/`compile` handl-callable:
/// when the eval engine encounters a `Scheme::Foreign("eval")` node, it
/// calls `lookup("eval")` which returns the [`EvalHostFn`] stored here.
pub struct MetaHostFunctions {
    /// The `eval` host function.
    eval_fn: EvalHostFn,
    /// The `self` host function.
    self_fn: SelfHostFn,
    /// The `compile` host function.
    compile_fn: CompileHostFn,
    /// Optional user host functions for non-meta names.
    delegate: Option<Rc<dyn HostFunctions>>,
}

impl MetaHostFunctions {
    /// Create a `MetaHostFunctions` with `registry`, `current` as the
    /// running language handle, and an optional `delegate` for user functions.
    pub fn new(
        registry: Rc<LanguageRegistry>,
        current: String,
        delegate: Option<Rc<dyn HostFunctions>>,
    ) -> Self {
        let eval_fn = EvalHostFn::new(registry, delegate.clone());
        let compile_fn = CompileHostFn::new(Rc::clone(&eval_fn.registry));
        Self {
            eval_fn,
            self_fn: SelfHostFn::new(current),
            compile_fn,
            delegate,
        }
    }
}

impl HostFunctions for MetaHostFunctions {
    fn lookup(&self, name: &str) -> Option<&dyn HostFn> {
        match name {
            "eval" => Some(&self.eval_fn),
            "self" => Some(&self.self_fn),
            "compile" => Some(&self.compile_fn),
            _ => self.delegate.as_ref().and_then(|d| d.lookup(name)),
        }
    }
}

/// Build a [`MetaHostFunctions`] for the canonical handl language, registering
/// it under the handle `"handl"` with the standard engine config
/// (`lex=data`, `parse=pratt`, `eval=tree-walk`).
///
/// This is the convenience entry point for hosts (e.g. `Kit::new`) that want
/// meta functions wired with zero ceremony. The returned `Rc<MetaHostFunctions>`
/// can be passed to [`Runtime::from_config`] as the host-functions argument.
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

    /// A handl program can `eval("handl", "1 + 2")` and get `3`.
    ///
    /// This is the "meta is gone" test: the eval host function dispatches to
    /// a real `Runtime::run`, proving `eval` is a handl-callable function
    /// over a first-class `Language` value (represented as a handle string
    /// in the execution value space).
    #[test]
    fn eval_host_function_evals_sub_program() {
        let canonical = handl_kit_data_canonical::assemble()
            .expect("canonical kit must assemble")
            .lang()
            .clone();

        let (_registry, meta) = canonical_meta_host_functions(canonical.clone());

        let config = RuntimeConfig::new()
            .with_lex("data")
            .with_parse("pratt")
            .with_eval("tree-walk");
        let runtime = Runtime::from_config(canonical, &config, Some(&*meta))
            .expect("runtime must build");

        // Build an AST that calls eval("handl", "1 + 2"):
        //   0: value "handl"  (the language handle)
        //   1: value "1 + 2"  (the source to eval)
        //   2: foreign.eval { children: [0, 1] }
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!("handl")));
        ast.push(AstNode::new("value").with_payload(json!("1 + 2")));
        ast.push(
            AstNode::new("foreign.eval")
                .with_payload(json!({"children": [0, 1]})),
        );

        let result = runtime.eval(&ast).expect("eval must succeed");
        assert_eq!(result, json!(3), "eval(handl, '1 + 2') should return 3");
    }

    /// `self()` returns the handle of the currently-running language.
    #[test]
    fn self_host_function_returns_current_handle() {
        let lang = handl_kit_data_canonical::assemble()
            .expect("canonical kit must assemble")
            .lang()
            .clone();

        let (_registry, meta) = canonical_meta_host_functions(lang.clone());

        let config = RuntimeConfig::new()
            .with_lex("data")
            .with_parse("pratt")
            .with_eval("tree-walk");
        let runtime = Runtime::from_config(lang, &config, Some(&*meta))
            .expect("runtime must build");

        // AST: foreign.self {} — no args
        let mut ast = Ast::new();
        ast.push(AstNode::new("foreign.self"));

        let result = runtime.eval(&ast).expect("self must succeed");
        assert_eq!(result, json!("handl"), "self() should return 'handl'");
    }

    /// `compile("handl", "native")` returns a compiled marker.
    #[test]
    fn compile_host_function_returns_marker() {
        let lang = handl_kit_data_canonical::assemble()
            .expect("canonical kit must assemble")
            .lang()
            .clone();

        let (_registry, meta) = canonical_meta_host_functions(lang.clone());

        let config = RuntimeConfig::new()
            .with_lex("data")
            .with_parse("pratt")
            .with_eval("tree-walk");
        let runtime = Runtime::from_config(lang, &config, Some(&*meta))
            .expect("runtime must build");

        // AST: 0: value "handl", 1: value "native", 2: foreign.compile {children:[0,1]}
        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!("handl")));
        ast.push(AstNode::new("value").with_payload(json!("native")));
        ast.push(
            AstNode::new("foreign.compile")
                .with_payload(json!({"children": [0, 1]})),
        );

        let result = runtime.eval(&ast).expect("compile must succeed");
        assert_eq!(
            result, json!("compiled:handl:native"),
            "compile should return a marker"
        );
    }

    /// `eval` with an unknown language handle returns an error.
    #[test]
    fn eval_unknown_language_returns_error() {
        let lang = handl_kit_data_canonical::assemble()
            .expect("canonical kit must assemble")
            .lang()
            .clone();

        let (_registry, meta) = canonical_meta_host_functions(lang.clone());

        let config = RuntimeConfig::new()
            .with_lex("data")
            .with_parse("pratt")
            .with_eval("tree-walk");
        let runtime = Runtime::from_config(lang, &config, Some(&*meta))
            .expect("runtime must build");

        let mut ast = Ast::new();
        ast.push(AstNode::new("value").with_payload(json!("nonexistent")));
        ast.push(AstNode::new("value").with_payload(json!("1")));
        ast.push(
            AstNode::new("foreign.eval")
                .with_payload(json!({"children": [0, 1]})),
        );

        let err = runtime.eval(&ast).unwrap_err();
        assert!(
            err.to_string().contains("unknown language"),
            "expected unknown language error, got: {}",
            err
        );
    }
}
