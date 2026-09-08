use handl_platform::Function;
use handl_platform::Module;

use handl_platform::{EvalContext, EvalError};
use handl_plugin_lib_stdlib::module_ref;

fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let func = module.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx).expect("call succeeds")
}

fn call_err(
    module: &dyn Module,
    name: &str,
    args: &[serde_json::Value],
) -> Result<serde_json::Value, EvalError> {
    let func = module.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx)
}

fn s(v: &str) -> serde_json::Value {
    serde_json::Value::from(v.to_string())
}

fn n(v: f64) -> serde_json::Value {
    serde_json::Value::from(serde_json::json!(v))
}

#[test]
fn module_name() {
    assert_eq!(module_ref("math").unwrap().name(), "math");
}

#[test]
fn abs_positive() {
    assert_eq!(
        call(module_ref("math").unwrap(), "abs", &[n(5.0)]).as_f64(),
        Some(5.0)
    );
}

#[test]
fn abs_negative() {
    assert_eq!(
        call(module_ref("math").unwrap(), "abs", &[n(-5.0)]).as_f64(),
        Some(5.0)
    );
}

#[test]
fn floor() {
    assert_eq!(
        call(module_ref("math").unwrap(), "floor", &[n(3.7)]).as_f64(),
        Some(3.0)
    );
}

#[test]
fn ceil() {
    assert_eq!(
        call(module_ref("math").unwrap(), "ceil", &[n(3.2)]).as_f64(),
        Some(4.0)
    );
}

#[test]
fn round() {
    assert_eq!(
        call(module_ref("math").unwrap(), "round", &[n(3.5)]).as_f64(),
        Some(4.0)
    );
    assert_eq!(
        call(module_ref("math").unwrap(), "round", &[n(3.4)]).as_f64(),
        Some(3.0)
    );
}

#[test]
fn min() {
    assert_eq!(
        call(module_ref("math").unwrap(), "min", &[n(3.0), n(7.0)]).as_f64(),
        Some(3.0)
    );
}

#[test]
fn max() {
    assert_eq!(
        call(module_ref("math").unwrap(), "max", &[n(3.0), n(7.0)]).as_f64(),
        Some(7.0)
    );
}

#[test]
fn sqrt() {
    assert_eq!(
        call(module_ref("math").unwrap(), "sqrt", &[n(9.0)]).as_f64(),
        Some(3.0)
    );
}

#[test]
fn pow() {
    assert_eq!(
        call(module_ref("math").unwrap(), "pow", &[n(2.0), n(3.0)]).as_f64(),
        Some(8.0)
    );
}

#[test]
fn sign_positive() {
    assert_eq!(
        call(module_ref("math").unwrap(), "sign", &[n(5.0)]).as_f64(),
        Some(1.0)
    );
}

#[test]
fn sign_negative() {
    assert_eq!(
        call(module_ref("math").unwrap(), "sign", &[n(-5.0)]).as_f64(),
        Some(-1.0)
    );
}

#[test]
fn sign_zero() {
    assert_eq!(
        call(module_ref("math").unwrap(), "sign", &[n(0.0)]).as_f64(),
        Some(1.0)
    );
}

#[test]
fn trunc() {
    assert_eq!(
        call(module_ref("math").unwrap(), "trunc", &[n(3.7)]).as_f64(),
        Some(3.0)
    );
    assert_eq!(
        call(module_ref("math").unwrap(), "trunc", &[n(-3.7)]).as_f64(),
        Some(-3.0)
    );
}

#[test]
fn clamp_within_range() {
    assert_eq!(
        call(
            module_ref("math").unwrap(),
            "clamp",
            &[n(5.0), n(0.0), n(10.0)]
        )
        .as_f64(),
        Some(5.0)
    );
}

#[test]
fn clamp_below_min() {
    assert_eq!(
        call(
            module_ref("math").unwrap(),
            "clamp",
            &[n(-5.0), n(0.0), n(10.0)]
        )
        .as_f64(),
        Some(0.0)
    );
}

#[test]
fn clamp_above_max() {
    assert_eq!(
        call(
            module_ref("math").unwrap(),
            "clamp",
            &[n(15.0), n(0.0), n(10.0)]
        )
        .as_f64(),
        Some(10.0)
    );
}

