use crate::Span;
use handl_ast_canonical::Expr;
use handl_vm_values_json::NonFunctionValue;

/// Severity of a diagnostic produced by an expression diagnostics plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Info,
}

/// A single diagnostic message with an optional source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub span: Option<Span>,
}

/// Diagnostics extension point for canonical expression trees.
///
/// Implementations inspect an `Expr` AST and emit warnings, errors, or info
/// without depending on parser internals.
pub trait Diagnostics: Send + Sync {
    /// Inspects `expr` and returns any diagnostics.
    fn diagnose(&self, expr: &Expr<NonFunctionValue>) -> Vec<Diagnostic>;
}
