//! # handl-plugin-engine-exec
//!
//! The **abstraction** crate for the eval operation (ADR-0044 §3, §4, §7).
//!
//! Splits `plugin-ext-eval` into an abstraction (operation marker + trait,
//! no engine) and an engine crate (`plugin-engine-exec-tree-walk`). This
//! crate defines the `Eval` operation marker, [`EvalRequest`], the
//! [`EvalEngine`] trait. `Foreign` scheme dispatch uses
//! [`handl_platform::ModuleLookup`]/[`handl_platform::Function`] from the
//! `handl-platform` crate (ADR-0044 §7 "Foreign host-function registration").
//! **No interpreter code lives here** — the abstraction is the contract; the
//! tree-walk (or bytecode/cranelift) engine implements it.
//!
//! Engines consume [`handl_language::Language`] **data** — they look up
//! `lang.eval[node.kind]` to find a [`handl_language::Term`] and interpret
//! it, not match on a static Rust enum of canonical kinds.

#![warn(missing_docs)]

pub mod native_ops;
pub mod pattern_match;

use std::rc::Rc;

use handl_language::{Ast, LangError, Language, Value};
use handl_value::TaggedValue;

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
/// [`handl_language::EvalSchemeDef`] → [`handl_language::Term`] and
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

    /// Produce a fresh owned engine with an [`ImportResolver`] wired in for
    /// `import` statement dispatch (Phase 9.5). Default: clone + ignore.
    /// Engines that support `import` (e.g. the tree-walk engine) override
    /// this to store the resolver; other engines silently drop it.
    fn with_import_resolver(&self, _resolver: Rc<dyn ImportResolver>) -> Box<dyn EvalEngine> {
        self.box_clone()
    }

    /// Evaluate an [`Ast`] held behind an [`Rc`], avoiding the deep arena
    /// clone into [`EvalRequest`] when the caller already shares the arena
    /// (e.g. a per-thread parsed-AST cache such as
    /// `plugin-lib-handl`'s `HandlFunction` bridge, which re-evaluates the
    /// same module AST on every call).
    ///
    /// Default: clone the arena into a request and delegate to
    /// [`eval`](Self::eval). Engines that hold the arena by `Rc` internally
    /// (the tree-walk engine installs it in a thread-local) override this
    /// to reuse the caller's `Rc` — zero arena clones per call.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on failure, as [`eval`](Self::eval) does.
    fn eval_shared(&self, lang: &Language, ast: &Rc<Ast>) -> Result<Value, LangError> {
        self.eval(lang, &EvalRequest::new((**ast).clone()))
    }

    /// Like [`eval_shared`](Self::eval_shared), but binds `args` directly
    /// into the call frame's scope rather than via per-call
    /// [`with_global`](Self::with_global) engine cloning, eliminating the N
    /// engine clones that [`Runtime::eval_shared_with_globals`] would incur
    /// for N arguments.
    ///
    /// The argument names are determined by the caller (typically
    /// `__arg_0`, `__arg_1`, … matching a cached applied-form AST) and are
    /// resolved through the engine's normal variable lookup at eval time.
    ///
    /// **Default:** clone the engine once and inject each arg via
    /// [`with_global`](Self::with_global), then delegate to
    /// [`eval_shared`](Self::eval_shared). Engines that can bind per-call
    /// names directly on the call frame (e.g. the tree-walk engine's
    /// [`ScopeStack`](plugin_engine_exec_tree_walk::ScopeStack)) override
    /// this for a zero-clone path. The default preserves the existing
    /// fallback behavior for engines that do not override it.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on failure, as [`eval_shared`](Self::eval_shared).
    fn eval_shared_with_call_args(
        &self,
        lang: &Language,
        ast: &Rc<Ast>,
        args: &[(String, Value)],
    ) -> Result<Value, LangError> {
        let mut engine: Box<dyn EvalEngine> = self.box_clone();
        for (name, val) in args {
            engine = engine.with_global(name, val.clone());
        }
        engine.eval_shared(lang, ast)
    }

    /// Evaluate `req.ast` into the engine's internal [`TaggedValue`] form,
    /// skipping the `serde_json::Value` materialization that
    /// [`eval`](Self::eval) performs at the boundary.
    ///
    /// Consumers that only *hold* the result (the module loader's cache) or
    /// re-import it into a tagged-value engine (the `import` op) use this
    /// path: the returned value keeps `Rc`-shared substructure — notably
    /// arrow `binds` maps shared across a `rec` region — intact instead of
    /// expanding it into a JSON tree. That keeps a cached module record
    /// proportional to its eval-time footprint rather than its serialized
    /// size.
    ///
    /// Default: [`eval`](Self::eval) + [`TaggedValue::from`]. This is
    /// correct for every engine; engines whose internal representation is
    /// already [`TaggedValue`] (the tree-walk engine) override it to skip
    /// the `Value` round-trip entirely.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on failure, as [`eval`](Self::eval) does.
    fn eval_tagged(&self, lang: &Language, req: &EvalRequest) -> Result<TaggedValue, LangError> {
        self.eval(lang, req).map(TaggedValue::from)
    }

    /// Seal a module's exported bindings before they cross into the
    /// importer's scope (Phase 9.5 `import "path"`).
    ///
    /// Engines whose closure/arrow values carry provenance tied to the
    /// *module's* eval (e.g. the tree-walk engine's `body` AST index) stamp
    /// them here so a later application routes back to the module's eval
    /// frame. `ast` is the module's post-desugar AST and `lang` the
    /// `Language` it was evaluated under.
    ///
    /// Default: identity — engines whose values carry no ambient-eval
    /// references need no sealing.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if sealing fails (e.g. no active eval frame
    /// could be registered).
    fn seal_module_value(
        &self,
        _ast: &Ast,
        _lang: &Language,
        value: Value,
    ) -> Result<Value, LangError> {
        Ok(value)
    }
}