#[test]
fn log() {
    let result = call(
        module_ref("math").unwrap(),
        "log",
        &[n(std::f64::consts::E)],
    )
    .as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - 1.0).abs() < 1e-10);
}

#[test]
fn log10() {
    assert_eq!(
        call(module_ref("math").unwrap(), "log10", &[n(100.0)]).as_f64(),
        Some(2.0)
    );
}

#[test]
fn exp() {
    let result = call(module_ref("math").unwrap(), "exp", &[n(1.0)]).as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - std::f64::consts::E).abs() < 1e-10);
}

#[test]
fn sin() {
    let result = call(module_ref("math").unwrap(), "sin", &[n(0.0)]).as_f64();
    assert!(result.is_some());
    assert!(result.unwrap().abs() < 1e-10);
}

#[test]
fn cos() {
    let result = call(module_ref("math").unwrap(), "cos", &[n(0.0)]).as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - 1.0).abs() < 1e-10);
}

#[test]
fn tan() {
    let result = call(module_ref("math").unwrap(), "tan", &[n(0.0)]).as_f64();
    assert!(result.is_some());
    assert!(result.unwrap().abs() < 1e-10);
}

#[test]
fn asin() {
    let result = call(module_ref("math").unwrap(), "asin", &[n(1.0)]).as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - std::f64::consts::FRAC_PI_2).abs() < 1e-10);
}

#[test]
fn acos() {
    let result = call(module_ref("math").unwrap(), "acos", &[n(1.0)]).as_f64();
    assert!(result.is_some());
    assert!(result.unwrap().abs() < 1e-10);
}

#[test]
fn atan() {
    let result = call(module_ref("math").unwrap(), "atan", &[n(1.0)]).as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - std::f64::consts::FRAC_PI_4).abs() < 1e-10);
}

#[test]
fn atan2() {
    let result = call(module_ref("math").unwrap(), "atan2", &[n(1.0), n(1.0)]).as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - std::f64::consts::FRAC_PI_4).abs() < 1e-10);
}

#[test]
fn hypot() {
    let result = call(module_ref("math").unwrap(), "hypot", &[n(3.0), n(4.0)]).as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - 5.0).abs() < 1e-10);
}

#[test]
fn mod_op() {
    assert_eq!(
        call(module_ref("math").unwrap(), "mod", &[n(10.0), n(3.0)]).as_f64(),
        Some(1.0)
    );
}

#[test]
fn pi() {
    let result = call(module_ref("math").unwrap(), "pi", &[]).as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - std::f64::consts::PI).abs() < 1e-10);
}

#[test]
fn e() {
    let result = call(module_ref("math").unwrap(), "e", &[]).as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - std::f64::consts::E).abs() < 1e-10);
}

#[test]
fn log2() {
    assert_eq!(
        call(module_ref("math").unwrap(), "log2", &[n(8.0)]).as_f64(),
        Some(3.0)
    );
}

#[test]
fn cbrt() {
    assert_eq!(
        call(module_ref("math").unwrap(), "cbrt", &[n(27.0)]).as_f64(),
        Some(3.0)
    );
}

#[test]
fn degrees() {
    let result = call(
        module_ref("math").unwrap(),
        "degrees",
        &[n(std::f64::consts::PI)],
    )
    .as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - 180.0).abs() < 1e-10);
}

#[test]
fn radians() {
    let result = call(module_ref("math").unwrap(), "radians", &[n(180.0)]).as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - std::f64::consts::PI).abs() < 1e-10);
}

#[test]
fn round_to_two_decimals() {
    let result = call(
        module_ref("math").unwrap(),
        "round_to",
        &[n(3.14159), n(2.0)],
    )
    .as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - 3.14).abs() < 1e-10);
}

#[test]
fn round_to_zero_decimals() {
    let result = call(module_ref("math").unwrap(), "round_to", &[n(3.7), n(0.0)]).as_f64();
    assert_eq!(result, Some(4.0));
}

#[test]
fn round_to_three_decimals() {
    let result = call(
        module_ref("math").unwrap(),
        "round_to",
        &[n(2.71828), n(3.0)],
    )
    .as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - 2.718).abs() < 1e-10);
}

