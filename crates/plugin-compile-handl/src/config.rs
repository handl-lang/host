//! Handl compiler configuration.

use std::any::Any;

use handl_plugin_compile::CompilerConfig;

/// Configuration for the Handl-to-Handl compiler.
#[derive(Debug)]
pub struct HandlConfig {
    /// The Handl output target (e.g. `hughes`).
    pub to: String,
}

impl HandlConfig {
    /// Create a new Handl config for `to`.
    pub fn new(to: impl Into<String>) -> Self {
        Self {
            to: to.into(),
        }
    }
}

impl CompilerConfig for HandlConfig {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn target(&self) -> &str {
        "handl"
    }
}
