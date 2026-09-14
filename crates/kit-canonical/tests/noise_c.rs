//! # noise_c.rs — `noise-c` and `noise-stub` end-to-end smoke tests.
//!
//! `noise-c` loads directly from `handl-packages/noise-c` and builds
//! deterministic value noise on top of `rand-c.seeded_int`. `noise-stub`
//! is the deterministic, no-effect backend.

use handl_kit_canonical::Kit;

fn eval_noise_c(input: &str) -> serde_json::Value {
    let kit = Kit::new().expect("kit must build");
    let src = format!(
        "do {{\n  from \"../../handl-packages/noise-c\" import {{ perlin_1d, perlin_2d, perlin_3d, simplex_2d, simplex_3d }};\n  {}\n}}",
        input
    );
    kit.eval(&src).expect("eval must succeed")
}

fn eval_noise_stub(input: &str) -> serde_json::Value {
    let kit = Kit::new().expect("kit must build");
    let src = format!(
        "do {{\n  from \"../../handl-packages/noise-stub\" import {{ perlin_1d, perlin_2d, perlin_3d, simplex_2d, simplex_3d }};\n  {}\n}}",
        input
    );
    kit.eval(&src).expect("eval must succeed")
}

// ---------------------------------------------------------------------------
// noise-c — deterministic value noise (loaded from file)
// ---------------------------------------------------------------------------

#[test]
fn noise_c_perlin_1d_is_number() {
    let v = eval_noise_c("perlin_1d(1.5)");
    assert!(v.is_number(), "perlin_1d must return a number: {v}");
}

#[test]
fn noise_c_perlin_2d_is_number() {
    let v = eval_noise_c("perlin_2d(1.0, 2.0)");
    assert!(v.is_number(), "perlin_2d must return a number: {v}");
}

#[test]
fn noise_c_perlin_3d_is_number() {
    let v = eval_noise_c("perlin_3d(1.0, 2.0, 3.0)");
    assert!(v.is_number(), "perlin_3d must return a number: {v}");
}

#[test]
fn noise_c_simplex_2d_is_number() {
    let v = eval_noise_c("simplex_2d(1.0, 2.0)");
    assert!(v.is_number(), "simplex_2d must return a number: {v}");
}

#[test]
fn noise_c_simplex_3d_is_number() {
    let v = eval_noise_c("simplex_3d(1.0, 2.0, 3.0)");
    assert!(v.is_number(), "simplex_3d must return a number: {v}");
}

#[test]
fn noise_c_perlin_1d_is_deterministic() {
    let a = eval_noise_c("perlin_1d(1.5)");
    let b = eval_noise_c("perlin_1d(1.5)");
    assert_eq!(a, b, "perlin_1d must be deterministic");
}

#[test]
fn noise_c_perlin_2d_is_deterministic() {
    let a = eval_noise_c("perlin_2d(1.0, 2.0)");
    let b = eval_noise_c("perlin_2d(1.0, 2.0)");
    assert_eq!(a, b, "perlin_2d must be deterministic");
}

#[test]
fn noise_c_perlin_3d_is_deterministic() {
    let a = eval_noise_c("perlin_3d(1.0, 2.0, 3.0)");
    let b = eval_noise_c("perlin_3d(1.0, 2.0, 3.0)");
    assert_eq!(a, b, "perlin_3d must be deterministic");
}

#[test]
fn noise_c_simplex_2d_is_deterministic() {
    let a = eval_noise_c("simplex_2d(1.0, 2.0)");
    let b = eval_noise_c("simplex_2d(1.0, 2.0)");
    assert_eq!(a, b, "simplex_2d must be deterministic");
}

#[test]
fn noise_c_simplex_3d_is_deterministic() {
    let a = eval_noise_c("simplex_3d(1.0, 2.0, 3.0)");
    let b = eval_noise_c("simplex_3d(1.0, 2.0, 3.0)");
    assert_eq!(a, b, "simplex_3d must be deterministic");
}

// ---------------------------------------------------------------------------
// noise-stub — deterministic, no-effect backend
// ---------------------------------------------------------------------------

#[test]
fn noise_stub_is_zero() {
    assert_eq!(eval_noise_stub("perlin_1d(1.5)"), serde_json::json!(0.0));
    assert_eq!(
        eval_noise_stub("perlin_2d(1.0, 2.0)"),
        serde_json::json!(0.0)
    );
    assert_eq!(
        eval_noise_stub("perlin_3d(1.0, 2.0, 3.0)"),
        serde_json::json!(0.0)
    );
    assert_eq!(
        eval_noise_stub("simplex_2d(1.0, 2.0)"),
        serde_json::json!(0.0)
    );
    assert_eq!(
        eval_noise_stub("simplex_3d(1.0, 2.0, 3.0)"),
        serde_json::json!(0.0)
    );
}
