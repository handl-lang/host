//! Compile extension: the `hnl compile` CLI request/response seam (ADR-0114
//! Phase 14, ADR-0128 §0.3) plus a re-export of the `handl-platform` compile
//! operation.
//!
//! PL-004 T-101: the feature-gated `CompileDispatcher`/`linked_targets`
//! registry over the legacy `Compiler` trait objects is **removed** — the
//! `hnl compile` CLI path (`ext-compile`) routes each target's kernel
//! `Emitter` impl through `hnl-tool-codegen`'s `Engine` directly, so the
//! dispatcher had no consumers left. [`CompileFileRequest`] stays: it is
//! the argv parser `ext-compile` still uses. The `Compiler` trait itself
//! remains in `handl-plugin-compile` — the six `tool-compile-*` target repos
//! still implement it alongside their `Emitter` impls.

pub mod request;

#[cfg(test)]
mod tests;

pub use handl_platform::{BasicCompiler, Compile, CompileBehavior, CompileRequest};
pub use request::CompileFileRequest;
