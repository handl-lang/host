//! Integration tests for the comparison, arithmetic, and arrow sugars.
//!
//! These verify that the sugar operators parse and desugar to the expected
//! `cmp.*`, `math.*`, and `fn.*` call AST nodes at parse time.

use handl_kit_canonical::Kit;
use handl_platform::AstNode;

fn parse(input: &str) -> AstNode {
    let kit = Kit::new();
    kit.parse_to_ast(input).expect("parse should succeed")
}

fn callee_name(node: &AstNode) -> Option<String> {
    if node.kind != "call" {
        return None;
    }
    let callee = node.payload.get("callee")?;
    if callee.get("kind").and_then(|v| v.as_str()) == Some("index") {
        let target = callee
            .get("payload")
            .and_then(|p| p.get("target"))
            .and_then(|t| t.get("payload"))
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())?;
        let index = callee
            .get("payload")
            .and_then(|p| p.get("index"))
            .and_then(|t| t.get("payload"))
            .and_then(|p| p.get("value"))
            .and_then(|v| v.as_str())?;
        return Some(format!("{target}.{index}"));
    }
    None
}

fn arg_count(node: &AstNode) -> Option<usize> {
    node.payload
        .get("args")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
}

#[test]
fn eq_sugar_desugars_to_cmp_eq() {
    let ast = parse("1 == 1");
    assert_eq!(ast.kind, "call");
    assert_eq!(callee_name(&ast).as_deref(), Some("cmp.eq"));
    assert_eq!(arg_count(&ast), Some(2));
}

#[test]
fn ne_sugar_desugars_to_cmp_ne() {
    let ast = parse("1 != 2");
    assert_eq!(callee_name(&ast).as_deref(), Some("cmp.ne"));
}

#[test]
fn lt_sugar_desugars_to_cmp_lt() {
    let ast = parse("1 < 2");
    assert_eq!(callee_name(&ast).as_deref(), Some("cmp.lt"));
}

#[test]
fn le_sugar_desugars_to_cmp_le() {
    let ast = parse("2 <= 2");
    assert_eq!(callee_name(&ast).as_deref(), Some("cmp.le"));
}

#[test]
fn gt_sugar_desugars_to_cmp_gt() {
    let ast = parse("2 > 1");
    assert_eq!(callee_name(&ast).as_deref(), Some("cmp.gt"));
}

#[test]
fn ge_sugar_desugars_to_cmp_ge() {
    let ast = parse("2 >= 2");
    assert_eq!(callee_name(&ast).as_deref(), Some("cmp.ge"));
}

#[test]
fn spaceship_sugar_desugars_to_cmp_compare() {
    let ast = parse("1 <=> 2");
    assert_eq!(callee_name(&ast).as_deref(), Some("cmp.compare"));
}

#[test]
fn add_sugar_desugars_to_math_add() {
    let ast = parse("1 + 2");
    assert_eq!(callee_name(&ast).as_deref(), Some("math.add"));
    assert_eq!(arg_count(&ast), Some(2));
}

#[test]
fn sub_sugar_desugars_to_math_sub() {
    let ast = parse("5 - 3");
    assert_eq!(callee_name(&ast).as_deref(), Some("math.sub"));
}

#[test]
fn mul_sugar_desugars_to_math_mul() {
    let ast = parse("4 * 3");
    assert_eq!(callee_name(&ast).as_deref(), Some("math.mul"));
}

#[test]
fn div_sugar_desugars_to_math_div() {
    let ast = parse("12 / 3");
    assert_eq!(callee_name(&ast).as_deref(), Some("math.div"));
}

#[test]
fn pow_sugar_desugars_to_math_pow() {
    let ast = parse("2 ** 3");
    assert_eq!(callee_name(&ast).as_deref(), Some("math.pow"));
}

