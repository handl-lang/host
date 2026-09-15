//! S-expression compiler configuration.

use std::any::Any;

use handl_plugin_compile::CompilerConfig;

/// Configuration for the S-expression compiler.
#[derive(Debug)]
pub struct SexpConfig;

impl SexpConfig {
    /// Create a new S-expression config.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SexpConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerConfig for SexpConfig {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn target(&self) -> &str {
        "sexp"
    }
}
