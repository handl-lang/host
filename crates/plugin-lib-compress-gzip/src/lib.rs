//! # handl-lib-compress-gzip
//!
//! Gzip implementation of the [`handl_plugin_lib_compress`] traits using [`flate2`].
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The gzip interface is declared in
//! `kit-stdlib/data/compress_gzip/compress_gzip.hnl` via `op`
//! declarations, and the Rust compress/decompress logic is registered as
//! handls (handlers) in an [`OpRegistry`]. The
//! [`CompressGzipLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`]
//! with the op registry, eliminating the per-library `Module`/`Plugin`
//! trait scaffolding.
//!
//! The [`GzipModule`] re-export is preserved for backward compatibility.

use std::sync::Arc;

use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};

mod gzip;
mod module;

#[cfg(test)]
mod gzip_tests;

pub use gzip::Gzip;
pub use module::{Compress, Decompress, GzipModule};

/// The handl source declaring the gzip `op` interface — the single source
/// of truth for the gzip API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const COMPRESS_GZIP_HNL: &str =
    include_str!("../../../kit-stdlib/data/compress_gzip/compress_gzip.hnl");

/// The op names declared in `compress_gzip.hnl`.
const COMPRESS_GZIP_OPS: &[&str] = &["gzip.compress", "gzip.decompress"];

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for gzip, registering the Rust handls under
/// their op names.
///
/// This registry maps:
/// - `"gzip.compress"` → [`Compress`] (no runtime checker; axioms trusted)
/// - `"gzip.decompress"` → [`Decompress`] (no runtime checker; axioms trusted)
pub fn compress_gzip_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("gzip.compress", Arc::new(Compress));
    registry.register("gzip.decompress", Arc::new(Decompress));
    registry
}

// ---------------------------------------------------------------------------
// CompressGzipLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `gzip` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The gzip interface is declared in `compress_gzip.hnl` via `op`/`handl`
/// declarations; the Rust compress/decompress handls are registered in the
/// op registry. The generic loader handles module registration.
pub struct CompressGzipLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl CompressGzipLibraryPlugin {
    /// Create a new `CompressGzipLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(compress_gzip_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "gzip",
                "lib.compress-gzip",
                "lib-compress-gzip",
                COMPRESS_GZIP_HNL,
                &[],
            )
            .with_ops(COMPRESS_GZIP_OPS, registry),
        }
    }
}

impl Default for CompressGzipLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for CompressGzipLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for gzip with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`CompressGzipLibraryPlugin`].
pub fn compress_gzip_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(compress_gzip_op_registry());
    HandlLibraryPlugin::new_with_caps(
        "gzip",
        "lib.compress-gzip",
        "lib-compress-gzip",
        COMPRESS_GZIP_HNL,
        &[],
    )
    .with_ops(COMPRESS_GZIP_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`CompressGzipLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_compress_gzip::LibraryPlugin`.
pub use CompressGzipLibraryPlugin as LibraryPlugin;
