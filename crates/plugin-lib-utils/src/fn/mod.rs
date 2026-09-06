//! The `combinators` module — function combinator utilities.
//!
//! Provides functions callable as `combinators.pipe(v, f, g)`,
//! `combinators.compose(f, g)`, `combinators.map(f, xs)`,
//! `combinators.filter(f, xs)`, `combinators.reduce(f, xs, init)`, etc. in
//! handl expressions.
//!
//! The combinators *interface* is declared in `handl-packages/utils/fn.hnl`
//! via `cap`/`op`/`handl` declarations (ADR-0077 Phase 2). All 25
//! functions are FFI atoms — they manipulate `serde_json::Value` callables
//! via `apply_fn` from `crate::shared` and cannot be expressed as pure
//! handl arrows without losing the exact `TypeMismatch` error messages the
//! tests rely on. The Rust `Function` impls in [`ops`] are registered as
//! handls (handlers) in an [`OpRegistry`], and the generic
//! [`HandlModule`](handl_plugin_lib_handl::HandlModule) loader dispatches
//! each call to the registered handler via [`OpFunction`].

mod ops;

#[allow(unused_imports)]
pub use ops::*;

use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use ops::{
    Apply, Choice, Compose, Constantly, Converge, Fanout, FanoutFn, Filter, Find, Flip, FlatMap,
    Flow, Every, GroupBy, Map, Match, Partition, Pipe, Reduce, SomeFn, Tap, Unless, When,
    CountBy, Partial,
};

/// The handl source declaring the combinators `op` interface — the single
/// source of truth for the combinators API surface. Loaded at compile time
/// from the kit-stdlib data dir.
///
/// `fn.hnl` contains `---` doc comments, `cap` declarations, `op`
/// declarations, and `handl` bindings for all 25 combinators functions. The
/// [`HandlModule`] loader strips comments and declaration lines, leaving
/// an empty expression (no pure-logic arrows); all functions are
/// resolved via [`OpFunction`] dispatching to the [`OpRegistry`].
const COMBINATORS_HNL: &str = include_str!("../../../../handl-packages/utils/fn.hnl");

/// The op names declared in `fn.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`].
const COMBINATORS_OPS: &[&str] = &[
    "combinators.pipe",
    "combinators.flow",
    "combinators.compose",
    "combinators.fanout",
    "combinators.fanout_fn",
    "combinators.choice",
    "combinators.apply",
    "combinators.map",
    "combinators.filter",
    "combinators.reduce",
    "combinators.flat_map",
    "combinators.find",
    "combinators.some",
    "combinators.every",
    "combinators.group_by",
    "combinators.partition",
    "combinators.count_by",
    "combinators.tap",
    "combinators.match",
    "combinators.constantly",
    "combinators.flip",
    "combinators.partial",
    "combinators.when",
    "combinators.unless",
    "combinators.converge",
];

/// Build the [`OpRegistry`] for combinators, registering the Rust handls
/// under their op names.
///
/// Each op name (e.g. `"combinators.pipe"`) maps to its Rust `Function` impl
/// (e.g. [`Pipe`]). No runtime guarantee checkers are attached — the
/// combinators atoms are trusted.
fn combinators_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("combinators.pipe", Arc::new(Pipe));
    registry.register("combinators.flow", Arc::new(Flow));
    registry.register("combinators.compose", Arc::new(Compose));
    registry.register("combinators.fanout", Arc::new(Fanout));
    registry.register("combinators.fanout_fn", Arc::new(FanoutFn));
    registry.register("combinators.choice", Arc::new(Choice));
    registry.register("combinators.apply", Arc::new(Apply));
    registry.register("combinators.map", Arc::new(Map));
    registry.register("combinators.filter", Arc::new(Filter));
    registry.register("combinators.reduce", Arc::new(Reduce));
    registry.register("combinators.flat_map", Arc::new(FlatMap));
    registry.register("combinators.find", Arc::new(Find));
    registry.register("combinators.some", Arc::new(SomeFn));
    registry.register("combinators.every", Arc::new(Every));
    registry.register("combinators.group_by", Arc::new(GroupBy));
    registry.register("combinators.partition", Arc::new(Partition));
    registry.register("combinators.count_by", Arc::new(CountBy));
    registry.register("combinators.tap", Arc::new(Tap));
    registry.register("combinators.match", Arc::new(Match));
    registry.register("combinators.constantly", Arc::new(Constantly));
    registry.register("combinators.flip", Arc::new(Flip));
    registry.register("combinators.partial", Arc::new(Partial));
    registry.register("combinators.when", Arc::new(When));
    registry.register("combinators.unless", Arc::new(Unless));
    registry.register("combinators.converge", Arc::new(Converge));
    registry
}

/// Build the [`HandlModule`] for combinators.
///
/// All 25 functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the `HandlModule` dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_combinators_module() -> HandlModule {
    let registry = Arc::new(combinators_op_registry());
    HandlModule::new("combinators", COMBINATORS_HNL, &[]).with_ops(COMBINATORS_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for combinators.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
static COMBINATORS_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for combinators, initializing it on first call.
fn combinators_module() -> &'static HandlModule {
    COMBINATORS_MODULE.get_or_init(build_combinators_module)
}

/// The `combinators` module — function combinator utilities.
///
/// Provides `combinators.pipe`, `combinators.flow`, `combinators.compose`,
/// `combinators.fanout`, `combinators.fanout_fn`, `combinators.choice`,
/// `combinators.apply`, `combinators.map`, `combinators.filter`,
/// `combinators.reduce`, `combinators.flat_map`, `combinators.find`,
/// `combinators.some`, `combinators.every`, `combinators.group_by`,
/// `combinators.partition`, `combinators.count_by`, `combinators.tap`,
/// `combinators.match`, `combinators.constantly`, `combinators.flip`,
/// `combinators.partial`, `combinators.when`, `combinators.unless`,
/// and `combinators.converge`. The interface is declared in `fn.hnl` via
/// `op`/`handl` declarations and loaded via the generic
/// [`HandlModule`](handl_plugin_lib_handl::HandlModule); this struct
/// only delegates [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct CombinatorsModule;

impl Module for CombinatorsModule {
    fn name(&self) -> &str {
        "combinators"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        combinators_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        combinators_module().functions()
    }
}
