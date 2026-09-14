//! LLVM compiler for the Handl arrow calculus (ADR-0114 Phase 8).

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod compiler;
pub mod config;
pub mod emit;
pub mod lower;
pub mod output;
pub mod rt;
pub mod target;

#[cfg(test)]
mod tests;

pub use compiler::LlvmCompiler;
pub use config::{AArch64, LlvmConfig, LlvmTarget, X86_64};
pub use emit::{
    DebugInfoBuilder, DefaultObjectWriter, DwarfLineTable, ObjectWriter, default_object_writer,
};
pub use output::LlvmOutput;
pub use target::{aarch64_triple, x86_64_triple};
