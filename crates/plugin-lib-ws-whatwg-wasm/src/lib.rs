#[cfg(not(target_arch = "wasm32"))]
pub mod stub;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use stub::WsWhatwgWasmModule;
#[cfg(target_arch = "wasm32")]
pub use wasm::WsWhatwgWasmModule;

pub mod plugin;
pub use plugin::LibraryPlugin;
