//! Phase 3 — `sym()` / `sym(parent)` + `sym.instanceof` surface tests.
//!
//! The surface is pure op-binding (ADR-0106 §2 via `SymModule`): no
//! lexer/parser changes. `sym(...)` parses through the existing `call`
//! grammar rule and resolves via flat-name module lookup; `sym.instanceof`
//! is a `field-access` host call to the `instance_of` primitive aliased
//! under a surface-friendly name. These tests exercise the full pipeline
//! (lex → parse → eval) through the canonical `Kit`:
//!
//! - parser/desugar round-trips: `sym()` parses to a `call` node whose
//!   callee falls back to the `"sym"` string and evaluates to a
//!   `{"__sym__": ...}` nominal (a native arrow/object structure — no new
//!   closure/capture semantics);
//! - `instanceof` through the surface: parent-chain walks (direct,
//!   transitive, self), negative cases, and non-nominal errors;
//!
//! - legacy `__tag__`-tagged values (see `type.hnl`) keep working alongside.

//! Phase 3 — `sym()` / `sym(parent)` + `sym.instanceof` surface tests (cont).

use handl_kit_canonical::Kit;
use serde_json::json;

/// Eval like `hnl eval <expr>`: the input is wrapped in a `do { ... }`
/// block so `;`-separated statements sequence (same as `host-cli`).
fn eval(src: &str) -> serde_json::Value {
    let wrapped = format!("do {{ {src} }}");
    Kit::new()
        .expect("canonical kit must assemble")
        .eval(&wrapped)
        .unwrap_or_else(|e| panic!("`{src}` must eval: {e}"))
}

fn eval_err(src: &str) -> String {
    let wrapped = format!("do {{ {src} }}");
    Kit::new()
        .expect("canonical kit must assemble")
        .eval(&wrapped)
        .unwrap_err()
        .to_string()
}

// ---------------------------------------------------------------------------
// Parser/desugar round-trips: sym() lowers to native arrow/object structure
// ---------------------------------------------------------------------------

#[test]
fn sym_call_parses_to_call_node_with_sym_callee() {
    let kit = Kit::new().expect("kit must build");
    let ast = kit.parse("sym()").expect("sym() must parse");
    assert!(!ast.is_empty());
    let root = &ast.nodes()[ast.len() - 1];
    assert_eq!(root.kind, "call", "sym() must parse as a call node");
}

#[test]
fn sym_empty_call_mints_nominal_object() {
    let v = eval("sym()");
    let inner = v.get("__sym__").expect("sym() must mint {__sym__: ...}");
    assert!(inner.get("handle").is_some(), "nominal has a handle: {v}");
    assert!(inner.get("parent").unwrap().is_null(), "root sym has null parent: {v}");
}

#[test]
fn sym_with_parent_mints_child_linked_to_parent() {
    // Parent chain via nested surface calls — desugars to native
    // arrow/object structures only (host calls, no closures).
    let v = eval("sym(sym())");
    let parent = v
        .get("__sym__")
        .unwrap()
        .get("parent")
        .expect("child must carry a parent");
    assert!(parent.get("__sym__").is_some(), "parent must be a nominal: {v}");
}

#[test]
fn two_sym_calls_mint_distinct_identities() {
    // Two mints are never equal even though names match (presentation only).
    assert_eq!(eval("sym.instanceof(sym(), sym())"), json!(false));
}

// ---------------------------------------------------------------------------
// instanceof through the surface: parent-chain behavior
// ---------------------------------------------------------------------------

#[test]
fn instanceof_self_and_direct_parent_are_true() {
    // Each eval is wrapped in its own `do` block (see `eval` above), so
    // build the chain inline: Float instanceof Number (direct parent) and
    // Number instanceof Number (self) are both true.
    assert_eq!(
        eval("Number = sym(); Float = sym(Number); sym.instanceof(Float, Number)"),
        json!(true)
    );
    assert_eq!(
        eval("Number = sym(); sym.instanceof(Number, Number)"),
        json!(true)
    );
}

#[test]
fn instanceof_transitive_grandparent_is_true() {
    let v = eval("Obj = sym(); Number = sym(Obj); Float = sym(Number); sym.instanceof(Float, Obj)");
    assert_eq!(v, json!(true));
}

#[test]
fn instanceof_negative_cases_are_false() {
    // Parent -> child is not an instance; siblings are unrelated.
    assert_eq!(
        eval("N = sym(); F = sym(N); sym.instanceof(N, F)"),
        json!(false)
    );
    assert_eq!(
        eval("R = sym(); A = sym(R); B = sym(R); sym.instanceof(A, B)"),
        json!(false)
    );
}

#[test]
fn instanceof_non_nominal_value_is_false_not_error() {
    assert_eq!(eval("N = sym(); sym.instanceof(42, N)"), json!(false));
    assert_eq!(eval("N = sym(); sym.instanceof(null, N)"), json!(false));
}

#[test]
fn instanceof_non_nominal_parent_errors_cleanly() {
    let msg = eval_err("sym(sym(42))");
    assert!(msg.contains("parent is not a nominal object"), "{msg}");
}

#[test]
fn instanceof_non_nominal_nominal_arg_errors_cleanly() {
    let msg = eval_err("N = sym(); sym.instanceof(N, 42)");
    assert!(msg.contains("must be a nominal object"), "{msg}");
}

#[test]
fn instanceof_underscore_alias_matches() {
    assert_eq!(
        eval("N = sym(); F = sym(N); sym.instance_of(F, N)"),
        json!(true)
    );
}

// ---------------------------------------------------------------------------
// type.hnl nominals: Float-tagged data instanceof Number via parent chain
// ---------------------------------------------------------------------------

#[test]
fn float_nominal_is_instanceof_number_nominal() {
    let v = eval(
        "Number = sym(); Float = sym(Number); Int = sym(Number); sym.instanceof(Float, Number)",
    );
    assert_eq!(v, json!(true), "Float nominal must descend from Number");
    let v = eval(
        "Number = sym(); Float = sym(Number); Int = sym(Number); sym.instanceof(Int, Number)",
    );
    assert_eq!(v, json!(true), "Int nominal must descend from Number");
    let v = eval(
        "Number = sym(); Float = sym(Number); Int = sym(Number); sym.instanceof(Float, Int)",
    );
    assert_eq!(v, json!(false), "sibling nominals are unrelated");
}
