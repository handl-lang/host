//! Canonical parity tests for the data-driven NBE typecheck engine.
//!
//! These tests assemble the **full canonical language** (all `types.yaml`
//! rules via `handl_kit_data_canonical::assemble`) and verify that the
//! data-driven [`NbeTypecheckEngine`] produces the correct synthesized types
//! for a representative suite of expressions.
//!
//! This is a *canonical parity* test — it checks the data-driven engine
//! against the canonical language's own type rules, not against the legacy
//! `plugin-typecheck-nbe` engine (which uses a different IR). The two engines
//! share the same `types.yaml` semantics, so correct results here imply
//! parity with the legacy engine's intended behavior.
//!
//! # Setup
//!
//! Each test uses three engines directly (bypassing `Runtime::parse`, which
//! applies a data-driven desugar pass — see the `tc` helper doc for details):
//! - lex: `DataLexerEngine` (`lex` free function)
//! - parse: `PrattEngine` (called directly, no desugar)
//! - typecheck: `NbeTypecheckEngine` ("nbe") wrapping a `TreeWalkEngine`
//!
//! No eval stage is bound — typechecking the canonical rules below uses only
//! judgment ops (`synth_child`, `construct`, `unify`, `extend`, `lookup`,
//! `node_payload`, `json_type_construct`, `payload_field`, `bind`, `result`,
//! `synth_last_child`), none of which require the eval engine.

use handl_kit_data_canonical;
use handl_language::{LangError, Language, Value};
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::lex as lex_src;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_engine_typecheck::{TypecheckEngine, TypecheckRequest};
use handl_plugin_engine_typecheck_nbe::NbeTypecheckEngine;
use serde_json::json;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build the full canonical [`Language`] (all `types.yaml` rules) via the
/// data-driven canonical kit.
fn lang() -> Language {
    let kit = handl_kit_data_canonical::assemble().expect("canonical assembly");
    kit.lang().clone()
}

/// Typecheck `src` through lex → parse → typecheck, returning the synthesized
/// type [`Value`].
///
/// Parsing uses the [`PrattEngine`] **directly** (not via `Runtime::parse`,
/// which applies a data-driven desugar pass that rewrites `if` → `match`
/// before typechecking). This tests the `types.yaml` rules as written —
/// against the un-desugared AST the Pratt parser produces.
///
/// # Panics
///
/// Panics if any stage fails — use [`tc_err`] for error-case tests.
fn tc(src: &str) -> Value {
    let lang = lang();

    // 1. Lex (data-driven pattern lexer).
    let tokens = lex_src(&lang, src).expect("lex");

    // 2. Parse (Pratt engine — no desugar pass).
    let pratt = PrattEngine::new();
    let ast = pratt
        .parse(&lang, &ParserRequest::new(tokens))
        .expect("parse");

    // 3. Typecheck (NBE engine wrapping a tree-walk engine for eval_child).
    let nbe = NbeTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    let result = nbe
        .typecheck(&lang, &TypecheckRequest::new(ast))
        .expect("typecheck");
    result.typ
}

/// Typecheck `src` through lex → parse → typecheck, returning the
/// [`LangError`] if any stage fails.
///
/// # Panics
///
/// Panics if all stages succeed (i.e. no error was produced).
fn tc_err(src: &str) -> LangError {
    let lang = lang();

    let tokens = match lex_src(&lang, src) {
        Ok(t) => t,
        Err(e) => return e,
    };

    let pratt = PrattEngine::new();
    let ast = match pratt.parse(&lang, &ParserRequest::new(tokens)) {
        Ok(a) => a,
        Err(e) => return e,
    };

    let nbe = NbeTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    nbe.typecheck(&lang, &TypecheckRequest::new(ast))
        .expect_err("expected a typecheck/parse/lex error")
}

// ---------------------------------------------------------------------------
// Expected type-value constructors (tagged values in `handl.canonical`)
// ---------------------------------------------------------------------------

fn t_number() -> Value {
    json!({"__tag__":"type.number","__registry__":"handl.canonical"})
}

fn t_string() -> Value {
    json!({"__tag__":"type.string","__registry__":"handl.canonical"})
}

fn t_bool() -> Value {
    json!({"__tag__":"type.bool","__registry__":"handl.canonical"})
}

fn t_null() -> Value {
    json!({"__tag__":"type.null","__registry__":"handl.canonical"})
}

// ===========================================================================
// Part B-1: Basic values
// ===========================================================================

#[test]
fn parity_number_literal() {
    assert_eq!(tc("42"), t_number());
}

#[test]
fn parity_string_literal() {
    assert_eq!(tc("\"hello\""), t_string());
}

