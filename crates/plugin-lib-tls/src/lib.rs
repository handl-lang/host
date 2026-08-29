//! # handl-lib-tls
//!
//! Abstract TLS library for the handl runtime.
//!
//! Defines the [`TlsService`] trait and a [`TlsModule`] that exposes TLS
//! primitives through the VM. This crate is I/O-free and depends on a
//! concrete backend such as `handl-lib-tls-native` to perform real
//! certificate generation and network operations.
//!
//! ## Architecture
//!
//! ```text
//! handl-plugin-vm-common          handl-lib-tls          handl-lib-tls-native
//! ─────────────            ─────────────          ────────────────────
//! Module trait             TlsService trait       NativeTls
//! ModuleHost               TlsModule              rcgen, rustls,
//!                          StubTls                tokio-rustls
//! ```
//!
//! ## Functions
//!
//! - [`TlsModule`] — `tls.version`, `tls.generate_self_signed`,
//!   `tls.parse_cert`, `tls.fingerprint`, `tls.client_config`,
//!   `tls.server_config`, `tls.connect`, `tls.accept`, `tls.read`,
//!   `tls.write`, `tls.close_conn`, `tls.close_listener`

mod error;
mod module;
mod service;
mod stub;

pub use error::TlsError;
pub use module::TlsModule;
pub use service::TlsService;
pub use stub::StubTls;


pub mod plugin;
pub use plugin::LibraryPlugin;
