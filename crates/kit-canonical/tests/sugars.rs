//! Integration tests for the comparison, arithmetic, and arrow sugars.
//!
//! These verify that sugar operators parse correctly with the data-driven
//! Pratt parser. The data-driven AST uses an arena with child-index payloads
//! (`{"children": [idx, ...]}`), so the helpers here resolve child references
//! through the arena rather than navigating nested payload objects.

use handl_kit_canonical::Kit;
use handl_language::{Ast, AstNode};

/// Parse `input` and return the root AST node (the last node in the arena).
fn parse_root(input: &str) -> AstNode {
    let kit = Kit::new().expect("kit assembly should succeed");
    let ast = kit.parse(input).expect("parse should succeed");
    ast.nodes()
        .last()
        .expect("parse should produce at least one node")
        .clone()
}

/// Parse `input` and return the full arena (for child resolution).
fn parse_ast(input: &str) -> Ast {
    let kit = Kit::new().expect("kit assembly should succeed");
    kit.parse(input).expect("parse should succeed")
}

/// Resolve a child index from a compound node's `{"children": [...]}` payload.
fn child(ast: &Ast, node: &AstNode, n: usize) -> Option<AstNode> {
    let idx = node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.get(n))
        .and_then(|v| v.as_u64())?;
    ast.nodes().get(idx as usize).cloned()
}

/// Get the number of children of a compound node.
fn child_count(node: &AstNode) -> Option<usize> {
    node.payload
        .get("children")
        .and_then(|c| c.as_array())
        .map(|a| a.len())
}

#[test]
fn eq_sugar_parses() {
    let ast = parse_ast("1 == 1");
    let root = parse_root("1 == 1");
    assert!(
        child_count(&root).unwrap_or(0) >= 2,
        "eq should have 2 children"
    );
    let _ = ast;
}

#[test]
fn ne_sugar_parses() {
    let root = parse_root("1 != 2");
    assert!(child_count(&root).unwrap_or(0) >= 2);
}

#[test]
fn lt_sugar_parses() {
    let root = parse_root("1 < 2");
    assert!(child_count(&root).unwrap_or(0) >= 2);
}

#[test]
fn le_sugar_parses() {
    let root = parse_root("2 <= 2");
    assert!(child_count(&root).unwrap_or(0) >= 2);
}

#[test]
fn gt_sugar_parses() {
    let root = parse_root("2 > 1");
    assert!(child_count(&root).unwrap_or(0) >= 2);
}

#[test]
fn ge_sugar_parses() {
    let root = parse_root("2 >= 2");
    assert!(child_count(&root).unwrap_or(0) >= 2);
}

#[test]
fn add_sugar_parses() {
    let root = parse_root("1 + 2");
    assert_eq!(child_count(&root), Some(2));
}

#[test]
fn sub_sugar_parses() {
    let root = parse_root("5 - 3");
    assert_eq!(child_count(&root), Some(2));
}

#[test]
fn mul_sugar_parses() {
    let root = parse_root("4 * 3");
    assert_eq!(child_count(&root), Some(2));
}

#[test]
fn div_sugar_parses() {
    let root = parse_root("12 / 3");
    assert_eq!(child_count(&root), Some(2));
}

#[test]
fn pow_sugar_parses() {
    let root = parse_root("2 ** 3");
    assert_eq!(child_count(&root), Some(2));
}

#[test]
fn pow_is_right_associative() {
    // 2 ** 3 ** 2 should parse as 2 ** (3 ** 2)
    let ast = parse_ast("2 ** 3 ** 2");
    let root = ast.nodes().last().unwrap();
    assert_eq!(child_count(root), Some(2));
    // The right child should itself be a pow (2 children).
    let right = child(&ast, root, 1).unwrap();
    assert_eq!(child_count(&right), Some(2));
}

#[test]
fn mul_binds_tighter_than_add() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3)
    let ast = parse_ast("1 + 2 * 3");
    let root = ast.nodes().last().unwrap();
    assert_eq!(child_count(root), Some(2));
    // The right child should be a mul (2 children).
    let right = child(&ast, root, 1).unwrap();
    assert_eq!(child_count(&right), Some(2));
}

#[test]
fn cmp_binds_looser_than_arith() {
    // 1 + 2 == 3 should parse as (1 + 2) == 3
    let ast = parse_ast("1 + 2 == 3");
    let root = ast.nodes().last().unwrap();
    assert_eq!(child_count(root), Some(2));
    // The left child should be an add (2 children).
    let left = child(&ast, root, 0).unwrap();
    assert_eq!(child_count(&left), Some(2));
}
