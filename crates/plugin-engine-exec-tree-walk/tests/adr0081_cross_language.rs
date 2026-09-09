//! ADR-0081 Phase 4: Cross-language acceptance test.
//!
//! Defines a language with **renamed kinds** (`set`/`block2`/`ret`/`params2`/
//! `func2`) but **identical roles** and eval schemes to the canonical language.
//! Runs it through the **unchanged** eval engines (tree-walk and bytecode).
//! If the engines handle it without edits, the last `kind ==` hardcoding is
//! gone and ADR-0081's acceptance criterion is met.

use handl_language::{
    ArrSource, Ast, AstNode, AstShapeDef, ChildRoleSpec, EvalSchemeDef, Language, Term, Value,
};
use handl_plugin_engine_exec::{EvalEngine, EvalRequest};
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use serde_json::json;

// --- Renamed language builder ---------------------------------------------

/// Build a language with renamed kinds but identical roles and eval schemes.
///
/// Renaming map:
///   `assign`  → `set`     (roles: `target`, `value`;       eval: `assign`)
///   `block`   → `block2`  (roles: `statements` [variadic]; eval: `eval_block`)
///   `return`  → `ret`     (roles: `value`;                 eval: `proj_child` lit:0)
///   `paren`   → `params2` (roles: `body` [variadic];       eval: `proj_child` lit:0)
///   `function`→ `func2`   (roles: `params`, `body`;        eval: `closure`)
fn renamed_language() -> Language {
    let mut lang = Language::new();

    // `var` — same as canonical: proj_env "$payload"
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.var",
            "var",
            Term::arr_native_lit("proj_env", vec![], Value::String("$payload".into())),
        ))
        .unwrap();

    // `set` (renamed `assign`) — same eval scheme: `assign` native op
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.set",
            "set",
            Term::arr_native("assign", vec![]),
        ))
        .unwrap();
    lang.ast
        .merge(AstShapeDef::new("ast.set", "set").with_children(vec![
            ChildRoleSpec {
                role: "target".into(),
                variadic: false,
            },
            ChildRoleSpec {
                role: "value".into(),
                variadic: false,
            },
        ]))
        .unwrap();

    // `block2` (renamed `block`) — same eval scheme: `eval_block` native op
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.block2",
            "block2",
            Term::arr_native("eval_block", vec![]),
        ))
        .unwrap();
    lang.ast
        .merge(
            AstShapeDef::new("ast.block2", "block2").with_children(vec![ChildRoleSpec {
                role: "statements".into(),
                variadic: true,
            }]),
        )
        .unwrap();

    // `ret` (renamed `return`) — same eval scheme: `proj_child` lit:0
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.ret",
            "ret",
            Term::arr_native_lit("proj_child", vec![], Value::from(0u64)),
        ))
        .unwrap();
    lang.ast
        .merge(
            AstShapeDef::new("ast.ret", "ret").with_children(vec![ChildRoleSpec {
                role: "value".into(),
                variadic: false,
            }]),
        )
        .unwrap();

    // `params2` (renamed `paren`) — same eval scheme: `proj_child` lit:0
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.params2",
            "params2",
            Term::arr_native_lit("proj_child", vec![], Value::from(0u64)),
        ))
        .unwrap();
    lang.ast
        .merge(
            AstShapeDef::new("ast.params2", "params2").with_children(vec![ChildRoleSpec {
                role: "body".into(),
                variadic: true,
            }]),
        )
        .unwrap();

    // `func2` (renamed `function`) — same eval scheme: `closure` native op
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.func2",
            "func2",
            Term::arr_native("closure", vec![]),
        ))
        .unwrap();
    lang.ast
        .merge(AstShapeDef::new("ast.func2", "func2").with_children(vec![
            ChildRoleSpec {
                role: "params".into(),
                variadic: false,
            },
            ChildRoleSpec {
                role: "body".into(),
                variadic: false,
            },
        ]))
        .unwrap();

    // `call` — same as canonical: proj_child(0) >>> route_on_type("string")
    // >>> choice(call_host(proj_rest), app(id, proj_rest))
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.call",
            "call",
            Term::seq(
                Term::seq(
                    Term::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                    Term::arr_native_lit("route_on_type", vec![], Value::from("string")),
                ),
                Term::choice(
                    Term::Op { name: "call_host".into(),
                        args: vec![Term::arr_native("proj_rest", vec![])],
                        source: ArrSource::DynamicForeign,
                        lit: None,
                    },
                    Term::app(
                        Term::arr_native("id", vec![]),
                        vec![Term::arr_native("proj_rest", vec![])],
                    ),
                ),
            ),
        ))
        .unwrap();

    lang
}

// --- Test ASTs -------------------------------------------------------------

/// `block2 { set x = 42; ret x }` → expected: `42`
fn block_with_binding_ast() -> Ast {
    let mut a = Ast::new();
    a.push(AstNode::new("lit").with_payload(Value::from(42))); // 0
    a.push(AstNode::new("var").with_payload(Value::from("x"))); // 1
    a.push(AstNode::new("set").with_payload(json!({"children": [1, 0]}))); // 2
    a.push(AstNode::new("var").with_payload(Value::from("x"))); // 3
    a.push(AstNode::new("ret").with_payload(json!({"children": [3]}))); // 4
    a.push(AstNode::new("block2").with_payload(json!({"children": [2, 4]}))); // 5
    a
}

/// `func2 (params2 (x)) => x` applied to 99 → expected: `99`
fn closure_ast() -> Ast {
    let mut a = Ast::new();
    a.push(AstNode::new("var").with_payload(Value::from("x"))); // 0
    a.push(AstNode::new("params2").with_payload(json!({"children": [0]}))); // 1
    a.push(AstNode::new("var").with_payload(Value::from("x"))); // 2
    a.push(AstNode::new("func2").with_payload(json!({"children": [1, 2]}))); // 3
    a.push(AstNode::new("lit").with_payload(Value::from(99))); // 4
    a.push(AstNode::new("call").with_payload(json!({"children": [3, 4]}))); // 5
    a
}

// --- Tests -----------------------------------------------------------------

#[test]
fn tree_walk_renamed_kinds_block_with_binding() {
    let engine = TreeWalkEngine::new();
    let lang = renamed_language();
    let req = EvalRequest::new(block_with_binding_ast());
    let result = engine.eval(&lang, &req).expect("eval must succeed");
    assert_eq!(result, Value::from(42));
}

#[test]
fn tree_walk_renamed_kinds_closure() {
    let engine = TreeWalkEngine::new();
    let lang = renamed_language();
    let req = EvalRequest::new(closure_ast());
    let result = engine.eval(&lang, &req).expect("eval must succeed");
    assert_eq!(result, Value::from(99));
}

// The bytecode engine does not implement `eval_block` (a pre-existing feature
// gap, unrelated to ADR-0081). The closure path — which exercises the
// `has_variadic_role` fix — is tested below.
#[test]
fn bytecode_renamed_kinds_closure() {
    use handl_plugin_engine_exec_bytecode::BytecodeEngine;
    let engine = BytecodeEngine::new();
    let lang = renamed_language();
    let req = EvalRequest::new(closure_ast());
    let result = engine.eval(&lang, &req).expect("eval must succeed");
    assert_eq!(result, Value::from(99));
}
