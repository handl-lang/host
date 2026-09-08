//! # handl-lib-otlp-std
//!
//! Concrete OTLP/HTTP exporter using `ureq`.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The OTLP interface is declared in
//! `handl-packages/otlp/otlp.hnl`, and this crate's `handl` bindings are
//! declared in `handl-packages/otlp_std/otlp_std.hnl`. The Rust handls
//! (wrapping [`HttpOtlpState`]) are registered in an [`OpRegistry`] via
//! [`otlp_std_op_registry`], which delegates to the shared
//! [`otlp_op_registry`](handl_plugin_lib_otlp::otlp_op_registry) builder.
//! The [`OtlpStdLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`]
//! with the op registry, eliminating the per-library `Plugin` trait
//! scaffolding.
//!
//! Provides [`HttpOtlpState`] and a preconfigured
//! [`StdOtlpModule`] ready for use with `ModuleRuntime`.

mod http_state;

use std::sync::Arc;

use handl_plugin_lib_handl::HandlLibraryPlugin;
use handl_plugin_lib_otlp::{OTLP_OPS, OtlpState, otlp_op_registry};

pub use handl_plugin_lib_otlp::{
    MetricKind, MetricOptions, OtlpConfig, OtlpError, OtlpEvent, OtlpModule, OtlpResult,
    RecordedEvent, RecordedSpan, StartSpanOptions, StubOtlpState,
};
pub use http_state::{HttpOtlpState, std_otlp_module};

/// The handl source declaring the otlp-std `handl` bindings — the
/// implementation-side declarations for the OTLP ops. Loaded at compile
/// time from the kit-stdlib data dir.
const OTLP_STD_HNL: &str = include_str!("../../../handl-packages/otlp_std/otlp_std.hnl");

/// Build the [`OpRegistry`](handl_plugin_lib_handl::OpRegistry) for
/// otlp-std, registering the Rust handls (wrapping [`HttpOtlpState`])
/// under the op names declared in the otlp interface.
pub fn otlp_std_op_registry() -> handl_plugin_lib_handl::OpRegistry {
    let state: Arc<dyn OtlpState> = Arc::new(HttpOtlpState::new());
    otlp_op_registry(state)
}

// ---------------------------------------------------------------------------
// OtlpStdLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `otlp-std` implementation via
/// the generic [`HandlLibraryPlugin`] with an
/// [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
pub struct OtlpStdLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl OtlpStdLibraryPlugin {
    /// Create a new `OtlpStdLibraryPlugin` backed by [`HttpOtlpState`].
    pub fn new() -> Self {
        let registry = Arc::new(otlp_std_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "otlp",
                "lib.otlp-std",
                "lib-otlp-std",
                OTLP_STD_HNL,
                &[],
            )
            .with_ops(OTLP_OPS, registry),
        }
    }
}

impl Default for OtlpStdLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for OtlpStdLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[handl_platform::CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(
        &self,
        capability: &handl_platform::CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        self.inner.metadata(capability)
    }

    fn contribute(
        &self,
        ctx: &mut handl_platform::ContributionContext,
    ) -> Result<(), handl_platform::PlatformError> {
        self.inner.contribute(ctx)
    }

    fn run(
        &self,
        request: &handl_platform::CapabilityRequest,
        ctx: &mut handl_platform::PluginContext,
    ) -> Result<handl_platform::CapabilityResponse, handl_platform::PlatformError> {
        self.inner.run(request, ctx)
    }
}

/// Build a [`HandlLibraryPlugin`] for otlp-std with the op registry
/// (backed by [`HttpOtlpState`]).
pub fn otlp_std_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(otlp_std_op_registry());
    HandlLibraryPlugin::new_with_caps("otlp", "lib.otlp-std", "lib-otlp-std", OTLP_STD_HNL, &[])
        .with_ops(OTLP_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`OtlpStdLibraryPlugin`].
pub use OtlpStdLibraryPlugin as LibraryPlugin;
