//! # os_c.rs — libc-backed and stub `os` backend smoke tests.
//!
//! `os-c` loads directly from `handl-packages/os-c` and uses `ffi.call` to
//! `libc.so.6` `getenv`. `os-stub` is the deterministic, no-effect backend.

use handl_kit_canonical::Kit;

fn eval_env_c(input: &str) -> serde_json::Value {
    let kit = Kit::new().expect("kit must build");
    let src = format!(
        "do {{\n  from \"../../handl-packages/os-c/env\" import {{ get, has }};\n  {}\n}}",
        input
    );
    kit.eval(&src).expect("eval must succeed")
}

fn eval_time_c(input: &str) -> serde_json::Value {
    let kit = Kit::new().expect("kit must build");
    let src = format!(
        "do {{\n  from \"../../handl-packages/os-c/time\" import {{ now, epoch }};\n  {}\n}}",
        input
    );
    kit.eval(&src).expect("eval must succeed")
}

fn eval_stub(input: &str) -> serde_json::Value {
    let kit = Kit::new().expect("kit must build");
    let src = format!(
        "do {{\n  from \"../../handl-packages/os-stub/env\" import {{ get, has }};\n  from \"../../handl-packages/os-stub/path\" import {{ join, basename, dirname, extname }};\n  from \"../../handl-packages/os-stub/time\" import {{ now, epoch }};\n  {}\n}}",
        input
    );
    kit.eval(&src).expect("eval must succeed")
}

// ---------------------------------------------------------------------------
// os-c — libc-backed OS operations (loaded from file)
// ---------------------------------------------------------------------------

#[test]
fn os_c_env_get_path_is_set() {
    let v = eval_env_c("get(\"PATH\")");
    assert!(v.is_string(), "os-c env.get(PATH) must be a string: {v}");
}

#[test]
fn os_c_env_has_missing() {
    let v = eval_env_c("has(\"__NOT_SET__\")");
    assert_eq!(
        v,
        serde_json::json!(false),
        "os-c env.has for missing must be false"
    );
}

#[test]
fn os_c_time_now_is_positive() {
    let v = eval_time_c("now()").as_f64().expect("now is a number");
    assert!(v > 0.0, "os-c time.now must be positive: {v}");
}

#[test]
fn os_c_time_epoch_matches_now() {
    let n = eval_time_c("now()").as_f64().unwrap();
    let e = eval_time_c("epoch()").as_f64().unwrap();
    assert!(
        (n / 1000.0 - e).abs() < 1.0,
        "os-c time.now / 1000 must be close to time.epoch"
    );
}

// ---------------------------------------------------------------------------
// os-stub — deterministic, no-effect backend
// ---------------------------------------------------------------------------

#[test]
fn os_stub_is_deterministic() {
    let i = eval_stub("get(\"HOME\")");
    let f = eval_stub("has(\"HOME\")");
    let n = eval_stub("now()");
    let e = eval_stub("epoch()");
    assert_eq!(i, serde_json::Value::Null);
    assert_eq!(f, serde_json::json!(false));
    assert_eq!(n, serde_json::json!(0));
    assert_eq!(e, serde_json::json!(0));
}
