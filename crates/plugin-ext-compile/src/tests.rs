//! Dogfood tests for the compile dispatcher (ADR-0114 Phase 14).

use handl_language::{Ast, AstNode, Language};
use handl_plugin_compile::{Compiler, CompilerInput};
use handl_plugin_compile_js::{JsCompiler, JsConfig};
use handl_plugin_compile_llvm::{LlvmCompiler, LlvmConfig, X86_64};
use plugin_engine::Term;
use serde_json::{Number, Value};

use crate::{CompileDispatcher, CompileFileRequest};

fn js_input(term: Term) -> CompilerInput {
    let mut ast = Ast::new();
    let root = ast.push(AstNode::new("module"));
    CompilerInput::new(
        "main = 1".to_string(),
        ast,
        root,
        Language::new(),
        term,
        plugin_engine_typecheck::TypecheckReport::default(),
    )
}

#[test]
fn js_compiler_emits_expected_output() {
    let input = js_input(Term::constant(Value::Number(Number::from(42))));
    let config = JsConfig::esm_deno();
    let output = JsCompiler.compile(&input, &config).unwrap();
    let js = String::from_utf8_lossy(output.primary_output().unwrap()).to_string();
    assert!(js.contains("export default 42;"), "got: {js}");
}

#[test]
fn compile_dispatcher_js_target_contains_export() {
    let input = js_input(Term::constant(Value::Number(Number::from(7))));
    let config = JsConfig::esm_deno();
    let dispatcher = CompileDispatcher::new();
    let output = dispatcher.compile("js", input, &config).unwrap();
    let js = String::from_utf8_lossy(output.primary_output().unwrap()).to_string();
    assert!(js.contains("export"), "expected `export` in: {js}");
}

#[test]
fn compile_dispatcher_llvm_produces_object_artifact() {
    let input = js_input(Term::constant(Value::Null));
    let config = LlvmConfig::new(Box::new(X86_64));
    let dispatcher = CompileDispatcher::new();
    let output = dispatcher.compile("llvm", input, &config).unwrap();
    assert!(
        output.artifacts().iter().any(|a| a.name.ends_with(".o")),
        "expected an artifact named `*.o`, got {:?}",
        output.artifacts()
    );
}

#[test]
fn llvm_compiler_alone_produces_object_artifact() {
    let input = js_input(Term::constant(Value::Null));
    let config = LlvmConfig::new(Box::new(X86_64));
    let output = LlvmCompiler::new().compile(&input, &config).unwrap();
    assert!(
        output.artifacts().iter().any(|a| a.name.ends_with(".o")),
        "expected an artifact named `*.o`, got {:?}",
        output.artifacts()
    );
}

#[test]
fn compile_file_request_builds_input_with_root_ref() {
    let req = CompileFileRequest {
        target: "js".to_string(),
        path: std::path::PathBuf::from("main.hnl"),
        output: None,
        source_map: false,
        debug: false,
        format: Some("esm".to_string()),
        to: None,
        emit: None,
    };
    let src = "x = 1";
    let ast = CompileFileRequest::build_ast(src);
    let lang = Language::new();
    let term = CompileFileRequest::root_term_for(&lang);
    let report = plugin_engine_typecheck::TypecheckReport::default();
    let input = req.build_input(src, &ast, &lang, &term, &report);
    assert_eq!(input.root.index(), 0);
    assert_eq!(input.ast.len(), 1);
    assert_eq!(input.language, lang);
}
