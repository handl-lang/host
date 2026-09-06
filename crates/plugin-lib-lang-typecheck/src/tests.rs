//! Unit tests for the `typecheck` meta library.
//!
//! These test the [`TypecheckModule`] / [`TypecheckFn`] plumbing in isolation
//! — module surface, argument extraction, the tagged `__ast__` vs
//! raw-serialized Ast acceptance paths, error paths, and delegate forwarding —
//! using a mock [`RuntimeFactory`] / [`RuntimeHandle`] so the leaf crate need
//! not pull in the full kit.

use std::cell::RefCell;
use std::rc::Rc;

use handl_language::LangError;
use handl_language::{ast_value, language_value, Ast, AstNode, Language};
use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup};
use handl_plugin_engine_typecheck::TypecheckResult;
use handl_runtime::{CompiledHandle, RuntimeFactory, RuntimeHandle};
use serde_json::{json, Value};

use crate::TypecheckModule;

// ---------------------------------------------------------------------------
// Mock RuntimeFactory / RuntimeHandle
// ---------------------------------------------------------------------------

/// A canned Ast used both as the input to `typecheck.typecheck` and (mirrored)
/// as the value the mock handle returns, so tests can verify the result is
/// forwarded verbatim.
fn sample_ast() -> Ast {
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast
}

/// A raw JSON-serialized [`Ast`] value (the non-tagged form accepted by
/// `typecheck.typecheck`).
fn raw_ast_arg() -> Value {
    serde_json::to_value(sample_ast()).expect("Ast serializes")
}

/// A tagged `__ast__` value (the form bound to an unevaluated expression).
fn tagged_ast_arg() -> Value {
    ast_value(&sample_ast())
}

struct MockHandle {
    typecheck_err: Option<LangError>,
    last_ast: Rc<RefCell<Option<Ast>>>,
    canned_result: Value,
}

impl RuntimeHandle for MockHandle {
    fn lex(&self, _src: &handl_language::Source) -> Result<handl_language::Tokens, LangError> {
        Err(LangError::NotImplemented)
    }

    fn parse(&self, _toks: &handl_language::Tokens) -> Result<Ast, LangError> {
        Err(LangError::NotImplemented)
    }

    fn eval(&self, _ast: &Ast) -> Result<Value, LangError> {
        Err(LangError::NotImplemented)
    }

    fn typecheck(&self, ast: &Ast) -> Result<TypecheckResult, LangError> {
        *self.last_ast.borrow_mut() = Some(ast.clone());
        match &self.typecheck_err {
            Some(e) => Err(e.clone()),
            None => Ok(TypecheckResult {
                typ: self.canned_result.clone(),
                diagnostics: vec![],
            }),
        }
    }

    fn compile(&self, _ast: &Ast, _target: &str) -> Result<CompiledHandle, LangError> {
        Err(LangError::NotImplemented)
    }

    fn run_compiled(&self, _handle: &CompiledHandle) -> Result<Value, LangError> {
        Err(LangError::NotImplemented)
    }
}

struct MockFactory {
    build_err: Option<LangError>,
    typecheck_err: Option<LangError>,
    canned_result: Value,
    last_ast: Rc<RefCell<Option<Ast>>>,
}

impl MockFactory {
    fn new() -> Self {
        Self {
            build_err: None,
            typecheck_err: None,
            canned_result: json!("Type"),
            last_ast: Rc::new(RefCell::new(None)),
        }
    }

    fn with_build_err(mut self, e: LangError) -> Self {
        self.build_err = Some(e);
        self
    }

    fn with_typecheck_err(mut self, e: LangError) -> Self {
        self.typecheck_err = Some(e);
        self
    }

    fn with_result(mut self, v: Value) -> Self {
        self.canned_result = v;
        self
    }

    fn last_ast(&self) -> Option<Ast> {
        self.last_ast.borrow().clone()
    }
}

impl RuntimeFactory for MockFactory {
    fn build(
        &self,
        _lang: Language,
        _delegate: Option<Rc<dyn ModuleLookup>>,
    ) -> Result<Box<dyn RuntimeHandle>, LangError> {
        if let Some(e) = &self.build_err {
            return Err(e.clone());
        }
        Ok(Box::new(MockHandle {
            typecheck_err: self.typecheck_err.clone(),
            last_ast: Rc::clone(&self.last_ast),
            canned_result: self.canned_result.clone(),
        }))
    }
}

/// A mock delegate [`ModuleLookup`] that resolves a single known name, used to
/// verify the [`TypecheckModule`] forwards unknown names to its delegate.
struct DelegateLookup {
    name: &'static str,
}

impl ModuleLookup for DelegateLookup {
    fn lookup(&self, qualified_name: &str) -> Option<&dyn Function> {
        if qualified_name == self.name {
            Some(&DelegateFn)
        } else {
            None
        }
    }
}

struct DelegateFn;

impl Function for DelegateFn {
    fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        Ok(json!("delegate-called"))
    }
}

// ---------------------------------------------------------------------------
// Module surface
// ---------------------------------------------------------------------------

fn make_module(factory: Rc<MockFactory>) -> TypecheckModule {
    TypecheckModule::new(factory, None)
}

