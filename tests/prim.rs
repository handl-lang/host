use handl_platform::{EvalContext, Module};
use handl_plugin_lib_stdlib::module_ref;

fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let func = module.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx).expect("call succeeds")
}

fn n(v: i64) -> serde_json::Value {
    serde_json::Value::from(v)
}

fn s(v: &str) -> serde_json::Value {
    serde_json::Value::from(v.to_string())
}

#[test]
fn module_name() {
    assert_eq!(module_ref("prim").unwrap().name(), "prim");
}

#[test]
fn unknown_function() {
    assert!(module_ref("prim").unwrap().function("nope").is_none());
}

#[test]
fn add_positive() {
    assert_eq!(call(module_ref("prim").unwrap(), "add", &[n(2), n(3)]).as_i64(), Some(5));
}

#[test]
fn add_negative() {
    assert_eq!(call(module_ref("prim").unwrap(), "add", &[n(-4), n(7)]).as_i64(), Some(3));
}

#[test]
fn add_zero() {
    assert_eq!(call(module_ref("prim").unwrap(), "add", &[n(0), n(0)]).as_i64(), Some(0));
}

#[test]
fn compare_less() {
    assert_eq!(
        call(module_ref("prim").unwrap(), "compare", &[n(1), n(2)]).as_i64(),
        Some(-1)
    );
}

#[test]
fn compare_equal() {
    assert_eq!(
        call(module_ref("prim").unwrap(), "compare", &[n(2), n(2)]).as_i64(),
        Some(0)
    );
}

#[test]
fn compare_greater() {
    assert_eq!(
        call(module_ref("prim").unwrap(), "compare", &[n(3), n(1)]).as_i64(),
        Some(1)
    );
}

#[test]
fn compare_strings() {
    assert_eq!(
        call(module_ref("prim").unwrap(), "compare", &[s("a"), s("b")]).as_i64(),
        Some(-1)
    );
}

#[test]
fn alloc_returns_empty_array() {
    let result = call(module_ref("prim").unwrap(), "alloc", &[n(5)]);
    assert!(result.is_array());
    assert_eq!(result.as_array().unwrap().len(), 0);
}

#[test]
fn alloc_zero_returns_empty_array() {
    let result = call(module_ref("prim").unwrap(), "alloc", &[n(0)]);
    assert!(result.is_array());
    assert_eq!(result.as_array().unwrap().len(), 0);
}

#[test]
fn or_basic() {
    assert_eq!(
        call(module_ref("prim").unwrap(), "or", &[n(0b0011), n(0b0101)]).as_i64(),
        Some(0b0111)
    );
}

#[test]
fn or_with_zero() {
    assert_eq!(
        call(module_ref("prim").unwrap(), "or", &[n(0b1010), n(0)]).as_i64(),
        Some(0b1010)
    );
}

#[test]
fn or_all_bits() {
    assert_eq!(call(module_ref("prim").unwrap(), "or", &[n(-1), n(0)]).as_i64(), Some(-1));
}
