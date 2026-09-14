//! JavaScript compiler for the Handl arrow calculus (ADR-0114 Phase 3).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod compiler;
pub mod config;
pub mod emit;
pub mod lower;
pub mod output;
pub mod runtime;
pub mod security;

pub use compiler::{JsCompiler, compile_to_string};
pub use config::{
    Browser, Bun, CommonJs, Deno, Esm, Iife, Inline, JsConfig, JsOutputFormat, JsTarget, Node,
};
pub use emit::JsEmitter;
pub use emit::SecurityScanner;
pub use lower::{JsAst, JsBinOp, JsUnaryOp, lower, lower_applied};
pub use output::JsOutput;
pub use security::{ForeignDescriptor, is_safe_for_inlining, validate_foreign_descriptor};

#[cfg(test)]
mod tests;
