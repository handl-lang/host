use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::HandlModule;

/// The pure-handl implementation of `prim` (`prim-handl`) — a record of
/// arrows providing `add`, `alloc`, and `or`.
const PRIM_HANDL_HNL: &str =
    include_str!("../../../handl-packages/prim-handl/prim-handl.hnl");

/// Record fields exposed by `prim-handl.hnl`.
const PRIM_HANDL_FUNCS: &[&str] = &["add", "alloc", "or"];

/// The `prim` op names that remain native — each maps to a Rust
/// `Function` impl registered in the
/// [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).
const PRIM_OPS: &[&str] = &["prim.compare"];

/// Build the [`HandlModule`] for prim.
///
/// `add`/`alloc`/`or` are pure-handl record fields from `prim-handl.hnl`;
/// `compare` is an op-backed (FFI atom) dispatched to the
/// [`OpRegistry`](handl_plugin_lib_handl::OpRegistry) via `OpFunction`.
/// The `math` table module is a dependency — `or` calls `math.bit_or`.
fn build_prim_module() -> HandlModule {
    let registry = Arc::new(super::prim_op_registry());
    HandlModule::new("prim", PRIM_HANDL_HNL, PRIM_HANDL_FUNCS)
        .with_dependency(super::math_module())
        .with_ops(PRIM_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for prim.
static PRIM_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for prim, initializing it on first call.
fn prim_module() -> &'static HandlModule {
    PRIM_MODULE.get_or_init(build_prim_module)
}

/// The `prim` module — the minimal FFI layer for handl's arrow-based stdlib.
///
/// Provides the four atom functions: `add`, `alloc`, and `or` (pure handl,
/// from the `prim-handl` record — `or` sits over `math.bit_or`), plus
/// `compare` (native op — see `prim-handl.hnl` for why it stays native).
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
