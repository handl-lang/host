//! Term compiler configuration.

use std::any::Any;

use handl_plugin_compile::CompilerConfig;

/// Configuration for the Term compiler.
#[derive(Debug)]
pub struct TermConfig {
    /// The emission format (e.g. `json`, `pretty`).
    pub emit: String,
}

impl TermConfig {
    /// Create a new Term config for `emit`.
    pub fn new(emit: impl Into<String>) -> Self {
        Self { emit: emit.into() }
    }
}

impl CompilerConfig for TermConfig {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn target(&self) -> &str {
        "term"
    }
}
