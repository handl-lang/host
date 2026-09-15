//! Graphviz DOT compiler for `Term` arrow graphs (ADR-0114).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod compiler;
pub mod config;
pub mod output;

pub use compiler::DotCompiler;
pub use config::DotConfig;
pub use output::DotOutput;
