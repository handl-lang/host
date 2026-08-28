//! Stub document-symbols plugin; full LSP port pending.

use handl_platform::PlatformError;
use handl_tooling::lsp::{
    DocumentSymbolsPlugin, DocumentSymbolsRequest, DocumentSymbolsResponse,
};

/// A minimal document-symbols plugin.
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicDocumentSymbols;

impl BasicDocumentSymbols {
    pub fn new() -> Self {
        Self
    }
}

impl DocumentSymbolsPlugin for BasicDocumentSymbols {
    fn run(
        &self,
        _request: &DocumentSymbolsRequest,
    ) -> Result<DocumentSymbolsResponse, PlatformError> {
        Ok(DocumentSymbolsResponse(Vec::new()))
    }
}
