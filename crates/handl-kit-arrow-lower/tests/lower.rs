//! Targeted tests for the new arrow lowering (Op-grade and handle-with).

use handl_kit_arrow_lower::lower_ast;
use handl_language::{ArrSource, Ast, AstNode, AstRef, Language, Term, Value};
use serde_json::json;

fn var_node(name: &str) -> AstNode {
    AstNode::new("var").with_payload(Value::String(name.to_string()))
}

fn value_node(v: Value) -> AstNode {
    AstNode::new("value").with_payload(v)
}

fn with_children(kind: &str, children: Vec<usize>) -> AstNode {
    AstNode::new(kind).with_payload(json!({ "children": children }))
}

fn with_children_and_name(kind: &str, children: Vec<usize>, name: &str) -> AstNode {
    AstNode::new(kind).with_payload(json!({ "children": children, "name": name }))
}

#[test]
fn graded_arrow_with_op_and_grade_product_lowers_to_named_op() {
    // `x = () ~[(Op, SomeGrade)]~> ()` should bind `x` to `Term::Op` whose
    // `dim` is the first non-`Op` grade as a *named* dimension
    // (`Const "SomeGrade"`, ADR-0120 Phase 3) — not `ArrSource::Foreign`,
    // which is the host-function boundary, not a dimension.
    //
    // Arena layout:
    //   0: var "x" (assign target)
    //   1: value null (inputs)
    //   2: var "Op"
    //   3: var "SomeGrade"
    //   4: value null (output)
    //   5: product [2, 3] (the grade)
    //   6: graded_arrow [1, 5, 4]
    //   7: var "x" (return expression)
    //   8: assign [0, 6]
    //   9: block [8, 7]
    let mut ast = Ast::new();
    ast.push(var_node("x"));
    ast.push(value_node(Value::Null));
    ast.push(var_node("Op"));
    ast.push(var_node("SomeGrade"));
    ast.push(value_node(Value::Null));
    ast.push(with_children("product", vec![2, 3]));
    ast.push(with_children("graded_arrow", vec![1, 5, 4]));
    ast.push(var_node("x"));
    ast.push(with_children("assign", vec![0, 6]));
    ast.push(with_children("block", vec![8, 7]));

    let term = lower_ast(&ast, &Language::new(), AstRef::new(9)).expect("lower should succeed");
    assert_eq!(
        term,
        Term::Op {
            dim: Box::new(Term::named_dim("SomeGrade")),
            name: "x".to_string(),
            args: vec![],
            source: ArrSource::Native,
            lit: None,
        }
    );
}

// ---------------------------------------------------------------------------
// Data-driven lowering — `lang.eval` scheme instantiation (ADR-0105/0120)
// ---------------------------------------------------------------------------

use handl_language::{AstShapeDef, ChildRoleSpec, EvalSchemeDef};

fn proj(i: u64) -> Term {
    Term::op_native_lit("proj_child", vec![], Value::from(i))
}

fn id_at_dim(d: u64) -> Term {
    Term::Op {
        dim: Box::new(Term::constant(Value::from(d))),
        name: "id".into(),
        args: vec![],
        source: ArrSource::Native,
        lit: None,
    }
}

fn arr_key(s: &str) -> Term {
    Term::Arr {
        dim: Term::default_dim(),
        value: s.into(),
    }
}

fn merge_scheme(lang: &mut Language, kind: &str, scheme: Term) {
    lang.eval
        .merge(EvalSchemeDef::new(format!("eval.{kind}"), kind, scheme))
        .unwrap();
}

fn merge_roles(lang: &mut Language, kind: &str, roles: &[&str]) {
    lang.ast
        .merge(
            AstShapeDef::new(format!("ast.{kind}"), kind).with_children(
                roles
                    .iter()
                    .map(|r| ChildRoleSpec {
                        role: (*r).into(),
                        variadic: false,
                        optional: false,
                    })
                    .collect(),
            ),
        )
        .unwrap();
}

/// `binary.add` has no Rust arm: it lowers by instantiating
/// `eval.binary.add`'s `Op{add, [proj_child 0, proj_child 1]}` scheme —
/// the placeholders become the lowered children.
#[test]
fn binary_op_lowers_via_eval_scheme() {
    let mut lang = Language::new();
    merge_scheme(
        &mut lang,
        "binary.add",
        Term::op_native("add", vec![proj(0), proj(1)]),
    );

    let mut ast = Ast::new();
    ast.push(value_node(json!(1)));
    ast.push(value_node(json!(2)));
    ast.push(with_children("binary.add", vec![0, 1]));

    let term = lower_ast(&ast, &lang, AstRef::new(2)).expect("lower should succeed");
    assert_eq!(
        term,
        Term::op_native(
            "add",
            vec![Term::const_value(json!(1)), Term::const_value(json!(2))]
        )
    );
}

