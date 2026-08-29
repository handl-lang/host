//! # handl-expr
//!
//! Minimal compatibility shim for the Handl platform migration.
//!
//! This crate no longer contains a fixed `StmtKind` enum or canonical
//! statement vocabulary; those live in `handl-kit-canonical`.  It re-exports
//! the token, expression, and diagnostics convenience types that legacy
//! plugins still consume during the ADR-0034 transition.

#![allow(dead_code)]

pub use handl_ast_common::ExprFactory;
pub use handl_kit_canonical::{
    CanonicalKind, Diagnostic, DiagnosticLevel, Diagnostics, Stmt, StmtKind,
};
pub use handl_lexer::{Span, Token};
pub use handl_vm::{Expr, Pattern};
use handl_vm_values_json::NonFunctionValue;

// TODO(ADR-0034): HandlerClause is not currently defined in handl-ast,
// handl-ast-common, or handl-lexer. Determine its new home and replace this
// comment with a `pub use` once the location is known.

/// Compatibility result type formerly produced by `ExpressionPipeline`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult<T> {
    pub value: T,
    pub diagnostics: Vec<Diagnostic>,
}

/// Formatting extension point for expression trees.
pub trait Formatter: Send + Sync {
    fn format(&self, expr: &Expr<NonFunctionValue>) -> Result<String, handl_platform::PlatformError>;
}
