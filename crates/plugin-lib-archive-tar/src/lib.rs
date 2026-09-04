//! # handl-lib-archive-tar
//!
//! Tar implementation of the [`handl_plugin_lib_archive`] [`Archive`] trait using [`tar`].
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The tar interface is declared in
//! `handl-packages/kit-stdlib/archive_tar/archive_tar.hnl` via `op` declarations,
//! and the Rust pack/unpack logic is registered as handls (handlers) in
//! an [`OpRegistry`]. The [`ArchiveTarLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the
//! per-library `Module`/`Plugin` trait scaffolding.
//!
//! The [`TarModule`] re-export is preserved for backward compatibility.

use std::sync::Arc;

use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};

mod module;
mod tar_archive;

#[cfg(test)]
mod tar_tests;

pub use module::{TarModule, TarPack, TarUnpack};
pub use tar_archive::TarArchive;

/// The handl source declaring the tar `op` interface — the single source
/// of truth for the tar API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const ARCHIVE_TAR_HNL: &str = include_str!("../../../handl-packages/kit-stdlib/archive_tar/archive_tar.hnl");

/// The op names declared in `archive_tar.hnl`.
const ARCHIVE_TAR_OPS: &[&str] = &["tar.pack", "tar.unpack"];

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for tar, registering the Rust handls under
/// their op names.
///
/// This registry maps:
/// - `"tar.pack"` → [`TarPack`] (no runtime checker; axioms trusted)
/// - `"tar.unpack"` → [`TarUnpack`] (no runtime checker; axioms trusted)
pub fn archive_tar_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("tar.pack", Arc::new(TarPack));
    registry.register("tar.unpack", Arc::new(TarUnpack));
    registry
}

// ---------------------------------------------------------------------------
// ArchiveTarLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `tar` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The tar interface is declared in `archive_tar.hnl` via `op`/`handl`
/// declarations; the Rust pack/unpack handls are registered in the op
/// registry. The generic loader handles module registration.
pub struct ArchiveTarLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl ArchiveTarLibraryPlugin {
    /// Create a new `ArchiveTarLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(archive_tar_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "tar",
                "lib.archive-tar",
                "lib-archive-tar",
                ARCHIVE_TAR_HNL,
                &[],
            )
            .with_ops(ARCHIVE_TAR_OPS, registry),
        }
    }
}

impl Default for ArchiveTarLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for ArchiveTarLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for tar with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`ArchiveTarLibraryPlugin`].
pub fn archive_tar_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(archive_tar_op_registry());
    HandlLibraryPlugin::new_with_caps(
        "tar",
        "lib.archive-tar",
        "lib-archive-tar",
        ARCHIVE_TAR_HNL,
        &[],
    )
    .with_ops(ARCHIVE_TAR_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`ArchiveTarLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_archive_tar::LibraryPlugin`.
pub use ArchiveTarLibraryPlugin as LibraryPlugin;