/// A resolver for `import "path"` statements (Phase 9.5).
///
/// When the eval engine encounters an `import` node, it calls the
/// configured [`ImportResolver`] to load the referenced module and return
/// its top-level bindings as a [`Value::Object`]. The engine then binds
/// each entry into the current scope.
///
/// This trait is intentionally narrow: it knows nothing about the engine's
/// internals, keeping the engine generic (ADR-0044 "language as data"). The
/// concrete resolution strategy — file I/O, lex/parse/eval, caching, cycle
/// detection — lives in the implementor (e.g. the runtime's module loader).
///
/// The trait is `?Send`/`?Sync`-agnostic: it uses `Rc` (not `Arc`) to match
/// the single-threaded eval model shared with [`handl_platform::ModuleLookup`].
pub trait ImportResolver {
    /// Resolve `path` to a module's bindings, returned as a [`Value::Object`]
    /// mapping binding names to their values.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] if the module cannot be found, read, parsed,
    /// or evaluated (including circular-import detection).
    fn resolve_import(&self, path: &str) -> Result<Value, LangError>;

    /// Tagged counterpart of [`resolve_import`](Self::resolve_import):
    /// returns the module's bindings as a shared [`TaggedValue`] (normally
    /// a [`TaggedValue::Obj`] record).
    ///
    /// Consumers that splice the record into a [`TaggedValue`]-based
    /// environment (the `import` op) use this path: a resolver backed by a
    /// tagged cache can hand out an `Rc` clone, so repeated imports of the
    /// same module are O(1) and never materialize a `serde_json::Value`
    /// tree.
    ///
    /// Default: [`resolve_import`](Self::resolve_import) +
    /// [`TaggedValue::from`] — a deep reconstruction, correct but without
    /// the sharing. Resolvers that cache [`TaggedValue`] records natively
    /// (the runtime's `ModuleLoader`) override it.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] as [`resolve_import`](Self::resolve_import).
    fn resolve_import_tagged(&self, path: &str) -> Result<Rc<TaggedValue>, LangError> {
        self.resolve_import(path)
            .map(|v| Rc::new(TaggedValue::from(v)))
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
