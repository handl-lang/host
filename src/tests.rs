//! Unit tests for the `parse` meta library.
//!
//! These test the [`ParseModule`] / [`ParseFn`] plumbing in isolation — module
//! surface, argument extraction, tokens deserialization, error paths, delegate
//! forwarding, and Ast serialization — using a mock [`RuntimeFactory`] /
//! [`RuntimeHandle`] so the leaf crate need not pull in the full kit.

use std::cell::RefCell;
use std::rc::Rc;

use handl_language::LangError;
use handl_language::{language_value, Ast, AstNode, Language, Token, Tokens};
use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup};
use handl_runtime::{RuntimeFactory, RuntimeHandle};
use serde_json::{json, Value};

use crate::ParseModule;

// ---------------------------------------------------------------------------
// Mock RuntimeFactory / RuntimeHandle
// ---------------------------------------------------------------------------

/// A canned Ast the mock handle returns from `parse`.
fn canned_ast() -> Ast {
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast
}

/// A valid serialized [`Tokens`] value usable as the second argument to
/// `parse.parse`.
fn tokens_arg() -> Value {
    serde_json::to_value(Tokens(vec![Token {
        kind: "num".into(),
        text: "42".into(),
        ..Default::default()
    }]))
    .expect("Tokens serialize")
}

struct MockHandle {
    parse_err: Option<LangError>,
    last_tokens: Rc<RefCell<Option<Tokens>>>,
}

impl RuntimeHandle for MockHandle {
    fn lex(&self, _src: &handl_language::Source) -> Result<Tokens, LangError> {
        Err(LangError::NotImplemented)
    }

    fn parse(&self, toks: &Tokens) -> Result<Ast, LangError> {
        *self.last_tokens.borrow_mut() = Some(toks.clone());
        match &self.parse_err {
            Some(e) => Err(e.clone()),
            None => Ok(canned_ast()),
        }
    }

    fn eval(&self, _ast: &Ast) -> Result<Value, LangError> {
        Err(LangError::NotImplemented)
    }

    fn typecheck(&self, _ast: &Ast) -> Result<handl_runtime::TypecheckResult, LangError> {
        Err(LangError::NotImplemented)
    }

    fn compile(
        &self,
        _ast: &Ast,
        _target: &str,
    ) -> Result<handl_runtime::CompiledHandle, LangError> {
        Err(LangError::NotImplemented)
    }

    fn run_compiled(&self, _handle: &handl_runtime::CompiledHandle) -> Result<Value, LangError> {
        Err(LangError::NotImplemented)
    }
}

struct MockFactory {
    build_err: Option<LangError>,
    parse_err: Option<LangError>,
    last_tokens: Rc<RefCell<Option<Tokens>>>,
}

impl MockFactory {
    fn new() -> Self {
        Self {
            build_err: None,
            parse_err: None,
            last_tokens: Rc::new(RefCell::new(None)),
        }
    }

    fn with_build_err(mut self, e: LangError) -> Self {
        self.build_err = Some(e);
        self
    }

    fn with_parse_err(mut self, e: LangError) -> Self {
        self.parse_err = Some(e);
        self
    }

    fn last_tokens(&self) -> Option<Tokens> {
        self.last_tokens.borrow().clone()
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
            parse_err: self.parse_err.clone(),
            last_tokens: Rc::clone(&self.last_tokens),
        }))
    }
}

/// A mock delegate [`ModuleLookup`] that resolves a single known name, used to
/// verify the `ParseModule` forwards unknown names to its delegate.
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

fn make_module(factory: Rc<MockFactory>) -> ParseModule {
    ParseModule::new(factory, None)
}

fn make_module_with_delegate(
    factory: Rc<MockFactory>,
    delegate: Rc<dyn ModuleLookup>,
) -> ParseModule {
    ParseModule::new(factory, Some(delegate))
}

#[test]
fn module_name_is_parse() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert_eq!(module.name(), "parse");
}

#[test]
fn module_function_lookup() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.function("parse").is_some());
    assert!(module.function("nope").is_none());
}

