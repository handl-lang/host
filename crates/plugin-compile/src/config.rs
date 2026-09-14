use std::any::Any;
use std::fmt;

/// Per-invocation configuration for a [`Compiler`](crate::Compiler).
pub trait CompilerConfig: fmt::Debug + Send + Sync {
    /// Upcast for downcasting to concrete config types.
    fn as_any(&self) -> &dyn Any;

    /// The target the compiler should emit.
    fn target(&self) -> &str;

    /// Whether debug information should be emitted.
    fn debug(&self) -> bool {
        false
    }

    /// Whether a source map should be emitted.
    fn source_map(&self) -> bool {
        false
    }
}
