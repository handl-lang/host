use handl_ast_canonical::Expr;
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;

/// Formatting extension point for canonical expression trees.
///
/// Implementations produce a canonical source string from an `Expr` AST
/// without depending on parser internals.
pub trait Formatter: Send + Sync {
    /// Formats `expr` as a canonical source string.
    fn format(&self, expr: &Expr<NonFunctionValue>) -> Result<String, PlatformError>;
}
