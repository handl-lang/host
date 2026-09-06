//! The `mood` module — mood (mode-of-data) operations for handl
//! expressions.
//!
//! Provides functions callable as `mood.ind(v, k)`, `mood.int(v, k)`,
//! `mood.norm(v, k)`, `mood.pot(v, k)`, `mood.neg(v, k)`, `mood.cnd(v, k,
//! d)` in handl expressions.
//!
//! The mood *interface* is declared in `handl-packages/utils/mood.hnl`
//! via `cap`/`op`/`handl` declarations (ADR-0077 Phase 2). All 6
//! functions are FFI atoms — they use `as_kind`, `project`, `complement`,
//! `matches`, `ok_result`, and `err_result` from `crate::shared` and
//! cannot be expressed as pure handl arrows without losing the exact
//! error messages the tests rely on. The Rust `Function` impls in [`ops`]
//! are registered as handls (handlers) in an [`OpRegistry`], and the
//! generic [`HandlModule`](handl_plugin_lib_handl::HandlModule) loader
//! dispatches each call to the registered handler via [`OpFunction`].

mod ops;

#[allow(unused_imports)]
pub use ops::*;

use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use ops::{MoodCnd, MoodInd, MoodInt, MoodNeg, MoodNorm, MoodPot};

/// The handl source declaring the mood `op` interface — the single
/// source of truth for the mood API surface. Loaded at compile time from
/// the kit-stdlib data dir.
///
/// `mood.hnl` contains `---` doc comments, `cap` declarations, `op`
/// declarations, and `handl` bindings for all 6 mood functions. The
/// [`HandlModule`] loader strips comments and declaration lines, leaving
/// an empty expression (no pure-logic arrows); all functions are
/// resolved via [`OpFunction`] dispatching to the [`OpRegistry`].
const MOOD_HNL: &str = include_str!("../../../../handl-packages/utils/mood.hnl");

/// The op names declared in `mood.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`].
const MOOD_OPS: &[&str] = &[
    "mood.ind",
    "mood.int",
    "mood.norm",
    "mood.pot",
    "mood.neg",
    "mood.cnd",
];

/// Build the [`OpRegistry`] for mood, registering the Rust handls under
/// their op names.
///
/// Each op name (e.g. `"mood.ind"`) maps to its Rust `Function` impl
/// (e.g. [`MoodInd`]). No runtime guarantee checkers are attached — the
/// mood atoms are trusted.
fn mood_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("mood.ind", Arc::new(MoodInd));
    registry.register("mood.int", Arc::new(MoodInt));
    registry.register("mood.norm", Arc::new(MoodNorm));
    registry.register("mood.pot", Arc::new(MoodPot));
    registry.register("mood.neg", Arc::new(MoodNeg));
    registry.register("mood.cnd", Arc::new(MoodCnd));
    registry
}

/// Build the [`HandlModule`] for mood.
///
/// All 6 functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the `HandlModule` dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_mood_module() -> HandlModule {
    let registry = Arc::new(mood_op_registry());
    HandlModule::new("mood", MOOD_HNL, &[]).with_ops(MOOD_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for mood.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
static MOOD_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for mood, initializing it on first call.
fn mood_module() -> &'static HandlModule {
    MOOD_MODULE.get_or_init(build_mood_module)
}

/// The `mood` module — mood (mode-of-data) operations.
///
/// Provides `mood.ind`, `mood.int`, `mood.norm`, `mood.pot`, `mood.neg`,
/// and `mood.cnd`. The interface is declared in `mood.hnl` via
/// `op`/`handl` declarations and loaded via the generic
/// [`HandlModule`](handl_plugin_lib_handl::HandlModule); this struct
/// only delegates [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct MoodModule;

impl Module for MoodModule {
    fn name(&self) -> &str {
        "mood"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        mood_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        mood_module().functions()
    }
}

#[cfg(test)]
mod mood_tests;