/// A construct that exists *only* as an eval scheme — no canonical kind,
/// no Rust arm — lowers automatically. This is the engine-genericity
/// property: adding a `binary.*`-shaped construct to the language is a
/// data change, not an engine edit.
#[test]
fn yaml_only_construct_lowers_via_scheme() {
    let mut lang = Language::new();
    // `tensor` is not a canonical kind; it has no Rust lowering arm and
    // no canonical data — only this merged scheme.
    merge_scheme(
        &mut lang,
        "tensor",
        Term::op_native("tensor", vec![proj(0), proj(1)]),
    );

    let mut ast = Ast::new();
    ast.push(value_node(json!(3)));
    ast.push(value_node(json!(4)));
    ast.push(with_children("tensor", vec![0, 1]));

    let term = lower_ast(&ast, &lang, AstRef::new(2)).expect("scheme-defined kind should lower");
    assert_eq!(
        term,
        Term::op_native(
            "tensor",
            vec![Term::const_value(json!(3)), Term::const_value(json!(4))]
        )
    );
}

/// A `Child{role}` placeholder resolves the role via `AstShapeDef`
/// (ADR-0081), not a positional index.
#[test]
fn child_role_placeholder_resolves_named_role() {
    let mut lang = Language::new();
    merge_roles(&mut lang, "pair_op", &["left", "right"]);
    merge_scheme(
        &mut lang,
        "pair_op",
        Term::op_native(
            "pair_op",
            vec![
                Term::Child {
                    role: "right".into(),
                },
                Term::Child {
                    role: "left".into(),
                },
            ],
        ),
    );

    let mut ast = Ast::new();
    ast.push(value_node(json!("l")));
    ast.push(value_node(json!("r")));
    ast.push(with_children("pair_op", vec![0, 1]));

    let term = lower_ast(&ast, &lang, AstRef::new(2)).expect("lower should succeed");
    assert_eq!(
        term,
        Term::op_native(
            "pair_op",
            vec![Term::const_value(json!("r")), Term::const_value(json!("l"))]
        )
    );
}

/// `cell_comp_h` (`=>|`, horizontal 2-cell composition) lowers through
/// `eval.cell_comp_h`: the dimension-2 `proj_child` placeholders lift the
/// operands to `t >>> id₂` — the same `Seq{axis:0}`/lifted-operand shape
/// the old `lower_frontier_cell` arm produced.
#[test]
fn frontier_cell_lowers_via_scheme() {
    let mut lang = Language::new();
    merge_scheme(
        &mut lang,
        "cell_comp_h",
        Term::Seq {
            axis: Box::new(Term::constant(Value::from(0))),
            fst: Box::new(Term::Op {
                dim: Box::new(Term::constant(Value::from(2))),
                name: "proj_child".into(),
                args: vec![],
                source: ArrSource::Native,
                lit: Some(Value::from(0u64)),
            }),
            snd: Box::new(Term::Op {
                dim: Box::new(Term::constant(Value::from(2))),
                name: "proj_child".into(),
                args: vec![],
                source: ArrSource::Native,
                lit: Some(Value::from(1u64)),
            }),
        },
    );

    let mut ast = Ast::new();
    ast.push(value_node(json!("a")));
    ast.push(value_node(json!("b")));
    ast.push(with_children("cell_comp_h", vec![0, 1]));

    let term = lower_ast(&ast, &lang, AstRef::new(2)).expect("lower should succeed");
    let lift = |t: Term| Term::seq(t, id_at_dim(2));
    assert_eq!(
        term,
        Term::seq_axis(
            Term::constant(Value::from(0)),
            lift(Term::const_value(json!("a"))),
            lift(Term::const_value(json!("b")))
        )
    );
}

