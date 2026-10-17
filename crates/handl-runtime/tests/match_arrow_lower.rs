//! Arrow-lowered `match` — the pattern shapes `lower_match` compiles to
//! `pair_tag`/`Choice` dispatch must produce the same result as the
//! tree-walk engine's `match_shell`/`try_pattern` walk.
//!
//! For every source the test lowers the desugared AST via
//! `handl_kit_arrow_lower::lower_ast`, interprets the `Term`, and
//! compares against `eval_with_globals` (which bypasses the lowerer) —
//! a decline or a mismatch fails the test. `__tag__` scrutinees cannot
//! be written as object literals (`__tag__` is a reserved key), so the
//! tag-pattern cases hand-build the arena like the tree-walk tests do.

use handl_kit_arrow_lower::lower_ast;
use handl_language::{Ast, AstNode, AstRef, Source, Value};
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::Runtime;
use plugin_engine::term_interpreter::interpret;
use serde_json::json;
use std::collections::HashMap;

fn canonical_lang() -> handl_language::Language {
    handl_kit_data_canonical::assemble()
        .expect("canonical kit assembles")
        .lang()
        .clone()
}

fn runtime() -> Runtime {
    Runtime::new(canonical_lang())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(Box::new(TreeWalkEngine::new()))
}

/// `src` must lower *and* interpret to the same value the bound eval
/// engine produces — no `lower_ast` decline, no interpreter fallback.
fn assert_lowered(src: &str) {
    let rt = runtime();
    let toks = rt.lex(&Source::new(src)).expect("lex must succeed");
    let ast = rt.parse(&toks).expect("parse+desugar must succeed");
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, rt.lang(), root)
        .unwrap_or_else(|e| panic!("lower_ast declined {src:?}: {e}"));
    let got = interpret(&term, Value::Null)
        .unwrap_or_else(|e| panic!("term interpreter failed on {src:?}: {e}"));
    let want = rt
        .eval_with_globals(&ast, &HashMap::new())
        .unwrap_or_else(|e| panic!("engine eval failed on {src:?}: {e}"));
    assert_eq!(got, want, "{src:?}");
}

/// `src` must *decline* to lower — the AST eval fallback covers the
/// semantics the arrow lowering cannot express.
fn assert_declines(src: &str) {
    let rt = runtime();
    let toks = rt.lex(&Source::new(src)).expect("lex must succeed");
    let ast = rt.parse(&toks).expect("parse+desugar must succeed");
    let root = AstRef::new(ast.len() - 1);
    assert!(
        lower_ast(&ast, rt.lang(), root).is_err(),
        "{src:?} should decline"
    );
}

/// Like [`assert_lowered`] but for a hand-built arena: the lowered term
/// must interpret to `want`.
fn assert_lowered_ast(ast: &Ast, want: Value) {
    let rt = runtime();
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(ast, rt.lang(), root)
        .unwrap_or_else(|e| panic!("lower_ast declined hand-built ast: {e}"));
    let got =
        interpret(&term, Value::Null).unwrap_or_else(|e| panic!("term interpreter failed: {e}"));
    assert_eq!(got, want);
}

// ---------------------------------------------------------------------------
// Structural patterns — the pre-existing set, kept honest.
// ---------------------------------------------------------------------------

