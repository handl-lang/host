//! Runtime / evaluation plugin trait.

use handl_expr::Stmt;
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;

/// Output of an evaluation pass.
#[derive(Debug, Clone, PartialEq)]
pub struct EvalOutput {
    /// The values produced by evaluating each expression statement.
    pub values: Vec<NonFunctionValue>,
}

/// A plugin that can evaluate a sequence of handl statements.
pub trait RuntimePlugin: Send + Sync {
    /// The plugin name.
    fn name(&self) -> &str;

    /// Evaluate the provided statements.
    fn eval(&self, stmts: &[Stmt<NonFunctionValue>]) -> Result<EvalOutput, PlatformError>;
}
