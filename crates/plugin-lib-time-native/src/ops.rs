//! Rust `Function` impls for the `time` module — the handls (handlers)
//! for the `op`/`handl` declarations in `time-native.hnl`.
//!
//! All 3 functions are FFI atoms over `std::time`:
//!
//! - `time.now_ms` — `SystemTime::now()` milliseconds since `UNIX_EPOCH`.
//!   A pre-epoch wall clock (a misconfigured host) is an error, not a
//!   negative reading.
//! - `time.instant_ns` — monotonic nanoseconds. `Instant` has no portable
//!   epoch, so readings are relative to a process-lifetime base captured
//!   on first use (`INSTANT_EPOCH`): `instant_ns()` returns
//!   `INSTANT_EPOCH.elapsed().as_nanos()`. Absolute values are therefore
//!   meaningless across processes — only differences between readings in
//!   the same process are, which is exactly what the `time` contract
//!   guarantees and what `benchmark` needs. `Instant` is monotonic, so
//!   successive calls are non-decreasing.
//! - `time.elapsed_ns` — `instant_ns() - input`, saturating at 0 when the
//!   input lies in the future (bogus or cross-epoch readings must not
//!   produce a negative elapsed time).
//!
//! Nanosecond readings are returned as `f64` (the handl `Int` transport);
//! precision is exact up to ~104 days of elapsed nanoseconds (2^53).

use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use handl_platform::{EvalContext, EvalError, Function};

/// Process-lifetime monotonic epoch for `instant_ns` readings.
///
/// Captured on first call — `Instant` exposes no usable epoch, so the
/// contract specifies readings as "nanoseconds from an unspecified epoch,
/// meaningful only as differences".
static INSTANT_EPOCH: OnceLock<Instant> = OnceLock::new();

fn instant_ns_now() -> f64 {
    INSTANT_EPOCH.get_or_init(Instant::now).elapsed().as_nanos() as f64
}

fn num(value: f64) -> serde_json::Value {
    serde_json::json!(value)
}

fn type_mismatch(expected: &str, got: &str) -> EvalError {
    EvalError::TypeMismatch(format!("expected {expected}, got {got}"))
}

#[derive(Clone, Copy)]
pub struct NowMs;
impl Function for NowMs {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if !args.is_empty() {
            return Err(type_mismatch("0 args", "extra args"));
        }
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                EvalError::Other(format!(
                    "time.now_ms: wall clock is before the Unix epoch: {e}"
                ))
            })?
            .as_millis();
        Ok(num(millis as f64))
    }
}

#[derive(Clone, Copy)]
pub struct InstantNs;
impl Function for InstantNs {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if !args.is_empty() {
            return Err(type_mismatch("0 args", "extra args"));
        }
        Ok(num(instant_ns_now()))
    }
}

#[derive(Clone, Copy)]
pub struct ElapsedNs;
impl Function for ElapsedNs {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let arg = args.first().ok_or_else(|| type_mismatch("1 args", "0"))?;
        let since = arg
            .as_f64()
            .ok_or_else(|| type_mismatch("number (an instant_ns reading)", "non-number"))?;
        // Saturating at 0: a reading in the future (bogus input or a
        // cross-epoch value) is not a meaningful elapsed time.
        Ok(num((instant_ns_now() - since).max(0.0)))
    }
}
