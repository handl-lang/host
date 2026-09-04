//! # handl-lib-kv-hashmap
//!
//! In-memory [`HashMap`]-backed [`KeyValueStore`] implementation.
//!
//! Provides [`HashKeyValueStore`] — a thread-safe, in-memory key-value store
//! suitable for testing, development, and ephemeral workloads.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The kv-hashmap interface is declared in
//! `handl-packages/kit-stdlib/kv_hashmap/kv_hashmap.hnl` via `op`/`handl`
//! declarations, and the Rust kv logic is registered as handls (handlers)
//! in an [`OpRegistry`]. The [`KvHashmapLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the
//! per-library `Module`/`Plugin` trait scaffolding.
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_module::ModuleRuntime;
//! use handl_plugin_lib_kv::{KeyValueStoreModule};
//! use handl_plugin_lib_kv_hashmap::HashKeyValueStore;
//!
//! let host = ModuleRuntime::new()
//!     .with(KeyValueStoreModule::new(HashKeyValueStore::default()));
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};
use handl_plugin_lib_kv::{KeyValueStore, KvError};

/// The handl source declaring the kv_hashmap `op` interface — the single
/// source of truth for the kv-hashmap API surface. Loaded at compile time
/// from the kit-stdlib data dir.
const KV_HASHMAP_HNL: &str =
    include_str!("../../../handl-packages/kit-stdlib/kv_hashmap/kv_hashmap.hnl");

/// The op names declared in `kv_hashmap.hnl`.
const KV_HASHMAP_OPS: &[&str] = &[
    "kv.get",
    "kv.set",
    "kv.remove",
    "kv.clear",
    "kv.has",
    "kv.size",
];

/// In-memory key-value store backed by [`HashMap`].
///
/// Thread-safe via `Mutex`. For persistent storage, consider a future
/// sled or SQLite backend.
#[derive(Default)]
pub struct HashKeyValueStore {
    data: Mutex<HashMap<String, String>>,
}

impl HashKeyValueStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(key: impl Into<String>, value: impl Into<String>) -> Self {
        let mut data = HashMap::new();
        data.insert(key.into(), value.into());
        HashKeyValueStore {
            data: Mutex::new(data),
        }
    }
}

impl KeyValueStore for HashKeyValueStore {
    fn get(&self, key: &str) -> Result<Option<String>, KvError> {
        Ok(self.data.lock().unwrap().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), KvError> {
        self.data.lock().unwrap().insert(key.into(), value.into());
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<(), KvError> {
        self.data.lock().unwrap().remove(key);
        Ok(())
    }

    fn clear(&self) -> Result<(), KvError> {
        self.data.lock().unwrap().clear();
        Ok(())
    }

    fn has(&self, key: &str) -> Result<bool, KvError> {
        Ok(self.data.lock().unwrap().contains_key(key))
    }

    fn size(&self) -> Result<usize, KvError> {
        Ok(self.data.lock().unwrap().len())
    }
}

// ---------------------------------------------------------------------------
// Op handls (Function impls registered in the OpRegistry)
// ---------------------------------------------------------------------------

/// Shared store backing the op handls. Each op function holds a clone of
/// this `Arc` so all ops operate on the same underlying store.
fn shared_store() -> Arc<HashKeyValueStore> {
    Arc::new(HashKeyValueStore::default())
}

/// Require exactly one string argument from the call args.
fn require_one_string(args: &[serde_json::Value]) -> Result<&str, EvalError> {
    let arg = args.first().ok_or(EvalError::TypeMismatch(format!(
        "expected {} args, got {}",
        1, 0,
    )))?;
    arg.as_str().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "string", "non-string",
    )))
}

/// Require exactly two string arguments from the call args.
fn require_two_strings(args: &[serde_json::Value]) -> Result<(&str, &str), EvalError> {
    if args.len() < 2 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            2,
            args.len(),
        )));
    }
    let key = args[0].as_str().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "string", "non-string",
    )))?;
    let value = args[1].as_str().ok_or(EvalError::TypeMismatch(format!(
        "expected {}, got {}",
        "string", "non-string",
    )))?;
    Ok((key, value))
}

/// Op handl for `kv.get` — retrieve a value by key.
pub struct KvGet {
    store: Arc<HashKeyValueStore>,
}

impl Function for KvGet {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let key = require_one_string(args)?;
        let value = self.store.get(key)?;
        Ok(match value {
            Some(v) => serde_json::Value::from(v),
            None => serde_json::Value::Null,
        })
    }
}

/// Op handl for `kv.set` — set a key-value pair.
pub struct KvSet {
    store: Arc<HashKeyValueStore>,
}

impl Function for KvSet {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let (key, value) = require_two_strings(args)?;
        self.store.set(key, value)?;
        Ok(serde_json::Value::Null)
    }
}

/// Op handl for `kv.remove` — remove a key.
pub struct KvRemove {
    store: Arc<HashKeyValueStore>,
}

impl Function for KvRemove {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let key = require_one_string(args)?;
        self.store.remove(key)?;
        Ok(serde_json::Value::Null)
    }
}

/// Op handl for `kv.clear` — clear all entries.
pub struct KvClear {
    store: Arc<HashKeyValueStore>,
}

