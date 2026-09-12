//! Validation for LSP surface rules (ADR-0100).
//!
//! The concrete implementation lives in [`handl_plugin_engine_transform`], which
//! can be consumed by lower-level crates without creating a dependency cycle.
//! `plugin-lsp` re-exports it so that the LSP engine surface is also the
//! canonical source for the validation pass.

pub use handl_plugin_engine_transform::validate::validate_lsp;
