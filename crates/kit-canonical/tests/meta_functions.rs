//! # meta_functions.rs — meta host function behavior (migrated from
//! //! `handl-runtime/src/meta.rs`)
//!
//! ADR-0047: the meta host functions (`lex`/`parse`/`eval`/`compile`/
//! `run_compiled`) now live in the five `plugin-lib-lang-*` leaf crates and
//! are wired into the runtime by [`Kit::new`] / [`runtime_for_lang`]. These
//! tests verify the handl-callable meta surface works end-to-end through the
//! `foreign.*` eval schemes.
//!
//! The `run` host function is DROPPED (ADR-0047 §4) — tests that previously
//! compared `eval(parse(lex(text)))` to `run(text)` now assert the composed
//! pipeline result directly.

use handl_kit_canonical::{Kit, runtime_for_lang};
use handl_language::{
    Ast, AstNode, EvalSchemeDef, Language, Scheme, as_ast, as_language, is_ast_value,
    is_language_value, language_value,
};
use handl_runtime::Runtime;
use serde_json::json;

/// The canonical handl language (assembled from data fragments).
fn canonical_lang() -> Language {
    handl_kit_data_canonical::assemble()
        .expect("canonical kit must assemble")
        .lang()
        .clone()
}

/// Build a runtime for the canonical language via [`Kit::new`].
fn make_kit() -> Kit {
    Kit::new().expect("kit must build")
}

/// Build a runtime from an explicit [`Language`] (for tests that override
/// eval schemes). Injects `self` and wires the meta chain via
/// [`runtime_for_lang`].
fn make_runtime_from_lang(lang: Language) -> Runtime {
    runtime_for_lang(lang).expect("runtime must build")
}

/// `lex(lang, "1 + 2")` returns a JSON-serialized Tokens value.
#[test]
fn lex_host_function_returns_tokens() {
    let kit = make_kit();
    let runtime = kit.runtime();
    let lang_val = language_value(&canonical_lang());
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(lang_val));
    ast.push(AstNode::new("value").with_payload(json!("1 + 2")));
    ast.push(AstNode::new("foreign.lex").with_payload(json!({"children": [0, 1]})));
    let result = runtime.eval(&ast).expect("lex must succeed");
    // Tokens serialize as a JSON array of {kind, text} objects.
    assert!(
        result.is_array(),
        "lex should return an array, got: {}",
        result
    );
}

/// `parse(lang, tokens)` returns a JSON-serialized Ast value.
#[test]
fn parse_host_function_returns_ast() {
    let kit = make_kit();
    let runtime = kit.runtime();
    let lang_val = language_value(&canonical_lang());
    // First lex "1 + 2" to get tokens, then parse them.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(lang_val.clone()));
    ast.push(AstNode::new("value").with_payload(json!("1 + 2")));
    ast.push(AstNode::new("foreign.lex").with_payload(json!({"children": [0, 1]})));
    let tokens = runtime.eval(&ast).expect("lex must succeed");

    // Now parse: 0: lang, 1: <tokens>, 2: foreign.parse {children:[0,1]}
    let mut ast2 = Ast::new();
    ast2.push(AstNode::new("value").with_payload(lang_val));
    ast2.push(AstNode::new("value").with_payload(tokens));
    ast2.push(AstNode::new("foreign.parse").with_payload(json!({"children": [0, 1]})));
    let result = runtime.eval(&ast2).expect("parse must succeed");
    // Ast serializes as {"nodes": [...]}
    assert!(
        result.get("nodes").is_some(),
        "parse should return an Ast with 'nodes', got: {}",
        result
    );
}

/// `eval(lang, ast)` evaluates a pre-parsed AST and returns the result.
#[test]
fn eval_host_function_evals_ast() {
    let kit = make_kit();
    let runtime = kit.runtime();
    let lang_val = language_value(&canonical_lang());

    // Build a simple AST: 0: value 42
    let mut sub_ast = Ast::new();
    sub_ast.push(AstNode::new("value").with_payload(json!(42)));
    let sub_ast_val = serde_json::to_value(&sub_ast).unwrap();

    // Build the calling AST: 0: lang, 1: <ast>, 2: foreign.eval
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(lang_val));
    ast.push(AstNode::new("value").with_payload(sub_ast_val));
    ast.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
    let result = runtime.eval(&ast).expect("eval must succeed");
    assert_eq!(result, json!(42), "eval(lang, ast[42]) should return 42");
}

