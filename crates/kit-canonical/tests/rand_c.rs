//! # rand_c.rs — libc-backed and stub `rand` backend smoke tests.
//!
//! `rand-c` loads directly from `handl-packages/rand-c` and uses
//! `ffi.call` to `libc.so.6` `rand`. `rand-stub` is the deterministic,
//! no-effect backend.

use handl_kit_canonical::Kit;

fn eval_rand_c(input: &str) -> serde_json::Value {
    let kit = Kit::new().expect("kit must build");
    let src = format!(
        "do {{\n  from \"../../handl-packages/rand-c\" import {{ int, float, bool }};\n  {}\n}}",
        input
    );
    kit.eval(&src).expect("eval must succeed")
}

fn eval_rand_stub(input: &str) -> serde_json::Value {
    let kit = Kit::new().expect("kit must build");
    let src = format!(
        "do {{\n  from \"../../handl-packages/rand-stub\" import {{ int, float, bool }};\n  {}\n}}",
        input
    );
    kit.eval(&src).expect("eval must succeed")
}

// ---------------------------------------------------------------------------
// rand-c.* — libc-backed random values (loaded from file)
// ---------------------------------------------------------------------------

#[test]
fn rand_c_int_is_bounded() {
    let v = eval_rand_c("int(6)").as_f64().expect("int is a number");
    assert!((0.0..=6.0).contains(&v), "rand-c int out of range: {v}");
}

#[test]
fn rand_c_float_is_bounded() {
    let v = eval_rand_c("float(10)")
        .as_f64()
        .expect("float is a number");
    assert!((0.0..10.0).contains(&v), "rand-c float out of range: {v}");
}

#[test]
fn rand_c_bool_is_boolean() {
    let v = eval_rand_c("bool()");
    assert!(v.is_boolean(), "rand-c bool must be a boolean");
}

// ---------------------------------------------------------------------------
// rand-stub.* — deterministic, no-effect backend
// ---------------------------------------------------------------------------

#[test]
fn rand_stub_is_deterministic() {
    let i = eval_rand_stub("int(6)").as_f64().unwrap();
    let f = eval_rand_stub("float(10)").as_f64().unwrap();
    let b = eval_rand_stub("bool()");
    assert_eq!(i, 4.0, "rand-stub int is fixed");
    assert_eq!(f, 0.5, "rand-stub float is fixed");
    assert_eq!(b, serde_json::json!(true), "rand-stub bool is fixed");
}
