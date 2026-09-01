//! Unit tests for the `run_compiled` meta library.
//!
//! These test the [`RunCompiledModule`] / [`RunCompiledFn`] plumbing in
//! isolation — module surface, compiled-handle extraction, error paths, and
//! delegate forwarding — using a mock [`RuntimeFactory`] / [`RuntimeHandle`]
//! so the leaf crate need not pull in the full kit.

use std::cell::RefCell;
use std::rc::Rc;

use handl_language::Language;
use handl_language::LangError;
use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup};
use handl_runtime::{
    CompiledHandle, RuntimeFactory, RuntimeHandle, as_compiled_id, as_compiled_target,
    compiled_value,
};
use serde_json::{Value, json};

use crate::RunCompiledModule;

// ---------------------------------------------------------------------------
// Mock RuntimeFactory / RuntimeHandle
// ---------------------------------------------------------------------------

struct MockHandle {
    run_err: Option<LangError>,
    last_handle: Rc<RefCell<Option<CompiledHandle>>>,
    canned_result: Value,
}

impl RuntimeHandle for MockHandle {
    fn lex(&self, _src: &handl_language::Source) -> Result<handl_language::Tokens, LangError> {
        Err(LangError::NotImplemented)
    }

    fn parse(&self, _toks: &handl_language::Tokens) -> Result<handl_language::Ast, LangError> {
        Err(LangError::NotImplemented)
    }

    fn eval(&self, _ast: &handl_language::Ast) -> Result<Value, LangError> {
        Err(LangError::NotImplemented)
    }

    fn typecheck(
        &self,
        _ast: &handl_language::Ast,
    ) -> Result<handl_runtime::TypecheckResult, LangError> {
        Err(LangError::NotImplemented)
    }

    fn compile(
        &self,
        _ast: &handl_language::Ast,
        _target: &str,
    ) -> Result<CompiledHandle, LangError> {
        Err(LangError::NotImplemented)
    }

    fn run_compiled(&self, handle: &CompiledHandle) -> Result<Value, LangError> {
        *self.last_handle.borrow_mut() = Some(handle.clone());
        match &self.run_err {
            Some(e) => Err(e.clone()),
            None => Ok(self.canned_result.clone()),
        }
    }
}

struct MockFactory {
    build_err: Option<LangError>,
    run_err: Option<LangError>,
    canned_result: Value,
    last_handle: Rc<RefCell<Option<CompiledHandle>>>,
}

impl MockFactory {
    fn new() -> Self {
        Self {
            build_err: None,
            run_err: None,
            canned_result: json!(3),
            last_handle: Rc::new(RefCell::new(None)),
        }
    }

    fn with_build_err(mut self, e: LangError) -> Self {
        self.build_err = Some(e);
        self
    }

    fn with_run_err(mut self, e: LangError) -> Self {
        self.run_err = Some(e);
        self
    }

    fn with_result(mut self, v: Value) -> Self {
        self.canned_result = v;
        self
    }

    fn last_handle(&self) -> Option<CompiledHandle> {
        self.last_handle.borrow().clone()
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
            run_err: self.run_err.clone(),
            last_handle: Rc::clone(&self.last_handle),
            canned_result: self.canned_result.clone(),
        }))
    }
}

/// A mock delegate [`ModuleLookup`] that resolves a single known name, used to
/// verify the `RunCompiledModule` forwards unknown names to its delegate.
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

fn make_module(factory: Rc<MockFactory>) -> RunCompiledModule {
    RunCompiledModule::new(factory, None)
}

fn make_module_with_delegate(
    factory: Rc<MockFactory>,
    delegate: Rc<dyn ModuleLookup>,
) -> RunCompiledModule {
    RunCompiledModule::new(factory, Some(delegate))
}

#[test]
fn module_name_is_run_compiled() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert_eq!(module.name(), "run_compiled");
}

#[test]
fn module_function_lookup() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.function("run_compiled").is_some());
    assert!(module.function("nope").is_none());
}

#[test]
fn module_functions_lists_run_compiled() {
    let module = make_module(Rc::new(MockFactory::new()));
    let fns = module.functions();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].0, "run_compiled");
}

// ---------------------------------------------------------------------------
// ModuleLookup
// ---------------------------------------------------------------------------

