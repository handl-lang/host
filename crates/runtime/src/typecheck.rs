//! Typecheck plugin trait.

use handl_platform::{AstNode, PlatformError, Value};

/// Output of a typecheck pass.
#[derive(Debug, Clone, PartialEq)]
pub struct TypecheckedOutput {
    /// Whether the typecheck succeeded.
    pub success: bool,
}

/// A plugin that can typecheck a sequence of handl statements.
pub trait TypecheckPlugin<N = AstNode, V = Value>: Send + Sync {
    /// The plugin name.
    fn name(&self) -> &str;

    /// Typecheck the provided statements.
    fn typecheck(&self, stmts: &[N]) -> Result<TypecheckedOutput, PlatformError>;
}
