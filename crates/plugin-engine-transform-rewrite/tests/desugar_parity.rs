//! Parity suite (ADR-0094 §"Testing"): `RewriteEngine::run_pass("desugar")`
//! must produce byte-for-byte identical arenas to the pre-migration
//! `handl_runtime::desugar_pass::apply_desugar`, and desugar must be
//! idempotent: `desugar(desugar(x)) == desugar(x)`.

use handl_kit_data_canonical::language_from_yaml;
use handl_language::{Ast, AstNode, AstRef, Language};
use handl_plugin_engine_transform_rewrite::RewriteEngine;
use serde_json::{Value, json};

fn canonical() -> Language {
    language_from_yaml().expect("canonical language loads")
}

/// Run the engine's `"desugar"` pass.
fn engine_desugar(lang: &Language, ast: &Ast) -> Ast {
    let engine = RewriteEngine::new().with_desugar_natives();
    let root = AstRef::new(ast.len() - 1);
    engine
        .run_pass(lang, ast, root, "desugar")
        .expect("engine desugar failed")
        .ast
}

/// Assert the runtime delegate `apply_desugar` agrees with a direct
/// `run_pass("desugar")` call, and idempotence on both.
fn assert_parity(lang: &Language, ast: &Ast) {
    let delegate =
        handl_runtime::desugar_pass::apply_desugar(lang, ast).expect("apply_desugar failed");
    let engine = engine_desugar(lang, ast);
    assert_eq!(
        delegate, engine,
        "engine output diverged from apply_desugar"
    );
    // Idempotence on both paths.
    let delegate2 = handl_runtime::desugar_pass::apply_desugar(lang, &delegate)
        .expect("apply_desugar failed on re-run");
    let engine2 = engine_desugar(lang, &engine);
    assert_eq!(delegate, delegate2, "apply_desugar is not idempotent");
    assert_eq!(engine, engine2, "engine desugar is not idempotent");
}

fn leaf(kind: &str, payload: Value) -> AstNode {
    AstNode::new(kind).with_payload(payload)
}

fn node(kind: &str, children: &[usize]) -> AstNode {
    AstNode::new(kind).with_payload(json!({ "children": children }))
}

#[test]
fn parity_identity_no_rules() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("lit", json!(1)));
    ast.push(leaf("lit", json!(2)));
    ast.push(node("binary.add", &[0, 1]));
    assert_parity(&lang, &ast);
}

#[test]
fn parity_call_json_declarative() {
    let lang = canonical();
    // call { children: [json-template, arg] } → json (declarative rule).
    let mut ast = Ast::new();
    ast.push(leaf("json", json!("hello $0"))); // 0
    ast.push(leaf("lit", json!(42))); // 1
    ast.push(node("call", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_call_non_json_when_declines() {
    let lang = canonical();
    // call { children: [var, arg] } — `when` fails → pass-through, no
    // native fallback.
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("f"))); // 0
    ast.push(leaf("lit", json!(1))); // 1
    ast.push(node("call", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_entry_bracket_key() {
    let lang = canonical();
    // entry { children: [array{[k]}, v] } → entry { paren{k}, v }.
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("k"))); // 0
    ast.push(node("array", &[0])); // 1
    ast.push(leaf("lit", json!(7))); // 2
    ast.push(node("entry", &[1, 2])); // 3
    assert_parity(&lang, &ast);
}

#[test]
fn parity_entry_non_bracket_key() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("k"))); // 0
    ast.push(leaf("lit", json!(7))); // 1
    ast.push(node("entry", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_while() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("x"))); // 0
    ast.push(node("block", &[0])); // 1
    ast.push(node("while", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_while_state_passing_guard() {
    let lang = canonical();
    // while { children: [cond, function, ...] } — the state-passing form is
    // NOT desugared (guard), passes through unchanged.
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("x"))); // 0
    ast.push(AstNode::new("function").with_payload(json!({
        "params": ["s"],
        "body": 0
    }))); // 1
    ast.push(node("while", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_do_while() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("x"))); // 0
    ast.push(node("block", &[0])); // 1
    ast.push(node("do-while", &[1, 0])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_for() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("x"))); // 0 pattern
    ast.push(leaf("var", json!("xs"))); // 1 collection
    ast.push(node("block", &[0])); // 2 body
    ast.push(node("for", &[0, 1, 2])); // 3
    assert_parity(&lang, &ast);
}