#[test]
fn parity_bool_true() {
    assert_eq!(tc("true"), t_bool());
}

#[test]
fn parity_bool_false() {
    assert_eq!(tc("false"), t_bool());
}

#[test]
fn parity_null_literal() {
    assert_eq!(tc("null"), t_null());
}

// ===========================================================================
// Part B-2: Arithmetic (all operands type.number → type.number)
// ===========================================================================

#[test]
fn parity_add() {
    assert_eq!(tc("1 + 2"), t_number());
}

#[test]
fn parity_sub() {
    assert_eq!(tc("1 - 2"), t_number());
}

#[test]
fn parity_mul() {
    assert_eq!(tc("1 * 2"), t_number());
}

#[test]
fn parity_div() {
    assert_eq!(tc("1 / 2"), t_number());
}

#[test]
fn parity_mod() {
    assert_eq!(tc("1 % 2"), t_number());
}

#[test]
fn parity_pow() {
    assert_eq!(tc("2 ** 3"), t_number());
}

// ===========================================================================
// Part B-3: Comparisons (result type.bool)
// ===========================================================================

#[test]
fn parity_eq() {
    assert_eq!(tc("1 == 2"), t_bool());
}

#[test]
fn parity_neq() {
    assert_eq!(tc("1 != 2"), t_bool());
}

#[test]
fn parity_lt() {
    assert_eq!(tc("1 < 2"), t_bool());
}

#[test]
fn parity_gt() {
    assert_eq!(tc("1 > 2"), t_bool());
}

#[test]
fn parity_lte() {
    assert_eq!(tc("1 <= 2"), t_bool());
}

#[test]
fn parity_gte() {
    assert_eq!(tc("1 >= 2"), t_bool());
}

// ===========================================================================
// Part B-4: Logic (both operands type.bool → type.bool)
// ===========================================================================

#[test]
fn parity_and() {
    assert_eq!(tc("true && false"), t_bool());
}

#[test]
fn parity_or() {
    assert_eq!(tc("true || false"), t_bool());
}

// ===========================================================================
// Part B-5: Paren
// ===========================================================================

#[test]
fn parity_paren_number() {
    assert_eq!(tc("(42)"), t_number());
}

#[test]
fn parity_paren_string() {
    assert_eq!(tc("(\"hello\")"), t_string());
}

#[test]
fn parity_paren_arithmetic() {
    assert_eq!(tc("(1 + 2)"), t_number());
}

// ===========================================================================
// Part B-6: If expressions
// ===========================================================================

#[test]
fn parity_if_number() {
    // v0.1: `if COND THEN else ELSE` (the `then` keyword was removed).
    assert_eq!(tc("if true 42 else 0"), t_number());
}

#[test]
fn parity_if_string() {
    assert_eq!(tc("if true \"hello\" else \"world\""), t_string());
}

#[test]
fn parity_if_bool() {
    assert_eq!(tc("if true false else true"), t_bool());
}

// ===========================================================================
// Part B-7: Let expressions
// ===========================================================================

#[test]
fn parity_let_number() {
    assert_eq!(tc("let x = 42 in x"), t_number());
}

#[test]
fn parity_let_string() {
    assert_eq!(tc("let x = 42 in \"hello\""), t_string());
}

#[test]
fn parity_let_nested() {
    assert_eq!(tc("let x = 42 in let y = \"hello\" in y"), t_string());
}

#[test]
fn parity_let_shadow() {
    assert_eq!(tc("let x = 1 in let x = \"hello\" in x"), t_string());
}

// ===========================================================================
// Part B-7b: Compound type annotations (interpret_type_expr)
// ===========================================================================

fn t_function(param: Value, body: Value) -> Value {
    json!({
        "__tag__": "type.function",
        "__registry__": "handl.canonical",
        "param": param,
        "body": body
    })
}

fn t_array(element: Value) -> Value {
    json!({
        "__tag__": "type.array",
        "__registry__": "handl.canonical",
        "element": element
    })
}

#[test]
fn parity_let_annotated_function_type() {
    // `let f: ((type.number) ~> type.number) = x ~> x in f`
    // The annotation `((type.number) ~> type.number)` is a compound type
    // expression parsed as a `function` node; interpret_type_expr must
    // recurse into param (paren → field-access) and body (field-access).
    // NOTE: per ADR-0074 ID.1, `:` (bp 12/13) binds tighter than `~>`
    // (bp 10/9), so the function-type annotation must be parenthesized to
    // prevent `~>` from escaping the type-annot's right operand.
    assert_eq!(
        tc("let f: ((type.number) ~> type.number) = x ~> x in f"),
        t_function(t_number(), t_number())
    );
}

