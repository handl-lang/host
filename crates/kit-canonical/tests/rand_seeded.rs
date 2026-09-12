//! # rand_seeded.rs — deterministic `rand` host-module smoke tests.
//!
//! `rand` remains a pre-loaded host module; these tests exercise the
//! deterministic `rand.seeded_*` operations.

use handl_kit_canonical::Kit;

fn eval(input: &str) -> serde_json::Value {
    let kit = Kit::new().expect("kit must build");
    kit.eval(input).expect("eval must succeed")
}

// ---------------------------------------------------------------------------
// rand.seeded_* — deterministic RNG (ADR-0092 `property`/`fuzz` primitive)
// ---------------------------------------------------------------------------

#[test]
fn rand_seeded_int_is_deterministic_and_bounded() {
    let a = eval("rand.seeded_int(42, 0, 10)").as_f64().unwrap();
    let b = eval("rand.seeded_int(42, 0, 10)").as_f64().unwrap();
    assert_eq!(a, b, "same seed + args must produce the same output");
    assert!((0.0..=10.0).contains(&a), "seeded_int out of range: {a}");
}

#[test]
fn rand_seeded_bool_is_deterministic() {
    let a = eval("rand.seeded_bool(7)");
    let b = eval("rand.seeded_bool(7)");
    assert!(a.is_boolean());
    assert_eq!(a, b, "same seed must produce the same output");
}
