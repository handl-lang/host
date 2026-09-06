use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::HandlModule;

/// The handl source declaring the crypto `op` interface — the single source
/// of truth for the crypto API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const CRYPTO_HNL: &str = include_str!("../../../handl-packages/crypto/crypto.hnl");

/// The op names declared in `crypto.hnl` — each maps to a Rust `Function`
/// impl registered in the [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).
const CRYPTO_OPS: &[&str] = &["crypto.sha256", "crypto.md5"];

/// Build the [`HandlModule`] for crypto.
///
/// All functions are op-backed (FFI atoms). No pure-logic arrows are
/// used (`func_names` is empty); the `HandlModule` dispatches every
/// function lookup to the [`OpRegistry`] via [`OpFunction`].
fn build_crypto_module() -> HandlModule {
    let registry = Arc::new(super::crypto_op_registry());
    HandlModule::new("crypto", CRYPTO_HNL, &[]).with_ops(CRYPTO_OPS, registry)
}

/// A thread-safe lazy-initialized [`HandlModule`] for crypto.
static CRYPTO_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for crypto, initializing it on first call.
fn crypto_module() -> &'static HandlModule {
    CRYPTO_MODULE.get_or_init(build_crypto_module)
}

/// The `crypto` module — hashing functions.
///
/// Provides functions callable as `crypto.sha256(data)` and
/// `crypto.md5(data)` in handl expressions.
///
/// Hashing functions accept a string (UTF-8 bytes) or an array of
/// integers (0–255) and return a lowercase hex-encoded digest.
///
/// The interface is declared in `crypto.hnl` via `op`/`handl` declarations
/// and loaded via the generic [`HandlModule`]; this struct only delegates
/// [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct CryptoModule;

impl Module for CryptoModule {
    fn name(&self) -> &str {
        "crypto"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        crypto_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        crypto_module().functions()
    }
}