#[test]
fn flow_sugar_desugars_to_fn_flow() {
    let ast = parse("(x) -> x + 1 >>> (y) -> y * 2");
    assert_eq!(callee_name(&ast).as_deref(), Some("fn.flow"));
    assert_eq!(arg_count(&ast), Some(2));
}

#[test]
fn fanout_sugar_desugars_to_fn_fanout_fn() {
    let ast = parse("(x) -> x + 1 &&& (y) -> y * 2");
    assert_eq!(callee_name(&ast).as_deref(), Some("fn.fanout_fn"));
    assert_eq!(arg_count(&ast), Some(2));
}

#[test]
fn pow_is_right_associative() {
    // 2 ** 3 ** 2 should parse as 2 ** (3 ** 2)
    let ast = parse("2 ** 3 ** 2");
    assert_eq!(callee_name(&ast).as_deref(), Some("math.pow"));
    let args = ast.payload.get("args").and_then(|v| v.as_array()).unwrap();
    // The right arg should itself be a math.pow call (nested).
    let right = &args[1];
    assert_eq!(right.get("kind").and_then(|v| v.as_str()), Some("call"));
    assert_eq!(
        right
            .get("payload")
            .and_then(|p| p.get("callee"))
            .and_then(|c| c.get("payload"))
            .and_then(|p| p.get("index"))
            .and_then(|i| i.get("payload"))
            .and_then(|p| p.get("value"))
            .and_then(|v| v.as_str()),
        Some("pow")
    );
}

#[test]
fn mul_binds_tighter_than_add() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3)
    let ast = parse("1 + 2 * 3");
    assert_eq!(callee_name(&ast).as_deref(), Some("math.add"));
    let args = ast.payload.get("args").and_then(|v| v.as_array()).unwrap();
    // The right arg should be a math.mul call.
    let right = &args[1];
    assert_eq!(right.get("kind").and_then(|v| v.as_str()), Some("call"));
    assert_eq!(
        right
            .get("payload")
            .and_then(|p| p.get("callee"))
            .and_then(|c| c.get("payload"))
            .and_then(|p| p.get("index"))
            .and_then(|i| i.get("payload"))
            .and_then(|p| p.get("value"))
            .and_then(|v| v.as_str()),
        Some("mul")
    );
}

#[test]
fn cmp_binds_looser_than_arith() {
    // 1 + 2 == 3 should parse as (1 + 2) == 3
    let ast = parse("1 + 2 == 3");
    assert_eq!(callee_name(&ast).as_deref(), Some("cmp.eq"));
    let args = ast.payload.get("args").and_then(|v| v.as_array()).unwrap();
    // The left arg should be a math.add call.
    let left = &args[0];
    assert_eq!(left.get("kind").and_then(|v| v.as_str()), Some("call"));
    assert_eq!(
        left.get("payload")
            .and_then(|p| p.get("callee"))
            .and_then(|c| c.get("payload"))
            .and_then(|p| p.get("index"))
            .and_then(|i| i.get("payload"))
            .and_then(|p| p.get("value"))
            .and_then(|v| v.as_str()),
        Some("add")
    );
}

#[test]
fn fanout_binds_tighter_than_flow() {
    // f &&& g >>> h should parse as (f &&& g) >>> h
    let ast = parse("(x) -> x >>> (y) -> y &&& (z) -> z");
    assert_eq!(callee_name(&ast).as_deref(), Some("fn.flow"));
    let args = ast.payload.get("args").and_then(|v| v.as_array()).unwrap();
    // The right arg should be a fn.fanout_fn call.
    let right = &args[1];
    assert_eq!(right.get("kind").and_then(|v| v.as_str()), Some("call"));
    assert_eq!(
        right
            .get("payload")
            .and_then(|p| p.get("callee"))
            .and_then(|c| c.get("payload"))
            .and_then(|p| p.get("index"))
            .and_then(|i| i.get("payload"))
            .and_then(|p| p.get("value"))
            .and_then(|v| v.as_str()),
        Some("fanout_fn")
    );
}