impl Function for KvClear {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        self.store.clear()?;
        Ok(serde_json::Value::Null)
    }
}

/// Op handl for `kv.has` — check if a key exists.
pub struct KvHas {
    store: Arc<HashKeyValueStore>,
}

impl Function for KvHas {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        let key = require_one_string(args)?;
        self.store
            .has(key)
            .map(serde_json::Value::from)
            .map_err(Into::into)
    }
}

/// Op handl for `kv.size` — return the number of entries.
pub struct KvSize {
    store: Arc<HashKeyValueStore>,
}

impl Function for KvSize {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        self.store
            .size()
            .map(|n| serde_json::Value::from(serde_json::json!(n)))
            .map_err(Into::into)
    }
}

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for kv-hashmap, registering the Rust handls
/// under their op names.
///
/// This registry maps:
/// - `"kv.get"` → [`KvGet`]
/// - `"kv.set"` → [`KvSet`]
/// - `"kv.remove"` → [`KvRemove`]
/// - `"kv.clear"` → [`KvClear`]
/// - `"kv.has"` → [`KvHas`]
/// - `"kv.size"` → [`KvSize`]
///
/// All handls share a single [`HashKeyValueStore`] instance so that state
/// persists across calls within the same plugin instance.
pub fn kv_hashmap_op_registry() -> OpRegistry {
    let store = shared_store();
    let mut registry = OpRegistry::new();
    registry.register("kv.get", Arc::new(KvGet { store: store.clone() }));
    registry.register("kv.set", Arc::new(KvSet { store: store.clone() }));
    registry.register("kv.remove", Arc::new(KvRemove { store: store.clone() }));
    registry.register("kv.clear", Arc::new(KvClear { store: store.clone() }));
    registry.register("kv.has", Arc::new(KvHas { store: store.clone() }));
    registry.register("kv.size", Arc::new(KvSize { store }));
    registry
}

// ---------------------------------------------------------------------------
// KvHashmapLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `kv-hashmap` implementation via
/// the generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The kv-hashmap interface is declared in `kv_hashmap.hnl` via
/// `op`/`handl` declarations; the Rust kv handls are registered in the op
/// registry. The generic loader handles module registration.
pub struct KvHashmapLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl KvHashmapLibraryPlugin {
    /// Create a new `KvHashmapLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(kv_hashmap_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "kv",
                "lib.kv-hashmap",
                "lib-kv-hashmap",
                KV_HASHMAP_HNL,
                &[],
            )
            .with_ops(KV_HASHMAP_OPS, registry),
        }
    }
}

impl Default for KvHashmapLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for KvHashmapLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for kv-hashmap with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`KvHashmapLibraryPlugin`].
pub fn kv_hashmap_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(kv_hashmap_op_registry());
    HandlLibraryPlugin::new_with_caps(
        "kv",
        "lib.kv-hashmap",
        "lib-kv-hashmap",
        KV_HASHMAP_HNL,
        &[],
    )
    .with_ops(KV_HASHMAP_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`KvHashmapLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_kv_hashmap::LibraryPlugin`.
pub use KvHashmapLibraryPlugin as LibraryPlugin;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use handl_platform::{EvalContext, Module};
    use handl_plugin_lib_kv::KeyValueStoreModule;

    use super::HashKeyValueStore;

    fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
        let func = module.function(name).expect("function exists");
        let mut ctx = EvalContext::new();
        func.call(args, &mut ctx).expect("call succeeds")
    }

    fn s(v: &str) -> serde_json::Value {
        serde_json::Value::from(v.to_string())
    }

    #[test]
    fn set_and_get() {
        let module = KeyValueStoreModule::new(HashKeyValueStore::default());
        call(&module, "set", &[s("key"), s("value")]);
        assert_eq!(call(&module, "get", &[s("key")]).as_str(), Some("value"));
    }

    #[test]
    fn get_missing_returns_null() {
        let module = KeyValueStoreModule::new(HashKeyValueStore::default());
        assert!(call(&module, "get", &[s("nope")]).is_null());
    }

    #[test]
    fn has() {
        let store = HashKeyValueStore::with("k", "v");
        let module = KeyValueStoreModule::new(store);
        assert_eq!(call(&module, "has", &[s("k")]).as_bool(), Some(true));
        assert_eq!(call(&module, "has", &[s("x")]).as_bool(), Some(false));
    }

    #[test]
    fn remove() {
        let store = HashKeyValueStore::with("k", "v");
        let module = KeyValueStoreModule::new(store);
        call(&module, "remove", &[s("k")]);
        assert_eq!(call(&module, "has", &[s("k")]).as_bool(), Some(false));
    }

    #[test]
    fn clear() {
        let store = HashKeyValueStore::with("a", "1");
        let module = KeyValueStoreModule::new(store);
        call(&module, "clear", &[]);
        assert_eq!(call(&module, "size", &[]).as_f64(), Some(0.0));
    }

    #[test]
    fn size() {
        let store = HashKeyValueStore::with("a", "1");
        let module = KeyValueStoreModule::new(store);
        assert_eq!(call(&module, "size", &[]).as_f64(), Some(1.0));
    }
}
