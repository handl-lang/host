//! Stub implementation of `ws_whatwg_wasm` for non-wasm targets.

use handl_vm::Function;
use handl_vm_common::Module;
use handl_vm_values_json::NonFunctionValue;

#[derive(Clone, Copy)]
pub struct WsWhatwgWasmModule;

impl Module for WsWhatwgWasmModule {
    fn name(&self) -> &str {
        "ws_whatwg_wasm"
    }

    fn function(&self, _name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        None
    }
}
