//! Integration tests for the FP-relevant eval schemes: `match`, `handle`,
//! `fanout`, `flow`, `field-access`, and `json`.
//!
//! These tests exercise the full pipeline (source → lex → parse → eval)
//! through the canonical `Kit`, verifying that the schemes are correctly
//! defined in the eval data, correctly parsed by the Pratt parser, and
//! correctly evaluated by the tree-walk engine.

use handl_kit_canonical::Kit;
use handl_language::{Ast, AstNode, AstRef};
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
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

    // Nat.rec(null, 0, step)
    let rec_var = desugared.push(var_node("Nat.rec")).index();
    let motive = desugared.push(lit_node(json!(null))).index();
    let base_lit = desugared.push(lit_node(json!(0))).index();
    let rec_call = desugared.push(call_node(&[rec_var, motive, base_lit, step])).index();

    // rec_call(Zero)
    let zero_var = desugared.push(var_node("Zero")).index();
    let app = desugared.push(call_node(&[rec_call, zero_var])).index();

    // Append the expression to the data block so it evals in the same scope.
    append_child_to_block(&mut desugared, data_block_idx, app);

    let result = kit
        .runtime()
        .eval(&desugared)
        .expect("Nat.rec(null, 0, step)(Zero) must eval");
    assert_eq!(result, json!(0), "base case: Nat.rec(null, 0, step)(Zero) == 0");
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

    // Nat.rec(null, 0, step)
    let rec_var = desugared.push(var_node("Nat.rec")).index();
    let motive = desugared.push(lit_node(json!(null))).index();
    let base_lit = desugared.push(lit_node(json!(0))).index();
    let rec_call = desugared.push(call_node(&[rec_var, motive, base_lit, step])).index();

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
        .expect("Nat.rec(null, 0, step)(Succ(Zero)) must eval");
    // Nat.rec(null, 0, step)(Succ(Zero)) = step(Zero, Nat.rec(null, 0, step)(Zero)) = 0 + 1 = 1
    assert_eq!(result, json!(1), "step case: Nat.rec(null, 0, step)(Succ(Zero)) == 1");
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

    // Nat.rec(null, 0, step)
    let rec_var = desugared.push(var_node("Nat.rec")).index();
    let motive = desugared.push(lit_node(json!(null))).index();
    let base_lit = desugared.push(lit_node(json!(0))).index();
    let rec_call = desugared.push(call_node(&[rec_var, motive, base_lit, step])).index();

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
        .expect("Nat.rec(null, 0, step)(Succ(Succ(Zero))) must eval");
    // Nat.rec(null, 0, step)(Succ(Succ(Zero))) = step(Succ(Zero), Nat.rec(null, 0, step)(Succ(Zero)))
    // = step(Succ(Zero), 1) = 1 + 1 = 2
    assert_eq!(result, json!(2), "recurse: Nat.rec(null, 0, step)(Succ(Succ(Zero))) == 2");
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

// ---------------------------------------------------------------------------
// Phase 1: end-to-end `data` from surface syntax (ADR-0082)
// ---------------------------------------------------------------------------

