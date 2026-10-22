//! End-to-end test for ADR-0082 step 6: dependent pattern matching on
//! value-indexed inductive families with an optional motive.
//!
//! Parses `Nat` and `Vec` from surface syntax and uses surface `match`
//! (with and without a `return ... with` motive) to desugar to a recursor
//! application.

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
fn vec_indexed_family_match_with_and_without_motive() {
    // The `desugar` pass's `handl:desugar.*` overlay (ADR-0094) evaluates
    // the `.hnl` desugar module under the tree-walk engine — a
    // metacircular program whose nested arrow applications legitimately
    // need ~16–32 MiB of *debug* stack. Run the body on a large-stack
    // thread, matching the `dependent_structure` pattern — the assertion
    // is about desugar/eval semantics, not the recursion budget.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(vec_indexed_family_match_with_and_without_motive_impl)
        .expect("spawn eval thread")
        .join()
        .expect("eval thread panicked");
}

fn vec_indexed_family_match_with_and_without_motive_impl() {
    let kit = Kit::new().expect("kit must assemble");

    let nat_src = "data Nat :: Zero | Succ(Nat)";
    let vec_src = "data Vec(n: Nat, a: Type) :: VNil : Vec(0, a)
                                       | VCons(x: a, xs: Vec(n - 1, a)) : Vec(n, a)";

    let match_src = |name, v, n| {
        format!("{name} :: match ({v} : Vec({n}, a)) {{ VNil: 0, VCons(x, xs): __rec_0 + 1 }}")
    };
    let match_motive_src = |name, v, n| {
        format!(
            "{name} :: match ({v} : Vec({n}, a)) return (n, a, v) ~> Nat with {{ VNil: 0, VCons(x, xs): __rec_0 + 1 }}"
        )
    };

    // Build a single raw AST containing both data declarations and the test
    // body, then run the desugar pass once over the whole graph. This lets
    // `desugar_match_to_recursor` see the `data` declarations when it resolves
    // constructor names to families.
    let mut combined = Ast::new();
    let mut children: Vec<usize> = Vec::new();

    children.extend(append_parsed(&mut combined, &parse_raw(&kit, nat_src)));
    children.extend(append_parsed(&mut combined, &parse_raw(&kit, vec_src)));

    children.extend(append_parsed(&mut combined, &parse_raw(&kit, "a :: 0")));
    children.extend(append_parsed(&mut combined, &parse_raw(&kit, "v0 :: VNil")));
    children.extend(append_parsed(
        &mut combined,
        &parse_raw(&kit, "v1 :: VCons(42, v0)"),
    ));

    children.extend(append_parsed(
        &mut combined,
        &parse_raw(&kit, &match_src("len0", "v0", "0")),
    ));
    children.extend(append_parsed(
        &mut combined,
        &parse_raw(&kit, &match_src("len1", "v1", "1")),
    ));
    children.extend(append_parsed(
        &mut combined,
        &parse_raw(&kit, &match_motive_src("len_m", "v1", "1")),
    ));

    // result = [len0, len1, len_m]
    let len0_var = push_var(&mut combined, "len0");
    let len1_var = push_var(&mut combined, "len1");
    let len_m_var = push_var(&mut combined, "len_m");
    let result_val = push_array(&mut combined, vec![len0_var, len1_var, len_m_var]);
    let result = push_assign(&mut combined, "result", result_val);
    children.push(result);

    combined.push(AstNode::new("block").with_payload(json!({ "children": children })));

    let mut combined = apply_desugar(kit.runtime().lang(), &combined).expect("desugar failed");
    flatten_top_level_blocks(&mut combined);

    let val = kit.runtime().eval(&combined).expect("eval must succeed");

    let arr = val.as_array().expect("result must be an array");
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0], json!(0), "length of Vec(0, ...) must be 0");
    assert_eq!(arr[1], json!(1), "length of Vec(1, ...) must be 1");
    assert_eq!(
        arr[2],
        json!(1),
        "motive-bearing match on Vec(1, ...) must be 1"
    );
}
