//! Parity tests: compare `CanonicalKit` (plugin-based) against the canonical
//! recursive-descent parser `handl_parser_canonical::CanonicalParser`.

use handl_ast_canonical::Expr;
use handl_kit_canonical::CanonicalKit;
use handl_lexer::Lexer;
use handl_lexer_canonical::CanonicalLexer;
use handl_parser::Parser;
use handl_parser_canonical::CanonicalParser;
use handl_vm_values_json::NonFunctionValue;

static PARITY_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn canonical(input: &str) -> Result<Expr<NonFunctionValue>, String> {
    let tokens = CanonicalLexer::default()
        .lex(input)
        .map_err(|e| e.to_string())?;
    CanonicalParser.parse(tokens).map_err(|e| e.to_string())
}

fn kit(input: &str) -> Result<Expr<NonFunctionValue>, String> {
    CanonicalKit::new().parse(input).map_err(|e| e.to_string())
}

fn assert_expr_parity(input: &str) {
    let _guard = PARITY_MUTEX
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let c = canonical(input).expect("canonical parser should succeed");
    let k = kit(input).expect("kit parser should succeed");
    assert_eq!(
        c, k,
        "divergence for input {:?}\ncanonical: {:?}\nkit: {:?}",
        input, c, k
    );
}

// --- literals ---

#[test]
fn parity_string_literal() {
    assert_expr_parity(r#""hello""#);
}

#[test]
fn parity_number_literal() {
    assert_expr_parity("42");
}

#[test]
fn parity_bool_literal() {
    assert_expr_parity("true");
    assert_expr_parity("false");
}

#[test]
fn parity_null_literal() {
    assert_expr_parity("null");
}

// --- calls ---

#[test]
fn parity_simple_call() {
    assert_expr_parity("add(1, 2)");
}

#[test]
fn parity_nested_call() {
    assert_expr_parity("add(mul(2, 3), 4)");
}

#[test]
fn parity_namespaced_call() {
    assert_expr_parity("math.sqrt(16)");
}

#[test]
fn parity_call_no_args() {
    assert_expr_parity("now()");
}

// --- array / object literals ---

#[test]
fn parity_empty_array() {
    assert_expr_parity("[]");
}

#[test]
fn parity_array_of_literals() {
    assert_expr_parity("[1, 2, 3]");
}

#[test]
fn parity_array_of_mixed() {
    assert_expr_parity(r#"["hello", 42, true, null]"#);
}

#[test]
fn parity_array_with_calls() {
    assert_expr_parity("[add(1, 2), mul(3, 4)]");
}

#[test]
fn parity_nested_array() {
    assert_expr_parity("[[1, 2], [3, 4]]");
}

#[test]
fn parity_empty_object() {
    assert_expr_parity("{}");
}

#[test]
fn parity_object_identifier_keys() {
    assert_expr_parity("{a: 1, b: 2}");
}

#[test]
fn parity_object_string_keys() {
    assert_expr_parity(r#"{"key": "value"}"#);
}

#[test]
fn parity_object_with_call_values() {
    assert_expr_parity("{result: add(1, 2)}");
}

#[test]
fn parity_nested_object() {
    assert_expr_parity("{outer: {inner: 42}}");
}

#[test]
fn parity_object_in_array() {
    assert_expr_parity("[{a: 1}, {b: 2}]");
}

#[test]
fn parity_object_punning() {
    assert_expr_parity("{a, b}");
}

// --- postfix shapes ---

#[test]
fn parity_index() {
    assert_expr_parity("arr[0]");
}

#[test]
fn parity_field_access() {
    assert_expr_parity("obj.key");
}

#[test]
fn parity_nested_field_access() {
    assert_expr_parity("a.b.c");
}

// --- primary operators ---

#[test]
fn parity_not() {
    assert_expr_parity("not true");
}

#[test]
fn parity_typeof() {
    assert_expr_parity("typeof 42");
}

// --- infix sugar ---

#[test]
fn parity_pipe_forward() {
    assert_expr_parity("5 |> add");
}

#[test]
fn parity_pipe_backward() {
    assert_expr_parity("add <| 5");
}

#[test]
fn parity_pipe_forward_partial() {
    assert_expr_parity("5 |> add(_, 10)");
}

#[test]
fn parity_pipe_backward_partial() {
    assert_expr_parity("add(_, 10) <| 5");
}

#[test]
fn parity_ternary() {
    assert_expr_parity("x ? a : b");
}

#[test]
fn parity_flow_combinator() {
    assert_expr_parity("f >>> g");
}

#[test]
fn parity_compose_combinator() {
    assert_expr_parity("f <<< g");
}

// --- top-level forms ---

#[test]
fn parity_assignment() {
    assert_expr_parity("a = 1");
}

#[test]
fn parity_function_literal() {
    assert_expr_parity("add(a) -> a");
}

// --- known divergences (by design or not yet aligned) ---

#[test]
fn parity_match_expression() {
    assert_expr_parity(r#"match 1 { 1 -> "one", _ -> "other" }"#);
}

#[test]
fn parity_handle_expression() {
    assert_expr_parity(r#"handle("fetch"(url) as k: k(42))"#);
}

#[test]
fn parity_fanout_combinator() {
    assert_expr_parity("f &&& g");
}

#[test]
fn parity_partial_application() {
    assert_expr_parity("add(_, 1)");
}

// Optional smoke tests for module statements (kit-only; canonical parser is expression-only).

#[test]
fn parity_module_let() {
    use handl_ast_canonical::Pattern;
    use handl_expr::{Stmt, StmtKind};
    let out = CanonicalKit::new().parse_module_recovered("let a = 1;");
    assert!(out.diagnostics.is_empty());
    assert_eq!(
        out.stmts,
        vec![Stmt::new(StmtKind::Let {
            pattern: Pattern::Identifier("a".to_string()),
            value: Expr::Value(NonFunctionValue::new(1.0)),
        })]
    );
}

#[test]
fn parity_summary() {
    // A lightweight sanity test that runs a few expressions through both parsers.
    for input in ["42", "add(1, 2)", "[1, 2]", "{a: 1}"] {
        assert_expr_parity(input);
    }
}