#[test]
fn parity_let_annotated_array_type() {
    // `let xs: [type.number] = [1, 2, 3] in xs`
    // The annotation `[type.number]` is a compound type expression parsed
    // as an `array` node; interpret_type_expr must recurse into the element.
    assert_eq!(
        tc("let xs: [type.number] = [1, 2, 3] in xs"),
        t_array(t_number())
    );
}

#[test]
fn parity_let_annotated_nested_function_array() {
    // `((type.number) ~> [type.string])` — function returning an array.
    // Parenthesized because `:` (bp 12/13) binds tighter than `~>` (bp 10/9).
    assert_eq!(
        tc("let f: ((type.number) ~> [type.string]) = x ~> [\"a\"] in f"),
        t_function(t_number(), t_array(t_string()))
    );
}

// ===========================================================================
// Part B-8: Typeof
// ===========================================================================
//
// The `typeof` keyword was removed in v0.1 (ADR-0074). Its functionality now
// lives in the `typeof` library module, which is not registered in this
// parity test's bare lex → parse → typecheck pipeline. The keyword-form
// parity tests (`parity_typeof_number`, `parity_typeof_string`,
// `parity_typeof_bool`) have been removed because the construct they tested
// no longer exists in the surface syntax.

// ===========================================================================
// Part B-9: Template literals (always type.string)
// ===========================================================================

#[test]
fn parity_template_simple() {
    assert_eq!(tc("`hello world`"), t_string());
}

#[test]
fn parity_template_empty() {
    assert_eq!(tc("``"), t_string());
}

// ===========================================================================
// Part B-10: Unary
// ===========================================================================

#[test]
fn parity_unary_neg() {
    assert_eq!(tc("-42"), t_number());
}

#[test]
fn parity_unary_not() {
    assert_eq!(tc("!true"), t_bool());
}

// ===========================================================================
// Part B-11: Ternary
// ===========================================================================

#[test]
fn parity_ternary_number() {
    assert_eq!(tc("true ? 42 : 0"), t_number());
}

#[test]
fn parity_ternary_string() {
    assert_eq!(tc("false ? \"a\" : \"b\""), t_string());
}

#[test]
fn parity_ternary_bool() {
    assert_eq!(tc("true ? false : true"), t_bool());
}

// ===========================================================================
// Part B-12: Block expressions (do { ... })
// ===========================================================================

#[test]
fn parity_block_single() {
    assert_eq!(tc("do { 42 }"), t_number());
}

#[test]
fn parity_block_multi() {
    assert_eq!(tc("do { 1; 2; \"hello\" }"), t_string());
}

#[test]
fn parity_block_empty() {
    // Empty block returns type.unit.
    let result = tc("do { }");
    assert!(result["__tag__"].as_str() == Some("type.unit") || result["__tag__"].as_str() == Some("type.null"),
        "expected type.unit or type.null, got: {:?}", result);
}

// ===========================================================================
// Part D: Error cases
// ===========================================================================

#[test]
fn parity_unbound_var_errors() {
    // `x` is an unbound variable: the `var` rule does `lookup` which fails
    // because Γ is empty.
    let err = tc_err("x");
    eprintln!("unbound var error: {:?}", err);
    assert!(matches!(err, LangError::Engine(_)),
        "expected Engine error for unbound var, got {:?}", err);
}

#[test]
fn parity_type_mismatch_add_errors() {
    // 1 + "hello" — binary.add unifies child 1 against type.number; the
    // string literal synthesizes type.string, so unification fails.
    let err = tc_err("1 + \"hello\"");
    eprintln!("type mismatch error: {:?}", err);
    assert!(matches!(err, LangError::Engine(ref msg) if msg.contains("cannot unify")),
        "expected unification failure for 1 + \"hello\", got {:?}", err);
}

#[test]
fn parity_type_mismatch_and_errors() {
    // true && 1 — binary.and unifies child 1 against type.bool; the number
    // literal synthesizes type.number, so unification fails.
    let err = tc_err("true && 1");
    eprintln!("type mismatch (&&) error: {:?}", err);
    assert!(matches!(err, LangError::Engine(ref msg) if msg.contains("cannot unify")),
        "expected unification failure for true && 1, got {:?}", err);
}

#[test]
fn parity_if_branch_mismatch_errors() {
    // v0.1: `if true 42 else "hello"` — the then-branch (type.number) and
    // else-branch (type.string) must unify; they don't.
    let err = tc_err("if true 42 else \"hello\"");
    eprintln!("if branch mismatch error: {:?}", err);
    assert!(matches!(err, LangError::Engine(ref msg) if msg.contains("cannot unify")),
        "expected unification failure for if branch mismatch, got {:?}", err);
}
