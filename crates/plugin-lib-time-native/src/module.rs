//! The `time` module — clock reads backed by Rust `std::time`.
//!
//! This is the *native* backend for the `time` contract
//! (`handl-packages/time/time.hnl`): a wall-clock read (`now_ms`) and a
//! monotonic clock read (`instant_ns`/`elapsed_ns`) for elapsed-time
//! measurement.
//!
//! The interface is declared in `handl-packages/time-native/time-native.hnl`
//! via `cap`/`op`/`handl` declarations (ADR-0077 Phase 2). The Rust
//! `Function` impls in [`ops`] are registered in an [`OpRegistry`], and the
//! generic [`HandlModule`] loader dispatches each call via [`OpFunction`].

use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use crate::ops::{ElapsedNs, InstantNs, NowMs};

/// The handl source declaring the `time` op interface — the single
/// source of truth for the time API surface. Loaded at compile time.
const TIME_NATIVE_HNL: &str = include_str!("../../../handl-packages/time-native/time-native.hnl");

/// The op names declared in `time-native.hnl` — each maps to a Rust
/// `Function` impl registered in the [`OpRegistry`].
const TIME_OPS: &[&str] = &["time.now_ms", "time.instant_ns", "time.elapsed_ns"];

/// Build the [`OpRegistry`] for time, registering the Rust handls under
/// their op names.
fn time_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("time.now_ms", Arc::new(NowMs));
    registry.register("time.instant_ns", Arc::new(InstantNs));
    registry.register("time.elapsed_ns", Arc::new(ElapsedNs));
    registry
}

/// Build the [`HandlModule`] for time.
///
/// All 3 functions are op-backed (FFI atoms); `func_names` is empty and
/// every lookup dispatches to the [`OpRegistry`] via [`OpFunction`].
fn build_time_module() -> HandlModule {
    let registry = Arc::new(time_op_registry());
    HandlModule::new("time", TIME_NATIVE_HNL, &[]).with_ops(TIME_OPS, registry)
}

static TIME_MODULE: OnceLock<HandlModule> = OnceLock::new();

fn time_module() -> &'static HandlModule {
    TIME_MODULE.get_or_init(build_time_module)
}

/// The `time` module — wall-clock and monotonic clock reads.
///
/// Provides `time.now_ms`, `time.instant_ns`, and `time.elapsed_ns`.
/// `now_ms` is wall-clock milliseconds since the Unix epoch; `instant_ns`
/// is monotonic nanoseconds from an unspecified epoch (only differences are
/// meaningful); `elapsed_ns(t)` returns the nanoseconds elapsed since a
/// prior `instant_ns` reading. Semantics follow Rust `std::time`
/// (`SystemTime` / `Instant`).
#[derive(Clone, Copy)]
pub struct TimeModule;

impl Module for TimeModule {
    fn name(&self) -> &str {
        "time"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        time_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        time_module().functions()
    }
}
