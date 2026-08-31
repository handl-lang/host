//! Unit tests for the `compile` meta library.
//!
//! These test the [`CompileModule`] / [`CompileFn`] plumbing in isolation —
//! module surface, argument extraction, the tagged `__ast__` vs raw-serialized
//! Ast acceptance paths, the default `target` fallback, the compiled-handle
//! value shape, error paths, and delegate forwarding — using a mock
//! [`RuntimeFactory`] / [`RuntimeHandle`] so the leaf crate need not pull in
//! the full kit.

use std::cell::RefCell;
use std::rc::Rc;

use handl_language::{Ast, AstNode, Language, ast_value, language_value};
use handl_language::LangError;
use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup};
use handl_runtime::{
    CompiledHandle, RuntimeFactory, RuntimeHandle, as_compiled_id, as_compiled_target,
    compiled_value,
};
use serde_json::{Value, json};

use crate::CompileModule;

// ---------------------------------------------------------------------------
// Mock RuntimeFactory / RuntimeHandle
// ---------------------------------------------------------------------------

/// A canned Ast used as the input to `compile.compile`.
fn sample_ast() -> Ast {
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast
}

/// A raw JSON-serialized [`Ast`] value (the non-tagged form accepted by
/// `compile.compile`).
fn raw_ast_arg() -> Value {
    serde_json::to_value(sample_ast()).expect("Ast serializes")
}

/// A tagged `__ast__` value (the form bound to an unevaluated expression).
fn tagged_ast_arg() -> Value {
    ast_value(&sample_ast())
}

struct MockHandle {
    compile_err: Option<LangError>,
    last_target: Rc<RefCell<Option<String>>>,
    next_id: u64,
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

    fn compile(&self, _ast: &Ast, target: &str) -> Result<CompiledHandle, LangError> {
        *self.last_target.borrow_mut() = Some(target.to_string());
        match &self.compile_err {
            Some(e) => Err(e.clone()),
            None => Ok(CompiledHandle {
                id: self.next_id,
                target: target.to_string(),
            }),
        }
    }

    fn run_compiled(&self, _handle: &CompiledHandle) -> Result<Value, LangError> {
        Err(LangError::NotImplemented)
    }
}

struct MockFactory {
    build_err: Option<LangError>,
    compile_err: Option<LangError>,
    last_target: Rc<RefCell<Option<String>>>,
    next_id: u64,
}

impl MockFactory {
    fn new() -> Self {
        Self {
            build_err: None,
            compile_err: None,
            last_target: Rc::new(RefCell::new(None)),
            next_id: 1234,
        }
    }

    fn with_build_err(mut self, e: LangError) -> Self {
        self.build_err = Some(e);
        self
    }

    fn with_compile_err(mut self, e: LangError) -> Self {
        self.compile_err = Some(e);
        self
    }

    fn last_target(&self) -> Option<String> {
        self.last_target.borrow().clone()
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
            compile_err: self.compile_err.clone(),
            last_target: Rc::clone(&self.last_target),
            next_id: self.next_id,
        }))
    }
}

/// A mock delegate [`ModuleLookup`] that resolves a single known name, used to
/// verify the `CompileModule` forwards unknown names to its delegate.
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

fn make_module(factory: Rc<MockFactory>) -> CompileModule {
    CompileModule::new(factory, None)
}

fn make_module_with_delegate(
    factory: Rc<MockFactory>,
    delegate: Rc<dyn ModuleLookup>,
) -> CompileModule {
    CompileModule::new(factory, Some(delegate))
}

#[test]
fn module_name_is_compile() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert_eq!(module.name(), "compile");
}

#[test]
fn module_function_lookup() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.function("compile").is_some());
    assert!(module.function("nope").is_none());
}

#[test]
fn module_functions_lists_compile() {
    let module = make_module(Rc::new(MockFactory::new()));
    let fns = module.functions();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].0, "compile");
}

// ---------------------------------------------------------------------------
// ModuleLookup
// ---------------------------------------------------------------------------

#[test]
fn lookup_resolves_flat_and_qualified_name() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.lookup("compile").is_some());
    assert!(module.lookup("compile.compile").is_some());
    assert!(module.lookup("other").is_none());
}

#[test]
fn lookup_forwards_unknown_names_to_delegate() {
    let delegate = Rc::new(DelegateLookup { name: "user.fn" });
    let module = make_module_with_delegate(Rc::new(MockFactory::new()), delegate);
    assert!(module.lookup("user.fn").is_some());
    assert!(module.lookup("compile").is_some());
    assert!(module.lookup("compile.compile").is_some());
}

