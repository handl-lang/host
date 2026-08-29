//! Generic, composable AST node type.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A span of byte offsets in source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Span {
    /// Inclusive start offset.
    pub start: usize,
    /// Exclusive end offset.
    pub end: usize,
}

/// A single, generic, composable AST node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstNode {
    /// The node kind, keyed to registered `Behavior`s.
    pub kind: String,
    /// Optional source span.
    pub span: Option<Span>,
    /// Arbitrary JSON payload.
    pub payload: Value,
}
