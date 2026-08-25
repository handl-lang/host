//! Runtime and typecheck tests for the canonical expression kit.

use handl_kit_canonical::CanonicalKit;
use handl_vm_values_json::NonFunctionValue;

fn kit() -> CanonicalKit {
    CanonicalKit::new()
}

fn eval(input: &str) -> Vec<NonFunctionValue> {
    let kit = kit();
    let output = kit.parse_module_recovered(input);
    assert!(output.diagnostics.is_empty(), "parse errors: {:?}", output.diagnostics);
    kit.eval(&output.stmts).unwrap().values
}

fn last(values: Vec<NonFunctionValue>) -> NonFunctionValue {
    values.last().cloned().expect("eval produced no values")
}

fn typecheck(input: &str) -> bool {
    let kit = kit();
    let output = kit.parse_module_recovered(input);
    assert!(output.diagnostics.is_empty(), "parse errors: {:?}", output.diagnostics);
    kit.typecheck(&output.stmts).unwrap().success
}

// --- literals ---

#[test]
fn number_literal() {
    assert_eq!(last(eval("1")).as_number(), Some(1.0));
}

#[test]
fn boolean_literals() {
    assert_eq!(last(eval("true")).as_bool(), Some(true));
    assert_eq!(last(eval("false")).as_bool(), Some(false));
}

#[test]
fn string_literal() {
    assert_eq!(last(eval("\"hello\"")).as_str(), Some("hello"));
}

#[test]
fn null_literal() {
    assert!(last(eval("null")).is_null());
}

// --- arithmetic (canonical call syntax) ---

#[test]
fn arithmetic_add() {
    assert_eq!(last(eval("add(1, 2)")).as_number(), Some(3.0));
}

#[test]
fn arithmetic_sub() {
    assert_eq!(last(eval("sub(3, 2)")).as_number(), Some(1.0));
}

#[test]
fn arithmetic_mul() {
    assert_eq!(last(eval("mul(2, 3)")).as_number(), Some(6.0));
}

#[test]
fn arithmetic_div() {
    assert_eq!(last(eval("div(10, 2)")).as_number(), Some(5.0));
}

// --- boolean logic ---

#[test]
fn boolean_and() {
    assert_eq!(last(eval("and(true, false)")).as_bool(), Some(false));
}

#[test]
fn boolean_or() {
    assert_eq!(last(eval("or(true, false)")).as_bool(), Some(true));
}

#[test]
fn boolean_not() {
    assert_eq!(last(eval("not(true)")).as_bool(), Some(false));
    assert_eq!(last(eval("not false")).as_bool(), Some(true));
}

// --- comparison ---

#[test]
fn comparison_eq() {
    assert_eq!(last(eval("eq(1, 1)")).as_bool(), Some(true));
}

#[test]
fn comparison_lt() {
    assert_eq!(last(eval("lt(1, 2)")).as_bool(), Some(true));
}

// --- arrays and objects ---

#[test]
fn array_literal() {
    let v = last(eval("[1, 2, 3]"));
    assert!(v.is_array());
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0].as_f64(), Some(1.0));
    assert_eq!(arr[1].as_f64(), Some(2.0));
    assert_eq!(arr[2].as_f64(), Some(3.0));
}

#[test]
fn object_field_access() {
    assert_eq!(last(eval("{x: 1}.x")).as_number(), Some(1.0));
}

// --- bindings ---

#[test]
fn let_binding() {
    assert_eq!(last(eval("let a = 1; a")).as_number(), Some(1.0));
}

#[test]
fn const_binding() {
    assert_eq!(last(eval("const a = 1; a")).as_number(), Some(1.0));
}

// --- functions and pipes ---

#[test]
fn function_application() {
    assert_eq!(last(eval("f(x) -> x; f(1)")).as_number(), Some(1.0));
}

#[test]
fn pipe_forward() {
    assert_eq!(last(eval("f(x) -> x; 1 |> f")).as_number(), Some(1.0));
}

// --- typecheck ---

#[test]
fn typecheck_add_number_string_is_error() {
    assert!(!typecheck("add(\"hello\", 1)"));
}

#[test]
fn typecheck_add_number_bool_is_error() {
    assert!(!typecheck("add(1, true)"));
}

#[test]
fn typecheck_let_then_use_is_ok() {
    assert!(typecheck("let a = 1; a"));
}

#[test]
fn typecheck_function_literal_is_ok() {
    assert!(typecheck("f(x) -> x"));
}

#[test]
fn typecheck_function_call_is_ok() {
    assert!(typecheck("f(x) -> x; f(1)"));
}

#[test]
fn typecheck_undefined_function_is_error() {
    assert!(!typecheck("f(1)"));
}
