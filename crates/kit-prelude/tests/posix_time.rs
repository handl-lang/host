//! # posix_time.rs — posix clock end-to-end smoke tests.
//!
//! `time`/`time-c` folded into the posix family: the clock reads now live
//! in `lib-posix-c` (`posix-c` backend, direct `clock_gettime` `ffi.call`).
//! The tests load `lib-posix-c/src/time` directly.

use handl_kit_prelude::Kit;

fn eval_time(input: &str) -> serde_json::Value {
    let kit = Kit::new().expect("kit must build");
    let src = format!(
        "do {{\n  from \"../../../lib-posix-c/src/time\" import {{ now_ms, instant_ns, elapsed_ns }};\n  {}\n}}",
        input
    );
    kit.eval(&src).expect("eval must succeed")
}

// ---------------------------------------------------------------------------
// posix clock reads (loaded from file)
// ---------------------------------------------------------------------------

#[test]
fn posix_time_now_ms_is_plausible_epoch_millis() {
    let v = eval_time("now_ms()").as_f64().expect("now_ms is a number");
    assert!(v > 1_577_836_800_000.0, "now_ms too small: {v}");
}

#[test]
fn posix_time_instant_ns_is_monotonic() {
    let a = eval_time("instant_ns()").as_f64().unwrap();
    let b = eval_time("instant_ns()").as_f64().unwrap();
    assert!(a >= 0.0 && b >= a, "instant_ns must be non-decreasing");
}

#[test]
fn posix_time_elapsed_ns_since_reading() {
    // Elapsed since a fresh reading: nested call evaluates the inner
    // `instant_ns` first, then `elapsed_ns` measures the (tiny) delta.
    let v = eval_time("elapsed_ns(instant_ns())");
    let ns = v.as_f64().expect("elapsed_ns is a number");
    assert!(ns >= 0.0);
}