/// `dim_arrow` lowers through `eval.dim_arrow`: the `dim`-slot `Child`
/// placeholder instantiates via `lower_dim_term` (a `var` is a named
/// dimension label `Const "S"`), and `Child{role:"dim"}` inside the
/// record args resolves to the same label position.
#[test]
fn dim_arrow_lowers_via_scheme() {
    let mut lang = Language::new();
    merge_roles(&mut lang, "dim_arrow", &["source", "dim", "target"]);
    merge_scheme(
        &mut lang,
        "dim_arrow",
        Term::Op {
            dim: Box::new(Term::Child { role: "dim".into() }),
            name: "tag.construct".into(),
            args: vec![
                arr_key("Arrow"),
                Term::op_native(
                    "record",
                    vec![
                        arr_key("inputs"),
                        Term::Child {
                            role: "source".into(),
                        },
                        arr_key("grade"),
                        Term::Child { role: "dim".into() },
                        arr_key("output"),
                        Term::Child {
                            role: "target".into(),
                        },
                    ],
                ),
            ],
            source: ArrSource::Native,
            lit: None,
        },
    );

    let mut ast = Ast::new();
    ast.push(var_node("f"));
    ast.push(var_node("S"));
    ast.push(var_node("g"));
    ast.push(with_children("dim_arrow", vec![0, 1, 2]));

    let term = lower_ast(&ast, &lang, AstRef::new(3)).expect("lower should succeed");
    assert_eq!(
        term,
        Term::Op {
            dim: Box::new(Term::named_dim("S")),
            name: "tag.construct".into(),
            args: vec![
                arr_key("Arrow"),
                Term::op_native(
                    "record",
                    vec![
                        arr_key("inputs"),
                        Term::op_native("f", vec![]),
                        arr_key("grade"),
                        Term::named_dim("S"),
                        arr_key("output"),
                        Term::op_native("g", vec![]),
                    ],
                ),
            ],
            source: ArrSource::Native,
            lit: None,
        }
    );
}

/// `if` keeps its Rust arm even when `eval.if` is present: the eval
/// scheme feeds the *condition value* into the branches, while the arrow
/// lowering feeds the *original input* via `product`/`pair_tag`.
#[test]
fn if_keeps_pair_tag_lowering_over_eval_scheme() {
    let mut lang = Language::new();
    merge_roles(&mut lang, "if", &["condition", "then", "else"]);
    // The canonical eval.if shape — present in the language but not the
    // arrow lowering.
    merge_scheme(
        &mut lang,
        "if",
        Term::seq(
            Term::seq(proj(0), Term::op_native("tag", vec![])),
            Term::choice(
                proj(1),
                Term::op_native_lit("proj_child_or", vec![], Value::from(2u64)),
            ),
        ),
    );

    let mut ast = Ast::new();
    ast.push(value_node(json!(true)));
    ast.push(value_node(json!("yes")));
    ast.push(value_node(json!("no")));
    ast.push(with_children("if", vec![0, 1, 2]));

    let term = lower_ast(&ast, &lang, AstRef::new(3)).expect("lower should succeed");
    assert_eq!(
        term,
        Term::seq(
            Term::op_native(
                "product",
                vec![
                    Term::const_value(json!(true)),
                    Term::op_native("id", vec![])
                ]
            ),
            Term::seq(
                Term::op_native("pair_tag", vec![]),
                Term::choice(
                    Term::const_value(json!("yes")),
                    Term::const_value(json!("no"))
                )
            )
        )
    );
}

/// A scheme whose placeholders sit in non-input positions (`Choice`
/// branches are fed the Either value, not the node input) cannot be
/// instantiated honestly — the lowerer declines so the runtime falls
/// back to the AST engine.
#[test]
fn scheme_with_placeholder_in_choice_branch_declines() {
    let mut lang = Language::new();
    merge_scheme(&mut lang, "fakebranch", Term::choice(proj(0), proj(1)));

    let mut ast = Ast::new();
    ast.push(value_node(json!("l")));
    ast.push(value_node(json!("r")));
    ast.push(with_children("fakebranch", vec![0, 1]));

    let err = lower_ast(&ast, &lang, AstRef::new(2)).expect_err("should decline");
    assert!(
        err.contains("non-input position"),
        "unexpected error: {err}"
    );
}

/// `return` declines: early-exit control flow is `eval_rec` interception
/// semantics the substitution model cannot express.
#[test]
fn return_declines_early_exit() {
    let mut ast = Ast::new();
    ast.push(value_node(json!(1)));
    ast.push(with_children("return", vec![0]));

    assert!(lower_ast(&ast, &Language::new(), AstRef::new(1)).is_err());
}

#[test]
fn handle_arrow_with_handler_lowers_to_term_handle() {
    // `handle read with NumberHandler` should lower to `Term::Handle`.
    //
    // Arena layout:
    //   0: var "read" (body arrow)
    //   1: var "NumberHandler" (handler)
    //   2: handle [0, 1] with payload.name = "read"
    let mut ast = Ast::new();
    ast.push(var_node("read"));
    ast.push(var_node("NumberHandler"));
    ast.push(with_children_and_name("handle", vec![0, 1], "read"));

    let term = lower_ast(&ast, &Language::new(), AstRef::new(2)).expect("lower should succeed");
    assert_eq!(
        term,
        Term::Handle {
            body: Box::new(Term::op_native("read", vec![])),
            target: "read".to_string(),
            impl_: Box::new(Term::op_native("NumberHandler", vec![])),
        }
    );
}