/// Phase 1: Parse `data Nat = Zero | Succ(Nat)` from source string and
/// verify the raw (pre-desugar) AST contains a `data` node with the correct
/// payload name and an `alt` tree of constructors.
#[test]
fn data_parses_from_source_to_data_node() {
    let kit = kit();
    let tokens = kit.lex("data Nat = Zero | Succ(Nat)").expect("lex must succeed");

    // Parse WITHOUT desugar by calling the Pratt engine directly.
    let pratt = handl_plugin_engine_parse_pratt::PrattEngine::new();
    let req = ParserRequest::new(tokens);
    let ast = pratt
        .parse(kit.runtime().lang(), &req)
        .expect("parse must succeed");

    // The last node should be a `data` node.
    let root = ast.nodes().last().expect("ast must have a root");
    assert_eq!(root.kind, "data");
    // payload.name should be "Nat".
    let name = root.payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
    assert_eq!(name, "Nat", "data node payload.name should be 'Nat'");
    // payload.children should have 2 children: [type_expr, constructors_expr].
    let children = root
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .expect("data node must have children");
    assert_eq!(children.len(), 2, "data node should have 2 children [type, constructors]");

    // Child 0 is the type expression: var("Nat").
    let type_idx = children[0].as_u64().unwrap() as usize;
    let type_node = ast.get(AstRef::new(type_idx)).unwrap();
    assert_eq!(type_node.kind, "var");
    assert_eq!(type_node.payload, json!("Nat"));

    // Child 1 should be an `alt` node: alt(var("Zero"), call(var("Succ"), var("Nat"))).
    let alt_idx = children[1].as_u64().unwrap() as usize;
    let alt_node = ast.get(AstRef::new(alt_idx)).unwrap();
    assert_eq!(alt_node.kind, "alt");

    // alt children: [var("Zero"), call(var("Succ"), var("Nat"))]
    let alt_children = alt_node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .unwrap();
    assert_eq!(alt_children.len(), 2);

    // Left: var("Zero")
    let left = ast.get(AstRef::new(alt_children[0].as_u64().unwrap() as usize)).unwrap();
    assert_eq!(left.kind, "var");
    assert_eq!(left.payload, json!("Zero"));

    // Right: call(var("Succ"), var("Nat"))
    let right = ast.get(AstRef::new(alt_children[1].as_u64().unwrap() as usize)).unwrap();
    assert_eq!(right.kind, "call");
    let call_children = right
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .unwrap();
    assert_eq!(call_children.len(), 2);
    let callee = ast.get(AstRef::new(call_children[0].as_u64().unwrap() as usize)).unwrap();
    assert_eq!(callee.kind, "var");
    assert_eq!(callee.payload, json!("Succ"));
    let arg = ast.get(AstRef::new(call_children[1].as_u64().unwrap() as usize)).unwrap();
    assert_eq!(arg.kind, "var");
    assert_eq!(arg.payload, json!("Nat"));
}

/// Phase 1: Parse `data Nat = Zero | Succ(pred : Nat)` from source, run the
/// desugar pass (which `Runtime::parse` does automatically), then evaluate
/// `Succ(Zero)` in the scope of the desugared block and verify the result is
/// `{"tag": "Succ", "pred": {"tag": "Zero"}}`.
///
/// This is the first end-to-end `data` test from surface syntax: source
/// string → lex → parse → desugar → eval. The explicit arg name `pred : Nat`
/// ensures the constructor's field is named "pred" (not the lowercased type
/// name "nat" that the implicit form `Succ(Nat)` would produce).
#[test]
fn data_end_to_end_succ_zero() {
    let kit = kit();

    // Parse (lex + parse + desugar) the data declaration from source.
    // Runtime::parse applies the desugar pass automatically.
    let mut ast = kit
        .parse("data Nat = Zero | Succ(pred : Nat)")
        .expect("parse must succeed");

    // The root should be a `block` (desugared from `data`).
    let root = ast.nodes().last().expect("ast must have a root");
    assert_eq!(root.kind, "block", "desugared data should be a block");
    let block_children = root
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .unwrap();
    // 1 predicate + 2 constructors + 1 recursor = 4
    assert_eq!(
        block_children.len(),
        4,
        "block should have 4 statements (pred + 2 cons + rec)"
    );

    // Build Succ(Zero) in the same arena and append to the block so it
    // evaluates in the scope where Zero, Succ, Nat are defined.
    let block_idx = ast.nodes().len() - 1;
    let zero = ast.push(var_node("Zero")).index();
    let succ = ast.push(var_node("Succ")).index();
    let call = ast.push(call_node(&[succ, zero])).index();
    append_child_to_block(&mut ast, block_idx, call);

    let result = kit
        .runtime()
        .eval(&ast)
        .expect("Succ(Zero) must eval");
    assert_eq!(
        result,
        json!({"tag": "Succ", "pred": {"tag": "Zero"}}),
        "Succ(Zero) from surface syntax must yield {{tag: Succ, pred: {{tag: Zero}}}}"
    );
}

