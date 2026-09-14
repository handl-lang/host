//! Compile extension: re-export of the `handl-platform` compile operation plus
//! the concrete `js`/`llvm` compiler dispatcher (ADR-0114 Phase 14).

pub mod dispatcher;
pub mod request;

#[cfg(test)]
mod tests;

pub use dispatcher::CompileDispatcher;
pub use handl_platform::{BasicCompiler, Compile, CompileBehavior, CompileRequest};
pub use request::CompileFileRequest;
