//! Unit tests for the `lex` meta library.
//!
//! These test the [`LexModule`] / [`LexFn`] plumbing in isolation — module
//! surface, argument extraction, error paths, delegate forwarding, and
//! serialization — using a mock [`RuntimeFactory`] / [`RuntimeHandle`] so the
//! leaf crate need not pull in the full kit.

use std::cell::RefCell;
use std::rc::Rc;

use handl_language::{Language, Source, Token, Tokens, language_value};
use handl_language::LangError;
use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup};
use handl_runtime::{RuntimeFactory, RuntimeHandle};
use serde_json::{Value, json};

use crate::LexModule;

// ---------------------------------------------------------------------------
// Mock RuntimeFactory / RuntimeHandle
// ---------------------------------------------------------------------------

/// A canned token stream the mock handle returns from `lex`.
fn canned_tokens() -> Tokens {
    Tokens(vec![
        Token {
            kind: "num".into(),
            text: "1".into(),
        },
        Token {
            kind: "plus".into(),
            text: "+".into(),
        },
        Token {
            kind: "num".into(),
            text: "2".into(),
        },
    ])
}

/// Configurable mock handle. Each stage either returns a canned value or the
/// configured error. `last_source` captures the source handed to `lex`.
struct MockHandle {
    lex_err: Option<LangError>,
    last_source: Rc<RefCell<Option<String>>>,
}

impl RuntimeHandle for MockHandle {
    fn lex(&self, src: &Source) -> Result<Tokens, LangError> {
        *self.last_source.borrow_mut() = Some(src.as_str().to_string());
        match &self.lex_err {
            Some(e) => Err(e.clone()),
            None => Ok(canned_tokens()),
        }
    }

    fn parse(&self, _toks: &Tokens) -> Result<handl_language::Ast, LangError> {
        Err(LangError::NotImplemented)
    }

    fn eval(&self, _ast: &handl_language::Ast) -> Result<Value, LangError> {
        Err(LangError::NotImplemented)
    }

    fn compile(
        &self,
        _ast: &handl_language::Ast,
        _target: &str,
    ) -> Result<handl_runtime::CompiledHandle, LangError> {
        Err(LangError::NotImplemented)
    }

    fn run_compiled(
        &self,
        _handle: &handl_runtime::CompiledHandle,
    ) -> Result<Value, LangError> {
        Err(LangError::NotImplemented)
    }
}

/// Configurable mock factory. `build_err` makes `build` fail; otherwise it
/// returns a [`MockHandle`] sharing the `last_source` cell so tests can inspect
/// what source text was forwarded.
struct MockFactory {
    build_err: Option<LangError>,
    lex_err: Option<LangError>,
    last_source: Rc<RefCell<Option<String>>>,
}

impl MockFactory {
    fn new() -> Self {
        Self {
            build_err: None,
            lex_err: None,
            last_source: Rc::new(RefCell::new(None)),
        }
    }

    fn with_build_err(mut self, e: LangError) -> Self {
        self.build_err = Some(e);
        self
    }

    fn with_lex_err(mut self, e: LangError) -> Self {
        self.lex_err = Some(e);
        self
    }

    fn last_source(&self) -> Option<String> {
        self.last_source.borrow().clone()
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
            lex_err: self.lex_err.clone(),
            last_source: Rc::clone(&self.last_source),
        }))
    }
}

/// A mock delegate [`ModuleLookup`] that resolves a single known name, used to
/// verify the `LexModule` forwards unknown names to its delegate.
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

fn make_module(factory: Rc<MockFactory>) -> LexModule {
    LexModule::new(factory, None)
}

fn make_module_with_delegate(
    factory: Rc<MockFactory>,
    delegate: Rc<dyn ModuleLookup>,
) -> LexModule {
    LexModule::new(factory, Some(delegate))
}

#[test]
fn module_name_is_lex() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert_eq!(module.name(), "lex");
}

#[test]
fn module_function_lookup() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.function("lex").is_some());
    assert!(module.function("nope").is_none());
}

