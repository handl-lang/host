//! C / libclang backend for the Handl SDK generator (ADR-0107).
//!
//! Pipeline, in module order:
//! - [`backend`] — `CBackend`: libclang translation-unit setup and the
//!   `SdkgenBackend::parse_header` driver (`clang_args` → TU →
//!   `diagnostics` → collect → parse → `macro_probe` → `prune`).
//! - [`cx`] — thin safe wrappers over libclang (strings, locations,
//!   visitor helpers).
//! - [`collect`] — top-level declaration collection honoring `include`/`exclude`.
//! - [`parse`] — lowers cursors into `SdkgenModule` IR (`Record`,
//!   `Typedef`, `Function`), including `(ptr, len)` pair collapse and
//!   `out_struct` resolution.
//! - [`map`] — C type → handl port + FFI descriptor assignment.
//! - [`config`] — `.sdkgen.yaml` overrides (`SdkgenConfig`).
//!
//! `testutil` (test-only) parses C snippets into live TUs so `map`/`parse`
//! internals are unit-testable; `tests/` holds end-to-end fixtures.

#![allow(non_upper_case_globals)]

mod backend;
mod collect;
mod config;
mod cx;
mod map;
mod parse;

pub use backend::{CBackend, ManPageDocSource};
pub use config::{SdkgenConfig, SymbolOverride};

#[cfg(test)]
mod collect_test;
#[cfg(test)]
mod config_test;
#[cfg(test)]
mod testutil;