#[test]
fn parity_nullish() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("a"))); // 0
    ast.push(leaf("lit", json!(0))); // 1
    ast.push(node("nullish", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_opt_chain() {
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("a"))); // 0
    ast.push(leaf("var", json!("field"))); // 1
    ast.push(node("opt-chain", &[0, 1])); // 2
    assert_parity(&lang, &ast);
}

#[test]
fn parity_data_structured_payload() {
    let lang = canonical();
    // Hand-built structured-JSON `data` payload (the backward-compat path).
    let mut ast = Ast::new();
    ast.push(AstNode::new("data").with_payload(json!({
        "name": "Nat",
        "constructors": [
            { "name": "Zero", "args": [] },
            { "name": "Succ", "args": [{ "name": "pred", "type": "Nat" }] }
        ]
    })));
    assert_parity(&lang, &ast);
}

#[test]
fn parity_match_no_data_passthrough() {
    let lang = canonical();
    // `match` with no `data` declaration in scope: the native declines and
    // the node passes through with remapped children.
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("x"))); // 0 subject
    ast.push(leaf("var", json!("_"))); // 1 pattern
    ast.push(leaf("lit", json!(1))); // 2 body
    ast.push(node("arm", &[1, 2])); // 3
    ast.push(node("block", &[3])); // 4
    ast.push(node("match", &[0, 4])); // 5
    assert_parity(&lang, &ast);
}

#[test]
fn parity_nested_mixed() {
    let lang = canonical();
    // while { children: [nullish{a, b}, block{ for{x, xs, body} }] }
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("a"))); // 0
    ast.push(leaf("lit", json!(0))); // 1
    ast.push(node("nullish", &[0, 1])); // 2 — cond
    ast.push(leaf("var", json!("x"))); // 3
    ast.push(leaf("var", json!("xs"))); // 4
    ast.push(node("block", &[3])); // 5 — for body
    ast.push(node("for", &[3, 4, 5])); // 6 — for
    ast.push(node("block", &[6])); // 7 — while body
    ast.push(node("while", &[2, 7])); // 8
    assert_parity(&lang, &ast);
}

// ---------------------------------------------------------------------------
// Structural tests for the declarative `transforms.yaml` rewrites — the
// fixed-shape sugar forms that used to be `desugar.*` natives.
// ---------------------------------------------------------------------------

/// Get the arena index of `node`'s child `i`.
fn child_idx(node: &AstNode, i: usize) -> usize {
    node.payload
        .get("children")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.get(i))
        .and_then(|v| v.as_u64())
        .unwrap() as usize
}

/// Get `node`'s child `i` from the arena.
fn child_node<'a>(out: &'a Ast, node: &AstNode, i: usize) -> &'a AstNode {
    &out.nodes()[child_idx(node, i)]
}

#[test]
fn nullish_desugars_to_match_shape() {
    // `a ?? b` → `match a { Some: rec_v, _: b }`
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("x"))); // 0: a
    ast.push(leaf("lit", json!("default"))); // 1: b
    ast.push(node("nullish", &[0, 1])); // 2

    let out = engine_desugar(&lang, &ast);
    let root = out.nodes().last().unwrap();
    assert_eq!(root.kind, "match");

    let left = child_node(&out, root, 0);
    assert_eq!(left.kind, "var");
    assert_eq!(left.payload, json!("x"));

    let block = child_node(&out, root, 1);
    assert_eq!(block.kind, "block");
    assert_eq!(
        block
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );

    // arm 1: pattern = object (some pattern), body = fresh var for `rec_v`
    let arm1 = child_node(&out, block, 0);
    assert_eq!(arm1.kind, "arm");
    let pat1 = child_node(&out, arm1, 0);
    assert_eq!(pat1.kind, "object");
    let body1 = child_node(&out, arm1, 1);
    assert_eq!(body1.kind, "var");
    assert_eq!(body1.payload, json!("__fresh_rec_v_0"));

    // arm 2: pattern = var("_"), body = b
    let arm2 = child_node(&out, block, 1);
    assert_eq!(arm2.kind, "arm");
    let pat2 = child_node(&out, arm2, 0);
    assert_eq!(pat2.kind, "var");
    assert_eq!(pat2.payload, json!("_"));
    let body2 = child_node(&out, arm2, 1);
    assert_eq!(body2.kind, "lit");
    assert_eq!(body2.payload, json!("default"));
}

