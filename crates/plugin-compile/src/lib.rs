//! Contract crate for the `hnl compile` plugin architecture (ADR-0114).
//!
//! PL-004 T-101: this crate is the *contract* the six `tool-compile-*` target
//! repos implement against — [`CompilerInput`], [`CompilerOutput`],
//! [`Artifact`], [`CompilerConfig`], and the [`Compiler`] trait (each
//! target repo implements it alongside its kernel `Emitter` impl).
//! The parallel-dispatch half (`CompilerRegistry`, `compile_many*`, the
//! `ArtifactCache`) is **removed**: its only consumer was
//! `handl-plugin-ext-compile`'s `CompileDispatcher`, which the PL-004
//! kernel path bypassed — `ext-compile` drives each target's `Emitter`
//! through `hnl-tool-codegen`'s `Engine` directly. The `Compiler` trait
//! stays because the target repos still implement it; whether those
//! impls should also go is a per-repo cleanup, not a host change.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Compiler contract and error types.
pub mod compiler;
/// Compiler configuration abstractions.
pub mod config;
/// Debug info and null representations.
pub mod debug_info;
/// Inputs to a compiler.
pub mod input;
/// Artifacts and compiler outputs.
pub mod output;
/// Source map types.
pub mod source_map;
/// Value ABI for lowering Handl values.
pub mod value_abi;

#[cfg(test)]
mod tests;

pub use compiler::{CompileError, Compiler};
pub use config::CompilerConfig;
pub use debug_info::{DebugInfo, Null};
pub use input::CompilerInput;
pub use output::{Artifact, ArtifactContent, CompilerOutput};
pub use source_map::{SourceMap, V3};
pub use value_abi::{Arrow, EvalCtx, Tag, Value, ValueAbi, helpers};
