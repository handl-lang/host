//! Runtime / evaluation plugin trait.

use handl_platform::{AstNode, PlatformError};
use handl_plugin_lib_type::Value;

/// Output of an evaluation pass.
#[derive(Debug, Clone, PartialEq)]
pub struct EvalOutput<V = Value> {
    /// The values produced by evaluating each expression statement.
    pub values: Vec<V>,
}

/// A plugin that can evaluate a sequence of handl statements.
pub trait RuntimePlugin<N = AstNode, V = Value>: Send + Sync {
    /// The plugin name.
    fn name(&self) -> &str;

    /// Evaluate the provided statements.
    fn eval(&self, stmts: &[N]) -> Result<EvalOutput<V>, PlatformError>;
}
