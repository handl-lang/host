use std::sync::Arc;

use handl_platform::Module;
use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;

use crate::error::OsError;

/// Abstract time service.
///
/// Implementations:
/// - `handl-lib-os-std` — backed by `std::time::SystemTime`
/// - [`time_stub::StubTime`] — fixed time for testing
pub trait Time: Send + Sync {
    /// Returns the current time as epoch milliseconds.
    fn now_millis(&self) -> Result<i64, OsError>;
    /// Returns the current time as epoch seconds.
    fn now_seconds(&self) -> Result<i64, OsError>;
}

/// Module adapter for [`Time`] — exposes `time.*` functions.
pub struct TimeModule {
    now: NowFn,
    epoch: EpochFn,
}

impl TimeModule {
    pub fn new(time: impl Time + 'static) -> Self {
        let time = Arc::new(time);
        TimeModule {
            now: NowFn { time: time.clone() },
            epoch: EpochFn { time },
        }
    }
}

impl Module for TimeModule {
    fn name(&self) -> &str {
        "time"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "now" => Some(&self.now),
            "epoch" => Some(&self.epoch),
            _ => None,
        }
    }
}

struct NowFn {
    time: Arc<dyn Time>,
}

impl Function for NowFn {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        self.time
            .now_millis()
            .map(|ms| serde_json::Value::from(serde_json::json!(ms)))
            .map_err(Into::into)
    }
}

struct EpochFn {
    time: Arc<dyn Time>,
}

impl Function for EpochFn {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        self.time
            .now_seconds()
            .map(|s| serde_json::Value::from(serde_json::json!(s)))
            .map_err(Into::into)
    }
}

// ---------------------------------------------------------------------------
// Op Registry (ADR-0077 Phase 4)
// ---------------------------------------------------------------------------

/// The op names declared in `os.hnl` for the `time` module.
pub const TIME_OPS: &[&str] = &[
    "time.now",
    "time.epoch",
];

/// Register the `time` ops into the given [`OpRegistry`], dispatching to the
/// given [`Time`] implementation.
pub fn register_time_ops(registry: &mut OpRegistry, time: impl Time + 'static) {
    let time: Arc<dyn Time> = Arc::new(time);
    registry.register("time.now", Arc::new(NowFn { time: time.clone() }));
    registry.register("time.epoch", Arc::new(EpochFn { time }));
}
