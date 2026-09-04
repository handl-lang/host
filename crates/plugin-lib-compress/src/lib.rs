//! # handl-lib-compress
//!
//! Compression abstractions for the handl runtime.
//!
//! Defines format-agnostic [`Compressor`], [`Decompressor`], and [`Codec`]
//! traits so individual format crates can be wired in as separate
//! dependencies and benchmarked independently.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The compress interface is declared in
//! `handl-packages/compress/compress.hnl`. The [`Compressor`],
//! [`Decompressor`], and [`Codec`] traits remain in Rust — they are
//! implemented by concrete compression crates (e.g. `plugin-lib-compress-gzip`).
//! The [`CompressLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`]
//! with the `.hnl` source, eliminating the hand-rolled `Plugin` trait
//! scaffolding.

pub mod error;
pub mod traits;

pub use error::CompressError;
pub use traits::{Codec, Compressor, Decompressor};

use handl_plugin_lib_handl::HandlLibraryPlugin;

/// The handl source declaring the compress interface — the single source
/// of truth for the compress API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const COMPRESS_HNL: &str = include_str!("../../../handl-packages/compress/compress.hnl");

// ---------------------------------------------------------------------------
// CompressLibraryPlugin (wraps HandlLibraryPlugin)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `compress` interface via the
/// generic [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The compress interface is declared in `compress.hnl`; the trait
/// definitions remain in Rust. The generic loader handles module
/// registration.
pub struct CompressLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl CompressLibraryPlugin {
    /// Create a new `CompressLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "compress",
                "lib.compress",
                "lib-compress",
                COMPRESS_HNL,
                &[],
            ),
        }
    }
}

impl Default for CompressLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for CompressLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for compress.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`CompressLibraryPlugin`].
pub fn compress_handl_plugin() -> HandlLibraryPlugin {
    HandlLibraryPlugin::new_with_caps(
        "compress",
        "lib.compress",
        "lib-compress",
        COMPRESS_HNL,
        &[],
    )
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`CompressLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_compress::LibraryPlugin`.
pub use CompressLibraryPlugin as LibraryPlugin;
