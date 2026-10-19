use handl_kit_prelude::Kit;
use handl_language::{Ast, AstNode};
use serde_json::Value;

#[test]
fn erased_node_evaluates_to_null() {
    let kit = Kit::new().expect("kit must assemble");
    let mut ast = Ast::new();
    ast.push(AstNode::new("erased").with_payload(Value::Null));
    let val = kit.runtime().eval(&ast).expect("eval must succeed");
    assert!(
        val.is_null(),
        "erased node must evaluate to null, got {:?}",
        val
    );
}
