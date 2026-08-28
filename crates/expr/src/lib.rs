//! # handl-expr
//!
//! Minimal compatibility shim for the Handl platform migration.

#![allow(dead_code)]

pub use handl_ast::{Expr, Pattern};
pub use handl_ast_common::ExprFactory;
pub use handl_lexer::{Span, Token};

pub mod statement;
pub mod diagnostics;

pub use diagnostics::{Diagnostic, DiagnosticLevel, Diagnostics};
pub use statement::{expr_to_pattern, ModuleOutput, Stmt, StmtKind};

// TODO: HandlerClause is not currently defined in handl-ast, handl-ast-common,
// or handl-lexer. Determine its new home and replace this comment with a
// `pub use` once the location is known.

/// Compatibility result type formerly produced by `ExpressionPipeline`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult<T> {
    pub value: T,
    pub diagnostics: Vec<Diagnostic>,
}

/// Formatting extension point for expression trees.
pub trait Formatter: Send + Sync {
    fn format<E>(&self, expr: &E) -> Result<String, handl_platform::PlatformError>;
}

/// Placeholder token kind used only to keep the legacy `Stmt` trivia fields well-typed.
#[derive(Debug, Clone, PartialEq)]
pub enum LegacyTokenKind {
    Eof,
}

impl std::fmt::Display for LegacyTokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl handl_lexer::TokenKind for LegacyTokenKind {
    type Factory = LegacyTokenFactory;
    fn is_eof(&self) -> bool { matches!(self, LegacyTokenKind::Eof) }
    fn is_error(&self) -> bool { false }
}

#[derive(Debug, Clone)]
pub struct LegacyTokenFactory;

impl handl_lexer::EofFactory<LegacyTokenKind> for LegacyTokenFactory {
    fn eof_kind(&self) -> LegacyTokenKind { LegacyTokenKind::Eof }
}
