//! Integration tests for the FP-relevant eval schemes: `match`, `handle`,
//! `fanout`, `flow`, `field-access`, and `json`.
//!
//! These tests exercise the full pipeline (source → lex → parse → eval)
//! through the canonical `Kit`, verifying that the schemes are correctly
//! defined in the eval data, correctly parsed by the Pratt parser, and
//! correctly evaluated by the tree-walk engine.

use handl_kit_canonical::Kit;
use handl_language::{Ast, AstNode, AstRef};
use handl_runtime::desugar_pass;
use serde_json::json;

// ---------------------------------------------------------------------------
// json — Prim op: parse a JSON template from payload
// ---------------------------------------------------------------------------

/// Bug #1: constructing `Succ(Zero)` via a `data` declaration must produce
/// `{"tag": "Succ", "pred": {"tag": "Zero"}}` — the `pred` field holds the
/// argument's value, not the literal string "pred".
#[test]
fn data_constructor_substitutes_arg_value() {
    let kit = kit();
    // The source syntax `data Nat = Zero | Succ(Nat)` can't be lexed yet
    // (no `|` token), so we construct the `data` AST node manually,
    // apply the desugar pass, then evaluate.
    let mut ast = Ast::new();
    ast.push(AstNode::new("data").with_payload(json!({
        "name": "Nat",
        "constructors": [
            {"name": "Zero", "args": []},
            {"name": "Succ", "args": [{"name": "pred", "type": "Nat"}]}
        ]
    })));
    let mut desugared = desugar_pass::apply_desugar(kit.runtime().lang(), &ast);
    let data_block_idx = desugared.nodes().len() - 1;

    // Build: Succ(Zero) in the same AST arena.
    let zero_idx = desugared.push(var_node("Zero")).index();
    let succ_idx = desugared.push(var_node("Succ")).index();
    let call_idx = desugared.push(call_node(&[succ_idx, zero_idx])).index();

    // Append the expression as a child of the data block (not a new outer
    // block — that would create a separate scope and the bindings wouldn't
    // be visible).
    append_child_to_block(&mut desugared, data_block_idx, call_idx);

    let result = kit
        .runtime()
        .eval(&desugared)
        .expect("Succ(Zero) must eval");
    assert_eq!(
        result,
        json!({"tag": "Succ", "pred": {"tag": "Zero"}}),
        "pred field must hold the evaluated argument, not the string \"pred\""
    );
}

/// Build a `Kit` for testing.
fn kit() -> Kit {
    Kit::new().expect("canonical kit must assemble")
}

/// Helper: append a child to a `block` node's children array and push a
/// new root block node as the last arena node (so `runtime.eval` evaluates
/// the block, not the appended child).
///
/// `runtime.eval` evaluates the LAST node in the AST arena. After pushing
/// expression nodes, the original block is no longer last. This helper
/// modifies the original block's children in-place AND pushes a new block
/// node (with the same children) as the last arena node.
fn append_child_to_block(ast: &mut Ast, block_idx: usize, child_idx: usize) {
    // Get the original block's children.
    let mut children = ast
        .get(AstRef::new(block_idx))
        .expect("block node must exist")
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    // Append the new child.
    children.push(json!(child_idx));
    // Push a new block node as the last arena node (the eval root).
    ast.push(AstNode::new("block").with_payload(json!({ "children": children })));
}

/// Helper: build a `var` node.
fn var_node(name: &str) -> AstNode {
    AstNode::new("var").with_payload(json!(name))
}

/// Helper: build a `call` node with children.
fn call_node(children: &[usize]) -> AstNode {
    AstNode::new("call").with_payload(json!({ "children": children }))
}

/// Helper: build a `lit` node.
fn lit_node(val: serde_json::Value) -> AstNode {
    AstNode::new("lit").with_payload(val)
}

/// Helper: build a `function` node with named params and a body arena index.
/// Uses the rich payload format (`params` + `body`) which the `closure` eval
/// primitive reads directly, correctly supporting multi-parameter functions.
fn function_node(params: &[&str], body_idx: usize) -> AstNode {
    AstNode::new("function").with_payload(json!({
        "params": params,
        "body": body_idx
    }))
}

/// Debug helper: parse and print the AST.
#[allow(dead_code)]
fn debug_parse(src: &str) {
    let kit = kit();
    match kit.parse(src) {
        Ok(ast) => {
            println!("=== {} ===", src);
            for (i, node) in ast.nodes().iter().enumerate() {
                println!("  [{}] {} {}", i, node.kind, node.payload);
            }
        }
        Err(e) => println!("=== {} === ERROR: {}", src, e),
    }
}

