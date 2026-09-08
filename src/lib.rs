//! # handl-lib-kv
//!
//! Key-value store abstraction for the handl runtime.
//!
//! Defines the [`KeyValueStore`] trait, [`KeyValueStoreModule`] adapter, and
//! [`KvError`]. The `kv.*` functions are callable from handl expressions:
//! `kv.get("key")`, `kv.set("key", "value")`, etc.
//!
//! Unlike platform services (filesystem, env, http), a key-value store is a
//! stateful data abstraction — not tied to any OS capability. Backends may
//! include in-memory HashMaps, sled, SQLite, Redis, etc.
//!
//! - [`handl_plugin_lib_kv_hashmap`] — in-memory `HashMap` backend
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The kv interface is declared in
//! `handl-packages/kv/kv.hnl`. The [`KeyValueStore`] trait and
//! [`KeyValueStoreModule`] adapter remain in Rust — they are implemented by
//! concrete backend crates (e.g. `plugin-lib-kv-hashmap`). The
//! [`KvLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`] with the
//! `.hnl` source, eliminating the hand-rolled `Plugin` trait scaffolding.
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_module::ModuleRuntime;
//! use handl_plugin_lib_kv::KeyValueStoreModule;
//! use handl_plugin_lib_kv_hashmap::HashKeyValueStore;
//!
//! let host = ModuleRuntime::new()
//!     .with(KeyValueStoreModule::new(HashKeyValueStore::default()));
//! ```

#![allow(clippy::module_inception)]

mod error;
mod key_value_store;

pub use error::KvError;
pub use key_value_store::{KeyValueStore, KeyValueStoreModule};

use handl_plugin_lib_handl::HandlLibraryPlugin;

/// The handl source declaring the kv interface — the single source of
/// truth for the kv API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const KV_HNL: &str = include_str!("../../../handl-packages/kv/kv.hnl");

// ---------------------------------------------------------------------------
// KvLibraryPlugin (wraps HandlLibraryPlugin)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `kv` interface via the
/// generic [`HandlLibraryPlugin`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The kv interface is declared in `kv.hnl`; the trait definitions remain
/// in Rust. The generic loader handles module registration.
pub struct KvLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl KvLibraryPlugin {
    /// Create a new `KvLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps("kv", "lib.kv", "lib-kv", KV_HNL, &[]),
        }
    }
}

impl Default for KvLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for KvLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for kv.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`KvLibraryPlugin`].
pub fn kv_handl_plugin() -> HandlLibraryPlugin {
    HandlLibraryPlugin::new_with_caps("kv", "lib.kv", "lib-kv", KV_HNL, &[])
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`KvLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_kv::LibraryPlugin`.
pub use KvLibraryPlugin as LibraryPlugin;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::Plugin;

    #[test]
    fn plugin_id_and_name() {
        let plugin = KvLibraryPlugin::new();
        assert_eq!(plugin.id(), "lib.kv");
        assert_eq!(plugin.name(), "lib-kv");
    }

    #[test]
    fn plugin_has_capabilities() {
        let plugin = KvLibraryPlugin::new();
        let caps = plugin.capabilities();
        assert_eq!(caps.len(), 6);
        let names: Vec<&str> = caps.iter().map(|c| c.as_str()).collect();
        assert!(names.contains(&"kv.get"));
        assert!(names.contains(&"kv.set"));
        assert!(names.contains(&"kv.remove"));
        assert!(names.contains(&"kv.clear"));
        assert!(names.contains(&"kv.has"));
        assert!(names.contains(&"kv.size"));
    }

    #[test]
    fn handl_plugin_id_and_name() {
        let plugin = kv_handl_plugin();
        assert_eq!(plugin.id(), "lib.kv");
        assert_eq!(plugin.name(), "lib-kv");
    }
}
