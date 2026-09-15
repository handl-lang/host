//! Handl-to-Handl (Hughes arrow dialect) compiler (ADR-0114).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod compiler;
pub mod config;
pub mod emit;
pub mod output;

pub use compiler::HandlCompiler;
pub use config::HandlConfig;
pub use emit::HughesEmitter;
pub use output::HandlOutput;