// ---------------------------------------------------------------------------
// fanout (&&&) — Foreign host fn `fn.fanout_fn(a, b)` → [a, b]
// ---------------------------------------------------------------------------

#[test]
fn debug_parse_all() {
    // This test just prints parse output for debugging.
    debug_parse("1 &&& 2");
    debug_parse("1 + 1 &&& 2 * 3");
    debug_parse("1 >>> x ~> x + 1");
    debug_parse("1.a");
}

#[test]
fn fanout_eager_pair_from_source() {
    let kit = kit();
    let result = kit.run("1 &&& 2").expect("fanout must eval");
    assert_eq!(result, json!([1, 2]));
}

#[test]
fn fanout_with_expressions() {
    let kit = kit();
    // No parens needed — `+` (bp 55) binds tighter than `&&&` (bp 35/36).
    let result = kit.run("1 + 1 &&& 2 * 3").expect("fanout must eval");
    assert_eq!(result, json!([2, 6]));
}

// ---------------------------------------------------------------------------
// field-access (.) — Foreign host fn `field_access(target, field)` → target[field]
// ---------------------------------------------------------------------------

#[test]
fn field_access_object_from_source() {
    let kit = kit();
    let result = kit
        .run("{ \"a\": 1, \"b\": 2 }.a")
        .expect("field-access must eval");
    assert_eq!(result, json!(1));
}

#[test]
fn field_access_array_from_source() {
    let kit = kit();
    let result = kit.run("[10, 20, 30].1").expect("field-access must eval");
    assert_eq!(result, json!(20));
}

// ---------------------------------------------------------------------------
// flow (>>>) — App scheme: a >>> b = b(a)
// ---------------------------------------------------------------------------

#[test]
fn flow_applies_closure_to_left() {
    let kit = kit();
    // `1 >>> (x ~> x + 1)` — flow applies the closure to 1, yielding 2.
    // Per ADR-0074, `>>>` (bp 25/26) binds tighter than `~>` (bp 10/9), so
    // the arrow closure must be parenthesized to avoid `(1 >>> x) ~> ...`.
    let result = kit.run("1 >>> (x ~> x + 1)").expect("flow must eval");
    assert_eq!(result, json!(2));
}

#[test]
fn flow_chains_left_associative() {
    let kit = kit();
    // `>>>` is left-associative (bp [25, 26]), so this parses as
    // `(1 >>> (x ~> x + 1)) >>> (y ~> y * 2)` == `2 >>> (y ~> y * 2)` == 4.
    // Arrows are parenthesized because `~>` (bp 10/9) is below `>>>` (bp 25/26).
    let result = kit
        .run("1 >>> (x ~> x + 1) >>> (y ~> y * 2)")
        .expect("chained flow must eval");
    assert_eq!(result, json!(4));
}

#[test]
fn flow_three_step_chain() {
    let kit = kit();
    // `1 >>> inc >>> double >>> inc` == `inc(1)=2 → double(2)=4 → inc(4)=5`.
    // Arrows are parenthesized because `~>` (bp 10/9) is below `>>>` (bp 25/26).
    let result = kit
        .run("1 >>> (x ~> x + 1) >>> (y ~> y * 2) >>> (z ~> z + 1)")
        .expect("three-step flow must eval");
    assert_eq!(result, json!(5));
}

// ---------------------------------------------------------------------------
// match — Prim op: pattern matching with arms
// ---------------------------------------------------------------------------

#[test]
fn match_literal_pattern() {
    let kit = kit();
    let result = kit
        .run("match 1 { 1: 100, 2: 200, _: 300 }")
        .expect("match must eval");
    assert_eq!(result, json!(100));
}

#[test]
fn match_second_arm() {
    let kit = kit();
    let result = kit
        .run("match 2 { 1: 100, 2: 200, _: 300 }")
        .expect("match must eval");
    assert_eq!(result, json!(200));
}

#[test]
fn match_wildcard_arm() {
    let kit = kit();
    let result = kit
        .run("match 99 { 1: 100, 2: 200, _: 300 }")
        .expect("match must eval");
    assert_eq!(result, json!(300));
}

#[test]
fn match_binder_binds_scrutinee() {
    let kit = kit();
    // `match 42 { x: x + 1 }` — binder `x` binds 42, body evaluates x + 1 = 43.
    let result = kit
        .run("match 42 { x: x + 1 }")
        .expect("match with binder must eval");
    assert_eq!(result, json!(43));
}

#[test]
fn match_no_arm_matched_errors() {
    let kit = kit();
    let err = kit
        .run("match 99 { 1: 100, 2: 200 }")
        .expect_err("match with no matching arm should error");
    assert!(err.to_string().contains("no arm matched"));
}

