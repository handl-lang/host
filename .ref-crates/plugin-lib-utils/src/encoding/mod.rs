//! The `encoding` module — generic encoding to/from string forms.
//!
//! Provides functions callable as `encoding.encode(value, codec)` and
//! `encoding.decode(value, codec)` in handl expressions.
//!
//! The encoding *interface* is declared in
//! `handl-packages/utils/encoding.hnl` via `cap`/`op`/`handl` declarations
//! (ADR-0077 Phase 2). Both functions are FFI atoms — they delegate to an
//! `EncodingRegistry` (from `plugin-lib-encoding`) wrapping Base64 and Hex
//! codecs and cannot be expressed as pure handl arrows without losing the
//! exact error messages the tests rely on. The Rust `Function` impls in
//! [`ops`] are registered as handls (handlers) in an [`OpRegistry`], and the
//! generic [`HandlModule`](handl_plugin_lib_handl::HandlModule) loader
//! dispatches each call to the registered handler via [`OpFunction`].

mod ops;

#[allow(unused_imports)]
pub use ops::*;

use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_encoding::EncodingRegistry;
use handl_plugin_lib_handl::{HandlModule, OpRegistry};

use ops::{Decode, Encode};

/// The handl source declaring the encoding `op` interface — the single
/// source of truth for the encoding API surface. Loaded at compile time
/// from the kit-stdlib data dir.
///
/// `encoding.hnl` contains `---` doc comments, `cap` declarations, `op`
/// declarations, and `handl` bindings for both encoding functions. The
/// [`HandlModule`] loader strips comments and declaration lines, leaving
/// an empty expression (no pure-logic arrows); all functions are
/// resolved via [`OpFunction`] dispatching to the [`OpRegistry`].
const ENCODING_HNL: &str = include_str!("../../../../handl-packages/utils/encoding.hnl");

/// The op names declared in `encoding.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`].
const ENCODING_OPS: &[&str] = &["encoding.encode", "encoding.decode"];

/// Build the [`OpRegistry`] for encoding, registering the Rust handls under
/// their op names.
///
/// Each op name (e.g. `"encoding.encode"`) maps to its Rust `Function`
/// impl (e.g. [`Encode`]). The impls are stateful — they hold an
/// `Arc<EncodingRegistry>` — so they are constructed with the shared
/// registry and `Arc::new`ed into the registry. No runtime guarantee
/// checkers are attached — the encoding atoms are trusted.
fn encoding_op_registry() -> OpRegistry {
    let registry = Arc::new(
        EncodingRegistry::new()
            .with("base64", handl_plugin_lib_base64::Base64)
            .with("hex", handl_plugin_lib_hex::Hex),
    );
    let mut registry_ops = OpRegistry::new();
    registry_ops.register(
        "encoding.encode",
        Arc::new(Encode {
            registry: Arc::clone(&registry),
        }),
    );
    registry_ops.register(
        "encoding.decode",
        Arc::new(Decode {
            registry: Arc::clone(&registry),
        }),
    );
    registry_ops
}

/// Build the [`HandlModule`] for encoding.
///
/// Both functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the `HandlModule` dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_encoding_module() -> HandlModule {
    let registry = Arc::new(encoding_op_registry());
    HandlModule::new("encoding", ENCODING_HNL, &[]).with_ops(ENCODING_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for encoding.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
static ENCODING_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for encoding, initializing it on first
/// call.
fn encoding_module() -> &'static HandlModule {
    ENCODING_MODULE.get_or_init(build_encoding_module)
}

/// The `encoding` module — generic encoding to/from string forms.
///
/// Provides `encoding.encode(value, codec)` and
/// `encoding.decode(value, codec)`. The interface is declared in
/// `encoding.hnl` via `op`/`handl` declarations and loaded via the generic
/// [`HandlModule`](handl_plugin_lib_handl::HandlModule); this struct
/// only delegates [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct EncodingModule;

impl EncodingModule {
    /// Creates an `EncodingModule` with the default encodings.
    ///
    /// The module is backed by a shared, lazily-initialized [`HandlModule`]
    /// holding an `EncodingRegistry` with base64/hex codecs.
    pub fn new() -> Self {
        Self
    }
}

impl Default for EncodingModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for EncodingModule {
    fn name(&self) -> &str {
        "encoding"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        encoding_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        encoding_module().functions()
    }
}
