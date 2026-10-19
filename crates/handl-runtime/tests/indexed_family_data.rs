//! End-to-end test for ADR-0082 step 5: value-indexed inductive families.
//!
//! Parses `Nat` and `Vec` from surface syntax and evaluates the generated
//! predicate, constructors, and recursor. The recursor is invoked by manually
//! constructing the `Vec.rec(...)(...)` call node, because the surface syntax
//! `Vec.rec` is not a plain identifier.
//!
//! The Pratt parser only parses one expression per call, and a `data`
//! declaration desugars into a `block` of assignments. To evaluate both
//! declarations plus the test body we parse each piece separately, copy the
//! nodes into a single arena, flatten `block` roots, and append the manually
//! built recursor calls.

use handl_kit_prelude::Kit;
use handl_language::{Ast, AstNode, AstRef};
use serde_json::{Value, json};

/// Parse `src` with the canonical kit.
fn parse(kit: &Kit, src: &str) -> Ast {
    kit.parse(src).expect("parse must succeed")
}

/// Append all nodes of `other` to `base`, shifting arena indices in `children`
/// and `body` fields so they refer into the combined arena.
fn append_ast(base: &mut Ast, other: &Ast) -> usize {
    let offset = base.len();
    for node in other.nodes() {
        let mut payload = node.payload.clone();
        if let Some(children) = payload.get_mut("children").and_then(|c| c.as_array_mut()) {
            for child in children.iter_mut() {
                if let Some(idx) = child.as_u64() {
                    *child = json!(idx as usize + offset);
                }
            }
        }
        if let Some(body) = payload.get_mut("body").and_then(|b| b.as_u64()) {
            *payload.get_mut("body").unwrap() = json!(body as usize + offset);
        }
        base.push(AstNode {
            kind: node.kind.clone(),
            span: node.span,
            payload,
        });
    }
    offset
}

/// Append a parsed AST and return the top-level statement indices for the
/// combined block. `block` roots are flattened; other roots are used as-is.
fn append_parsed(combined: &mut Ast, other: &Ast) -> Vec<usize> {
    let offset = append_ast(combined, other);
    let root_idx = other.len() - 1 + offset;
    let root = combined
        .get(AstRef::new(root_idx))
        .expect("root must exist");
    if root.kind == "block" {
        root.payload
            .get("children")
            .and_then(|c| c.as_array())
            .expect("block must have children")
            .iter()
            .map(|v| v.as_u64().unwrap() as usize)
            .collect()
    } else {
        vec![root_idx]
    }
}

fn push_value(out: &mut Ast, v: Value) -> usize {
    out.push(AstNode::new("value").with_payload(v)).index()
}

fn push_var(out: &mut Ast, name: &str) -> usize {
    out.push(AstNode::new("var").with_payload(json!(name)))
        .index()
}

fn push_call(out: &mut Ast, callee: usize, args: Vec<usize>) -> usize {
    let mut children = vec![callee];
    children.extend(args);
    out.push(AstNode::new("call").with_payload(json!({ "children": children })))
        .index()
}

fn push_binary_add(out: &mut Ast, left: usize, right: usize) -> usize {
    out.push(AstNode::new("binary.add").with_payload(json!({ "children": [left, right] })))
        .index()
}

fn push_function(out: &mut Ast, params: &[&str], body: usize) -> usize {
    let params: Vec<Value> = params.iter().map(|p| json!(*p)).collect();
    out.push(AstNode::new("function").with_payload(json!({
        "params": params,
        "body": body
    })))
    .index()
}

fn push_assign(out: &mut Ast, name: &str, value: usize) -> usize {
    let target = push_var(out, name);
    out.push(AstNode::new("assign").with_payload(json!({ "children": [target, value] })))
        .index()
}

fn push_array(out: &mut Ast, elems: Vec<usize>) -> usize {
    out.push(AstNode::new("array").with_payload(json!({ "children": elems })))
        .index()
}

