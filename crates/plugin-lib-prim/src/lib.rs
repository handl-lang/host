//! # handl-lib-prim
//!
//! Minimal FFI layer for handl's arrow-based stdlib.
//!
//! Provides the `prim` module — the three irreducible atom functions that
//! cannot be expressed as handl arrow graphs:
//!
//! - `prim.add(a: Int, b: Int) -> Int` — integer addition
//! - `prim.compare(a: Int, b: Int) -> Int` — returns -1, 0, or 1
//! - `prim.alloc(n: Int) -> List` — creates an empty array of size n
//! - `prim.or(a: Int, b: Int) -> Int` — bitwise OR (flag composition)
//!
//! Everything else in the stdlib is built atop these via handl arrow graphs.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 2). The prim interface is declared in
//! `handl-packages/prim/prim.hnl` via `op` declarations, and the Rust
//! atom logic is registered as handls (handlers) in an [`OpRegistry`]. The
//! [`PrimLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`] with the
//! op registry, eliminating the per-library hand-rolled `Plugin` trait
//! scaffolding.
//!
//! No runtime guarantee checkers are attached to the prim ops — the atoms
//! are trusted; their guarantees are structural and handled by the
//! inference engine's `ContractRegistry`.
//!
//! The [`PrimModule`] re-export is preserved for backward compatibility.

use std::sync::Arc;

use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};

use ops::{Add, Alloc, Compare, Or};

/// The handl source declaring the prim `op` interface — the single source
/// of truth for the prim API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const PRIM_HNL: &str = include_str!("../../../handl-packages/prim/prim.hnl");

/// The op names declared in `prim.hnl`.
const PRIM_OPS: &[&str] = &["prim.add", "prim.compare", "prim.alloc", "prim.or"];

pub mod module;
pub mod ops;

pub use module::PrimModule;

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for prim, registering the Rust handls under
/// their op names.
///
/// This registry maps:
/// - `"prim.add"` → [`Add`] (no runtime checker; atom is trusted)
/// - `"prim.compare"` → [`Compare`] (no runtime checker; atom is trusted)
/// - `"prim.alloc"` → [`Alloc`] (no runtime checker; atom is trusted)
/// - `"prim.or"` → [`Or`] (no runtime checker; atom is trusted)
///
/// No guarantee checkers are attached — the prim atoms are trusted. Their
/// guarantees are structural and handled by the inference engine's
/// `ContractRegistry`.
pub fn prim_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("prim.add", Arc::new(Add));
    registry.register("prim.compare", Arc::new(Compare));
    registry.register("prim.alloc", Arc::new(Alloc));
    registry.register("prim.or", Arc::new(Or));
    registry
}

// ---------------------------------------------------------------------------
// PrimLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `prim` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 2 replacement for the old hand-rolled `LibraryPlugin`.
/// The prim interface is declared in `prim.hnl` via `op`/`handl`
/// declarations; the Rust atom handls are registered in the op registry.
/// The generic loader handles module registration.
pub struct PrimLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl PrimLibraryPlugin {
    /// Create a new `PrimLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(prim_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps("prim", "lib.prim", "lib-prim", PRIM_HNL, &[])
                .with_ops(PRIM_OPS, registry),
        }
    }
}

impl Default for PrimLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for PrimLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for prim with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`PrimLibraryPlugin`].
pub fn prim_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(prim_op_registry());
    HandlLibraryPlugin::new_with_caps("prim", "lib.prim", "lib-prim", PRIM_HNL, &[])
        .with_ops(PRIM_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`PrimLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_prim::LibraryPlugin`.
pub use PrimLibraryPlugin as LibraryPlugin;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