#[test]
fn lookup_resolves_flat_and_qualified_name() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.lookup("run_compiled").is_some());
    assert!(module.lookup("run_compiled.run_compiled").is_some());
    assert!(module.lookup("other").is_none());
}

#[test]
fn lookup_forwards_unknown_names_to_delegate() {
    let delegate = Rc::new(DelegateLookup { name: "user.fn" });
    let module = make_module_with_delegate(Rc::new(MockFactory::new()), delegate);
    assert!(module.lookup("user.fn").is_some());
    assert!(module.lookup("run_compiled").is_some());
    assert!(module.lookup("run_compiled.run_compiled").is_some());
}

#[test]
fn lookup_returns_none_without_delegate_for_unknown_name() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.lookup("user.fn").is_none());
}

// ---------------------------------------------------------------------------
// RunCompiledFn::call — happy path
// ---------------------------------------------------------------------------

fn call(module: &RunCompiledModule, args: &[Value]) -> Result<Value, EvalError> {
    let fn_ = module
        .function("run_compiled")
        .expect("run_compiled fn present");
    let mut ctx = EvalContext::new();
    fn_.call(args, &mut ctx)
}

#[test]
fn call_with_compiled_handle_returns_result() {
    let factory = Rc::new(MockFactory::new().with_result(json!(11)));
    let module = make_module(Rc::clone(&factory));
    let handle_val = compiled_value(777, "native");
    let result = call(&module, &[handle_val]).expect("run_compiled must succeed");
    assert_eq!(result, json!(11));
    // The handle id/target were forwarded to the handle.
    let forwarded = factory.last_handle().expect("handle was forwarded");
    assert_eq!(forwarded.id, 777);
    assert_eq!(forwarded.target, "native");
}

#[test]
fn call_with_handle_missing_target_defaults_to_empty() {
    // A handle value without a "target" field: `as_compiled_target` returns
    // None and the function falls back to an empty target string.
    let factory = Rc::new(MockFactory::new());
    let module = make_module(Rc::clone(&factory));
    let handle_val = json!({ "__compiled__": 5 });
    assert_eq!(as_compiled_id(&handle_val), Some(5));
    assert_eq!(as_compiled_target(&handle_val), None);
    let result = call(&module, &[handle_val]).expect("run_compiled must succeed");
    assert_eq!(result, json!(3));
    let forwarded = factory.last_handle().expect("handle was forwarded");
    assert_eq!(forwarded.id, 5);
    assert_eq!(forwarded.target, "");
}

// ---------------------------------------------------------------------------
// RunCompiledFn::call — error paths
// ---------------------------------------------------------------------------

#[test]
fn call_with_no_args_errors_about_handle() {
    let module = make_module(Rc::new(MockFactory::new()));
    let err = call(&module, &[]).unwrap_err();
    assert!(err.to_string().contains("expected (handle)"), "got: {}", err);
}

#[test]
fn call_with_non_handle_value_errors() {
    let module = make_module(Rc::new(MockFactory::new()));
    let err = call(&module, &[json!("not-a-handle")]).unwrap_err();
    assert!(
        err.to_string().contains("compiled-program handle"),
        "got: {}",
        err
    );
}

#[test]
fn call_with_null_value_errors() {
    let module = make_module(Rc::new(MockFactory::new()));
    let err = call(&module, &[Value::Null]).unwrap_err();
    assert!(
        err.to_string().contains("compiled-program handle"),
        "got: {}",
        err
    );
}

#[test]
fn call_propagates_factory_build_error() {
    let factory = Rc::new(
        MockFactory::new()
            .with_build_err(LangError::Engine("build-boom".into())),
    );
    let module = make_module(factory);
    let err = call(&module, &[compiled_value(1, "native")]).unwrap_err();
    assert!(err.to_string().contains("build-boom"), "got: {}", err);
}

#[test]
fn call_propagates_handle_run_error() {
    let factory = Rc::new(
        MockFactory::new()
            .with_run_err(LangError::Engine("run-boom".into())),
    );
    let module = make_module(factory);
    let err = call(&module, &[compiled_value(1, "native")]).unwrap_err();
    assert!(err.to_string().contains("run-boom"), "got: {}", err);
}
