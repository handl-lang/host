//! Generic expression formatter trait for the platform.

use handl_vm::Expr;
use handl_vm_values_json::NonFunctionValue;

use crate::PlatformError;

/// Formatting extension point for expression trees.
pub trait Formatter: Send + Sync {
    /// Formats the given expression as a string.
    fn format(&self, expr: &Expr<NonFunctionValue>) -> Result<String, PlatformError>;
}
