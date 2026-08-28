//! Typecheck plugin trait.

use handl_expr::Stmt;
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;

/// Output of a typecheck pass.
#[derive(Debug, Clone, PartialEq)]
pub struct TypecheckedOutput {
    /// Whether the typecheck succeeded.
    pub success: bool,
}

/// A plugin that can typecheck a sequence of handl statements.
pub trait TypecheckPlugin: Send + Sync {
    /// The plugin name.
    fn name(&self) -> &str;

    /// Typecheck the provided statements.
    fn typecheck(&self, stmts: &[Stmt<NonFunctionValue>]) -> Result<TypecheckedOutput, PlatformError>;
}