// --- Array destructuring patterns ---

#[test]
fn match_array_destructuring() {
    let kit = kit();
    let result = kit
        .run("match [1, 2] { [a, b]: a + b }")
        .expect("array destructuring must eval");
    assert_eq!(result, json!(3));
}

#[test]
fn match_array_destructuring_with_rest() {
    let kit = kit();
    let result = kit
        .run("match [1, 2, 3, 4] { [a, ...rest]: rest }")
        .expect("array rest must eval");
    assert_eq!(result, json!([2, 3, 4]));
}

#[test]
fn match_array_destructuring_exact_length() {
    let kit = kit();
    // Without rest, lengths must match exactly.
    let err = kit
        .run("match [1, 2, 3] { [a, b]: a + b }")
        .expect_err("length mismatch should not match");
    assert!(err.to_string().contains("no arm matched"));
}

#[test]
fn match_array_destructuring_nested_binder() {
    let kit = kit();
    let result = kit
        .run("match [1, 2] { [a, _]: a }")
        .expect("array with wildcard must eval");
    assert_eq!(result, json!(1));
}

// --- Object destructuring patterns ---

#[test]
fn match_object_destructuring() {
    let kit = kit();
    let result = kit
        .run("match { \"x\": 1, \"y\": 2 } { {x: a, y: b}: a + b }")
        .expect("object destructuring must eval");
    assert_eq!(result, json!(3));
}

#[test]
fn match_object_destructuring_with_rest() {
    let kit = kit();
    let result = kit
        .run("match { \"x\": 1, \"y\": 2, \"z\": 3 } { {x: a, ...rest}: rest }")
        .expect("object rest must eval");
    assert_eq!(result, json!({"y": 2, "z": 3}));
}

#[test]
fn match_object_destructuring_missing_key() {
    let kit = kit();
    let err = kit
        .run("match { \"x\": 1 } { {x: a, y: b}: a + b }")
        .expect_err("missing key should not match");
    assert!(err.to_string().contains("no arm matched"));
}

#[test]
fn match_object_destructuring_partial() {
    let kit = kit();
    let result = kit
        .run("match { \"x\": 1, \"y\": 2, \"z\": 3 } { {x: a}: a }")
        .expect("partial object destructuring must eval");
    assert_eq!(result, json!(1));
}

// ---------------------------------------------------------------------------
// handle — REMOVED in v0.1 (ADR-0074)
// ---------------------------------------------------------------------------
// The `handle` keyword (effect-handler dispatch) was removed from the
// language in v0.1. It has no direct v0.1 equivalent — the closest constructs
// are `match` (value dispatch) and `|||` (arrow/choice dispatch). The three
// `handle` integration tests (`handle_dispatches_to_matching_clause`,
// `handle_no_clause_returns_body`, `handle_binder_binds_body_result`)
// referenced the removed syntax and were deleted rather than migrated.

// ---------------------------------------------------------------------------
// json — Prim op: parse a JSON template from payload
// ---------------------------------------------------------------------------

/// Build a manual AST for the `json` prim to test it directly, since the
/// source syntax for `json` may not have a parser rule yet.
#[test]
fn json_prim_parses_literal_string() {
    let kit = kit();
    // ADR-0053: json node payload is the JSON template string directly.
    let mut ast = Ast::new();
    let json_node = AstNode::new("json").with_payload(json!("{\"a\": 1}"));
    ast.push(json_node);
    let result = kit.runtime().eval(&ast).expect("json prim must eval");
    assert_eq!(result, json!({"a": 1}));
}

#[test]
fn json_prim_with_placeholder_substitution() {
    let kit = kit();
    // ADR-0053/0060: json{...}(args) is parsed as call { children: [json_node, arg_node] }.
    // The desugar pass rewrites call(json, args...) → json node with children,
    // which the json eval op uses for $0/$1/… placeholder substitution.
    let mut ast = Ast::new();
    // Node 0: literal 42 (value to substitute into $0).
    let lit_node = AstNode::new("lit").with_payload(json!(42));
    let lit_idx = ast.push(lit_node).index();
    // Node 1: the json node with template containing $0.
    let json_node = AstNode::new("json").with_payload(json!("{\"value\": $0}"));
    let json_idx = ast.push(json_node).index();
    // Node 2: call node wrapping json with args.
    let call_node = AstNode::new("call").with_payload(json!({ "children": [json_idx, lit_idx] }));
    ast.push(call_node);
    // Apply the desugar pass (same as Runtime::parse does) so the call
    // node is rewritten into a json node before evaluation.
    let ast = desugar_pass::apply_desugar(kit.runtime().lang(), &ast);
    let result = kit
        .runtime()
        .eval(&ast)
        .expect("json prim with args must eval");
    assert_eq!(result, json!({"value": 42}));
}

