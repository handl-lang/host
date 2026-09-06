use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::HandlModule;

/// The pure-handl implementation of `prim` (`prim-handl`) — a record of
/// arrows providing `add` and `alloc`.
const PRIM_HANDL_HNL: &str =
    include_str!("../../../handl-packages/prim-handl/prim-handl.hnl");

/// Record fields exposed by `prim-handl.hnl`.
const PRIM_HANDL_FUNCS: &[&str] = &["add", "alloc"];

/// The `prim` op names that remain native — each maps to a Rust
/// `Function` impl registered in the
/// [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).
const PRIM_OPS: &[&str] = &["prim.compare", "prim.or"];

/// Build the [`HandlModule`] for prim.
///
/// `add`/`alloc` are pure-handl record fields from `prim-handl.hnl`;
/// `compare`/`or` are op-backed (FFI atoms) dispatched to the
/// [`OpRegistry`](handl_plugin_lib_handl::OpRegistry) via `OpFunction`.
fn build_prim_module() -> HandlModule {
    let registry = Arc::new(super::prim_op_registry());
    HandlModule::new("prim", PRIM_HANDL_HNL, PRIM_HANDL_FUNCS).with_ops(PRIM_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for prim.
static PRIM_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for prim, initializing it on first call.
fn prim_module() -> &'static HandlModule {
    PRIM_MODULE.get_or_init(build_prim_module)
}

/// The `prim` module — the minimal FFI layer for handl's arrow-based stdlib.
///
/// Provides the four atom functions: `add` and `alloc` (pure handl, from
/// the `prim-handl` record), plus `compare` and `or` (native ops — see
/// `prim-handl.hnl` for why they cannot be expressed in handl today).
///
/// The interface is declared in `prim.hnl` via `op`/`handl` declarations and
/// loaded via the generic [`HandlModule`]; this struct only delegates
/// [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct PrimModule;

impl Module for PrimModule {
    fn name(&self) -> &str {
        "prim"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        prim_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        prim_module().functions()
    }
}
