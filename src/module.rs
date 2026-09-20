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

/// Pure-handl record fields exposed by `crypto.hnl`.
///
/// `crypto.sha1` is implemented in Handl (over the `math`/`bytes`/`typeof`
/// atoms) rather than as a native Rust handl — needed by the pure-Handl
/// WebSocket client, whose handshake computes
/// `base64(sha1(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"))` (RFC 6455).
const CRYPTO_FUNCS: &[&str] = &["sha1"];

/// Build the [`HandlModule`] for crypto.
///
/// `crypto.sha256`/`crypto.md5` are op-backed (FFI atoms dispatched to the
/// [`OpRegistry`] via [`OpFunction`]); `crypto.sha1` is a pure-handl arrow
/// defined in `crypto.hnl` itself, so it is exposed as a record-field
/// function via [`CRYPTO_FUNCS`]. The pure-handl body imports `math`,
/// `bytes`, and `typeof` — wired here as module dependencies.
fn build_crypto_module() -> HandlModule {
    let registry = Arc::new(super::crypto_op_registry());
    HandlModule::new("crypto", CRYPTO_HNL, CRYPTO_FUNCS)
        .with_ops(CRYPTO_OPS, registry)
        .with_dependency(
            handl_plugin_lib_stdlib::module("math").expect("crypto requires the math module"),
        )
        .with_dependency(
            handl_plugin_lib_stdlib::module("bytes").expect("crypto requires the bytes module"),
        )
        .with_dependency(
            handl_plugin_lib_stdlib::module("typeof").expect("crypto requires the typeof module"),
        )
}

/// A thread-safe lazy-initialized [`HandlModule`] for crypto.
static CRYPTO_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for crypto, initializing it on first call.
fn crypto_module() -> &'static HandlModule {
    // `with_loader_disabled`: `HandlModule::new`'s `analyze_source` would
    // otherwise run `loader.analyze` — a nested eval whose `call_host`
    // misses probe `modules.lookup`, which can re-enter this `OnceLock`
    // and deadlock. The bootstrap path is pure text transformation.
    CRYPTO_MODULE.get_or_init(|| {
        handl_plugin_lib_handl::with_loader_disabled(build_crypto_module)
    })
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

impl CryptoModule {
    /// Eagerly initialize the shared [`HandlModule`].
    ///
    /// `function()`/`functions()` lazily init through `OnceLock`; a
    /// `modules.lookup` miss that probes `crypto` while its own
    /// `HandlModule::new` is mid-init (its `analyze_source` runs a nested
    /// eval that itself does `modules.lookup`) re-enters the same
    /// `OnceLock` and deadlocks. Hosts must call this at registration
    /// time, before any eval can reach `function()`.
    pub fn prewarm() {
        crypto_module();
    }
}

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
