#[cfg(target_arch = "wasm32")]
pub mod wasm;
#[cfg(not(target_arch = "wasm32"))]
pub mod stub;

#[cfg(target_arch = "wasm32")]
pub use wasm::WsWhatwgWasmModule;
#[cfg(not(target_arch = "wasm32"))]
pub use stub::WsWhatwgWasmModule;