/// Full pipeline: `eval(lang, parse(lang, lex(lang, text)))` returns the
/// value. (`run` is dropped per ADR-0047 §4 — the composed pipeline is the
/// idiomatic path.)
#[test]
fn pipeline_eval_parse_lex_returns_value() {
    let kit = make_kit();
    let runtime = kit.runtime();
    let lang_val = language_value(&canonical_lang());

    // Build: lex(lang, "1 + 2")
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(lang_val.clone()));
    ast.push(AstNode::new("value").with_payload(json!("1 + 2")));
    ast.push(AstNode::new("foreign.lex").with_payload(json!({"children": [0, 1]})));
    let tokens = runtime.eval(&ast).expect("lex must succeed");

    // parse(lang, tokens)
    let mut ast2 = Ast::new();
    ast2.push(AstNode::new("value").with_payload(lang_val.clone()));
    ast2.push(AstNode::new("value").with_payload(tokens));
    ast2.push(AstNode::new("foreign.parse").with_payload(json!({"children": [0, 1]})));
    let parsed_ast = runtime.eval(&ast2).expect("parse must succeed");

    // eval(lang, parsed_ast)
    let mut ast3 = Ast::new();
    ast3.push(AstNode::new("value").with_payload(lang_val));
    ast3.push(AstNode::new("value").with_payload(parsed_ast));
    ast3.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
    let pipeline_result = runtime.eval(&ast3).expect("eval must succeed");

    assert_eq!(pipeline_result, json!(3));
}

/// `self` is injected as a global variable bound to the current
/// [`Language`] value (ADR-0041 Phase 3: `self` is an ambient variable,
/// not a fn call).
#[test]
fn self_global_returns_current_language() {
    let kit = make_kit();
    let runtime = kit.runtime();
    // `self` is a var node — the tree-walk looks it up in the root scope
    // where `Kit::new` injected `self = language_value(lang)`.
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("self")));
    let result = runtime.eval(&ast).expect("self must succeed");
    assert!(
        is_language_value(&result),
        "self should return a Language value, got: {}",
        result
    );
    assert_eq!(
        as_language(&result),
        Some(canonical_lang()),
        "self should carry the canonical Language"
    );
}

/// `eval(self, parse(self, lex(self, "1 + 2")))` works end-to-end: `self`
/// is a Language value passed as the first argument to each stage.
/// (`run(self, text)` is dropped per ADR-0047 §4 — the composed pipeline
/// using `self` is the idiomatic path.)
#[test]
fn pipeline_with_self_global() {
    let kit = make_kit();
    let runtime = kit.runtime();

    // lex(self, "1 + 2")
    let mut ast = Ast::new();
    ast.push(AstNode::new("var").with_payload(json!("self")));
    ast.push(AstNode::new("value").with_payload(json!("1 + 2")));
    ast.push(AstNode::new("foreign.lex").with_payload(json!({"children": [0, 1]})));
    let tokens = runtime.eval(&ast).expect("lex must succeed");

    // parse(self, tokens)
    let mut ast2 = Ast::new();
    ast2.push(AstNode::new("var").with_payload(json!("self")));
    ast2.push(AstNode::new("value").with_payload(tokens));
    ast2.push(AstNode::new("foreign.parse").with_payload(json!({"children": [0, 1]})));
    let parsed_ast = runtime.eval(&ast2).expect("parse must succeed");

    // eval(self, parsed_ast)
    let mut ast3 = Ast::new();
    ast3.push(AstNode::new("var").with_payload(json!("self")));
    ast3.push(AstNode::new("value").with_payload(parsed_ast));
    ast3.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
    let result = runtime.eval(&ast3).expect("eval must succeed");
    assert_eq!(result, json!(3));
}

/// Parse `source` under the canonical language into an [`Ast`] value
/// (JSON-serialized), for use as the `ast` argument to `compile`.
fn parse_ast_value(runtime: &Runtime, source: &str) -> serde_json::Value {
    let lang_val = language_value(&canonical_lang());
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(lang_val));
    ast.push(AstNode::new("value").with_payload(json!(source)));
    ast.push(AstNode::new("foreign.lex").with_payload(json!({"children": [0, 1]})));
    let tokens = runtime.eval(&ast).expect("lex must succeed");
    let lang_val = language_value(&canonical_lang());
    let mut ast2 = Ast::new();
    ast2.push(AstNode::new("value").with_payload(lang_val));
    ast2.push(AstNode::new("value").with_payload(tokens));
    ast2.push(AstNode::new("foreign.parse").with_payload(json!({"children": [0, 1]})));
    runtime.eval(&ast2).expect("parse must succeed")
}

