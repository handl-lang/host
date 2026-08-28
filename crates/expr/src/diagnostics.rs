//! Compatibility diagnostic types for handl expressions.

use crate::Span;

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

/// Diagnostics extension point for expression trees.
pub trait Diagnostics: Send + Sync {
    fn diagnose<N>(&self, expr: &N) -> Vec<Diagnostic>;
}
