//! Integration test: `compile` / `run_compiled` via the meta host
//! functions, using `kit-canonical` (which registers all engines including
//! compile engines). Lives in `tests/` to avoid the dev-dependency cycle
//! that prevents `handl-runtime` lib tests from using compile-engine crates.

use handl_kit_prelude::Kit;
use handl_language::{Ast, AstNode, Value, language_value};
use handl_runtime::Runtime;
use serde_json::json;

/// Parse `source` under the canonical language into an Ast value
/// (JSON-serialized), for use as the `ast` argument to `compile`.
fn parse_ast_value(runtime: &Runtime, lang_val: &Value, source: &str) -> Value {
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(lang_val.clone()));
    ast.push(AstNode::new("value").with_payload(json!(source)));
    ast.push(AstNode::new("foreign.lex").with_payload(json!({"children": [0, 1]})));
    let tokens = runtime.eval(&ast).expect("lex must succeed");
    let mut ast2 = Ast::new();
    ast2.push(AstNode::new("value").with_payload(lang_val.clone()));
    ast2.push(AstNode::new("value").with_payload(tokens));
    ast2.push(AstNode::new("foreign.parse").with_payload(json!({"children": [0, 1]})));
    runtime.eval(&ast2).expect("parse must succeed")
}

/// Check that a value is a compiled-program handle (tagged object with
/// `__compiled__` key).
fn assert_is_compiled_handle(handle: &Value) {
    assert!(
        !handle.is_string(),
        "compile should return a handle, not a stub string, got: {}",
        handle
    );
    let obj = handle
        .as_object()
        .unwrap_or_else(|| panic!("compile should return a handle object, got: {}", handle));
    assert!(
        obj.contains_key("__compiled__"),
        "compile should return a tagged handle with __compiled__ key, got: {}",
        handle
    );
}

/// `compile(lang, ast, "bytecode")` returns a compiled-program handle
/// (not the old stub string), and `run_compiled(handle)` produces the
/// same result as `eval(lang, ast)`.
#[test]
fn compile_bytecode_run_compiled_matches_eval() {
    let kit = Kit::new().expect("kit must assemble");
    let runtime = kit.runtime();
    let lang_val = language_value(&runtime.lang());
    let ast_val = parse_ast_value(runtime, &lang_val, "1 + 2");

    // compile(lang, ast, "bytecode")
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(lang_val.clone()));
    ast.push(AstNode::new("value").with_payload(ast_val.clone()));
    ast.push(AstNode::new("value").with_payload(json!("bytecode")));
    ast.push(AstNode::new("foreign.compile").with_payload(json!({"children": [0, 1, 2]})));
    let handle = runtime.eval(&ast).expect("compile must succeed");
    assert_is_compiled_handle(&handle);

    // run_compiled(handle)
    let mut ast2 = Ast::new();
    ast2.push(AstNode::new("value").with_payload(handle));
    ast2.push(AstNode::new("foreign.run_compiled").with_payload(json!({"children": [0]})));
    let compiled_result = runtime.eval(&ast2).expect("run_compiled must succeed");

    // eval(lang, ast) for comparison
    let mut ast3 = Ast::new();
    ast3.push(AstNode::new("value").with_payload(lang_val));
    ast3.push(AstNode::new("value").with_payload(ast_val));
    ast3.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
    let eval_result = runtime.eval(&ast3).expect("eval must succeed");

    assert_eq!(
        compiled_result, eval_result,
        "run_compiled should match eval for the same ast"
    );
    assert_eq!(compiled_result, json!(3));
}

/// `compile(lang, ast, "native")` returns a compiled-program handle and
/// `run_compiled(handle)` produces the same result as `eval(lang, ast)`.
#[test]
fn compile_native_run_compiled_matches_eval() {
    let kit = Kit::new().expect("kit must assemble");
    let runtime = kit.runtime();
    let lang_val = language_value(&runtime.lang());
    let ast_val = parse_ast_value(runtime, &lang_val, "1 + 2");

    // compile(lang, ast, "native")
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(lang_val.clone()));
    ast.push(AstNode::new("value").with_payload(ast_val.clone()));
    ast.push(AstNode::new("value").with_payload(json!("native")));
    ast.push(AstNode::new("foreign.compile").with_payload(json!({"children": [0, 1, 2]})));
    let handle = runtime.eval(&ast).expect("compile must succeed");
    assert_is_compiled_handle(&handle);

    // run_compiled(handle)
    let mut ast2 = Ast::new();
    ast2.push(AstNode::new("value").with_payload(handle));
    ast2.push(AstNode::new("foreign.run_compiled").with_payload(json!({"children": [0]})));
    let compiled_result = runtime.eval(&ast2).expect("run_compiled must succeed");

    // eval(lang, ast) for comparison
    let mut ast3 = Ast::new();
    ast3.push(AstNode::new("value").with_payload(lang_val));
    ast3.push(AstNode::new("value").with_payload(ast_val));
    ast3.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
    let eval_result = runtime.eval(&ast3).expect("eval must succeed");

    assert_eq!(
        compiled_result, eval_result,
        "run_compiled (native) should match eval for the same ast"
    );
    assert_eq!(compiled_result, json!(3));
}