#[test]
fn round_to_negative_errors() {
    let func = module_ref("math").unwrap().function("round_to").unwrap();
    let mut ctx = EvalContext::new();
    let result = func.call(&[n(1.5), n(-1.0)], &mut ctx);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn unknown_function_returns_none() {
    assert!(module_ref("math").unwrap().function("nope").is_none());
}

#[test]
fn type_mismatch_on_non_number() {
    let func = module_ref("math").unwrap().function("abs").unwrap();
    let mut ctx = EvalContext::new();
    let result = func.call(&[serde_json::Value::from("not a number")], &mut ctx);
    assert_eq!(
        result,
        Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "number", "non-number"
        )))
    );
}

#[test]
fn argument_count_on_missing_args() {
    let func = module_ref("math").unwrap().function("abs").unwrap();
    let mut ctx = EvalContext::new();
    let result = func.call(&[], &mut ctx);
    assert_eq!(
        result,
        Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0
        )))
    );
}

// ── additional edge-case tests ─────────────────────────────────────

// Unary functions: missing args and type mismatch for all

#[test]
fn floor_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "floor", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn floor_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "floor", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn ceil_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "ceil", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn ceil_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "ceil", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn round_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "round", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn round_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "round", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn sqrt_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "sqrt", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn sqrt_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "sqrt", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn sign_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "sign", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn sign_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "sign", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn sign_negative_edge() {
    assert_eq!(
        call(module_ref("math").unwrap(), "sign", &[n(-5.0)]).as_f64(),
        Some(-1.0)
    );
}

#[test]
fn sign_zero_edge() {
    assert_eq!(
        call(module_ref("math").unwrap(), "sign", &[n(0.0)]).as_f64(),
        Some(1.0)
    );
}

#[test]
fn trunc_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "trunc", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn trunc_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "trunc", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn log_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "log", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn log_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "log", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn log10_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "log10", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn log10_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "log10", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn exp_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "exp", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn exp_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "exp", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn sin_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "sin", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn sin_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "sin", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn cos_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "cos", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn cos_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "cos", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn tan_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "tan", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn tan_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "tan", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn asin_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "asin", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn asin_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "asin", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn acos_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "acos", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn acos_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "acos", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn atan_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "atan", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn atan_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "atan", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn log2_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "log2", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn log2_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "log2", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn cbrt_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "cbrt", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn cbrt_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "cbrt", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn cbrt_negative() {
    assert_eq!(
        call(module_ref("math").unwrap(), "cbrt", &[n(-27.0)]).as_f64(),
        Some(-3.0)
    );
}

#[test]
fn degrees_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "degrees", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn degrees_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "degrees", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn radians_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "radians", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn radians_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "radians", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

// Binary functions: missing args and type mismatch

