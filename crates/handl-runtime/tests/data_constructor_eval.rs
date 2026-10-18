//! End-to-end regression for Bug #1 (ADR-0082 / misc.md task 0.1):
//! the `data`-desugar constructor body must substitute the *argument value*
//! into the `json` template (via a `var` reference), not embed the argument's
//! *name* as a string literal.
//!
//! We desugar `data Nat = Zero | Succ(Nat)`, extract the `Succ` constructor's
//! `json` body node straight from the desugared output (proving the desugar
//! emits the `var`-based template), then eval that body with a `Zero` value
//! bound as the `pred` argument under the tree-walk engine. The result must
//! carry the argument *value* (`{"tag":"Zero"}`), not the string `"pred"`.

use handl_language::{Ast, AstNode, AstRef, EvalSchemeDef, Language, Value};
use hnl_engine_exec_tree_walk::TreeWalkEngine;
use handl_runtime::{Runtime, desugar_pass::apply_desugar};
use hnl_engine::{ArrSource, TermData};
use serde_json::json;

/// A `Language` with just the eval schemes needed for this regression: `var`
/// (env lookup by payload name), `assign` (let-binding in a new scope), `json`
/// (template + arg substitution), and the no-scheme fallback for `lit`.
fn eval_lang() -> Language {
    let mut lang = Language::new();
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.var",
            "var",
            TermData::op(
                "proj_env",
                vec![],
                &ArrSource::Native,
                Some(Value::String("$payload".into())),
            )
            .to_term()
            .expect("TermData -> Term"),
        ))
        .unwrap();
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.assign",
            "assign",
            TermData::op("assign", vec![], &ArrSource::Native, None)
                .to_term()
                .expect("TermData -> Term"),
        ))
        .unwrap();
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.json",
            "json",
            TermData::op("json", vec![], &ArrSource::Native, None)
                .to_term()
                .expect("TermData -> Term"),
        ))
        .unwrap();
    lang
}

/// Helper: build a `data` AST node exactly like the one the parser would emit
/// (mirrors the `data_node` test helper in `desugar_pass.rs`).
fn data_node(name: &str, constructors: &[(&str, &[(&str, &str)])]) -> AstNode {
    let cons: Vec<serde_json::Value> = constructors
        .iter()
        .map(|(cname, args)| {
            let args_arr: Vec<serde_json::Value> = args
                .iter()
                .map(|(an, at)| json!({ "name": an, "type": at }))
                .collect();
            json!({ "name": cname, "args": args_arr })
        })
        .collect();
    AstNode::new("data").with_payload(json!({ "name": name, "constructors": cons }))
}

#[test]
fn succ_constructor_evaluates_arg_value_not_arg_name() {
    let lang = eval_lang();
    let rt = Runtime::new(lang.clone()).with_eval_engine(Box::new(TreeWalkEngine::new()));

    // Build and desugar `data Nat = Zero | Succ(Nat)`.
    let mut ast = Ast::new();
    ast.push(data_node(
        "Nat",
        &[("Zero", &[]), ("Succ", &[("pred", "Nat")])],
    ));
    let mut desugared = apply_desugar(&lang, &ast).expect("desugar failed");

    // Walk the desugared `block` to the `Succ` constructor's `json` body node.
    // block children: [Nat assign, Zero assign, Succ assign, Nat.rec assign]
    let block_idx = desugared.len() - 1;
    let block = desugared.get(AstRef::new(block_idx)).unwrap();
    assert_eq!(block.kind, "block");
    let kids: Vec<usize> = block
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as usize)
        .collect();
    assert_eq!(kids.len(), 4, "block should have 4 statements");

    let succ_assign = desugared.get(AstRef::new(kids[2])).unwrap();
    assert_eq!(succ_assign.kind, "assign");
    let succ_children: Vec<usize> = succ_assign
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as usize)
        .collect();
    assert_eq!(succ_children.len(), 2, "assign {{ target, value }}");
    let succ_fn_idx = succ_children[1];
    let succ_fn = desugared.get(AstRef::new(succ_fn_idx)).unwrap();
    assert_eq!(succ_fn.kind, "function");

    // function payload: { params: ["pred"], body: <body_idx> }
    let body_json_idx = succ_fn
        .payload
        .get("body")
        .and_then(|v| v.as_u64())
        .unwrap() as usize;
    let body_json = desugared.get(AstRef::new(body_json_idx)).unwrap();
    assert_eq!(
        body_json.kind, "json",
        "the Succ body must be a json template node (Bug #1 fix)"
    );

    // Now eval the desugared `json` body with a `Zero` value bound to `pred`.
    // We build: assign { children: [var("pred"), lit({"tag":"Zero"}), <body_json>] }
    // — i.e. `pred = {"tag":"Zero"} in <body_json>`. The `assign` op (3 children)
    // binds `pred` in a fresh scope and evaluates the body, so the body's
    // `var("pred")` resolves to the Zero value and the `json` op substitutes
    // that value into the template.
    let zero_idx = desugared.len();
    desugared.push(AstNode::new("lit").with_payload(json!({ "tag": "Zero" })));

    let pred_target_idx = desugared.len();
    desugared.push(AstNode::new("var").with_payload(json!("pred")));

    let _assign_idx = desugared.len();
    desugared.push(AstNode::new("assign").with_payload(json!({
        "children": [pred_target_idx as u64, zero_idx as u64, body_json_idx as u64]
    })));

    // Eval the root (last node = the assign).
    let result = rt.eval(&desugared).expect("eval should succeed");
    let expected = json!({ "tag": "Succ", "pred": { "tag": "Zero" } });
    assert_eq!(
        result, expected,
        "Succ(Zero) must yield {{ tag: Succ, pred: {{ tag: Zero }} }} — \
         the arg VALUE substituted into the template, not the arg NAME string"
    );
}
