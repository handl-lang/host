//! Generic, composable AST node type for ADR-0002.

use crate::language::Span;
use serde_json::Value;

/// A single, generic, composable AST node.
#[derive(Debug, Clone)]
pub struct AstNode {
    /// The node kind, keyed to registered `Behavior`s.
    pub kind: String,
    /// Optional source span.
    pub span: Option<Span>,
    /// Arbitrary JSON payload.
    pub payload: Value,
}
