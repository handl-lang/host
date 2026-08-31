//! # handl-plugin-engine-eval
//!
//! The **abstraction** crate for the eval operation (ADR-0044 §3, §4, §7).
//!
//! Splits `plugin-ext-eval` into an abstraction (operation marker + trait,
//! no engine) and an engine crate (`plugin-engine-eval-tree-walk`). This
//! crate defines the `Eval` operation marker, [`EvalRequest`], the
//! [`EvalEngine`] trait. `Foreign` scheme dispatch uses
//! [`handl_platform::ModuleLookup`]/[`handl_platform::Function`] from the
//! `handl-platform` crate (ADR-0044 §7 "Foreign host-function registration").
//! **No interpreter code lives here** — the abstraction is the contract; the
//! tree-walk (or bytecode/cranelift) engine implements it.
//!
//! Engines consume [`handl_language::Language`] **data** — they look up
//! `lang.eval[node.kind]` to find a [`handl_language::Scheme`] and interpret
//! it, not match on a static Rust enum of canonical kinds.

#![warn(missing_docs)]

use std::rc::Rc;

use handl_language::{Ast, LangError, Language, Value};

/// The eval operation marker.
///
/// A zero-sized type identifying the eval stage. It carries no state; its
/// purpose is to namespace the eval abstraction and pair with
/// [`EvalRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Eval;

/// Request passed to an [`EvalEngine`].
///
/// Carries the [`Ast`] to evaluate plus a serializable options map. The
/// options are opaque to this crate — a concrete engine interprets them
/// against the `Language`'s `options` fragment set.
#[derive(Debug, Clone, Default)]
pub struct EvalRequest {
    /// The AST to evaluate.
    pub ast: Ast,
    /// Engine-specific options (opaque key/value pairs).
    pub options: serde_json::Value,
}

impl EvalRequest {
    /// Create a new request from an `Ast` with no options.
    pub fn new(ast: Ast) -> Self {
        Self {
            ast,
            options: serde_json::Value::Null,
        }
    }

    /// Attach an options value (builder style).
    pub fn with_options(mut self, options: serde_json::Value) -> Self {
        self.options = options;
        self
    }
}

/// The trait an eval engine implements.
///
/// An engine looks up `lang.eval[node.kind]` →
/// [`handl_language::EvalSchemeDef`] → [`handl_language::Scheme`] and
/// interprets the scheme. The engine is pure data-driven: it does not match on
/// a static `CanonicalKind` enum.
///
/// # Errors
///
/// Returns [`LangError`] on evaluation failure (unknown variable, unknown
/// primitive, unregistered foreign function, etc.).
pub trait EvalEngine {
    /// Evaluate `req.ast` into a [`Value`] using the language's eval schemes.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on failure.
    fn eval(&self, lang: &Language, req: &EvalRequest) -> Result<Value, LangError>;

    /// Produce an owned boxed clone of this engine (ADR-0047 §2).
    fn box_clone(&self) -> Box<dyn EvalEngine>;

    /// Produce a fresh owned engine with the given module registry wired in
    /// for `Foreign` dispatch. Default: clone + ignore. ADR-0047 §2.
    fn with_modules(&self, _modules: Rc<dyn handl_platform::ModuleLookup>) -> Box<dyn EvalEngine> {
        self.box_clone()
    }

    /// Produce a fresh owned engine with an additional global variable
    /// injected into the root scope. Default: clone + ignore. ADR-0047 §2.
    fn with_global(&self, _name: &str, _val: Value) -> Box<dyn EvalEngine> {
        self.box_clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{Ast, AstNode};
    use handl_platform::{EvalContext, Function, Module, ModuleLookup};
    use handl_plugin_module::ModuleRegistry;
    use serde_json::json;

    #[test]
    fn eval_marker_constructs() {
        let _ = Eval;
        let e1 = Eval;
        let e2 = Eval;
        assert_eq!(e1, e2);
        let e3 = Eval::default();
        assert_eq!(e1, e3);
    }

    #[test]
    fn eval_request_new_from_ast() {
        let mut ast = Ast::new();
        ast.push(AstNode::new("lit"));
        let req = EvalRequest::new(ast);
        assert_eq!(req.ast.len(), 1);
        assert_eq!(req.options, serde_json::Value::Null);
    }

    #[test]
    fn eval_request_with_options() {
        let req = EvalRequest::new(Ast::new()).with_options(json!({"max_depth": 100}));
        assert_eq!(req.options, json!({"max_depth": 100}));
    }

    /// A stub engine that returns `NotImplemented`, used to verify trait
    /// dispatch shape.
    #[derive(Clone)]
    struct StubEval;

    impl EvalEngine for StubEval {
        fn eval(&self, _lang: &Language, _req: &EvalRequest) -> Result<Value, LangError> {
            Err(LangError::NotImplemented)
        }

        fn box_clone(&self) -> Box<dyn EvalEngine> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn trait_dispatch_returns_engine_result() {
        let engine = StubEval;
        let lang = Language::new();
        let req = EvalRequest::new(Ast::new());
        let result = engine.eval(&lang, &req);
        assert_eq!(result.unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn trait_dispatch_via_dyn_reference() {
        let engine: Box<dyn EvalEngine> = Box::new(StubEval);
        let lang = Language::new();
        let req = EvalRequest::new(Ast::new());
        let result = engine.eval(&lang, &req);
        assert!(matches!(result, Err(LangError::NotImplemented)));
    }

    /// A host function that returns the sum of its integer arguments.
    struct AddHostFn;

    impl Function for AddHostFn {
        fn call(
            &self,
            args: &[Value],
            _ctx: &mut EvalContext,
        ) -> Result<Value, handl_platform::EvalError> {
            let sum: i64 = args.iter().filter_map(|v| v.as_i64()).sum();
            Ok(Value::from(sum))
        }
    }

    /// A simple module with an `add` function.
    struct AddModule;

    impl Module for AddModule {
        fn name(&self) -> &str {
            "test"
        }
        fn function(&self, name: &str) -> Option<&dyn Function> {
            if name == "add" {
                Some(&AddHostFn)
            } else {
                None
            }
        }
    }

    /// Helper: call a [`Function`] with a fresh [`EvalContext`].
    fn call_fn(f: &dyn Function, args: &[Value]) -> Result<Value, handl_platform::EvalError> {
        let mut ctx = EvalContext::new();
        f.call(args, &mut ctx)
    }

    #[test]
    fn host_functions_lookup_registered() {
        let reg: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(AddModule));
        let f = reg.lookup("add").expect("add should be registered");
        let result = call_fn(f, &[json!(2), json!(3)]).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn host_functions_lookup_unregistered_returns_none() {
        let reg: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(AddModule));
        assert!(reg.lookup("missing").is_none());
    }

    #[test]
    fn host_fn_call_with_no_args() {
        let result = call_fn(&AddHostFn, &[]).unwrap();
        assert_eq!(result, json!(0));
    }

    #[test]
    fn host_functions_via_dyn_trait() {
        let reg: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(AddModule));
        let f = reg.lookup("add").expect("add should be registered");
        let result = call_fn(f, &[json!(10), json!(20)]).unwrap();
        assert_eq!(result, json!(30));
    }
}