#[test]
fn opt_chain_desugars_to_match_shape() {
    // `a?.field` → `match a { Some: rec_v.field, _: None }`
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("user"))); // 0: a
    ast.push(leaf("var", json!("name"))); // 1: field
    ast.push(node("opt-chain", &[0, 1])); // 2

    let out = engine_desugar(&lang, &ast);
    let root = out.nodes().last().unwrap();
    assert_eq!(root.kind, "match");

    let left = child_node(&out, root, 0);
    assert_eq!(left.kind, "var");
    assert_eq!(left.payload, json!("user"));

    let block = child_node(&out, root, 1);
    assert_eq!(block.kind, "block");

    // arm 1: pattern = object (some pattern), body = field-access
    let arm1 = child_node(&out, block, 0);
    assert_eq!(arm1.kind, "arm");
    let pat1 = child_node(&out, arm1, 0);
    assert_eq!(pat1.kind, "object");
    let body1 = child_node(&out, arm1, 1);
    assert_eq!(body1.kind, "field-access");
    let fv = child_node(&out, body1, 0);
    assert_eq!(fv.kind, "var");
    assert_eq!(fv.payload, json!("__fresh_rec_v_0"));
    let ff = child_node(&out, body1, 1);
    assert_eq!(ff.kind, "var");
    assert_eq!(ff.payload, json!("name"));

    // arm 2: pattern = var("_"), body = lit({"_tag": "None"})
    let arm2 = child_node(&out, block, 1);
    assert_eq!(arm2.kind, "arm");
    let pat2 = child_node(&out, arm2, 0);
    assert_eq!(pat2.kind, "var");
    assert_eq!(pat2.payload, json!("_"));
    let body2 = child_node(&out, arm2, 1);
    assert_eq!(body2.kind, "lit");
    assert_eq!(body2.payload, json!({"_tag": "None"}));
}

#[test]
fn while_desugars_to_rec_shape() {
    // `while cond { body }` → `rec { __continue = cond, __result = if ... }`
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("n"))); // 0
    ast.push(leaf("lit", json!(0))); // 1
    ast.push(node("binary.gt", &[0, 1])); // 2: cond
    ast.push(leaf("var", json!("n"))); // 3
    ast.push(leaf("lit", json!(1))); // 4
    ast.push(node("assign", &[3, 4])); // 5: body
    ast.push(node("while", &[2, 5])); // 6

    let out = engine_desugar(&lang, &ast);
    let root = out.nodes().last().unwrap();
    assert_eq!(root.kind, "rec");

    // rec has 2 children: [assign(__continue, cond), assign(__result, if)]
    // — binder names are `!Fresh`-minted (`__fresh_<hint>_<n>`).
    let children = root.payload.get("children").unwrap().as_array().unwrap();
    assert_eq!(children.len(), 2);

    let assign0 = &out.nodes()[children[0].as_u64().unwrap() as usize];
    assert_eq!(assign0.kind, "assign");
    let a0_var = child_node(&out, assign0, 0);
    assert_eq!(a0_var.payload, json!("__fresh___continue_0"));
    let a0_val = child_node(&out, assign0, 1);
    assert_eq!(a0_val.kind, "binary.gt");

    let assign1 = &out.nodes()[children[1].as_u64().unwrap() as usize];
    assert_eq!(assign1.kind, "assign");
    let a1_var = child_node(&out, assign1, 0);
    assert_eq!(a1_var.payload, json!("__fresh___result_1"));
    let a1_val = child_node(&out, assign1, 1);
    assert_eq!(a1_val.kind, "if");

    // if has 3 children: [__continue, block, __result]
    let if_cond = child_node(&out, a1_val, 0);
    assert_eq!(if_cond.payload, json!("__fresh___continue_0"));
    let if_then = child_node(&out, a1_val, 1);
    assert_eq!(if_then.kind, "block");
    let if_else = child_node(&out, a1_val, 2);
    assert_eq!(if_else.payload, json!("__fresh___result_1"));

    // block contains body + reassign(__continue, cond)
    let block_children = if_then.payload.get("children").unwrap().as_array().unwrap();
    assert_eq!(block_children.len(), 2);
    let block_body = &out.nodes()[block_children[0].as_u64().unwrap() as usize];
    assert_eq!(block_body.kind, "assign");
    let block_reassign = &out.nodes()[block_children[1].as_u64().unwrap() as usize];
    assert_eq!(block_reassign.kind, "assign");
    let reassign_var = child_node(&out, block_reassign, 0);
    assert_eq!(reassign_var.payload, json!("__fresh___continue_0"));
}

