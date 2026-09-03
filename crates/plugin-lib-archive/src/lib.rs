//! # handl-lib-archive
//!
//! Archive format abstractions for the handl runtime.
//!
//! Defines format-agnostic [`Archive`] trait and [`ArchiveEntry`] so
//! individual archive format crates can be wired in as separate
//! dependencies and benchmarked independently.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The archive interface is declared in
//! `kit-stdlib/data/archive/archive.hnl`. The [`Archive`] trait and
//! [`ArchiveEntry`] remain in Rust — they are implemented by concrete
//! archive format crates (e.g. `plugin-lib-archive-tar`). The
//! [`ArchiveLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`] with
//! the `.hnl` source, eliminating the hand-rolled `Plugin` trait
//! scaffolding.

pub mod entry;
pub mod error;
pub mod traits;

pub use entry::ArchiveEntry;
pub use error::ArchiveError;
pub use traits::Archive;

use handl_plugin_lib_handl::HandlLibraryPlugin;

/// The handl source declaring the archive interface — the single source
/// of truth for the archive API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const ARCHIVE_HNL: &str = include_str!("../../../kit-stdlib/data/archive/archive.hnl");

// ---------------------------------------------------------------------------
// ArchiveLibraryPlugin (wraps HandlLibraryPlugin)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `archive` interface via the
/// generic [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The archive interface is declared in `archive.hnl`; the trait
/// definitions remain in Rust. The generic loader handles module
/// registration.
pub struct ArchiveLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl ArchiveLibraryPlugin {
    /// Create a new `ArchiveLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "archive",
                "lib.archive",
                "lib-archive",
                ARCHIVE_HNL,
                &[],
            ),
        }
    }
}

impl Default for ArchiveLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for ArchiveLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for archive.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`ArchiveLibraryPlugin`].
pub fn archive_handl_plugin() -> HandlLibraryPlugin {
    HandlLibraryPlugin::new_with_caps(
        "archive",
        "lib.archive",
        "lib-archive",
        ARCHIVE_HNL,
        &[],
    )
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`ArchiveLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_archive::LibraryPlugin`.
pub use ArchiveLibraryPlugin as LibraryPlugin;
