//! End-to-end integration tests for the generic canonical kit.

use handl_kit_canonical::{CanonicalKit, KitNode, KitValue};

#[test]
fn parse_number_literal() {
    let kit = CanonicalKit::new();
    let node = kit.parse("42").unwrap();
    assert!(matches!(node, KitNode::Literal(KitValue::Number(42.0))));
}

#[test]
fn parse_string_literal() {
    let kit = CanonicalKit::new();
    let node = kit.parse(r#""hi""#).unwrap();
    assert!(matches!(node, KitNode::Literal(KitValue::String(s)) if s == "hi"));
}

#[test]
fn parse_bool_and_null_literals() {
    let kit = CanonicalKit::new();
    assert!(matches!(kit.parse("true").unwrap(), KitNode::Literal(KitValue::Bool(true))));
    assert!(matches!(kit.parse("false").unwrap(), KitNode::Literal(KitValue::Bool(false))));
    assert!(matches!(kit.parse("null").unwrap(), KitNode::Literal(KitValue::Null)));
}

#[test]
fn parse_call() {
    let kit = CanonicalKit::new();
    let node = kit.parse("add(1, 2)").unwrap();
    assert!(matches!(node, KitNode::Call(_)));
}

#[test]
fn parse_index() {
    let kit = CanonicalKit::new();
    let node = kit.parse("arr[0]").unwrap();
    assert!(matches!(node, KitNode::Index(_)));
}

#[test]
fn parse_pipe() {
    let kit = CanonicalKit::new();
    let node = kit.parse("1 |> add(2, 3)").unwrap();
    assert!(matches!(node, KitNode::Pipe(_)));
}

#[test]
fn parse_let() {
    let kit = CanonicalKit::new();
    let node = kit.parse("let a = 1;").unwrap();
    assert!(matches!(node, KitNode::Let(_)));
}
