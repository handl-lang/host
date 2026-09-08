//! # handl-lib-crypto
//!
//! Cryptographic hashing library for the handl runtime.
//!
//! Provides functions for computing SHA-256 and MD5 hashes.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The crypto interface is declared in
//! `handl-packages/crypto/crypto.hnl` via `op` declarations, and the Rust
//! hash logic is registered as handls (handlers) in an [`OpRegistry`]. The
//! [`CryptoLibraryPlugin`] wraps the generic [`HandlLibraryPlugin`] with the
//! op registry, eliminating the per-library `Module`/`Plugin` trait
//! scaffolding.
//!
//! The [`CryptoModule`] re-export is preserved for backward compatibility.
//!
//! ## Functions
//!
//! - `crypto.sha256(data)` — SHA-256 hash, hex-encoded
//! - `crypto.md5(data)` — MD5 hash, hex-encoded
//!
//! ## Input conventions
//!
//! Hashing functions accept either:
//! - A **string** — treated as UTF-8 bytes
//! - An **array of integers** — each value must be in the range 0–255
//!
//! Hashing functions return a lowercase hex-encoded digest.

mod common;
mod error;
mod hash;
pub mod module;

pub use error::CryptoError;
pub use hash::{Md5, Sha256};
pub use module::CryptoModule;

use std::sync::Arc;

use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};

/// The handl source declaring the crypto `op` interface — the single source
/// of truth for the crypto API surface. Loaded at compile time from the
/// kit-stdlib data dir.
const CRYPTO_HNL: &str = include_str!("../../../handl-packages/crypto/crypto.hnl");

/// The op names declared in `crypto.hnl`.
const CRYPTO_OPS: &[&str] = &["crypto.sha256", "crypto.md5"];

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for crypto, registering the Rust handls under
/// their op names.
///
/// This registry maps:
/// - `"crypto.sha256"` → [`Sha256`] (no runtime checker; axioms trusted)
/// - `"crypto.md5"` → [`Md5`] (no runtime checker; axioms trusted)
pub fn crypto_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("crypto.sha256", Arc::new(Sha256));
    registry.register("crypto.md5", Arc::new(Md5));
    registry
}

// ---------------------------------------------------------------------------
// CryptoLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `crypto` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The crypto interface is declared in `crypto.hnl` via `op`/`handl`
/// declarations; the Rust hash handls are registered in the op registry. The
/// generic loader handles module registration.
pub struct CryptoLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl CryptoLibraryPlugin {
    /// Create a new `CryptoLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(crypto_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "crypto",
                "lib.crypto",
                "lib-crypto",
                CRYPTO_HNL,
                &[],
            )
            .with_ops(CRYPTO_OPS, registry),
        }
    }
}

impl Default for CryptoLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for CryptoLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for crypto with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`CryptoLibraryPlugin`].
pub fn crypto_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(crypto_op_registry());
    HandlLibraryPlugin::new_with_caps("crypto", "lib.crypto", "lib-crypto", CRYPTO_HNL, &[])
        .with_ops(CRYPTO_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`CryptoLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_crypto::LibraryPlugin`.
pub use CryptoLibraryPlugin as LibraryPlugin;
