//! # time_rand.rs — `time.*`/`rand.*` native backend end-to-end smoke tests.
//!
//! Verifies the `time` and `rand` modules (offered by the canonical kit's
//! stdlib registry) are
//! callable from handl source via the unbound-identifier → `field_access`
//! → `ModuleLookup` path — the same resolution `tag.*` uses (ADR-0055).

use handl_kit_canonical::Kit;

fn eval(input: &str) -> serde_json::Value {
    let kit = Kit::new().expect("kit must build");
    kit.eval(input).expect("eval must succeed")
}

// ---------------------------------------------------------------------------
// time.* — clock reads
// ---------------------------------------------------------------------------

#[test]
fn time_now_ms_is_plausible_epoch_millis() {
    let v = eval("time.now_ms()").as_f64().expect("now_ms is a number");
    assert!(v > 1_577_836_800_000.0, "now_ms too small: {v}");
}

#[test]
fn time_instant_ns_is_monotonic() {
    let a = eval("time.instant_ns()").as_f64().unwrap();
    let b = eval("time.instant_ns()").as_f64().unwrap();
    assert!(a >= 0.0 && b >= a, "instant_ns must be non-decreasing");
}

#[test]
fn time_elapsed_ns_since_reading() {
    // Elapsed since a fresh reading: nested call evaluates the inner
    // `instant_ns` first, then `elapsed_ns` measures the (tiny) delta.
    let v = eval("time.elapsed_ns(time.instant_ns())");
    let ns = v.as_f64().expect("elapsed_ns is a number");
    assert!(ns >= 0.0);
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
