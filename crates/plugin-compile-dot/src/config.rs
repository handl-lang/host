//! DOT compiler configuration.

use std::any::Any;

use handl_plugin_compile::CompilerConfig;

/// Configuration for the DOT compiler.
#[derive(Debug)]
pub struct DotConfig;

impl DotConfig {
    /// Create a new DOT config.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DotConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerConfig for DotConfig {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn target(&self) -> &str {
        "dot"
    }
}
