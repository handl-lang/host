//! Minimal typed LSP JSON contract used by `handl-host-lsp`.
//!
//! These are local glue types (not the engine-level `handl_host_tooling::lsp`
//! types) and serialise directly to the wire shapes expected by LSP clients.

use serde::{Deserialize, Serialize};

/// LSP 0-based line/character position.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// LSP range defined by a start and end position.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// LSP location: a URI plus a range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

/// LSP `DocumentSymbol` for `textDocument/documentSymbol` responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub range: Range,
    #[serde(rename = "selectionRange")]
    pub selection_range: Range,
}

/// LSP `SymbolInformation` for `workspace/symbol` responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolInformation {
    pub name: String,
    pub kind: u32,
    pub location: Location,
    #[serde(rename = "containerName")]
    pub container_name: String,
}
