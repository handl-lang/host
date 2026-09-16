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
    // source is the first non-`Op` grade, `SomeGrade`.
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
            dim: Term::default_dim(),
            name: "x".to_string(),
            args: vec![],
            source: ArrSource::Foreign("SomeGrade".to_string()),
            lit: None,
        }
    );
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