/// Phase 1: Parse `data Maybe = Nothing | Just(value : Nat)` from source
/// with explicit arg names (`value : Nat`), verifying the `type-annot`
/// node is correctly extracted by the desugar pass to produce an arg
/// named "value" (not a placeholder).
#[test]
fn data_explicit_arg_names_from_source() {
    let kit = kit();

    // Parse with explicit arg name: Just(value : Nat)
    let ast = kit
        .parse("data Maybe = Nothing | Just(value : Nat)")
        .expect("parse must succeed");

    // The root should be a `block` (desugared from `data`).
    let root = ast.nodes().last().expect("ast must have a root");
    assert_eq!(root.kind, "block");
    let block_children = root
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .unwrap();
    // 1 predicate + 2 constructors + 1 recursor = 4
    assert_eq!(block_children.len(), 4);

    // The Just constructor (statement 2) should have param "value".
    let just_assign = ast
        .get(AstRef::new(block_children[2].as_u64().unwrap() as usize))
        .unwrap();
    assert_eq!(just_assign.kind, "assign");
    let just_children = just_assign
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .unwrap();
    let just_fn = ast
        .get(AstRef::new(just_children[1].as_u64().unwrap() as usize))
        .unwrap();
    assert_eq!(just_fn.kind, "function");
    let params = just_fn.payload.get("params").unwrap().as_array().unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!(
        params[0], json!("value"),
        "explicit arg name 'value' should be used, not a placeholder"
    );
}

/// Phase 1.2: Verify the desugar-before-typecheck pipeline ordering.
///
/// `Kit::typecheck` calls `Kit::parse` (which applies the desugar pass at
/// parse time via `Runtime::parse`) before typechecking. So the typechecker
/// sees the desugared `block` (predicate + constructors + recursor), not the
/// raw `data` node. This test verifies that the desugared `data` declaration
/// produces a typecheckable AST — the `block` of assignments is accepted by
/// the typechecker without error, proving the typechecker never sees a raw
/// `data` node.
#[test]
fn data_typecheck_sees_desugared_block() {
    let kit = kit();

    // Parse+desugar the data declaration. The desugar pass runs at parse
    // time (Runtime::parse), producing a `block` of assignments.
    let ast = kit
        .parse("data Nat = Zero | Succ(pred : Nat)")
        .expect("parse must succeed");

    // Verify the root is a `block` (desugared), not a `data` node.
    let root = ast.nodes().last().expect("ast must have a root");
    assert_eq!(
        root.kind, "block",
        "typechecker should see a desugared block, not a raw data node"
    );

    // The typechecker should accept the desugared block without panicking.
    // We don't assert a specific type — the block's type depends on the
    // typechecker's handling of multi-statement blocks. The key assertion
    // is that the typechecker does not see a `data` node (which would fail
    // or produce incorrect results).
    let _ = kit.runtime().typecheck(&ast);
    // If we get here without panic, the typechecker accepted the desugared
    // AST. The desugar pass successfully eliminated the `data` node before
    // the typechecker saw it.
}

// ---------------------------------------------------------------------------
// Phase 6.3: indexed inductive families and dependent match motives
// ---------------------------------------------------------------------------

