//! # handl-lib-collections
//!
//! Collection types for the handl runtime.
//!
//! Provides the `collections` module with `vec`, `list`, `queue`, `stack`,
//! `deque`, `hash_map`, `hash_set`, `array`, `btree_map`, `btree_set`,
//! `priority_queue`, and `bit_set`.
//!
//! Currently implemented:
//!
//! - `vec.*` — back-ended dynamic array
//! - `list.*` — front-ended list
//! - `queue.*` — FIFO queue
//! - `stack.*` — LIFO stack
//! - `deque.*` — double-ended queue
//! - `hash_map.*` — key/value dictionary backed by a hash table
//! - `hash_set.*` — unordered set of unique values
//! - `array.*` — functional array operations
//!
//! Reserved for future implementation: `btree_map`, `btree_set`,
//! `priority_queue`, `bit_set`.
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_vm_common::ModuleRuntime;
//! use handl_plugin_lib_collections::CollectionsModule;
//!
//! let host = ModuleRuntime::new()
//!     .with(CollectionsModule);
//! ```
//!
//! ## ADR-0077 Phase 4: op/handl migration
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The aggregated collections interface is declared in
//! `handl-packages/collections/collections.hnl` via `---` doc comments and
//! `cap` declarations. The [`CollectionsLibraryPlugin`] wraps a generic
//! [`HandlLibraryPlugin`] for the interface declaration (id, name,
//! capabilities) while contributing the [`CollectionsModule`] into the
//! [`ModuleRuntime`]. All the Rust `Module`/`Function` impls are preserved
//! unchanged; only the plugin scaffolding was migrated.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_handl::HandlLibraryPlugin;
use handl_plugin_module::{ModuleRule, ModuleRuntime};

mod array;
mod bit_set;
mod btree_map;
mod btree_set;
mod common;
mod deque;
mod error;
mod function_value;
mod hash_map;
mod hash_set;
mod list;
mod module;
mod ops;
pub mod priority_queue;
pub mod queue;
pub mod stack;
pub mod vec;

pub use error::CollectionsError;
pub use module::CollectionsModule;

/// The handl source declaring the aggregated collections interface — the
/// single source of truth for the collections API surface. Loaded at
/// compile time from the kit-stdlib data dir.
///
/// The `.hnl` file contains `---` doc comments describing every sub-module
/// and its ops, plus `cap` declarations for the collections-owned modules.
const COLLECTIONS_HNL: &str = include_str!("../../../handl-packages/collections/collections.hnl");

// ---------------------------------------------------------------------------
// CollectionsLibraryPlugin (wraps HandlLibraryPlugin for interface declaration)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `collections` implementation into
/// the [`ModuleRuntime`] and declares the aggregated interface via a
/// [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// Because `plugin-lib-collections` is an **aggregator** crate that
/// registers a single [`CollectionsModule`] (which dispatches to many
/// sub-modules: vec, list, queue, stack, deque, hash_map, hash_set, array),
/// it uses the [`HandlLibraryPlugin`] inner for the interface declaration
/// — id, name, and capabilities parsed from `collections.hnl` — while the
/// custom [`Plugin::contribute`] implementation registers the
/// [`CollectionsModule`] as a [`ModuleRule`].
///
/// All the Rust `Module`/`Function` impls are preserved unchanged; only the
/// plugin scaffolding was migrated to the op/handl form.
pub struct CollectionsLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl CollectionsLibraryPlugin {
    /// Create a new `CollectionsLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "collections",
                "lib.collections",
                "lib-collections",
                COLLECTIONS_HNL,
                &[],
            ),
        }
    }
}

impl Default for CollectionsLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for CollectionsLibraryPlugin {
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
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(CollectionsModule))?;
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

/// Legacy `LibraryPlugin` — delegates to [`CollectionsLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_collections::LibraryPlugin`. Use
/// `LibraryPlugin::new()` to construct.
pub use CollectionsLibraryPlugin as LibraryPlugin;
