//! Temporary debug test for the data recursor.
use handl_kit_canonical::Kit;
use handl_language::{Ast, AstNode, AstRef};
use handl_runtime::desugar_pass;
use serde_json::json;

fn kit() -> Kit {
    Kit::new().expect("canonical kit must assemble")
}

fn var_node(name: &str) -> AstNode {
    AstNode::new("var").with_payload(json!(name))
}
fn call_node(children: &[usize]) -> AstNode {
    AstNode::new("call").with_payload(json!({ "children": children }))
}
fn lit_node(val: serde_json::Value) -> AstNode {
    AstNode::new("lit").with_payload(val)
}
fn function_node(params: &[&str], body_idx: usize) -> AstNode {
    AstNode::new("function").with_payload(json!({ "params": params, "body": body_idx }))
}
fn append_child_to_block(ast: &mut Ast, block_idx: usize, child_idx: usize) {
    let mut children = ast
        .get(AstRef::new(block_idx))
        .expect("block node must exist")
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    children.push(json!(child_idx));
    ast.push(AstNode::new("block").with_payload(json!({ "children": children })));
}

#[test]
fn debug_step() {
    let kit = kit();
    let mut ast = Ast::new();
    ast.push(AstNode::new("data").with_payload(json!({
        "name": "Nat",
        "constructors": [
            {"name": "Zero", "args": []},
            {"name": "Succ", "args": [{"name": "pred", "type": "Nat"}]}
        ]
    })));
    let mut desugared =
        desugar_pass::apply_desugar(kit.runtime().lang(), &ast).expect("desugar failed");
    let data_block_idx = desugared.nodes().len() - 1;

    // dump desugared AST
    for (i, n) in desugared.nodes().iter().enumerate() {
        eprintln!("{}: {} {}", i, n.kind, n.payload);
    }

    let rec_pred_var = desugared.push(var_node("rec_pred")).index();
    let one = desugared.push(lit_node(json!(1))).index();
    let add = desugared
        .push(AstNode::new("binary.add").with_payload(json!({
            "children": [rec_pred_var, one]
        })))
        .index();
    let step = desugared
        .push(function_node(&["pred", "rec_pred"], add))
        .index();

    let rec_var = desugared.push(var_node("Nat.rec")).index();
    let motive = desugared.push(lit_node(json!(null))).index();
    let base_lit = desugared.push(lit_node(json!(0))).index();
    let rec_call = desugared
        .push(call_node(&[rec_var, motive, base_lit, step]))
        .index();

    let zero_var = desugared.push(var_node("Zero")).index();
    let succ_var = desugared.push(var_node("Succ")).index();
    let succ_zero = desugared.push(call_node(&[succ_var, zero_var])).index();
    let app = desugared.push(call_node(&[rec_call, succ_zero])).index();
    append_child_to_block(&mut desugared, data_block_idx, app);

    // Eval just `Nat.rec` alone to see the bound value
    let just_rec = desugared.push(var_node("Nat.rec")).index();
    let mut ast2 = desugared.clone();
    append_child_to_block(&mut ast2, data_block_idx, just_rec);
    eprintln!("NAT.REC ALONE: {:?}", kit.runtime().eval(&ast2));

    // Eval `Nat.rec(null, 0, step)` — should be an arrow (v) -> dispatch
    let mut ast3 = desugared.clone();
    append_child_to_block(&mut ast3, data_block_idx, rec_call);
    eprintln!("REC_APPLIED: {:?}", kit.runtime().eval(&ast3));

    let mut ast4 = desugared.clone();
    append_child_to_block(&mut ast4, data_block_idx, app);
    let result = kit.runtime().eval(&ast4);
    eprintln!("RESULT: {:?}", result);
}
