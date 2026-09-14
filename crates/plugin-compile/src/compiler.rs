use std::fmt;

use crate::{CompilerConfig, CompilerInput, CompilerOutput};

/// Error returned by a [`Compiler`](super::Compiler) when compilation fails.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    /// An I/O operation failed.
    #[error("io error: {0}")]
    Io(String),
    /// The requested target is not supported by this compiler.
    #[error("unsupported target: {0}")]
    UnsupportedTarget(String),
    /// Lowering the input to the target IR failed.
    #[error("lower error: {0}")]
    Lower(String),
    /// Emitting the target artifact failed.
    #[error("emit error: {0}")]
    Emit(String),
    /// Linking emitted artifacts failed.
    #[error("link error: {0}")]
    Link(String),
    /// The compiler input was malformed or missing required data.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// Any other compile failure, captured as a free-form message.
    #[error("{0}")]
    Other(String),
}

impl CompileError {
    /// Build an unsupported-target error for `name`.
    pub fn target(name: &str) -> Self {
        Self::UnsupportedTarget(name.to_string())
    }

    /// Build an unsupported-target error with a helpful message.
    pub fn unsupported_target(name: &str) -> Self {
        Self::UnsupportedTarget(name.to_string())
    }

    /// Build a generic compile error from `msg`.
    pub fn other(msg: &str) -> Self {
        Self::Other(msg.to_string())
    }
}

/// A source-to-source or object compiler for a single target.
///
/// Implementations are trait-object-safe and thread-safe.
pub trait Compiler: fmt::Debug + Send + Sync {
    /// Compile `input` using `config`.
    fn compile(
        &self,
        input: &CompilerInput,
        config: &dyn CompilerConfig,
    ) -> Result<Box<dyn CompilerOutput>, CompileError>;

    /// The target name this compiler produces (e.g. `"c"`, `"js"`, `"wasm"`).
    fn target(&self) -> &str;
}
