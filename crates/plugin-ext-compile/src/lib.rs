//! Compile extension: re-export of the `handl-platform` compile operation plus
//! the feature-gated compiler dispatcher (ADR-0114 Phase 14, ADR-0128 §0.3).

pub mod dispatcher;
pub mod request;

#[cfg(test)]
mod tests;

pub use dispatcher::{CompileDispatcher, linked_targets};
pub use handl_platform::{BasicCompiler, Compile, CompileBehavior, CompileRequest};
pub use request::CompileFileRequest;
