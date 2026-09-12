//! Generic, data-driven LSP engine (ADR-0100).
//!
//! `plugin-lsp` consumes `Language::lsp` and dispatches LSP requests by the
//! method-keyed rule map.  Each rule's `kind` selects a provider; provider
//! configuration lives in `LspRule::config` as free-form data.

use handl_host_tooling::lsp::{
    CompletionRequest, CompletionResponse, DocumentSymbolsRequest, DocumentSymbolsResponse,
    GotoDefRequest, GotoDefResponse, HoverRequest, HoverResponse,
};
use handl_language::{Language, LspDef};

/// Generic LSP engine bound to a `Language`.
pub struct LspEngine<'l> {
    _lang: &'l Language,
    /// The first `LspDef` in `Language::lsp` (multiple fragments can be merged;
    /// for now the engine consults the canonical one with id `canonical.lsp`).
    _def: Option<&'l LspDef>,
}

impl<'l> LspEngine<'l> {
    /// Create an engine for `lang`.
    pub fn new(lang: &'l Language) -> Self {
        let def = lang
            .lsp
            .get_by_id("canonical.lsp")
            .or_else(|| lang.lsp.iter().next());
        Self { _lang: lang, _def: def }
    }

    /// Evaluate `textDocument/completion` against the language's rules.
    pub fn completion(&self, _request: &CompletionRequest) -> CompletionResponse {
        // TODO: dispatch `kind` handlers from `Language::lsp` rules.
        CompletionResponse(Vec::new())
    }

    /// Evaluate `textDocument/hover` against the language's rules.
    pub fn hover(&self, _request: &HoverRequest) -> HoverResponse {
        // TODO: dispatch `kind` handlers from `Language::lsp` rules.
        HoverResponse(None)
    }

    /// Evaluate `textDocument/definition` against the language's rules.
    pub fn goto(&self, _request: &GotoDefRequest) -> GotoDefResponse {
        // TODO: dispatch `kind` handlers from `Language::lsp` rules.
        GotoDefResponse(None)
    }

    /// Evaluate `textDocument/documentSymbol` against the language's rules.
    pub fn document_symbols(&self, _request: &DocumentSymbolsRequest) -> DocumentSymbolsResponse {
        // TODO: dispatch `kind` handlers from `Language::lsp` rules.
        DocumentSymbolsResponse(Vec::new())
    }
}
