//! Unit tests for the `time` module's native ops.
//!
//! Covers the wall-clock read (`now_ms` returns a plausible epoch-ms
//! value), the monotonic read (`instant_ns` is non-decreasing), and
//! `elapsed_ns` (deltas, argument validation, future-input saturation).

use serde_json::json;

use crate::TimeModule;
use handl_platform::Module;

fn call(name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let f = TimeModule
        .function(name)
        .unwrap_or_else(|| panic!("time.{name} must exist"));
    f.call(args, &mut handl_platform::EvalContext::new())
        .unwrap_or_else(|e| panic!("time.{name}({args:?}) failed: {e}"))
}

fn call_err(name: &str, args: &[serde_json::Value]) -> String {
    let f = TimeModule
        .function(name)
        .unwrap_or_else(|| panic!("time.{name} must exist"));
    f.call(args, &mut handl_platform::EvalContext::new())
        .unwrap_err()
        .to_string()
}

#[test]
fn now_ms_is_plausible_epoch_millis() {
    let v = call("now_ms", &[]).as_f64().unwrap();
    // After 2020-01-01 (1577836800000) and before the year 3000.
    assert!(v > 1_577_836_800_000.0, "now_ms too small: {v}");
    assert!(v < 32_503_680_000_000.0, "now_ms too large: {v}");
    assert_eq!(v.fract(), 0.0, "now_ms must be integral ms");
}

#[test]
fn instant_ns_is_monotonic_and_nonnegative() {
    let a = call("instant_ns", &[]).as_f64().unwrap();
    let b = call("instant_ns", &[]).as_f64().unwrap();
    assert!(a >= 0.0);
    assert!(b >= a, "instant_ns must be non-decreasing: {a} then {b}");
}

#[test]
fn elapsed_ns_measures_since_reading() {
    let start = call("instant_ns", &[]).as_f64().unwrap();
    let elapsed = call("elapsed_ns", &[json!(start)]).as_f64().unwrap();
    assert!(elapsed >= 0.0);
    // A fresh elapsed reading is tiny — well under a minute.
    assert!(
        elapsed < 60_000_000_000.0,
        "elapsed_ns too large: {elapsed}"
    );

    // A reading far in the past yields a large elapsed time (nonzero).
    let past = start - 1_000_000.0;
    let e2 = call("elapsed_ns", &[json!(past)]).as_f64().unwrap();
    assert!(e2 >= 1_000_000.0, "elapsed_ns should cover the gap: {e2}");
}

#[test]
fn elapsed_ns_saturates_on_future_input() {
    // A reading in the future must not produce a negative elapsed time.
    let future = call("instant_ns", &[]).as_f64().unwrap() + 1e15;
    assert_eq!(call("elapsed_ns", &[json!(future)]), json!(0.0));
}

#[test]
fn arity_and_type_errors() {
    assert!(call_err("now_ms", &[json!(1.0)]).contains("expected"));
    assert!(call_err("instant_ns", &[json!(1.0)]).contains("expected"));
    assert!(call_err("elapsed_ns", &[]).contains("expected"));
    assert!(call_err("elapsed_ns", &[json!("soon")]).contains("expected"));
}
