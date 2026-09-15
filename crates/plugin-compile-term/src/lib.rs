//! Term JSON compiler for canonical Handl sources (ADR-0114).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod compiler;
pub mod config;
pub mod output;

pub use compiler::TermCompiler;
pub use config::TermConfig;
pub use output::TermOutput;
