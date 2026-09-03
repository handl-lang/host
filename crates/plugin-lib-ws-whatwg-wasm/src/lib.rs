//! WASM WHATWG-style WebSocket client module for the handl runtime.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The ws_whatwg_wasm interface is declared in
//! `kit-stdlib/data/ws_whatwg_wasm/ws_whatwg_wasm.hnl` via `---` doc
//! comments and `cap` declarations. The [`WsWhatwgWasmLibraryPlugin`]
//! wraps a generic [`HandlLibraryPlugin`] for the interface declaration
//! (id, name, capabilities) while contributing the
//! [`WsWhatwgWasmModule`] into the [`ModuleRuntime`]. All the Rust
//! `Module`/`Function` impls are preserved unchanged; only the plugin
//! scaffolding was migrated.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_handl::HandlLibraryPlugin;
use handl_plugin_module::{ModuleRule, ModuleRuntime};

#[cfg(not(target_arch = "wasm32"))]
pub mod stub;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use stub::WsWhatwgWasmModule;
#[cfg(target_arch = "wasm32")]
pub use wasm::WsWhatwgWasmModule;

/// The handl source declaring the ws_whatwg_wasm interface — the
/// single source of truth for the ws_whatwg_wasm API surface. Loaded
/// at compile time from the kit-stdlib data dir.
const WS_WHATWG_WASM_HNL: &str =
    include_str!("../../../kit-stdlib/data/ws_whatwg_wasm/ws_whatwg_wasm.hnl");

// ---------------------------------------------------------------------------
// WsWhatwgWasmLibraryPlugin (wraps HandlLibraryPlugin for interface declaration)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `ws_whatwg_wasm` module into
/// the [`ModuleRuntime`] and declares the interface via a
/// [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The [`HandlLibraryPlugin`] inner is used for the interface declaration
/// — id, name, and capabilities parsed from `ws_whatwg_wasm.hnl` — while
/// the custom [`Plugin::contribute`] implementation registers the
/// [`WsWhatwgWasmModule`].
///
/// All the Rust `Module`/`Function` impls are preserved unchanged; only the
/// plugin scaffolding was migrated to the op/handl form.
pub struct WsWhatwgWasmLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl WsWhatwgWasmLibraryPlugin {
    /// Create a new `WsWhatwgWasmLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "ws_whatwg_wasm",
                "lib.ws-whatwg-wasm",
                "lib-ws-whatwg-wasm",
                WS_WHATWG_WASM_HNL,
                &[],
            ),
        }
    }
}

impl Default for WsWhatwgWasmLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for WsWhatwgWasmLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(WsWhatwgWasmModule))?;
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`WsWhatwgWasmLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_ws_whatwg_wasm::LibraryPlugin`. Use
/// `LibraryPlugin::new()` to construct.
pub use WsWhatwgWasmLibraryPlugin as LibraryPlugin;