#[test]
fn min_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "min", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn min_one_arg_errors() {
    let result = call_err(module_ref("math").unwrap(), "min", &[n(1.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn min_first_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "min", &[s("x"), n(1.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn min_second_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "min", &[n(1.0), s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn max_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "max", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn max_one_arg_errors() {
    let result = call_err(module_ref("math").unwrap(), "max", &[n(1.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn max_first_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "max", &[s("x"), n(1.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn max_second_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "max", &[n(1.0), s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn pow_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "pow", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn pow_one_arg_errors() {
    let result = call_err(module_ref("math").unwrap(), "pow", &[n(2.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn pow_first_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "pow", &[s("x"), n(2.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn pow_second_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "pow", &[n(2.0), s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn atan2_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "atan2", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn atan2_one_arg_errors() {
    let result = call_err(module_ref("math").unwrap(), "atan2", &[n(1.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn atan2_first_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "atan2", &[s("x"), n(1.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn atan2_second_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "atan2", &[n(1.0), s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn hypot_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "hypot", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn hypot_one_arg_errors() {
    let result = call_err(module_ref("math").unwrap(), "hypot", &[n(3.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn hypot_first_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "hypot", &[s("x"), n(4.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn hypot_second_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "hypot", &[n(3.0), s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn mod_op_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "mod", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn mod_op_one_arg_errors() {
    let result = call_err(module_ref("math").unwrap(), "mod", &[n(10.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn mod_op_first_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "mod", &[s("x"), n(3.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn mod_op_second_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "mod", &[n(10.0), s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

// Clamp: 3-arg function

#[test]
fn clamp_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "clamp", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn clamp_one_arg_errors() {
    let result = call_err(module_ref("math").unwrap(), "clamp", &[n(5.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn clamp_two_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "clamp", &[n(5.0), n(0.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn clamp_first_arg_non_number_errors() {
    let result = call_err(
        module_ref("math").unwrap(),
        "clamp",
        &[s("x"), n(0.0), n(10.0)],
    );
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn clamp_second_arg_non_number_errors() {
    let result = call_err(
        module_ref("math").unwrap(),
        "clamp",
        &[n(5.0), s("x"), n(10.0)],
    );
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn clamp_third_arg_non_number_errors() {
    let result = call_err(
        module_ref("math").unwrap(),
        "clamp",
        &[n(5.0), n(0.0), s("x")],
    );
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn clamp_edge_values() {
    assert_eq!(
        call(
            module_ref("math").unwrap(),
            "clamp",
            &[n(5.0), n(0.0), n(10.0)]
        )
        .as_f64(),
        Some(5.0)
    );
}

#[test]
fn clamp_below_min_edge() {
    assert_eq!(
        call(
            module_ref("math").unwrap(),
            "clamp",
            &[n(-3.0), n(0.0), n(10.0)]
        )
        .as_f64(),
        Some(0.0)
    );
}

#[test]
fn clamp_above_max_edge() {
    assert_eq!(
        call(
            module_ref("math").unwrap(),
            "clamp",
            &[n(15.0), n(0.0), n(10.0)]
        )
        .as_f64(),
        Some(10.0)
    );
}

// RoundTo: 2-arg function

#[test]
fn round_to_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "round_to", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn round_to_one_arg_errors() {
    let result = call_err(module_ref("math").unwrap(), "round_to", &[n(1.5)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn round_to_first_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "round_to", &[s("x"), n(2.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn round_to_second_arg_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "round_to", &[n(1.5), s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn round_to_zero_decimals_alt() {
    assert_eq!(
        call(module_ref("math").unwrap(), "round_to", &[n(1.567), n(0.0)]).as_f64(),
        Some(2.0)
    );
}

#[test]
fn round_to_two_decimals_alt() {
    assert_eq!(
        call(module_ref("math").unwrap(), "round_to", &[n(1.567), n(2.0)]).as_f64(),
        Some(1.57)
    );
}

#[test]
fn round_to_two_decimals_edge() {
    assert_eq!(
        call(
            module_ref("math").unwrap(),
            "round_to",
            &[n(3.14159), n(2.0)]
        )
        .as_f64(),
        Some(3.14)
    );
}

#[test]
fn round_to_zero_decimals_edge() {
    assert_eq!(
        call(module_ref("math").unwrap(), "round_to", &[n(3.7), n(0.0)]).as_f64(),
        Some(4.0)
    );
}

// Pi and E constants

#[test]
fn pi_returns_constant() {
    assert_eq!(
        call(module_ref("math").unwrap(), "pi", &[]).as_f64(),
        Some(std::f64::consts::PI)
    );
}

#[test]
fn e_returns_constant() {
    assert_eq!(
        call(module_ref("math").unwrap(), "e", &[]).as_f64(),
        Some(std::f64::consts::E)
    );
}

// Edge values

#[test]
fn abs_zero() {
    assert_eq!(
        call(module_ref("math").unwrap(), "abs", &[n(0.0)]).as_f64(),
        Some(0.0)
    );
}

#[test]
fn floor_already_integer() {
    assert_eq!(
        call(module_ref("math").unwrap(), "floor", &[n(5.0)]).as_f64(),
        Some(5.0)
    );
}

#[test]
fn ceil_already_integer() {
    assert_eq!(
        call(module_ref("math").unwrap(), "ceil", &[n(5.0)]).as_f64(),
        Some(5.0)
    );
}

#[test]
fn round_half_up() {
    assert_eq!(
        call(module_ref("math").unwrap(), "round", &[n(2.5)]).as_f64(),
        Some(3.0)
    );
}

#[test]
fn round_half_down() {
    assert_eq!(
        call(module_ref("math").unwrap(), "round", &[n(2.4)]).as_f64(),
        Some(2.0)
    );
}

#[test]
fn trunc_positive() {
    assert_eq!(
        call(module_ref("math").unwrap(), "trunc", &[n(2.7)]).as_f64(),
        Some(2.0)
    );
}

#[test]
fn trunc_negative() {
    assert_eq!(
        call(module_ref("math").unwrap(), "trunc", &[n(-2.7)]).as_f64(),
        Some(-2.0)
    );
}

#[test]
fn sqrt_zero() {
    assert_eq!(
        call(module_ref("math").unwrap(), "sqrt", &[n(0.0)]).as_f64(),
        Some(0.0)
    );
}

#[test]
fn pow_zero_exponent() {
    assert_eq!(
        call(module_ref("math").unwrap(), "pow", &[n(5.0), n(0.0)]).as_f64(),
        Some(1.0)
    );
}

#[test]
fn mod_op_normal() {
    assert_eq!(
        call(module_ref("math").unwrap(), "mod", &[n(10.0), n(3.0)]).as_f64(),
        Some(1.0)
    );
}

#[test]
fn degrees_zero() {
    assert_eq!(
        call(module_ref("math").unwrap(), "degrees", &[n(0.0)]).as_f64(),
        Some(0.0)
    );
}

#[test]
fn radians_zero() {
    assert_eq!(
        call(module_ref("math").unwrap(), "radians", &[n(0.0)]).as_f64(),
        Some(0.0)
    );
}

// ── is_nan / is_finite / is_infinite ───────────────────────────────
// Note: NaN and Infinity cannot be represented in JSON (serde_json
// serializes them as null). These predicates work correctly for all
// valid JSON numbers; is_nan/is_infinite will always return false and
// is_finite will always return true for representable values.

#[test]
fn is_nan_false() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_nan", &[n(1.0)]).as_bool(),
        Some(false)
    );
}

#[test]
fn is_nan_zero() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_nan", &[n(0.0)]).as_bool(),
        Some(false)
    );
}

#[test]
fn is_nan_large() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_nan", &[n(1e308)]).as_bool(),
        Some(false)
    );
}

#[test]
fn is_finite_normal() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_finite", &[n(1.0)]).as_bool(),
        Some(true)
    );
}

#[test]
fn is_finite_zero() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_finite", &[n(0.0)]).as_bool(),
        Some(true)
    );
}

#[test]
fn is_finite_large() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_finite", &[n(1e308)]).as_bool(),
        Some(true)
    );
}

#[test]
fn is_finite_negative() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_finite", &[n(-1.0)]).as_bool(),
        Some(true)
    );
}

#[test]
fn is_infinite_false() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_infinite", &[n(1.0)]).as_bool(),
        Some(false)
    );
}

#[test]
fn is_infinite_zero() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_infinite", &[n(0.0)]).as_bool(),
        Some(false)
    );
}

#[test]
fn is_nan_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "is_nan", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn is_nan_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "is_nan", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn is_finite_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "is_finite", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn is_infinite_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "is_infinite", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

// ── is_integer ─────────────────────────────────────────────────────

#[test]
fn is_integer_true() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_integer", &[n(5.0)]).as_bool(),
        Some(true)
    );
}

#[test]
fn is_integer_zero() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_integer", &[n(0.0)]).as_bool(),
        Some(true)
    );
}

#[test]
fn is_integer_negative() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_integer", &[n(-3.0)]).as_bool(),
        Some(true)
    );
}

#[test]
fn is_integer_false() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_integer", &[n(3.7)]).as_bool(),
        Some(false)
    );
}

#[test]
fn is_integer_large() {
    assert_eq!(
        call(module_ref("math").unwrap(), "is_integer", &[n(1e15)]).as_bool(),
        Some(true)
    );
}

#[test]
fn is_integer_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "is_integer", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn is_integer_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "is_integer", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

// ── nan / infinity / tau constants ─────────────────────────────────
// Note: NaN and Infinity cannot be represented in JSON. The nan and
// infinity constants return null (serde_json's representation of these
// values).

#[test]
fn nan_constant() {
    assert!(call(module_ref("math").unwrap(), "nan", &[]).is_null());
}

#[test]
fn infinity_constant() {
    assert!(call(module_ref("math").unwrap(), "infinity", &[]).is_null());
}

#[test]
fn tau_constant() {
    let result = call(module_ref("math").unwrap(), "tau", &[]).as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - std::f64::consts::TAU).abs() < 1e-10);
}

#[test]
fn tau_is_two_pi() {
    let result = call(module_ref("math").unwrap(), "tau", &[]).as_f64();
    assert!(result.is_some());
    assert!((result.unwrap() - 2.0 * std::f64::consts::PI).abs() < 1e-10);
}

// ── lerp ───────────────────────────────────────────────────────────

#[test]
fn lerp_start() {
    assert_eq!(
        call(
            module_ref("math").unwrap(),
            "lerp",
            &[n(0.0), n(10.0), n(0.0)]
        )
        .as_f64(),
        Some(0.0)
    );
}

#[test]
fn lerp_end() {
    assert_eq!(
        call(
            module_ref("math").unwrap(),
            "lerp",
            &[n(0.0), n(10.0), n(1.0)]
        )
        .as_f64(),
        Some(10.0)
    );
}

#[test]
fn lerp_mid() {
    assert_eq!(
        call(
            module_ref("math").unwrap(),
            "lerp",
            &[n(0.0), n(10.0), n(0.5)]
        )
        .as_f64(),
        Some(5.0)
    );
}

#[test]
fn lerp_negative_range() {
    assert_eq!(
        call(
            module_ref("math").unwrap(),
            "lerp",
            &[n(-10.0), n(10.0), n(0.5)]
        )
        .as_f64(),
        Some(0.0)
    );
}

#[test]
fn lerp_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "lerp", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn lerp_two_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "lerp", &[n(0.0), n(10.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn lerp_non_number_errors() {
    let result = call_err(
        module_ref("math").unwrap(),
        "lerp",
        &[s("x"), n(10.0), n(0.5)],
    );
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

// ── map_range ──────────────────────────────────────────────────────

#[test]
fn map_range_basic() {
    let result = call(
        module_ref("math").unwrap(),
        "map_range",
        &[n(5.0), n(0.0), n(10.0), n(0.0), n(100.0)],
    )
    .as_f64();
    assert_eq!(result, Some(50.0));
}

#[test]
fn map_range_start() {
    let result = call(
        module_ref("math").unwrap(),
        "map_range",
        &[n(0.0), n(0.0), n(10.0), n(0.0), n(100.0)],
    )
    .as_f64();
    assert_eq!(result, Some(0.0));
}

#[test]
fn map_range_end() {
    let result = call(
        module_ref("math").unwrap(),
        "map_range",
        &[n(10.0), n(0.0), n(10.0), n(0.0), n(100.0)],
    )
    .as_f64();
    assert_eq!(result, Some(100.0));
}

#[test]
fn map_range_negative() {
    let result = call(
        module_ref("math").unwrap(),
        "map_range",
        &[n(0.0), n(-10.0), n(10.0), n(0.0), n(100.0)],
    )
    .as_f64();
    assert_eq!(result, Some(50.0));
}

#[test]
fn map_range_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "map_range", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn map_range_four_args_errors() {
    let result = call_err(
        module_ref("math").unwrap(),
        "map_range",
        &[n(5.0), n(0.0), n(10.0), n(0.0)],
    );
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn map_range_non_number_errors() {
    let result = call_err(
        module_ref("math").unwrap(),
        "map_range",
        &[s("x"), n(0.0), n(10.0), n(0.0), n(100.0)],
    );
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

// ── fract ──────────────────────────────────────────────────────────

#[test]
fn fract_positive() {
    let result = call(module_ref("math").unwrap(), "fract", &[n(3.7)])
        .as_f64()
        .unwrap();
    assert!((result - 0.7).abs() < 1e-10);
}

#[test]
fn fract_negative() {
    let result = call(module_ref("math").unwrap(), "fract", &[n(-3.7)])
        .as_f64()
        .unwrap();
    assert!((result - (-0.7)).abs() < 1e-10);
}

#[test]
fn fract_integer() {
    assert_eq!(
        call(module_ref("math").unwrap(), "fract", &[n(5.0)]).as_f64(),
        Some(0.0)
    );
}

#[test]
fn fract_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "fract", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn fract_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "fract", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

// ── gcd / lcm ──────────────────────────────────────────────────────

#[test]
fn gcd_basic() {
    assert_eq!(
        call(module_ref("math").unwrap(), "gcd", &[n(12.0), n(8.0)]).as_f64(),
        Some(4.0)
    );
}

#[test]
fn gcd_with_one() {
    assert_eq!(
        call(module_ref("math").unwrap(), "gcd", &[n(7.0), n(1.0)]).as_f64(),
        Some(1.0)
    );
}

#[test]
fn gcd_same() {
    assert_eq!(
        call(module_ref("math").unwrap(), "gcd", &[n(5.0), n(5.0)]).as_f64(),
        Some(5.0)
    );
}

#[test]
fn gcd_zero() {
    assert_eq!(
        call(module_ref("math").unwrap(), "gcd", &[n(0.0), n(5.0)]).as_f64(),
        Some(5.0)
    );
}

#[test]
fn gcd_both_zero() {
    assert_eq!(
        call(module_ref("math").unwrap(), "gcd", &[n(0.0), n(0.0)]).as_f64(),
        Some(0.0)
    );
}

#[test]
fn lcm_basic() {
    assert_eq!(
        call(module_ref("math").unwrap(), "lcm", &[n(4.0), n(6.0)]).as_f64(),
        Some(12.0)
    );
}

#[test]
fn lcm_with_one() {
    assert_eq!(
        call(module_ref("math").unwrap(), "lcm", &[n(7.0), n(1.0)]).as_f64(),
        Some(7.0)
    );
}

#[test]
fn lcm_zero() {
    assert_eq!(
        call(module_ref("math").unwrap(), "lcm", &[n(0.0), n(5.0)]).as_f64(),
        Some(0.0)
    );
}

#[test]
fn gcd_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "gcd", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn gcd_one_arg_errors() {
    let result = call_err(module_ref("math").unwrap(), "gcd", &[n(12.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn gcd_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "gcd", &[s("x"), n(8.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn lcm_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "lcm", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn lcm_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "lcm", &[s("x"), n(6.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

// ── sinh / cosh / tanh ─────────────────────────────────────────────

#[test]
fn sinh_zero() {
    let result = call(module_ref("math").unwrap(), "sinh", &[n(0.0)]).as_f64();
    assert!(result.unwrap().abs() < 1e-10);
}

#[test]
fn sinh_one() {
    let result = call(module_ref("math").unwrap(), "sinh", &[n(1.0)]).as_f64();
    assert!((result.unwrap() - 1.1752011936438014).abs() < 1e-10);
}

#[test]
fn cosh_zero() {
    let result = call(module_ref("math").unwrap(), "cosh", &[n(0.0)]).as_f64();
    assert!((result.unwrap() - 1.0).abs() < 1e-10);
}

#[test]
fn cosh_one() {
    let result = call(module_ref("math").unwrap(), "cosh", &[n(1.0)]).as_f64();
    assert!((result.unwrap() - 1.5430806348152437).abs() < 1e-10);
}

#[test]
fn tanh_zero() {
    let result = call(module_ref("math").unwrap(), "tanh", &[n(0.0)]).as_f64();
    assert!(result.unwrap().abs() < 1e-10);
}

#[test]
fn tanh_one() {
    let result = call(module_ref("math").unwrap(), "tanh", &[n(1.0)]).as_f64();
    assert!((result.unwrap() - 0.7615941559557649).abs() < 1e-10);
}

#[test]
fn sinh_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "sinh", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn cosh_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "cosh", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn tanh_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "tanh", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn sinh_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "sinh", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

// ── asinh / acosh / atanh ──────────────────────────────────────────

#[test]
fn asinh_zero() {
    let result = call(module_ref("math").unwrap(), "asinh", &[n(0.0)]).as_f64();
    assert!(result.unwrap().abs() < 1e-10);
}

#[test]
fn asinh_one() {
    let result = call(module_ref("math").unwrap(), "asinh", &[n(1.0)]).as_f64();
    assert!((result.unwrap() - 0.881373587019543).abs() < 1e-10);
}

#[test]
fn acosh_one() {
    let result = call(module_ref("math").unwrap(), "acosh", &[n(1.0)]).as_f64();
    assert!(result.unwrap().abs() < 1e-10);
}

#[test]
fn acosh_two() {
    let result = call(module_ref("math").unwrap(), "acosh", &[n(2.0)]).as_f64();
    assert!((result.unwrap() - 1.3169578969248166).abs() < 1e-10);
}

#[test]
fn atanh_zero() {
    let result = call(module_ref("math").unwrap(), "atanh", &[n(0.0)]).as_f64();
    assert!(result.unwrap().abs() < 1e-10);
}

#[test]
fn atanh_half() {
    let result = call(module_ref("math").unwrap(), "atanh", &[n(0.5)]).as_f64();
    assert!((result.unwrap() - 0.5493061443340549).abs() < 1e-10);
}

#[test]
fn asinh_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "asinh", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn acosh_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "acosh", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn atanh_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "atanh", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn atanh_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "atanh", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

// ── factorial ──────────────────────────────────────────────────────

#[test]
fn factorial_zero() {
    assert_eq!(
        call(module_ref("math").unwrap(), "factorial", &[n(0.0)]).as_f64(),
        Some(1.0)
    );
}

#[test]
fn factorial_one() {
    assert_eq!(
        call(module_ref("math").unwrap(), "factorial", &[n(1.0)]).as_f64(),
        Some(1.0)
    );
}

#[test]
fn factorial_five() {
    assert_eq!(
        call(module_ref("math").unwrap(), "factorial", &[n(5.0)]).as_f64(),
        Some(120.0)
    );
}

#[test]
fn factorial_ten() {
    assert_eq!(
        call(module_ref("math").unwrap(), "factorial", &[n(10.0)]).as_f64(),
        Some(3628800.0)
    );
}

#[test]
fn factorial_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "factorial", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn factorial_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "factorial", &[s("x")]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn factorial_negative_errors() {
    let result = call_err(module_ref("math").unwrap(), "factorial", &[n(-1.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn factorial_non_integer_errors() {
    let result = call_err(module_ref("math").unwrap(), "factorial", &[n(3.5)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

// ── floor_div ──────────────────────────────────────────────────────

#[test]
fn floor_div_positive() {
    assert_eq!(
        call(module_ref("math").unwrap(), "floor_div", &[n(7.0), n(3.0)]).as_f64(),
        Some(2.0)
    );
}

#[test]
fn floor_div_exact() {
    assert_eq!(
        call(module_ref("math").unwrap(), "floor_div", &[n(6.0), n(3.0)]).as_f64(),
        Some(2.0)
    );
}

#[test]
fn floor_div_negative_dividend() {
    assert_eq!(
        call(module_ref("math").unwrap(), "floor_div", &[n(-7.0), n(3.0)]).as_f64(),
        Some(-3.0)
    );
}

#[test]
fn floor_div_negative_divisor() {
    assert_eq!(
        call(module_ref("math").unwrap(), "floor_div", &[n(7.0), n(-3.0)]).as_f64(),
        Some(-3.0)
    );
}

#[test]
fn floor_div_both_negative() {
    assert_eq!(
        call(
            module_ref("math").unwrap(),
            "floor_div",
            &[n(-7.0), n(-3.0)]
        )
        .as_f64(),
        Some(2.0)
    );
}

#[test]
fn floor_div_no_args_errors() {
    let result = call_err(module_ref("math").unwrap(), "floor_div", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn floor_div_one_arg_errors() {
    let result = call_err(module_ref("math").unwrap(), "floor_div", &[n(7.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn floor_div_non_number_errors() {
    let result = call_err(module_ref("math").unwrap(), "floor_div", &[s("x"), n(3.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}

#[test]
fn floor_div_zero_divisor_errors() {
    let result = call_err(module_ref("math").unwrap(), "floor_div", &[n(7.0), n(0.0)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch { .. })));
}
