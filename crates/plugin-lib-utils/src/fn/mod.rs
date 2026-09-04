//! The `fn` module — function combinator utilities.
//!
//! Provides functions callable as `fn.pipe(v, f, g)`, `fn.compose(f, g)`,
//! `fn.map(f, xs)`, `fn.filter(f, xs)`, `fn.reduce(f, xs, init)`, etc. in
//! handl expressions.
//!
//! The fn *interface* is declared in `handl-packages/utils/fn.hnl`
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

/// The handl source declaring the fn `op` interface — the single
/// source of truth for the fn API surface. Loaded at compile time from
/// the kit-stdlib data dir.
///
/// `fn.hnl` contains `---` doc comments, `cap` declarations, `op`
/// declarations, and `handl` bindings for all 25 fn functions. The
/// [`HandlModule`] loader strips comments and declaration lines, leaving
/// an empty expression (no pure-logic arrows); all functions are
/// resolved via [`OpFunction`] dispatching to the [`OpRegistry`].
const FN_HNL: &str = include_str!("../../../../handl-packages/utils/fn.hnl");

/// The op names declared in `fn.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`].
const FN_OPS: &[&str] = &[
    "fn.pipe",
    "fn.flow",
    "fn.compose",
    "fn.fanout",
    "fn.fanout_fn",
    "fn.choice",
    "fn.apply",
    "fn.map",
    "fn.filter",
    "fn.reduce",
    "fn.flat_map",
    "fn.find",
    "fn.some",
    "fn.every",
    "fn.group_by",
    "fn.partition",
    "fn.count_by",
    "fn.tap",
    "fn.match",
    "fn.constantly",
    "fn.flip",
    "fn.partial",
    "fn.when",
    "fn.unless",
    "fn.converge",
];

/// Build the [`OpRegistry`] for fn, registering the Rust handls under
/// their op names.
///
/// Each op name (e.g. `"fn.pipe"`) maps to its Rust `Function` impl
/// (e.g. [`Pipe`]). No runtime guarantee checkers are attached — the fn
/// atoms are trusted.
fn fn_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("fn.pipe", Arc::new(Pipe));
    registry.register("fn.flow", Arc::new(Flow));
    registry.register("fn.compose", Arc::new(Compose));
    registry.register("fn.fanout", Arc::new(Fanout));
    registry.register("fn.fanout_fn", Arc::new(FanoutFn));
    registry.register("fn.choice", Arc::new(Choice));
    registry.register("fn.apply", Arc::new(Apply));
    registry.register("fn.map", Arc::new(Map));
    registry.register("fn.filter", Arc::new(Filter));
    registry.register("fn.reduce", Arc::new(Reduce));
    registry.register("fn.flat_map", Arc::new(FlatMap));
    registry.register("fn.find", Arc::new(Find));
    registry.register("fn.some", Arc::new(SomeFn));
    registry.register("fn.every", Arc::new(Every));
    registry.register("fn.group_by", Arc::new(GroupBy));
    registry.register("fn.partition", Arc::new(Partition));
    registry.register("fn.count_by", Arc::new(CountBy));
    registry.register("fn.tap", Arc::new(Tap));
    registry.register("fn.match", Arc::new(Match));
    registry.register("fn.constantly", Arc::new(Constantly));
    registry.register("fn.flip", Arc::new(Flip));
    registry.register("fn.partial", Arc::new(Partial));
    registry.register("fn.when", Arc::new(When));
    registry.register("fn.unless", Arc::new(Unless));
    registry.register("fn.converge", Arc::new(Converge));
    registry
}

/// Build the [`HandlModule`] for fn.
///
/// All 25 functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the `HandlModule` dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_fn_module() -> HandlModule {
    let registry = Arc::new(fn_op_registry());
    HandlModule::new("fn", FN_HNL, &[]).with_ops(FN_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for fn.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
static FN_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for fn, initializing it on first call.
fn fn_module() -> &'static HandlModule {
    FN_MODULE.get_or_init(build_fn_module)
}

/// The `fn` module — function combinator utilities.
///
/// Provides `fn.pipe`, `fn.flow`, `fn.compose`, `fn.fanout`,
/// `fn.fanout_fn`, `fn.choice`, `fn.apply`, `fn.map`, `fn.filter`,
/// `fn.reduce`, `fn.flat_map`, `fn.find`, `fn.some`, `fn.every`,
/// `fn.group_by`, `fn.partition`, `fn.count_by`, `fn.tap`, `fn.match`,
/// `fn.constantly`, `fn.flip`, `fn.partial`, `fn.when`, `fn.unless`,
/// and `fn.converge`. The interface is declared in `fn.hnl` via
/// `op`/`handl` declarations and loaded via the generic
/// [`HandlModule`](handl_plugin_lib_handl::HandlModule); this struct
/// only delegates [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct FnModule;

impl Module for FnModule {
    fn name(&self) -> &str {
        "fn"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        fn_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        fn_module().functions()
    }
}
