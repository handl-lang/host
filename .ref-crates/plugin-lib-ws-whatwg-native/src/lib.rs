//! Native (non-wasm) WHATWG-style WebSocket client using `tokio-tungstenite`.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The ws_whatwg_native interface is declared in
//! `handl-packages/ws_whatwg_native/ws_whatwg_native.hnl` via `---` doc
//! comments and `cap` declarations. The [`WsWhatwgNativeLibraryPlugin`]
//! wraps a generic [`HandlLibraryPlugin`] for the interface declaration
//! (id, name, capabilities) while contributing the
//! [`WsWhatwgNativeModule`] into the [`ModuleRuntime`]. All the Rust
//! `Module`/`Function` impls are preserved unchanged; only the plugin
//! scaffolding was migrated.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_handl::HandlLibraryPlugin;
use handl_plugin_module::{ModuleRule, ModuleRuntime};

pub mod module;
pub use module::{WS_WHATWG_NATIVE_OPS, WsWhatwgNativeModule, ws_whatwg_native_op_registry};

/// The handl source declaring the ws_whatwg_native interface — the
/// single source of truth for the ws_whatwg_native API surface. Loaded
/// at compile time from the kit-stdlib data dir.
const WS_WHATWG_NATIVE_HNL: &str =
    include_str!("../../../handl-packages/ws_whatwg_native/ws_whatwg_native.hnl");

// ---------------------------------------------------------------------------
// WsWhatwgNativeLibraryPlugin (wraps HandlLibraryPlugin for interface declaration)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `ws_whatwg_native` module into
/// the [`ModuleRuntime`] and declares the interface via a
/// [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The [`HandlLibraryPlugin`] inner is used for the interface declaration
/// — id, name, and capabilities parsed from `ws_whatwg_native.hnl` — while
/// the custom [`Plugin::contribute`] implementation registers the
/// [`WsWhatwgNativeModule`].
///
/// All the Rust `Module`/`Function` impls are preserved unchanged; only the
/// plugin scaffolding was migrated to the op/handl form.
pub struct WsWhatwgNativeLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl WsWhatwgNativeLibraryPlugin {
    /// Create a new `WsWhatwgNativeLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "ws_whatwg_native",
                "lib.ws-whatwg-native",
                "lib-ws-whatwg-native",
                WS_WHATWG_NATIVE_HNL,
                &[],
            ),
        }
    }
}

impl Default for WsWhatwgNativeLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for WsWhatwgNativeLibraryPlugin {
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
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(WsWhatwgNativeModule))?;
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

/// Legacy `LibraryPlugin` — delegates to [`WsWhatwgNativeLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_ws_whatwg_native::LibraryPlugin`. Use
/// `LibraryPlugin::new()` to construct.
pub use WsWhatwgNativeLibraryPlugin as LibraryPlugin;
