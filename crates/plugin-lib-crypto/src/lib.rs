//! # handl-plugin-lib-crypto
//!
//! Cryptographic hashing library for the handl runtime.
//!
//! Provides functions for computing SHA-256 and MD5 hashes.
//!
//! ## Architecture
//!
//! ```text
//! handl-plugin-vm-common          handl-plugin-lib-crypto
//! ─────────────            ─────────────
//! Module trait             CryptoModule (Copy, zero-alloc)
//! ModuleHost               *Function structs
//!                          CryptoError → VmError
//! ```
//!
//! All function structs are `Copy` with no internal state.
//!
//! ## Functions
//!
//! - [`CryptoModule`] — `crypto.sha256`, `crypto.md5`
//!
//! ## Input conventions
//!
//! Hashing functions accept either:
//! - A **string** — treated as UTF-8 bytes
//! - An **array of integers** — each value must be in the range 0–255
//!
//! Hashing functions return a lowercase hex-encoded digest.
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_vm_common::ModuleHost;
//! use handl_plugin_lib_crypto::CryptoModule;
//!
//! let host = ModuleHost::new()
//!     .with(CryptoModule);
//! ```
//!
//! Expressions can then call `crypto.sha256("hello")`,
//! `crypto.md5("hello")`, etc.

mod common;
mod error;
mod hash;
pub mod module;

pub use error::CryptoError;
pub use module::CryptoModule;

