//! End-to-end test for ADR-0082 step 5.3: `Fin` value-indexed family.
//!
//! Parses `Nat` and `Fin` from surface syntax and checks the generated
//! former (family membership via `instanceof` on the ctor `type` link,
//! ADR-0152), constructors, recursor, and surface `match` desugaring.

use handl_kit_prelude::Kit;
use handl_language::{Ast, AstNode, AstRef};
use hnl_engine_lex_pattern::lex;
use hnl_engine_parse::{ParserEngine, ParserRequest};
use hnl_engine_parse_pratt::PrattEngine;
use handl_runtime::desugar_pass::apply_desugar;
use serde_json::json;

/// Parse `src` without running the desugar pass, so `data` declarations remain
/// `data` nodes when the AST is combined with `match` expressions.
fn parse_raw(kit: &Kit, src: &str) -> Ast {
    let tokens = lex(kit.runtime().lang(), src).expect("lex must succeed");
    PrattEngine::new()
        .parse(kit.runtime().lang(), &ParserRequest::new(tokens))
        .expect("parse must succeed")
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

fn push_var(out: &mut Ast, name: &str) -> usize {
    out.push(AstNode::new("var").with_payload(json!(name)))
        .index()
}

fn push_array(out: &mut Ast, elems: Vec<usize>) -> usize {
    out.push(AstNode::new("array").with_payload(json!({ "children": elems })))
        .index()
}

fn push_assign(out: &mut Ast, name: &str, value: usize) -> usize {
    let target = push_var(out, name);
    out.push(AstNode::new("assign").with_payload(json!({ "children": [target, value] })))
        .index()
}

/// After desugaring, each `data` declaration has become a `block` of
/// assignments. Flatten those nested blocks into the outer `block` so that the
/// constructor and recursor bindings are in the same scope as the test body.
fn flatten_top_level_blocks(ast: &mut Ast) {
    let root_idx = ast.len().saturating_sub(1);
    let root = ast.get(AstRef::new(root_idx)).expect("root must exist");
    let root_children: Vec<usize> = root
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .expect("root block must have children")
        .iter()
        .map(|v| v.as_u64().unwrap() as usize)
        .collect();

    let mut flat: Vec<usize> = Vec::new();
    for &child_idx in &root_children {
        let child = ast.get(AstRef::new(child_idx)).expect("child must exist");
        if child.kind == "block" {
            let inner: Vec<usize> = child
                .payload
                .get("children")
                .and_then(|c| c.as_array())
                .expect("nested block must have children")
                .iter()
                .map(|v| v.as_u64().unwrap() as usize)
                .collect();
            flat.extend(inner);
        } else {
            flat.push(child_idx);
        }
    }

    let root = ast
        .get_mut(AstRef::new(root_idx))
        .expect("root must be mutable");
    root.payload
        .as_object_mut()
        .expect("root payload must be an object")
        .insert("children".to_string(), json!(flat));
}

#[test]
fn fin_indexed_family_membership_and_match() {
    // The `desugar` pass's `handl:desugar.*` overlay (ADR-0094) evaluates
    // the `.hnl` desugar module under the tree-walk engine — a
    // metacircular program whose nested arrow applications legitimately
    // need ~16–32 MiB of *debug* stack. Run the body on a large-stack
    // thread, matching the `dependent_structure` pattern — the assertion
    // is about desugar/eval semantics, not the recursion budget.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(fin_indexed_family_membership_and_match_impl)
        .expect("spawn eval thread")
        .join()
        .expect("eval thread panicked");
}

fn fin_indexed_family_membership_and_match_impl() {
    let kit = Kit::new().expect("kit must assemble");

    let nat_src = "data Nat :: Zero | Succ(Nat)";
    let fin_src = "data Fin(n: Nat) :: FZ : Fin(n) | FS(i: Fin(n - 1)) : Fin(n)";

    let mut combined = Ast::new();
    let mut children: Vec<usize> = Vec::new();

    children.extend(append_parsed(&mut combined, &parse_raw(&kit, nat_src)));
    children.extend(append_parsed(&mut combined, &parse_raw(&kit, fin_src)));

    // Construct some `Fin` values.
    children.extend(append_parsed(&mut combined, &parse_raw(&kit, "f0 :: FZ")));
    children.extend(append_parsed(
        &mut combined,
        &parse_raw(&kit, "f1 :: FS(FZ)"),
    ));
    children.extend(append_parsed(
        &mut combined,
        &parse_raw(&kit, "f0_in_Fin0_bad :: FS(FZ)"),
    ));

    // ADR-0152: `Fin(i)` is the canonical `{"__former__", "indices"}`
    // family application, not a predicate arrow — membership is read off
    // the ctor value's `type` link via `instanceof`. `instanceof` reads
    // the *former head* only: index equality is a type-level fact the
    // checker owns (the runtime `indices` lower to `null` where no
    // extractor exists), so `FS(FZ)` is a `Fin` member at runtime.
    children.extend(append_parsed(
        &mut combined,
        &parse_raw(&kit, "p0 :: sym.instanceof(f0, Fin(0))"),
    ));
    children.extend(append_parsed(
        &mut combined,
        &parse_raw(&kit, "p1 :: sym.instanceof(f1, Fin(1))"),
    ));
    children.extend(append_parsed(
        &mut combined,
        &parse_raw(&kit, "p_bad :: sym.instanceof(f0_in_Fin0_bad, Fin(0))"),
    ));

    // Surface `match` without a motive.
    children.extend(append_parsed(
        &mut combined,
        &parse_raw(
            &kit,
            "m0 :: match (f0 : Fin(0)) { FZ: 0, FS(i): __rec_0 + 1 }",
        ),
    ));
    children.extend(append_parsed(
        &mut combined,
        &parse_raw(
            &kit,
            "m1 :: match (f1 : Fin(1)) { FZ: 0, FS(i): __rec_0 + 1 }",
        ),
    ));

    // Surface `match` with an explicit motive.
    children.extend(append_parsed(
        &mut combined,
        &parse_raw(
            &kit,
            "m_motive :: match (f1 : Fin(1)) return (n, v) ~> Nat with { FZ: 0, FS(i): __rec_0 + 1 }",
        ),
    ));

    // result = [p0, p1, p_bad, m0, m1, m_motive]
    let p0_var = push_var(&mut combined, "p0");
    let p1_var = push_var(&mut combined, "p1");
    let p_bad_var = push_var(&mut combined, "p_bad");
    let m0_var = push_var(&mut combined, "m0");
    let m1_var = push_var(&mut combined, "m1");
    let m_motive_var = push_var(&mut combined, "m_motive");
    let result_val = push_array(
        &mut combined,
        vec![p0_var, p1_var, p_bad_var, m0_var, m1_var, m_motive_var],
    );
    let result = push_assign(&mut combined, "result", result_val);
    children.push(result);

    combined.push(AstNode::new("block").with_payload(json!({ "children": children })));

    let mut combined = apply_desugar(kit.runtime().lang(), &combined).expect("desugar failed");
    flatten_top_level_blocks(&mut combined);

    let val = kit.runtime().eval(&combined).expect("eval must succeed");

    let arr = val.as_array().expect("result must be an array");
    assert_eq!(arr.len(), 6);
    assert_eq!(arr[0], json!(true), "FZ must be a Fin member");
    assert_eq!(arr[1], json!(true), "FS(FZ) must be a Fin member");
    assert_eq!(
        arr[2],
        json!(true),
        "FS(FZ) is a Fin member (former-head read; the index is type-level)"
    );
    assert_eq!(arr[3], json!(0), "match on f0 in Fin(0) must be 0");
    assert_eq!(arr[4], json!(1), "match on f1 in Fin(1) must be 1");
    assert_eq!(
        arr[5],
        json!(1),
        "motive-bearing match on f1 in Fin(1) must be 1"
    );
}