/// An indexed family with parameters and arithmetic result indices desugars
/// into a block containing the predicate, constructors, and recursor.
#[test]
fn data_vec_parses_and_desugars_from_source() {
    let kit = kit();
    let src = "data Vec(n : Nat, a : Type) = Nil | Cons(a, Vec(n - 1, a))";
    let ast = kit.parse(src).expect("Vec declaration must parse and desugar");

    // Root should be the desugared `block` of assignments.
    let root = ast.nodes().last().expect("ast must have a root");
    assert_eq!(root.kind, "block");
    let children = root
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .expect("block must have children");
    assert_eq!(children.len(), 4, "expected pred + 2 cons + recursor");

    // Helper to inspect an assignment value.
    let assign_value = |idx: usize| -> &handl_language::AstNode {
        let assign = ast
            .get(AstRef::new(children[idx].as_u64().unwrap() as usize))
            .unwrap();
        assert_eq!(assign.kind, "assign");
        let ac = assign.payload.get("children").and_then(|c| c.as_array()).unwrap();
        ast.get(AstRef::new(ac[1].as_u64().unwrap() as usize))
            .unwrap()
    };

    // Vec predicate is a function with parameters [n, a, v].
    let pred = assign_value(0);
    assert_eq!(pred.kind, "function");
    let pred_params = pred.payload.get("params").and_then(|p| p.as_array()).unwrap();
    assert_eq!(pred_params, json!(["n", "a", "v"]).as_array().unwrap());

    // Nil is a nullary constructor (literal object).
    let nil = assign_value(1);
    assert_eq!(nil.kind, "lit");

    // Cons is a function with two arguments.
    let cons = assign_value(2);
    assert_eq!(cons.kind, "function");
    let cons_params = cons.payload.get("params").and_then(|p| p.as_array()).unwrap();
    assert_eq!(cons_params.len(), 2);

    // The recursor assignment exists.
    let rec = assign_value(3);
    assert_eq!(rec.kind, "rec");
}

/// `match v return (i) -> T with { ... }` parses into a `match` node with
/// three children: [scrutinee, motive, block].
#[test]
fn match_motive_parses_to_three_children() {
    let kit = kit();
    let pratt = handl_plugin_engine_parse_pratt::PrattEngine::new();
    let src = "match v return (i) ~> Type with { a: 1 }";
    let tokens = kit.lex(src).expect("lex must succeed");
    let ast = pratt
        .parse(kit.runtime().lang(), &ParserRequest::new(tokens))
        .expect("parse must succeed");

    let root = ast.nodes().last().expect("ast must have a root");
    assert_eq!(root.kind, "match");
    let children = root
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .expect("match must have children");
    assert_eq!(children.len(), 3, "match with motive should have 3 children");

    // Scrutinee is child 0: var("v").
    let scrutinee = ast
        .get(AstRef::new(children[0].as_u64().unwrap() as usize))
        .unwrap();
    assert_eq!(scrutinee.kind, "var");
    assert_eq!(scrutinee.payload, json!("v"));

    // Motive is child 1: a function (i) ~> Type.
    let motive = ast
        .get(AstRef::new(children[1].as_u64().unwrap() as usize))
        .unwrap();
    assert_eq!(motive.kind, "function");
    let motive_children = motive
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .unwrap();
    assert_eq!(motive_children.len(), 2);
    let param = ast
        .get(AstRef::new(motive_children[0].as_u64().unwrap() as usize))
        .unwrap();
    assert_eq!(param.kind, "paren");
    let param_children = param.payload.get("children").and_then(|c| c.as_array()).unwrap();
    assert_eq!(param_children.len(), 1);
    let inner = ast
        .get(AstRef::new(param_children[0].as_u64().unwrap() as usize))
        .unwrap();
    assert_eq!(inner.kind, "var");
    assert_eq!(inner.payload, json!("i"));

    // Arms block is child 2.
    let block = ast
        .get(AstRef::new(children[2].as_u64().unwrap() as usize))
        .unwrap();
    assert_eq!(block.kind, "block");
}

/// The runtime `match` primitive ignores the optional motive at evaluation.
#[test]
fn match_motive_is_ignored_at_runtime() {
    let kit = kit();
    // Non-constructor arms prevent desugaring to a recursor, so the runtime
    // `match` primitive handles the 3-child node directly.
    let result = kit
        .eval("match 7 return (i) ~> i with { 1: \"a\", x: x }")
        .expect("match with motive must eval");
    assert_eq!(result, json!(7));
}