#[test]
fn vec_indexed_family_predicate_and_recursor() {
    let kit = Kit::new().expect("kit must assemble");

    let nat_src = "data Nat :: Zero | Succ(Nat)";
    let vec_src = "data Vec(n: Nat, a: Type) :: VNil : Vec(0, a)
                                       | VCons(x: a, xs: Vec(n - 1, a)) : Vec(n, a)";

    // Build a single AST containing both data declarations and the test body.
    let mut combined = Ast::new();
    let mut children: Vec<usize> = Vec::new();

    children.extend(append_parsed(&mut combined, &parse(&kit, nat_src)));
    children.extend(append_parsed(&mut combined, &parse(&kit, vec_src)));

    // Setup expressions: a = 0, v0 = VNil, v1 = VCons(42, v0),
    // r0 = Vec(0, a)(v0), r1 = Vec(1, a)(v1).
    children.extend(append_parsed(&mut combined, &parse(&kit, "a :: 0")));
    children.extend(append_parsed(&mut combined, &parse(&kit, "v0 :: VNil")));
    children.extend(append_parsed(
        &mut combined,
        &parse(&kit, "v1 :: VCons(42, v0)"),
    ));
    children.extend(append_parsed(
        &mut combined,
        &parse(&kit, "r0 :: Vec(0, a, v0)"),
    ));
    children.extend(append_parsed(
        &mut combined,
        &parse(&kit, "r1 :: Vec(1, a, v1)"),
    ));

    // Manually call the generated recursor: Vec.rec(null, 0, step)(n, a, v).
    let vec_rec = push_var(&mut combined, "Vec.rec");
    let null_idx = push_value(&mut combined, Value::Null);
    let zero = push_value(&mut combined, json!(0));
    let one = push_value(&mut combined, json!(1));

    // step = (x, xs, ih) ~> ih + 1
    let ih = push_var(&mut combined, "ih");
    let one_lit = push_value(&mut combined, json!(1));
    let step_body = push_binary_add(&mut combined, ih, one_lit);
    let step = push_function(&mut combined, &["x", "xs", "ih"], step_body);

    // len0 = Vec.rec(null, 0, step)(0, a, v0)
    let a_var = push_var(&mut combined, "a");
    let v0_var = push_var(&mut combined, "v0");
    let rec_partial = push_call(&mut combined, vec_rec, vec![null_idx, zero, step]);
    let len0_call = push_call(&mut combined, rec_partial, vec![zero, a_var, v0_var]);
    let len0 = push_assign(&mut combined, "len0", len0_call);
    children.push(len0);

    // len1 = Vec.rec(null, 0, step)(1, a, v1)
    let v1_var = push_var(&mut combined, "v1");
    let rec_partial2 = push_call(&mut combined, vec_rec, vec![null_idx, zero, step]);
    let len1_call = push_call(&mut combined, rec_partial2, vec![one, a_var, v1_var]);
    let len1 = push_assign(&mut combined, "len1", len1_call);
    children.push(len1);

    // result = [r0, r1, len0, len1]
    let r0_var = push_var(&mut combined, "r0");
    let r1_var = push_var(&mut combined, "r1");
    let len0_var = push_var(&mut combined, "len0");
    let len1_var = push_var(&mut combined, "len1");
    let result = push_array(&mut combined, vec![r0_var, r1_var, len0_var, len1_var]);
    children.push(result);

    combined.push(AstNode::new("block").with_payload(json!({ "children": children })));

    let val = kit.runtime().eval(&combined).expect("eval must succeed");

    let arr = val.as_array().expect("result must be an array");
    assert_eq!(arr.len(), 4);
    assert_eq!(arr[0], json!(true), "Vec(0, a)(VNil) must be true");
    assert_eq!(
        arr[1],
        json!(true),
        "Vec(1, a)(VCons(42, VNil)) must be true"
    );
    assert_eq!(arr[2], json!(0), "length of Vec(0, ...) must be 0");
    assert_eq!(arr[3], json!(1), "length of Vec(1, ...) must be 1");
}
