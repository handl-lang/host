//! # handl-lib-prim
//!
//! Minimal FFI layer for handl's arrow-based stdlib.
//!
//! Provides the `prim` module — the four atom functions of the `prim`
//! contract (`handl-packages/prim/prim.hnl`):
//!
//! - `prim.add(a: Int, b: Int) -> Int` — integer addition (**pure handl**:
//!   `prim-handl` over the builtin `+`)
//! - `prim.compare(a, b) -> Int` — deep total order over arbitrary values;
//!   returns -1, 0, or 1 (**native `op`** — a handl encoding is unblocked
//!   by `type.of`/`obj.keys` but deferred to avoid ordering-semantics
//!   drift)
//! - `prim.alloc(n: Int) -> List` — creates an empty array (**pure handl**)
//! - `prim.or(a: Int, b: Int) -> Int` — bitwise OR, flag composition
//!   (**pure handl** — `[a, b] ~> math.bit_or(a, b)` over the `math`
//!   module's integer bitwise op)
//!
//! Everything else in the stdlib is built atop these via handl arrow graphs.
//!
//! This crate follows the `op`/`handl` declaration form (ADR-0077): the
//! `prim` interface is declared in `handl-packages/prim/prim.hnl`, the
//! pure-handl impls live in `handl-packages/prim-handl/prim-handl.hnl`
//! (loaded as a record module like `str-handl`/`unicode-handl`), and the
//! remaining native atoms are registered as handls in an [`OpRegistry`].
//! The [`PrimLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`] with
//! the op registry.
//!
//! No runtime guarantee checkers are attached to the prim ops — the atoms
//! are trusted; their guarantees are structural and handled by the
//! inference engine's `ContractRegistry`.
//!
//! The [`PrimModule`] re-export is preserved for backward compatibility.

use std::sync::Arc;

use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry, parse_cap_declarations};

use ops::Compare;

/// The handl source declaring the prim `op` interface — the single source
/// of truth for the prim API surface and its `cap` capability
/// declarations. Loaded at compile time from the kit-stdlib data dir.
const PRIM_HNL: &str = include_str!("../../../handl-packages/prim/prim.hnl");

/// The pure-handl implementation of `prim` (`prim-handl`) — a record of
/// arrows providing `add` and `alloc`. The remaining `prim.*` functions
/// stay native `op`s.
const PRIM_HANDL_HNL: &str =
    include_str!("../../../handl-packages/prim-handl/prim-handl.hnl");

/// Record fields exposed by `prim-handl.hnl` — these become callable
/// [`HandlFunction`](handl_plugin_lib_handl::HandlFunction)s.
const PRIM_HANDL_FUNCS: &[&str] = &["add", "alloc", "or"];

/// The `prim` op names that remain native — each maps to a Rust
/// `Function` impl registered in the [`OpRegistry`].
const PRIM_OPS: &[&str] = &["prim.compare"];

/// The `math` table module — a dependency of `prim-handl`'s `or`
/// (`math.bit_or`).
fn math_module() -> handl_plugin_lib_handl::HandlModule {
    handl_plugin_lib_stdlib::module("math").expect("math table module")
}

/// The capability ids declared by the `prim` contract (`prim.hnl`).
fn prim_capabilities() -> Vec<handl_platform::CapabilityId> {
    parse_cap_declarations(PRIM_HNL)
        .into_iter()
        .map(handl_platform::CapabilityId::from)
        .collect()
}

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
/// - `"prim.compare"` → [`Compare`] (no runtime checker; atom is trusted)
///
/// `prim.add`, `prim.alloc`, and `prim.or` have no entries here — they are
/// pure handl, provided by the `prim-handl` record.
///
/// No guarantee checkers are attached — the prim atoms are trusted. Their
/// guarantees are structural and handled by the inference engine's
/// `ContractRegistry`.
pub fn prim_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("prim.compare", Arc::new(Compare));
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
            inner: HandlLibraryPlugin::new(
                "prim",
                "lib.prim",
                "lib-prim",
                PRIM_HANDL_HNL,
                PRIM_HANDL_FUNCS,
            )
            .with_capabilities(prim_capabilities())
            .with_dependency(math_module())
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
    HandlLibraryPlugin::new("prim", "lib.prim", "lib-prim", PRIM_HANDL_HNL, PRIM_HANDL_FUNCS)
        .with_capabilities(prim_capabilities())
        .with_dependency(math_module())
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