fn make_module_with_delegate(
    factory: Rc<MockFactory>,
    delegate: Rc<dyn ModuleLookup>,
) -> TypecheckModule {
    TypecheckModule::new(factory, Some(delegate))
}

#[test]
fn module_name_is_typecheck() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert_eq!(module.name(), "typecheck");
}

#[test]
fn module_function_lookup() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.function("typecheck").is_some());
    assert!(module.function("nope").is_none());
}

#[test]
fn module_functions_lists_typecheck() {
    let module = make_module(Rc::new(MockFactory::new()));
    let fns = module.functions();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].0, "typecheck");
}

// ---------------------------------------------------------------------------
// ModuleLookup
// ---------------------------------------------------------------------------

#[test]
fn lookup_resolves_flat_and_qualified_name() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.lookup("typecheck").is_some());
    assert!(module.lookup("typecheck.typecheck").is_some());
    assert!(module.lookup("other").is_none());
}

#[test]
fn lookup_forwards_unknown_names_to_delegate() {
    let delegate = Rc::new(DelegateLookup { name: "user.fn" });
    let module = make_module_with_delegate(Rc::new(MockFactory::new()), delegate);
    assert!(module.lookup("user.fn").is_some());
    assert!(module.lookup("typecheck").is_some());
    assert!(module.lookup("typecheck.typecheck").is_some());
}

#[test]
fn lookup_returns_none_without_delegate_for_unknown_name() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.lookup("user.fn").is_none());
}

// ---------------------------------------------------------------------------
// TypecheckFn::call — happy path (raw + tagged Ast forms)
// ---------------------------------------------------------------------------

fn call(module: &TypecheckModule, args: &[Value]) -> Result<Value, EvalError> {
    let typecheck_fn = module.function("typecheck").expect("typecheck fn present");
    let mut ctx = EvalContext::new();
    typecheck_fn.call(args, &mut ctx)
}

fn lang_arg() -> Value {
    language_value(&Language::new())
}

#[test]
fn call_with_raw_ast_returns_handle_result() {
    let factory = Rc::new(MockFactory::new().with_result(json!("Nat")));
    let module = make_module(Rc::clone(&factory));
    let result = call(&module, &[lang_arg(), raw_ast_arg()]).expect("typecheck must succeed");
    assert_eq!(result, json!("Nat"));
    // The deserialized Ast was forwarded to the handle.
    let forwarded = factory.last_ast().expect("ast was forwarded");
    assert_eq!(forwarded.len(), 1);
    assert_eq!(forwarded.nodes()[0].kind, "value");
}

#[test]
fn call_with_tagged_ast_returns_handle_result() {
    let factory = Rc::new(MockFactory::new().with_result(json!("Bool")));
    let module = make_module(Rc::clone(&factory));
    let result = call(&module, &[lang_arg(), tagged_ast_arg()]).expect("typecheck must succeed");
    assert_eq!(result, json!("Bool"));
    // The tagged Ast was extracted via `as_ast` and forwarded.
    let forwarded = factory.last_ast().expect("ast was forwarded");
    assert_eq!(forwarded.len(), 1);
    assert_eq!(forwarded.nodes()[0].kind, "value");
}

// ---------------------------------------------------------------------------
// TypecheckFn::call — error paths
// ---------------------------------------------------------------------------

#[test]
fn call_with_no_args_errors_about_language() {
    let module = make_module(Rc::new(MockFactory::new()));
    let err = call(&module, &[]).unwrap_err();
    assert!(err.to_string().contains("Language value"), "got: {}", err);
}

#[test]
fn call_with_non_language_first_arg_errors() {
    let module = make_module(Rc::new(MockFactory::new()));
    let err = call(&module, &[json!("not-a-language"), raw_ast_arg()]).unwrap_err();
    assert!(err.to_string().contains("Language value"), "got: {}", err);
}

#[test]
fn call_with_missing_ast_arg_errors() {
    let module = make_module(Rc::new(MockFactory::new()));
    let err = call(&module, &[lang_arg()]).unwrap_err();
    assert!(
        err.to_string().contains("expected (lang, ast)"),
        "got: {}",
        err
    );
}

#[test]
fn call_with_non_deserializable_ast_errors() {
    let module = make_module(Rc::new(MockFactory::new()));
    // A plain string is neither a tagged `__ast__` value nor a valid
    // serialized `Ast` (which is an object with `nodes`).
    let err = call(&module, &[lang_arg(), json!("not-an-ast")]).unwrap_err();
    assert!(err.to_string().contains("typecheck:"), "got: {}", err);
}

#[test]
fn call_propagates_factory_build_error() {
    let factory = Rc::new(MockFactory::new().with_build_err(LangError::engine("build-boom")));
    let module = make_module(factory);
    let err = call(&module, &[lang_arg(), raw_ast_arg()]).unwrap_err();
    assert!(err.to_string().contains("build-boom"), "got: {}", err);
}

#[test]
fn call_propagates_handle_typecheck_error() {
    let factory =
        Rc::new(MockFactory::new().with_typecheck_err(LangError::engine("typecheck-boom")));
    let module = make_module(factory);
    let err = call(&module, &[lang_arg(), raw_ast_arg()]).unwrap_err();
    assert!(err.to_string().contains("typecheck-boom"), "got: {}", err);
}