/// `compile` with an unknown target returns a clear error.
#[test]
fn compile_unknown_target_returns_error() {
    let kit = make_kit();
    let runtime = kit.runtime();
    let lang_val = language_value(&canonical_lang());
    let ast_val = parse_ast_value(runtime, "1");
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(lang_val));
    ast.push(AstNode::new("value").with_payload(ast_val));
    ast.push(AstNode::new("value").with_payload(json!("wasm")));
    ast.push(AstNode::new("foreign.compile").with_payload(json!({"children": [0, 1, 2]})));
    let err = runtime.eval(&ast).unwrap_err();
    assert!(
        err.to_string().contains("unknown compile target"),
        "expected unknown-target error, got: {}",
        err
    );
}

/// `run_compiled` with a non-handle value returns a clear error.
#[test]
fn run_compiled_non_handle_returns_error() {
    let kit = make_kit();
    let runtime = kit.runtime();
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!("not-a-handle")));
    ast.push(AstNode::new("foreign.run_compiled").with_payload(json!({"children": [0]})));
    let err = runtime.eval(&ast).unwrap_err();
    assert!(
        err.to_string().contains("compiled-program handle"),
        "expected handle error, got: {}",
        err
    );
}

/// `eval` with a non-Language first argument returns an error.
#[test]
fn eval_non_language_first_arg_returns_error() {
    let kit = make_kit();
    let runtime = kit.runtime();
    let mut sub_ast = Ast::new();
    sub_ast.push(AstNode::new("value").with_payload(json!(1)));
    let sub_ast_val = serde_json::to_value(&sub_ast).unwrap();
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!("not-a-language")));
    ast.push(AstNode::new("value").with_payload(sub_ast_val));
    ast.push(AstNode::new("foreign.eval").with_payload(json!({"children": [0, 1]})));
    let err = runtime.eval(&ast).unwrap_err();
    assert!(err.to_string().contains("Language value"), "got: {}", err);
}

// --- Vau (fexpr) tests (ADR-0041 §6, ADR-0045) ---

/// The canonical `vau` node kind binds its child unevaluated and
/// returns the `__ast__` value. The child is never evaluated.
#[test]
fn vau_canonical_returns_unevaluated_ast() {
    let runtime = make_runtime_from_lang(canonical_lang());
    // 0: value 42, 1: vau {children:[0]}
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(42)));
    ast.push(AstNode::new("vau").with_payload(json!({"children": [0]})));
    let result = runtime.eval(&ast).expect("vau must succeed");
    // The result is a tagged __ast__ value wrapping the subtree.
    assert!(
        is_ast_value(&result),
        "vau should return an __ast__ value, got: {}",
        result
    );
    let inner = as_ast(&result).expect("must deserialize");
    // The subtree root is the `value` node.
    let root = &inner.nodes()[inner.len() - 1];
    assert_eq!(root.kind, "value");
    assert_eq!(root.payload, json!(42));
}

/// The ArrowApply escape: a `vau` node binds its child unevaluated,
/// then the body calls `eval(self, expr)` to evaluate it on demand.
/// This composes `Scheme::Vau` with the meta `eval` host function.
#[test]
fn vau_eval_escape_evaluates_child_on_demand() {
    // Build a language where vau's body is
    //   eval(self, expr)  i.e. App{var eval, [var self, var expr]}
    let mut lang = canonical_lang();
    lang.eval
        .merge(
            EvalSchemeDef::new(
                "eval.vau-escape",
                "vau",
                Scheme::vau(
                    vec!["expr".into()],
                    handl_language::Scheme::App {
                        callee: Box::new(handl_language::Scheme::var("eval")),
                        args: vec![
                            handl_language::Scheme::var("self"),
                            handl_language::Scheme::var("expr"),
                        ],
                    },
                ),
            )
            .with_replaces(vec!["eval.vau".to_string()]),
        )
        .unwrap();
    let runtime = make_runtime_from_lang(lang);
    // 0: value 1, 1: value 2, 2: binary.add {children:[0,1]}
    // 3: vau {children:[2]}
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!(1)));
    ast.push(AstNode::new("value").with_payload(json!(2)));
    ast.push(AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})));
    ast.push(AstNode::new("vau").with_payload(json!({"children": [2]})));
    let result = runtime.eval(&ast).expect("vau eval escape must succeed");
    assert_eq!(result, json!(3));
}
