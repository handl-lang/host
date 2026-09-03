//! # handl-lib-otlp
//!
//! OpenTelemetry/OTLP module for the handl runtime.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The OTLP interface is declared in
//! `kit-stdlib/data/otlp/otlp.hnl` via `op` declarations, and the Rust
//! handls (handlers) that delegate to an [`OtlpState`] are registered in
//! an [`OpRegistry`] via [`otlp_op_registry`]. The [`OtlpLibraryPlugin`]
//! wraps the generic [`HandlLibraryPlugin`] with the op registry (backed
//! by [`StubOtlpState`] by default), eliminating the per-library `Plugin`
//! trait scaffolding.
//!
//! Defines an abstract [`OtlpState`] and a [`Module`] adapter that
//! exposes `otlp.*` functions to handl programs. Concrete exporters live
//! in `handl-lib-otlp-std`.

pub mod error;
pub mod id;
pub mod otlp_module;
pub mod state;
pub mod stub;

use std::sync::Arc;

use handl_plugin_lib_handl::HandlLibraryPlugin;

pub use error::{OtlpError, OtlpResult};
pub use id::{gen_span_id, gen_trace_id, now_ns};
pub use otlp_module::{register_otlp_ops, OtlpModule, OTLP_OPS};
pub use state::{MetricKind, MetricOptions, OtlpConfig, OtlpState, StartSpanOptions};
pub use stub::{OtlpEvent, RecordedEvent, RecordedSpan, StubOtlpState};

/// The handl source declaring the otlp `op` interface — the single source
/// of truth for the OTLP API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const OTLP_HNL: &str = include_str!("../../../kit-stdlib/data/otlp/otlp.hnl");

/// Build an [`OpRegistry`](handl_plugin_lib_handl::OpRegistry) for the
/// OTLP ops, registering Rust handls that delegate to the given
/// [`OtlpState`] under their op names.
pub fn otlp_op_registry(state: Arc<dyn OtlpState>) -> handl_plugin_lib_handl::OpRegistry {
    let mut registry = handl_plugin_lib_handl::OpRegistry::new();
    register_otlp_ops(&mut registry, state);
    registry
}

// ---------------------------------------------------------------------------
// OtlpLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `otlp` interface via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The OTLP interface is declared in `otlp.hnl` via `op`/`handl`
/// declarations; the Rust handls (delegating to [`StubOtlpState`] by
/// default) are registered in the op registry.
///
/// Concrete backends such as `handl-lib-otlp-std` provide their own
/// plugin ([`OtlpStdLibraryPlugin`]) with a real [`OtlpState`] implementation.
pub struct OtlpLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl OtlpLibraryPlugin {
    /// Create a new `OtlpLibraryPlugin` backed by [`StubOtlpState`].
    pub fn new() -> Self {
        let registry = Arc::new(otlp_op_registry(Arc::new(StubOtlpState::default())));
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "otlp",
                "lib.otlp",
                "lib-otlp",
                OTLP_HNL,
                &[],
            )
            .with_ops(OTLP_OPS, registry),
        }
    }
}

impl Default for OtlpLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for OtlpLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for otlp with the op registry
/// (backed by [`StubOtlpState`]).
pub fn otlp_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(otlp_op_registry(Arc::new(StubOtlpState::default())));
    HandlLibraryPlugin::new_with_caps(
        "otlp",
        "lib.otlp",
        "lib-otlp",
        OTLP_HNL,
        &[],
    )
    .with_ops(OTLP_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`OtlpLibraryPlugin`].
pub use OtlpLibraryPlugin as LibraryPlugin;