// --- json source syntax: json{...} and json{...}(args) ---

#[test]
fn json_source_syntax_literal() {
    let kit = kit();
    let result = kit.run("json{\"a\": 1}").expect("json source must eval");
    assert_eq!(result, json!({"a": 1}));
}

#[test]
fn json_source_syntax_nested() {
    let kit = kit();
    let result = kit
        .run("json{\"a\": {\"b\": 1}}")
        .expect("nested json must eval");
    assert_eq!(result, json!({"a": {"b": 1}}));
}

#[test]
fn json_source_syntax_with_placeholder() {
    let kit = kit();
    let result = kit
        .run("json{\"value\": $0}(42)")
        .expect("json with args must eval");
    assert_eq!(result, json!({"value": 42}));
}

#[test]
fn json_source_syntax_with_expr_arg() {
    let kit = kit();
    let result = kit
        .run("json{\"sum\": $0}(1 + 2)")
        .expect("json with expr arg must eval");
    assert_eq!(result, json!({"sum": 3}));
}

// ---------------------------------------------------------------------------
// data recursor eval tests (Bugs 0.2, 0.3)
// ---------------------------------------------------------------------------

/// Bug 0.2: `Nat.rec(0, step)(Zero)` must return the base value (0).
/// The recursor dispatch uses nested 3-child `if` nodes. If the `if` had
/// 5 children, eval would fail.
#[test]
fn data_recursor_base_case_zero() {
    let kit = kit();
    let mut ast = Ast::new();
    ast.push(AstNode::new("data").with_payload(json!({
        "name": "Nat",
        "constructors": [
            {"name": "Zero", "args": []},
            {"name": "Succ", "args": [{"name": "pred", "type": "Nat"}]}
        ]
    })));
    let mut desugared = desugar_pass::apply_desugar(kit.runtime().lang(), &ast);
    let data_block_idx = desugared.nodes().len() - 1;

    // Build step function: (pred, rec_pred) -> rec_pred
    let body = desugared.push(var_node("rec_pred")).index();
    let step = desugared.push(function_node(&["pred", "rec_pred"], body)).index();

    // Nat.rec(0, step)
    let rec_var = desugared.push(var_node("Nat.rec")).index();
    let base_lit = desugared.push(lit_node(json!(0))).index();
    let rec_call = desugared.push(call_node(&[rec_var, base_lit, step])).index();

    // rec_call(Zero)
    let zero_var = desugared.push(var_node("Zero")).index();
    let app = desugared.push(call_node(&[rec_call, zero_var])).index();

    // Append the expression to the data block so it evals in the same scope.
    append_child_to_block(&mut desugared, data_block_idx, app);

    let result = kit
        .runtime()
        .eval(&desugared)
        .expect("Nat.rec(0, step)(Zero) must eval");
    assert_eq!(result, json!(0), "base case: Nat.rec(0, step)(Zero) == 0");
}

/// Bug 0.2 + 0.3: `Nat.rec(0, step)(Succ(Zero))` must return `step(Zero, 0)`.
/// This tests both the 3-child `if` dispatch (Bug 0.2) and the recursive
/// self-application (Bug 0.3).
#[test]
fn data_recursor_step_case_one() {
    let kit = kit();
    let mut ast = Ast::new();
    ast.push(AstNode::new("data").with_payload(json!({
        "name": "Nat",
        "constructors": [
            {"name": "Zero", "args": []},
            {"name": "Succ", "args": [{"name": "pred", "type": "Nat"}]}
        ]
    })));
    let mut desugared = desugar_pass::apply_desugar(kit.runtime().lang(), &ast);
    let data_block_idx = desugared.nodes().len() - 1;

    // step = (pred, rec_pred) -> rec_pred + 1
    let rec_pred_var = desugared.push(var_node("rec_pred")).index();
    let one = desugared.push(lit_node(json!(1))).index();
    let add = desugared.push(AstNode::new("binary.add").with_payload(json!({
        "children": [rec_pred_var, one]
    }))).index();
    let step = desugared.push(function_node(&["pred", "rec_pred"], add)).index();

    // Nat.rec(0, step)
    let rec_var = desugared.push(var_node("Nat.rec")).index();
    let base_lit = desugared.push(lit_node(json!(0))).index();
    let rec_call = desugared.push(call_node(&[rec_var, base_lit, step])).index();

    // Succ(Zero)
    let zero_var = desugared.push(var_node("Zero")).index();
    let succ_var = desugared.push(var_node("Succ")).index();
    let succ_zero = desugared.push(call_node(&[succ_var, zero_var])).index();

    // rec_call(Succ(Zero))
    let app = desugared.push(call_node(&[rec_call, succ_zero])).index();

    // Append the expression to the data block so it evals in the same scope.
    append_child_to_block(&mut desugared, data_block_idx, app);

    let result = kit
        .runtime()
        .eval(&desugared)
        .expect("Nat.rec(0, step)(Succ(Zero)) must eval");
    // Nat.rec(0, step)(Succ(Zero)) = step(Zero, Nat.rec(0, step)(Zero)) = 0 + 1 = 1
    assert_eq!(result, json!(1), "step case: Nat.rec(0, step)(Succ(Zero)) == 1");
}