#[test]
fn module_functions_lists_lex() {
    let module = make_module(Rc::new(MockFactory::new()));
    let fns = module.functions();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].0, "lex");
}

// ---------------------------------------------------------------------------
// ModuleLookup
// ---------------------------------------------------------------------------

#[test]
fn lookup_resolves_flat_and_qualified_name() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.lookup("lex").is_some());
    assert!(module.lookup("lex.lex").is_some());
    assert!(module.lookup("other").is_none());
}

#[test]
fn lookup_forwards_unknown_names_to_delegate() {
    let delegate = Rc::new(DelegateLookup { name: "user.fn" });
    let module = make_module_with_delegate(Rc::new(MockFactory::new()), delegate);
    // Unknown name resolves through the delegate.
    assert!(module.lookup("user.fn").is_some());
    // The lex names still resolve locally, not via the delegate.
    assert!(module.lookup("lex").is_some());
    assert!(module.lookup("lex.lex").is_some());
}

#[test]
fn lookup_returns_none_without_delegate_for_unknown_name() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.lookup("user.fn").is_none());
}

// ---------------------------------------------------------------------------
// LexFn::call — happy path
// ---------------------------------------------------------------------------

fn call(module: &LexModule, args: &[Value]) -> Result<Value, EvalError> {
    let lex_fn = module.function("lex").expect("lex fn present");
    let mut ctx = EvalContext::new();
    lex_fn.call(args, &mut ctx)
}

fn lang_arg() -> Value {
    language_value(&Language::new())
}

#[test]
fn call_returns_serialized_tokens() {
    let factory = Rc::new(MockFactory::new());
    let module = make_module(Rc::clone(&factory));
    let result = call(&module, &[lang_arg(), json!("1 + 2")]).expect("lex must succeed");
    assert!(
        result.is_array(),
        "lex should return a JSON array of tokens, got: {}",
        result
    );
    // The canned token stream has three tokens.
    assert_eq!(result.as_array().unwrap().len(), 3);
    // The source text was forwarded to the handle.
    assert_eq!(factory.last_source().as_deref(), Some("1 + 2"));
}

// ---------------------------------------------------------------------------
// LexFn::call — error paths
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
    let err = call(&module, &[json!("not-a-language"), json!("text")]).unwrap_err();
    assert!(err.to_string().contains("Language value"), "got: {}", err);
}

#[test]
fn call_with_missing_text_arg_errors() {
    let module = make_module(Rc::new(MockFactory::new()));
    let err = call(&module, &[lang_arg()]).unwrap_err();
    assert!(err.to_string().contains("expected (lang, text)"), "got: {}", err);
}

#[test]
fn call_with_non_string_text_arg_errors() {
    let module = make_module(Rc::new(MockFactory::new()));
    let err = call(&module, &[lang_arg(), json!(42)]).unwrap_err();
    assert!(err.to_string().contains("expected (lang, text)"), "got: {}", err);
}

#[test]
fn call_propagates_factory_build_error() {
    let factory = Rc::new(
        MockFactory::new()
            .with_build_err(LangError::Engine("build-boom".into())),
    );
    let module = make_module(factory);
    let err = call(&module, &[lang_arg(), json!("1 + 2")]).unwrap_err();
    assert!(err.to_string().contains("build-boom"), "got: {}", err);
}

#[test]
fn call_propagates_handle_lex_error() {
    let factory = Rc::new(
        MockFactory::new()
            .with_lex_err(LangError::LexError {
                position: 3,
                message: "bad char".into(),
            }),
    );
    let module = make_module(factory);
    let err = call(&module, &[lang_arg(), json!("1 + 2")]).unwrap_err();
    assert!(err.to_string().contains("bad char"), "got: {}", err);
}

#[test]
fn lex_fn_is_send_safe_via_module_construction() {
    // Smoke test: the module can be constructed and dropped without panic.
    let factory = Rc::new(MockFactory::new());
    let module = make_module(factory);
    assert_eq!(module.name(), "lex");
}