#[test]
fn while_state_passing_form_not_desugared() {
    // `while { state } { closure }` (child 1 is a `function`) keeps the
    // `loop_step` eval protocol — the `when` guard declines.
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(node("object", &[])); // 0: state
    ast.push(AstNode::new("function").with_payload(json!({
        "params": ["s"],
        "body": 0
    }))); // 1: closure
    ast.push(node("while", &[0, 1])); // 2

    let out = engine_desugar(&lang, &ast);
    let root = out.nodes().last().unwrap();
    assert_eq!(root.kind, "while");
}

#[test]
fn do_while_desugars_to_rec_shape() {
    // `do { body } while cond` →
    // `block { body, rec { __continue = cond, __result = if ... } }` —
    // the body runs once before the `while`-expansion feedback region
    // (the old `rec { __result = body, … }` shape could never reach a
    // fixpoint since `rec` re-evaluates the init binding every pass).
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(node("block", &[])); // 0: body
    ast.push(leaf("var", json!("n"))); // 1
    ast.push(leaf("lit", json!(0))); // 2
    ast.push(node("binary.gt", &[1, 2])); // 3: cond
    ast.push(node("do-while", &[0, 3])); // 4

    let out = engine_desugar(&lang, &ast);
    let root = out.nodes().last().unwrap();
    assert_eq!(root.kind, "block");

    let children = root.payload.get("children").unwrap().as_array().unwrap();
    assert_eq!(children.len(), 2);

    // child 0: the body, run once before the loop.
    let body = &out.nodes()[children[0].as_u64().unwrap() as usize];
    assert_eq!(body.kind, "block");

    // child 1: rec { assign(__continue, cond), assign(__result, if ...) }
    let rec = &out.nodes()[children[1].as_u64().unwrap() as usize];
    assert_eq!(rec.kind, "rec");
    let rec_children = rec.payload.get("children").unwrap().as_array().unwrap();
    assert_eq!(rec_children.len(), 2);

    let assign0 = &out.nodes()[rec_children[0].as_u64().unwrap() as usize];
    assert_eq!(assign0.kind, "assign");
    assert_eq!(
        child_node(&out, assign0, 0).payload,
        json!("__fresh___continue_0")
    );
    assert_eq!(child_node(&out, assign0, 1).kind, "binary.gt");

    let assign1 = &out.nodes()[rec_children[1].as_u64().unwrap() as usize];
    assert_eq!(assign1.kind, "assign");
    assert_eq!(
        child_node(&out, assign1, 0).payload,
        json!("__fresh___result_1")
    );
    assert_eq!(child_node(&out, assign1, 1).kind, "if");
}

