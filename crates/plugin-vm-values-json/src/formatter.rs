//! Generic expression formatter trait for the JSON-backed VM.

use handl_plugin_vm::Expr;

use crate::{Error, NonFunctionValue};

/// Formatting extension point for expression trees.
pub trait Formatter: Send + Sync {
    /// Formats the given expression as a string.
    fn format(&self, expr: &Expr<NonFunctionValue>) -> Result<String, Error>;
}
