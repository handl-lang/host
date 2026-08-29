//! Optional host capabilities.
//!
//! Each capability is behind a Cargo feature so `handl-plugin-vm-common`
//! stays cheap by default.

#[cfg(feature = "cache")]
pub mod cache;
