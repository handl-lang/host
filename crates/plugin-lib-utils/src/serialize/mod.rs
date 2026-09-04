//! The `serialize` module — generic serialization to/from strings.
//!
//! Provides functions callable as `serialize.serialize(value, format)` and
//! `serialize.deserialize(text, format)` in handl expressions.
//!
//! The serialize *interface* is declared in
//! `handl-packages/utils/serialize.hnl` via `cap`/`op`/`handl` declarations
//! (ADR-0077 Phase 2). Both functions are FFI atoms — they delegate to a
//! `SerializerRegistry` (from `plugin-lib-serialize`) wrapping Json/Yaml/Toml
//! serializers and cannot be expressed as pure handl arrows without losing
//! the exact error messages the tests rely on. The Rust `Function` impls in
//! [`ops`] are registered as handls (handlers) in an [`OpRegistry`], and the
//! generic [`HandlModule`](handl_plugin_lib_handl::HandlModule) loader
//! dispatches each call to the registered handler via [`OpFunction`].

mod ops;

#[allow(unused_imports)]
pub use ops::*;

use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::{HandlModule, OpRegistry};
use handl_plugin_lib_serialize::SerializerRegistry;

use ops::{Deserialize, Serialize};

/// The handl source declaring the serialize `op` interface — the single
/// source of truth for the serialize API surface. Loaded at compile time
/// from the kit-stdlib data dir.
///
/// `serialize.hnl` contains `---` doc comments, `cap` declarations, `op`
/// declarations, and `handl` bindings for both serialize functions. The
/// [`HandlModule`] loader strips comments and declaration lines, leaving
/// an empty expression (no pure-logic arrows); all functions are
/// resolved via [`OpFunction`] dispatching to the [`OpRegistry`].
const SERIALIZE_HNL: &str = include_str!("../../../../handl-packages/utils/serialize.hnl");

/// The op names declared in `serialize.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`].
const SERIALIZE_OPS: &[&str] = &["serialize.serialize", "serialize.deserialize"];

/// Build the [`OpRegistry`] for serialize, registering the Rust handls under
/// their op names.
///
/// Each op name (e.g. `"serialize.serialize"`) maps to its Rust `Function`
/// impl (e.g. [`Serialize`]). The impls are stateful — they hold an
/// `Arc<SerializerRegistry>` — so they are constructed with the shared
/// registry and `Arc::new`ed into the registry. No runtime guarantee
/// checkers are attached — the serialize atoms are trusted.
fn serialize_op_registry() -> OpRegistry {
    let registry = Arc::new(
        SerializerRegistry::new()
            .with("json", handl_plugin_lib_json::Json)
            .with("yaml", handl_plugin_lib_yaml::Yaml)
            .with("toml", handl_plugin_lib_toml::Toml),
    );
    let mut registry_ops = OpRegistry::new();
    registry_ops.register(
        "serialize.serialize",
        Arc::new(Serialize {
            registry: Arc::clone(&registry),
        }),
    );
    registry_ops.register(
        "serialize.deserialize",
        Arc::new(Deserialize {
            registry: Arc::clone(&registry),
        }),
    );
    registry_ops
}

/// Build the [`HandlModule`] for serialize.
///
/// Both functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the `HandlModule` dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_serialize_module() -> HandlModule {
    let registry = Arc::new(serialize_op_registry());
    HandlModule::new("serialize", SERIALIZE_HNL, &[]).with_ops(SERIALIZE_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for serialize.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
static SERIALIZE_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for serialize, initializing it on first
/// call.
fn serialize_module() -> &'static HandlModule {
    SERIALIZE_MODULE.get_or_init(build_serialize_module)
}

/// The `serialize` module — generic serialization to/from strings.
///
/// Provides `serialize.serialize(value, format)` and
/// `serialize.deserialize(text, format)`. The interface is declared in
/// `serialize.hnl` via `op`/`handl` declarations and loaded via the generic
/// [`HandlModule`](handl_plugin_lib_handl::HandlModule); this struct
/// only delegates [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct SerializeModule;

impl SerializeModule {
    /// Creates a `SerializeModule` with the default serializers.
    ///
    /// The module is backed by a shared, lazily-initialized [`HandlModule`]
    /// holding a `SerializerRegistry` with json/yaml/toml serializers.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SerializeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for SerializeModule {
    fn name(&self) -> &str {
        "serialize"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        serialize_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        serialize_module().functions()
    }
}
