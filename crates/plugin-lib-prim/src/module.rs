use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::HandlModule;

/// The handl source declaring the prim `op` interface — the single source
/// of truth for the prim API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const PRIM_HNL: &str = include_str!("../../../handl-packages/prim/prim.hnl");

/// The op names declared in `prim.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).
const PRIM_OPS: &[&str] = &["prim.add", "prim.compare", "prim.alloc", "prim.or"];

/// Build the [`HandlModule`] for prim.
///
/// All four functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the `HandlModule` dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_prim_module() -> HandlModule {
    let registry = Arc::new(super::prim_op_registry());
    HandlModule::new("prim", PRIM_HNL, &[]).with_ops(PRIM_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for prim.
static PRIM_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for prim, initializing it on first call.
fn prim_module() -> &'static HandlModule {
    PRIM_MODULE.get_or_init(build_prim_module)
}

/// The `prim` module — the minimal FFI layer for handl's arrow-based stdlib.
///
/// Provides the four irreducible atom functions: `add`, `compare`, `alloc`,
/// and `or`. Everything else in the stdlib is built atop these via handl arrow
/// graphs.
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