#[test]
fn for_desugars_to_rec_shape() {
    // `for x in xs { body }` →
    // `block { __xs = xs, __result = [], rec { __result = match ... } }`
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("x"))); // 0: pattern
    ast.push(leaf("var", json!("xs"))); // 1: collection
    ast.push(leaf("var", json!("x"))); // 2
    ast.push(leaf("lit", json!(2))); // 3
    ast.push(node("binary.mul", &[2, 3])); // 4: body
    ast.push(node("for", &[0, 1, 4])); // 5

    let out = engine_desugar(&lang, &ast);
    let root = out.nodes().last().unwrap();
    assert_eq!(root.kind, "block");

    // block has 3 children: [assign(__xs, xs), assign(__result, []), rec]
    // — the init bindings are hoisted out of `rec` (it would re-evaluate
    // and reset them every fixpoint pass otherwise).
    let children = root.payload.get("children").unwrap().as_array().unwrap();
    assert_eq!(children.len(), 3);

    // child 0: assign(__xs, collection)
    let assign0 = &out.nodes()[children[0].as_u64().unwrap() as usize];
    assert_eq!(assign0.kind, "assign");
    assert_eq!(
        child_node(&out, assign0, 0).payload,
        json!("__fresh___xs_0")
    );
    let a0_val = child_node(&out, assign0, 1);
    assert_eq!(a0_val.kind, "var");
    assert_eq!(a0_val.payload, json!("xs"));

    // child 1: assign(__result, [])
    let assign1 = &out.nodes()[children[1].as_u64().unwrap() as usize];
    assert_eq!(assign1.kind, "assign");
    assert_eq!(
        child_node(&out, assign1, 0).payload,
        json!("__fresh___result_1")
    );
    assert_eq!(child_node(&out, assign1, 1).kind, "array");

    // child 2: rec { assign(__result, match ...) }
    let rec = &out.nodes()[children[2].as_u64().unwrap() as usize];
    assert_eq!(rec.kind, "rec");
    let rec_children = rec.payload.get("children").unwrap().as_array().unwrap();
    assert_eq!(rec_children.len(), 1);
    let assign2 = &out.nodes()[rec_children[0].as_u64().unwrap() as usize];
    assert_eq!(assign2.kind, "assign");
    let a2_val = child_node(&out, assign2, 1);
    assert_eq!(a2_val.kind, "match");

    // match has 2 children: [__xs, block]
    assert_eq!(child_node(&out, a2_val, 0).payload, json!("__fresh___xs_0"));
    let match_block = child_node(&out, a2_val, 1);
    assert_eq!(match_block.kind, "block");
    let block_children = match_block
        .payload
        .get("children")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(block_children.len(), 2);

    // arm 1: pattern = [] (empty array), body = __result
    let arm1 = &out.nodes()[block_children[0].as_u64().unwrap() as usize];
    assert_eq!(arm1.kind, "arm");
    assert_eq!(child_node(&out, arm1, 0).kind, "array");
    assert_eq!(
        child_node(&out, arm1, 1).payload,
        json!("__fresh___result_1")
    );

    // arm 2: pattern = [x, ...rec_tail], body = block { body, __xs = rec_tail }
    let arm2 = &out.nodes()[block_children[1].as_u64().unwrap() as usize];
    assert_eq!(arm2.kind, "arm");
    let arm2_pat = child_node(&out, arm2, 0);
    assert_eq!(arm2_pat.kind, "array");
    let pat_children = arm2_pat
        .payload
        .get("children")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(pat_children.len(), 2);
    let pat_x = &out.nodes()[pat_children[0].as_u64().unwrap() as usize];
    assert_eq!(pat_x.payload, json!("x"));
    let pat_rest = &out.nodes()[pat_children[1].as_u64().unwrap() as usize];
    assert_eq!(pat_rest.kind, "rest");
    let arm2_body = child_node(&out, arm2, 1);
    assert_eq!(arm2_body.kind, "block");
}

#[test]
fn nested_nullish_desugars() {
    // Nested `nullish` — both should desugar to `match`.
    let lang = canonical();
    let mut ast = Ast::new();
    ast.push(leaf("var", json!("a"))); // 0
    ast.push(leaf("lit", json!("b"))); // 1
    ast.push(node("nullish", &[0, 1])); // 2 — inner
    ast.push(leaf("lit", json!("default"))); // 3
    ast.push(node("nullish", &[2, 3])); // 4 — outer

    let out = engine_desugar(&lang, &ast);
    let root = out.nodes().last().unwrap();
    assert_eq!(root.kind, "match");
    let left = child_node(&out, root, 0);
    assert_eq!(
        left.kind, "match",
        "nested nullish should be desugared to match"
    );
}
