//! Generic, composable AST node type for ADR-0002.

use serde_json::Value;

/// A span of byte offsets in source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    /// Inclusive start offset.
    pub start: usize,
    /// Exclusive end offset.
    pub end: usize,
}

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
