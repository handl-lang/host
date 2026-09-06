//! Stub implementation of `ws_whatwg_wasm` for non-wasm targets.

use handl_platform::Function;
use handl_platform::Module;

#[derive(Clone, Copy)]
pub struct WsWhatwgWasmModule;

impl Module for WsWhatwgWasmModule {
    fn name(&self) -> &str {
        "ws_whatwg_wasm"
    }

    fn function(&self, _name: &str) -> Option<&dyn Function> {
        None
    }
}