#[test]
fn module_functions_lists_parse() {
    let module = make_module(Rc::new(MockFactory::new()));
    let fns = module.functions();
    assert_eq!(fns.len(), 1);
    assert_eq!(fns[0].0, "parse");
}

// ---------------------------------------------------------------------------
// ModuleLookup
// ---------------------------------------------------------------------------

#[test]
fn lookup_resolves_flat_and_qualified_name() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.lookup("parse").is_some());
    assert!(module.lookup("parse.parse").is_some());
    assert!(module.lookup("other").is_none());
}

#[test]
fn lookup_forwards_unknown_names_to_delegate() {
    let delegate = Rc::new(DelegateLookup { name: "user.fn" });
    let module = make_module_with_delegate(Rc::new(MockFactory::new()), delegate);
    assert!(module.lookup("user.fn").is_some());
    assert!(module.lookup("parse").is_some());
    assert!(module.lookup("parse.parse").is_some());
}

#[test]
fn lookup_returns_none_without_delegate_for_unknown_name() {
    let module = make_module(Rc::new(MockFactory::new()));
    assert!(module.lookup("user.fn").is_none());
}

// ---------------------------------------------------------------------------
// ParseFn::call — happy path
// ---------------------------------------------------------------------------

fn call(module: &ParseModule, args: &[Value]) -> Result<Value, EvalError> {
    let parse_fn = module.function("parse").expect("parse fn present");
    let mut ctx = EvalContext::new();
    parse_fn.call(args, &mut ctx)
}

fn lang_arg() -> Value {
    language_value(&Language::new())
}

#[test]
fn call_returns_serialized_ast() {
    let factory = Rc::new(MockFactory::new());
    let module = make_module(Rc::clone(&factory));
    let result = call(&module, &[lang_arg(), tokens_arg()]).expect("parse must succeed");
    // Ast serializes as {"nodes": [...]}.
    assert!(
        result.get("nodes").is_some(),
        "parse should return an Ast with 'nodes', got: {}",
        result
    );
    // The canned Ast has one node.
    assert_eq!(result.get("nodes").unwrap().as_array().unwrap().len(), 1);
    // The tokens were forwarded to the handle.
    let forwarded = factory.last_tokens().expect("tokens were forwarded");
    assert_eq!(forwarded.0.len(), 1);
    assert_eq!(forwarded.0[0].text, "42");
}

// ---------------------------------------------------------------------------
// ParseFn::call — error paths
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
    let err = call(&module, &[json!("not-a-language"), tokens_arg()]).unwrap_err();
    assert!(err.to_string().contains("Language value"), "got: {}", err);
}

#[test]
fn call_with_missing_tokens_arg_errors() {
    let module = make_module(Rc::new(MockFactory::new()));
    let err = call(&module, &[lang_arg()]).unwrap_err();
    assert!(
        err.to_string().contains("expected (lang, tokens)"),
        "got: {}",
        err
    );
}

#[test]
fn call_with_non_deserializable_tokens_errors() {
    let module = make_module(Rc::new(MockFactory::new()));
    // A plain string is not a valid serialized `Tokens` (which is an array).
    let err = call(&module, &[lang_arg(), json!("not-tokens")]).unwrap_err();
    assert!(err.to_string().contains("parse:"), "got: {}", err);
}

#[test]
fn call_propagates_factory_build_error() {
    let factory = Rc::new(MockFactory::new().with_build_err(LangError::engine("build-boom")));
    let module = make_module(factory);
    let err = call(&module, &[lang_arg(), tokens_arg()]).unwrap_err();
    assert!(err.to_string().contains("build-boom"), "got: {}", err);
}

#[test]
fn call_propagates_handle_parse_error() {
    let factory = Rc::new(MockFactory::new().with_parse_err(LangError::engine("parse-boom")));
    let module = make_module(factory);
    let err = call(&module, &[lang_arg(), tokens_arg()]).unwrap_err();
    assert!(err.to_string().contains("parse-boom"), "got: {}", err);
}