#[test]
fn lookup_returns_none_without_delegate_for_unknown_name() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.lookup("user.fn").is_none());
}

// ---------------------------------------------------------------------------
// CompileFn::call — happy path
// ---------------------------------------------------------------------------

fn call(module: &CompileModule, args: &[Value]) -> Result<Value, EvalError> {
    let compile_fn = module.function("compile").expect("compile fn present");
    let mut ctx = EvalContext::new();
    compile_fn.call(args, &mut ctx)
}

fn lang_arg() -> Value {
    language_value(&Language::new())
}

#[test]
fn call_with_raw_ast_and_target_returns_compiled_handle() {
    let factory = Rc::new(MockFactory::new());
    let module = make_module(Rc::clone(&factory));
    let result =
        call(&module, &[lang_arg(), raw_ast_arg(), json!("bytecode")])
            .expect("compile must succeed");
    // The result is a tagged compiled-program handle.
    assert_eq!(as_compiled_id(&result), Some(1234));
    assert_eq!(as_compiled_target(&result), Some("bytecode"));
    // The target was forwarded to the handle.
    assert_eq!(factory.last_target().as_deref(), Some("bytecode"));
}

#[test]
fn call_with_tagged_ast_returns_compiled_handle() {
    let factory = Rc::new(MockFactory::new());
    let module = make_module(Rc::clone(&factory));
    let result =
        call(&module, &[lang_arg(), tagged_ast_arg(), json!("native")])
            .expect("compile must succeed");
    assert_eq!(as_compiled_id(&result), Some(1234));
    assert_eq!(as_compiled_target(&result), Some("native"));
}

#[test]
fn call_defaults_target_to_native_when_missing() {
    let factory = Rc::new(MockFactory::new());
    let module = make_module(Rc::clone(&factory));
    // Only lang + ast — no target argument.
    let result = call(&module, &[lang_arg(), raw_ast_arg()]).expect("compile must succeed");
    assert_eq!(as_compiled_target(&result), Some("native"));
    assert_eq!(factory.last_target().as_deref(), Some("native"));
}

#[test]
fn call_defaults_target_to_native_when_non_string() {
    let factory = Rc::new(MockFactory::new());
    let module = make_module(Rc::clone(&factory));
    // A non-string target falls back to "native".
    let result =
        call(&module, &[lang_arg(), raw_ast_arg(), json!(42)]).expect("compile must succeed");
    assert_eq!(as_compiled_target(&result), Some("native"));
}

#[test]
fn call_result_matches_compiled_value_helper() {
    let factory = Rc::new(MockFactory::new());
    let module = make_module(factory);
    let result =
        call(&module, &[lang_arg(), raw_ast_arg(), json!("native")])
            .expect("compile must succeed");
    // The returned value is exactly what `compiled_value` produces.
    assert_eq!(result, compiled_value(1234, "native"));
}

// ---------------------------------------------------------------------------
// CompileFn::call — error paths
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
    let err =
        call(&module, &[json!("not-a-language"), raw_ast_arg(), json!("native")]).unwrap_err();
    assert!(err.to_string().contains("Language value"), "got: {}", err);
}

#[test]
fn call_with_missing_ast_arg_errors() {
    let module = make_module(Rc::new(MockFactory::new()));
    let err = call(&module, &[lang_arg()]).unwrap_err();
    assert!(
        err.to_string().contains("expected (lang, ast, target)"),
        "got: {}",
        err
    );
}

#[test]
fn call_with_non_deserializable_ast_errors() {
    let module = make_module(Rc::new(MockFactory::new()));
    // A plain string is neither a tagged `__ast__` value nor a valid
    // serialized `Ast`.
    let err =
        call(&module, &[lang_arg(), json!("not-an-ast"), json!("native")]).unwrap_err();
    assert!(err.to_string().contains("compile:"), "got: {}", err);
}

#[test]
fn call_propagates_factory_build_error() {
    let factory = Rc::new(
        MockFactory::new()
            .with_build_err(LangError::Engine("build-boom".into())),
    );
    let module = make_module(factory);
    let err =
        call(&module, &[lang_arg(), raw_ast_arg(), json!("native")]).unwrap_err();
    assert!(err.to_string().contains("build-boom"), "got: {}", err);
}

#[test]
fn call_propagates_handle_compile_error() {
    let factory = Rc::new(
        MockFactory::new()
            .with_compile_err(LangError::Engine("compile-boom".into())),
    );
    let module = make_module(factory);
    let err =
        call(&module, &[lang_arg(), raw_ast_arg(), json!("native")]).unwrap_err();
    assert!(err.to_string().contains("compile-boom"), "got: {}", err);
}
