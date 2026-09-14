//! Contract crate for the `hnl compile` plugin architecture (ADR-0114).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Caching for compile artifacts.
pub mod cache;
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
/// Registry and parallel dispatch over compilers.
pub mod registry;
/// Source map types.
pub mod source_map;
/// Value ABI for lowering Handl values.
pub mod value_abi;

#[cfg(test)]
mod tests;

pub use cache::{ArtifactCache, CachedOutput, MemoryArtifactCache, cache_key};
pub use compiler::{CompileError, Compiler};
pub use config::CompilerConfig;
pub use debug_info::{DebugInfo, Null};
pub use input::CompilerInput;
pub use output::{Artifact, ArtifactContent, CompilerOutput};
pub use registry::CompilerRegistry;
pub use source_map::{SourceMap, V3};
pub use value_abi::{Arrow, EvalCtx, Tag, Value, ValueAbi, helpers};
