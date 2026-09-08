//! The `option` module — constructors and operations for `Option` values
//! in handl expressions.
//!
//! Provides functions callable as `option.some(v)`, `option.none()`,
//! `option.is_some(v)`, `option.unwrap(v)`, `option.map(v, f)`, etc.
//!
//! An `Option` is represented as `{"_tag": "Some", "value": <T>}` or
//! `{"_tag": "None"}`.
//!
//! The option *interface* is declared in `handl-packages/utils/option.hnl`
//! via `cap`/`op`/`handl` declarations (ADR-0077 Phase 2). All 15
//! functions are FFI atoms — they manipulate `serde_json::Value` option
//! values and call `apply_fn` from `crate::shared` and cannot be expressed
//! as pure handl arrows without losing the exact `TypeMismatch` error
//! messages the tests rely on. The Rust `Function` impls in [`ops`] are
//! registered as handls (handlers) in an [`OpRegistry`], and the generic
//! [`HandlModule`](handl_plugin_lib_handl::HandlModule) loader dispatches
//! each call to the registered handler via [`OpFunction`].

mod ops;

#[allow(unused_imports)]
pub use ops::*;

use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use ops::{
    And, Filter, FlatMap, IsNone, IsSome, Map, MapOr, NoneFn, Or, OrElse, SomeFn, Unwrap, UnwrapOr,
    UnwrapOrElse, Xor,
};

/// The handl source declaring the option `op` interface — the single
/// source of truth for the option API surface. Loaded at compile time from
/// the kit-stdlib data dir.
///
/// `option.hnl` contains `---` doc comments, `cap` declarations, `op`
/// declarations, and `handl` bindings for all 15 option functions. The
/// [`HandlModule`] loader strips comments and declaration lines, leaving
/// an empty expression (no pure-logic arrows); all functions are
/// resolved via [`OpFunction`] dispatching to the [`OpRegistry`].
const OPTION_HNL: &str = include_str!("../../../../handl-packages/utils/option.hnl");

/// The op names declared in `option.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`].
const OPTION_OPS: &[&str] = &[
    "option.some",
    "option.none",
    "option.is_some",
    "option.is_none",
    "option.unwrap",
    "option.unwrap_or",
    "option.unwrap_or_else",
    "option.map",
    "option.flat_map",
    "option.and_then",
    "option.filter",
    "option.map_or",
    "option.or",
    "option.or_else",
    "option.and",
    "option.xor",
];

/// Build the [`OpRegistry`] for option, registering the Rust handls under
/// their op names.
///
/// Each op name (e.g. `"option.some"`) maps to its Rust `Function` impl
/// (e.g. [`SomeFn`]). No runtime guarantee checkers are attached — the
/// option atoms are trusted.
fn option_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("option.some", Arc::new(SomeFn));
    registry.register("option.none", Arc::new(NoneFn));
    registry.register("option.is_some", Arc::new(IsSome));
    registry.register("option.is_none", Arc::new(IsNone));
    registry.register("option.unwrap", Arc::new(Unwrap));
    registry.register("option.unwrap_or", Arc::new(UnwrapOr));
    registry.register("option.unwrap_or_else", Arc::new(UnwrapOrElse));
    registry.register("option.map", Arc::new(Map));
    registry.register("option.flat_map", Arc::new(FlatMap));
    registry.register("option.and_then", Arc::new(FlatMap));
    registry.register("option.filter", Arc::new(Filter));
    registry.register("option.map_or", Arc::new(MapOr));
    registry.register("option.or", Arc::new(Or));
    registry.register("option.or_else", Arc::new(OrElse));
    registry.register("option.and", Arc::new(And));
    registry.register("option.xor", Arc::new(Xor));
    registry
}

/// Build the [`HandlModule`] for option.
///
/// All 15 functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the `HandlModule` dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_option_module() -> HandlModule {
    let registry = Arc::new(option_op_registry());
    HandlModule::new("option", OPTION_HNL, &[]).with_ops(OPTION_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for option.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
static OPTION_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for option, initializing it on first call.
fn option_module() -> &'static HandlModule {
    OPTION_MODULE.get_or_init(build_option_module)
}

/// The `option` module — constructors and operations for `Option` values.
///
/// Provides `option.some`, `option.none`, `option.is_some`,
/// `option.is_none`, `option.unwrap`, `option.unwrap_or`,
/// `option.unwrap_or_else`, `option.map`, `option.flat_map`,
/// `option.and_then`, `option.filter`, `option.map_or`, `option.or`,
/// `option.or_else`, `option.and`, and `option.xor`. The interface is
/// declared in `option.hnl` via `op`/`handl` declarations and loaded via
/// the generic [`HandlModule`](handl_plugin_lib_handl::HandlModule); this
/// struct only delegates [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct OptionModule;

impl Module for OptionModule {
    fn name(&self) -> &str {
        "option"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        option_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        option_module().functions()
    }
}

#[cfg(test)]
mod option_tests;