/// Bug 0.3: `Nat.rec` can recurse to depth 2.
/// `Nat.rec(0, step)(Succ(Succ(Zero)))` should return 2.
/// This verifies the self-application recursion works for depth > 1.
#[test]
fn data_recursor_recurse_depth_two() {
    let kit = kit();
    let mut ast = Ast::new();
    ast.push(AstNode::new("data").with_payload(json!({
        "name": "Nat",
        "constructors": [
            {"name": "Zero", "args": []},
            {"name": "Succ", "args": [{"name": "pred", "type": "Nat"}]}
        ]
    })));
    let mut desugared = desugar_pass::apply_desugar(kit.runtime().lang(), &ast);
    let data_block_idx = desugared.nodes().len() - 1;

    // step = (pred, rec_pred) -> rec_pred + 1
    let rec_pred_var = desugared.push(var_node("rec_pred")).index();
    let one = desugared.push(lit_node(json!(1))).index();
    let add = desugared.push(AstNode::new("binary.add").with_payload(json!({
        "children": [rec_pred_var, one]
    }))).index();
    let step = desugared.push(function_node(&["pred", "rec_pred"], add)).index();

    // Nat.rec(0, step)
    let rec_var = desugared.push(var_node("Nat.rec")).index();
    let base_lit = desugared.push(lit_node(json!(0))).index();
    let rec_call = desugared.push(call_node(&[rec_var, base_lit, step])).index();

    // Succ(Succ(Zero))
    let zero_var = desugared.push(var_node("Zero")).index();
    let succ_var = desugared.push(var_node("Succ")).index();
    let succ_zero = desugared.push(call_node(&[succ_var, zero_var])).index();
    let succ_succ_zero = desugared.push(call_node(&[succ_var, succ_zero])).index();

    // rec_call(Succ(Succ(Zero)))
    let app = desugared.push(call_node(&[rec_call, succ_succ_zero])).index();

    // Append the expression to the data block so it evals in the same scope.
    append_child_to_block(&mut desugared, data_block_idx, app);

    let result = kit
        .runtime()
        .eval(&desugared)
        .expect("Nat.rec(0, step)(Succ(Succ(Zero))) must eval");
    // Nat.rec(0, step)(Succ(Succ(Zero))) = step(Succ(Zero), Nat.rec(0, step)(Succ(Zero)))
    // = step(Succ(Zero), 1) = 1 + 1 = 2
    assert_eq!(result, json!(2), "recurse: Nat.rec(0, step)(Succ(Succ(Zero))) == 2");
}

// ---------------------------------------------------------------------------
// Bug 0.4: desugar-error raises LangError at eval time
// ---------------------------------------------------------------------------

/// Bug 0.4: A strict positivity violation must raise a `LangError` at eval
/// time, not be silently swallowed as a string literal.
#[test]
fn data_strict_positivity_violation_raises_error() {
    let kit = kit();
    let mut ast = Ast::new();
    // data Bad = MkBad(Bad -> Int) — negative occurrence
    ast.push(AstNode::new("data").with_payload(json!({
        "name": "Bad",
        "constructors": [
            {"name": "MkBad", "args": [{"name": "f", "type": "Bad -> Int"}]}
        ]
    })));
    let desugared = desugar_pass::apply_desugar(kit.runtime().lang(), &ast);

    // The desugared AST should contain a `desugar-error` node.
    let has_error = desugared.nodes().iter().any(|n| n.kind == "desugar-error");
    assert!(has_error, "desugar should emit a desugar-error node");

    // Evaluating it should return an error, not a string.
    let result = kit.runtime().eval(&desugared);
    assert!(result.is_err(), "desugar-error must raise a LangError");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("strict positivity"),
        "error should mention strict positivity, got: {}",
        err_msg
    );
}