#[test]
fn literal_and_binder_patterns() {
    assert_lowered(r#"match 2 { 1: "one", 2: "two", _: "other" }"#);
    assert_lowered(r#"match "x" { "x": 1, _: 0 }"#);
    assert_lowered(r#"match 1.0 { 1: "hit", _: "miss" }"#); // cross-numeric eq
    assert_lowered(r#"match null { null: "n", _: "z" }"#);
    assert_lowered(r#"match 7 { x: x }"#); // bare binder
}

#[test]
fn global_value_patterns() {
    // `true`/`false` are `Language::globals` names — they compare by
    // equality in pattern position *and* resolve to their value in
    // scrutinee position.
    assert_lowered(r#"match true { true: "yes", false: "no", _: "z" }"#);
    assert_lowered(r#"match false { true: "yes", false: "no", _: "z" }"#);
    assert_lowered(r#"match 1 { true: "yes", false: "no", _: "z" }"#);
}

#[test]
fn array_patterns() {
    assert_lowered(r#"match [1, 2] { []: 0, [h, ...t]: h, _: 9 }"#);
    assert_lowered(r#"match [] { []: 0, [h, ...t]: h, _: 9 }"#);
    assert_lowered(r#"match "abc" { []: 0, _: 9 }"#); // non-array falls through
    assert_lowered(r#"match [1, 2, 3] { [a, b, c]: a + b + c, _: 0 }"#);
    assert_lowered(r#"match [1, 2, 3] { [...t]: t, _: 0 }"#); // rest-only
}

#[test]
fn array_patterns_with_sub_patterns() {
    // Literal elements, nested arrays, wildcards — the decline set that
    // drove `["-", ...t]` in `desugar.hnl` to the AST fallback.
    assert_lowered(r#"match ["-", "a"] { ["-", ...t]: t, _: [] }"#);
    assert_lowered(r#"match [] { ["-", ...t]: 1, _: 0 }"#);
    assert_lowered(r#"match [1, 2] { [1, 2]: "hit", _: "miss" }"#);
    assert_lowered(r#"match [1, 3] { [1, 2]: "hit", _: "miss" }"#);
    assert_lowered(r#"match [[1, 2], [3]] { [[a, b], ...t]: a + b, _: 0 }"#);
    assert_lowered(r#"match [[1]] { [[a, b], ...t]: a + b, _: 0 }"#);
    assert_lowered(r#"match [[1, 2]] { [[a, 2]]: a, _: 0 }"#);
    assert_lowered(r#"match [["x"]] { [["x"]]: 1, _: 0 }"#);
    // A too-short scrutinee falls through rather than erroring.
    assert_lowered(r#"match [1] { [1, 2]: "hit", _: "miss" }"#);
    assert_lowered(r#"match [1, 2, 3] { [1, _, 3]: "hit", _: "miss" }"#);
}

#[test]
fn object_patterns() {
    assert_lowered(r#"match { "a": 1 } { {}: "obj", _: "no" }"#);
    assert_lowered(r#"match 5 { {}: "obj", _: "no" }"#);
    assert_lowered(r#"match [] { {}: "obj", _: "no" }"#);
    assert_lowered(r#"match { "a": 5, "b": 2 } { { "a": x }: x, _: 0 }"#);
    assert_lowered(r#"match { "a": 5, "b": 2 } { { "a": 5 }: "hit", _: "miss" }"#);
    assert_lowered(r#"match { "a": 5, "b": 2 } { { "a": 6 }: "hit", _: "miss" }"#);
    // Missing key / non-object falls through — `try_object_pattern`'s
    // `Ok(false)`, not an error.
    assert_lowered(r#"match { "a": 5, "b": 2 } { { "c": x }: x, _: 0 }"#);
    assert_lowered(r#"match 5 { { "a": x }: x, _: 0 }"#);
    assert_lowered(r#"match { "a": 5, "b": 2 } { { "a": x, ...r }: r.b, _: 0 }"#);
    assert_lowered(r#"match { "a": 5, "b": 2 } { { "a": x, ...r }: r, _: 0 }"#);
    // Nested sub-patterns under entries.
    assert_lowered(r#"match { "k": { "v": 7 } } { { "k": { "v": x } }: x, _: 0 }"#);
    assert_lowered(r#"match { "a": [1, 2] } { { "a": [x, ...t] }: x, _: 0 }"#);
    assert_lowered(r#"match { "a": [1] } { { "a": [x, ...t] }: x, _: 0 }"#);
}

#[test]
fn tag_record_patterns() {
    // `{_tag: "Some", value: x}` — the `nullish`/`opt-chain` desugar
    // shape, plus quoted keys routed through the general entries path.
    assert_lowered(
        r#"match { "_tag": "Some", "value": 5 } { { "_tag": "Some", "value": x }: x, _: 0 }"#,
    );
    assert_lowered(r#"match { "_tag": "None" } { { "_tag": "Some", "value": x }: x, _: 0 }"#);
}

#[test]
fn desugared_match_patterns() {
    // `??`/`?.` desugar to `match` on `{_tag: ...}` records with a
    // `__fresh_` payload binder — the wildcard filter must bind those
    // names, not drop them.
    assert_lowered(r#"null ?? 7"#);
    assert_lowered(r#"5 ?? 7"#);
    assert_lowered(r#"{ "_tag": "Some", "value": 7 } ?? 9"#);
}

#[test]
fn arm_bodies_see_rebased_env() {
    // Regression: a binding lowered to consume the node's input `in`
    // must be re-anchored through `snd` of the `[sv, in]` dispatch pair —
    // previously `y` evaluated to the pair itself.
    assert_lowered(r#"((xs) ~> do { y :: xs, match xs { []: y, _: 99 } })([])"#);
    assert_lowered(r#"((xs) ~> do { y :: xs, match xs { []: y, _: 99 } })([1])"#);
    assert_lowered(r#"((xs) ~> match xs { []: 1, _: 2 })([])"#);
    assert_lowered(r#"((xs) ~> match xs { []: 1, _: 2 })([3])"#);
    assert_lowered(r#"do { f :: (xs) ~> match xs { []: 1, _: 2 }, f([]) }"#);
}

#[test]
fn paren_literal_eval_patterns() {
    // `(expr)` in pattern position is `try_pattern`'s literal-eval tail:
    // evaluate the expression in the ambient env and `eq`-compare. The
    // `[sv, in]` pair feeds the lowered expression `in`, so env bindings
    // *and* enclosing params resolve — not just literals.
    assert_lowered(r#"match 5 { (5): "hit", _: "miss" }"#);
    assert_lowered(r#"match 4 { (5): "hit", _: "miss" }"#);
    assert_lowered(r#"match 3 { (1 + 2): "hit", _: "miss" }"#);
    assert_lowered(r#"match 2 { (1 + 2): "hit", _: "miss" }"#);
    // Env-bound constants and param values both resolve.
    assert_lowered(r#"do { n :: 5, match 5 { (n): "hit", _: "miss" } }"#);
    assert_lowered(r#"do { n :: 5, match 4 { (n): "hit", _: "miss" } }"#);
    assert_lowered(r#"((n) ~> match 5 { (n): "hit", _: "miss" })(5)"#);
    assert_lowered(r#"((n) ~> match 5 { (n): "hit", _: "miss" })(4)"#);
    // Compound expressions — field access, literals.
    assert_lowered(r#"do { o :: {"a": 7}, match 7 { (o.a): "hit", _: "miss" } }"#);
    assert_lowered(r#"match [1, 2] { ([1, 2]): "hit", _: "miss" }"#);
    assert_lowered(r#"match [1, 3] { ([1, 2]): "hit", _: "miss" }"#);
    // Literal-eval *nested* inside structural patterns — `in` reaches
    // any depth through the re-paired `[v', in]` sub-scrutinee.
    assert_lowered(r#"do { n :: 2, match [1, 2] { [1, (n)]: "hit", _: "miss" } }"#);
    assert_lowered(r#"do { n :: 2, match [1, 3] { [1, (n)]: "hit", _: "miss" } }"#);
    assert_lowered(r#"do { n :: 2, match { "a": 2 } { { "a": (n) }: "hit", _: "miss" } }"#);
    assert_lowered(r#"((n) ~> match { "a": 9 } { { "a": (n) }: "hit", _: "miss" })(9)"#);
    assert_lowered(r#"((n) ~> match { "a": 9 } { { "a": (n) }: "hit", _: "miss" })(8)"#);
}

#[test]
fn computed_object_pattern_keys() {
    // `[expr]`/`(expr)` entry keys desugar to `paren` key children — a
    // computed key the interpreter evaluates in the ambient env. When
    // that expression is a compile-time constant (an env-bound constant
    // like `k :: "a"`), the key string resolves statically.
    assert_lowered(r#"do { k :: "a", match { "a": 1 } { {[k]: x}: x, _: 0 } }"#);
    assert_lowered(r#"do { k :: "a", match { "b": 1, "a": 2 } { {[k]: x}: x, _: 0 } }"#);
    // `...rest` excludes the statically-resolved matched key.
    assert_lowered(r#"do { k :: "a", match { "b": 1, "a": 2 } { {[k]: x, ...r}: r.b, _: 0 } }"#);
    assert_lowered(r#"do { k :: "a", match { "b": 1 } { {[k]: x}: x, _: 0 } }"#);
    // `paren`-wrapped literal keys keep working through the same path.
    assert_lowered(r#"match { "a": 1 } { {("a"): x}: x, _: 0 }"#);
    // A computed key whose value is genuinely runtime (a param, a
    // non-const binding) cannot become a static `has_field` literal —
    // the pattern declines to the AST fallback.
    assert_declines(r#"((k) ~> match { "a": 1 } { {[k]: x}: x, _: 0 })("a")"#);
}

#[test]
fn literal_eval_tail_patterns() {
    // `try_pattern`'s tail evaluates any non-structural pattern node as
    // an expression — a bare field-access, an object literal, a nested
    // `paren`.
    assert_lowered(r#"do { o :: {"a": 7}, match 7 { o.a: "hit", _: "miss" } }"#);
    assert_lowered(r#"do { o :: {"a": 7}, match 8 { o.a: "hit", _: "miss" } }"#);
    assert_lowered(r#"do { p :: [9], match 9 { (p.0): "hit", _: "miss" } }"#);
}

#[test]
fn match_fail_on_total_miss() {
    // No arm matches → the interpreter raises "match: no arm matched".
    // The lowered `match_fail` op errors the same way; `Runtime::eval`
    // then falls back to the engine, which reports it identically.
    let rt = runtime();
    let src = r#"match 3 { 1: "one", 2: "two" }"#;
    let toks = rt.lex(&Source::new(src)).unwrap();
    let ast = rt.parse(&toks).unwrap();
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, rt.lang(), root).expect("must lower");
    let err = interpret(&term, Value::Null).unwrap_err();
    assert!(err.contains("no arm matched"), "{err}");
    assert!(rt.eval_with_globals(&ast, &HashMap::new()).is_err());
}

// ---------------------------------------------------------------------------
// Tag patterns (ADR-0077) — `call` patterns on `__tag__` payloads.
// ---------------------------------------------------------------------------

fn var(name: &str) -> AstNode {
    AstNode::new("var").with_payload(json!(name))
}

/// `match scrut { ok(v): v, err(e): 0 }` — two tag-pattern arms.
fn ok_err_ast(scrut: Value) -> Ast {
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(scrut)); // 0 scrutinee
    ast.push(var("ok")); // 1
    ast.push(var("v")); // 2
    ast.push(AstNode::new("call").with_payload(json!({ "children": [1, 2] }))); // 3 pat ok(v)
    ast.push(var("v")); // 4 body
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [3, 4] }))); // 5
    ast.push(var("err")); // 6
    ast.push(var("e")); // 7
    ast.push(AstNode::new("call").with_payload(json!({ "children": [6, 7] }))); // 8 pat err(e)
    ast.push(AstNode::new("value").with_payload(json!(0))); // 9 body
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [8, 9] }))); // 10
    ast.push(AstNode::new("block").with_payload(json!({ "children": [5, 10] }))); // 11
    ast.push(AstNode::new("match").with_payload(json!({ "children": [0, 11] }))); // 12
    ast
}

#[test]
fn tag_pattern_binds_payload() {
    let ast = ok_err_ast(json!({ "__tag__": "ok", "value": 42 }));
    assert_lowered_ast(&ast, json!(42));
}

#[test]
fn tag_pattern_falls_through_on_tag_miss() {
    let ast = ok_err_ast(json!({ "__tag__": "err", "value": 99 }));
    assert_lowered_ast(&ast, json!(0));
}

#[test]
fn tag_pattern_no_match_errors() {
    // A non-object scrutinee misses every tag arm → `match_fail` — the
    // same "no arm matched" error `match_shell` raises (the tree-walk
    // `match_tag_pattern_non_tagged_value_no_match` case).
    let rt = runtime();
    let ast = ok_err_ast(json!(42));
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, rt.lang(), root).expect("must lower");
    let err = interpret(&term, Value::Null).unwrap_err();
    assert!(err.contains("no arm matched"), "{err}");
}

#[test]
fn tag_pattern_multi_binder_positional() {
    // `Pair(a, b)` binds the sorted non-`__tag__` payload values
    // positionally — `{x, y}` yields `a=x, b=y` (alphabetical order).
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!({ "__tag__": "Pair", "x": 10, "y": 20 }))); // 0
    ast.push(var("Pair")); // 1
    ast.push(var("a")); // 2
    ast.push(var("b")); // 3
    ast.push(AstNode::new("call").with_payload(json!({ "children": [1, 2, 3] }))); // 4 pat
    ast.push(var("a")); // 5
    ast.push(var("b")); // 6
    ast.push(AstNode::new("array").with_payload(json!({ "children": [5, 6] }))); // 7 body
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [4, 7] }))); // 8
    ast.push(var("_")); // 9 wildcard pat
    ast.push(AstNode::new("value").with_payload(json!(0))); // 10 fallback body
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [9, 10] }))); // 11
    ast.push(AstNode::new("block").with_payload(json!({ "children": [8, 11] }))); // 12
    ast.push(AstNode::new("match").with_payload(json!({ "children": [0, 12] }))); // 13
    assert_lowered_ast(&ast, json!([10, 20]));
}

#[test]
fn tag_pattern_wildcard_and_zero_binder() {
    // `ok(_)` matches the tag and binds nothing; `ok()` the same.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!({ "__tag__": "ok", "value": 1 }))); // 0
    ast.push(var("ok")); // 1
    ast.push(var("_")); // 2
    ast.push(AstNode::new("call").with_payload(json!({ "children": [1, 2] }))); // 3
    ast.push(AstNode::new("value").with_payload(json!("hit"))); // 4
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [3, 4] }))); // 5
    ast.push(AstNode::new("block").with_payload(json!({ "children": [5] }))); // 6
    ast.push(AstNode::new("match").with_payload(json!({ "children": [0, 6] }))); // 7
    assert_lowered_ast(&ast, json!("hit"));
}

#[test]
fn nested_tag_pattern_inside_array() {
    // `[ok(v), ...t]` — a tag pattern nested inside an array pattern.
    let mut ast = Ast::new();
    ast.push(AstNode::new("value").with_payload(json!([{ "__tag__": "ok", "value": 8 }, 0]))); // 0 scrutinee
    ast.push(var("ok")); // 1
    ast.push(var("v")); // 2
    ast.push(AstNode::new("call").with_payload(json!({ "children": [1, 2] }))); // 3 ok(v)
    ast.push(var("t")); // 4
    ast.push(AstNode::new("rest").with_payload(json!({ "children": [4] }))); // 5 ...t
    ast.push(AstNode::new("array").with_payload(json!({ "children": [3, 5] }))); // 6 [ok(v), ...t]
    ast.push(var("v")); // 7 body
    ast.push(AstNode::new("arm").with_payload(json!({ "children": [6, 7] }))); // 8
    ast.push(AstNode::new("block").with_payload(json!({ "children": [8] }))); // 9
    ast.push(AstNode::new("match").with_payload(json!({ "children": [0, 9] }))); // 10
    assert_lowered_ast(&ast, json!(8));
}
