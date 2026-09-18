//! Canonical handl adapter for the generic `plugin-lsp` engine (ADR-0100).
//!
//! This crate is intentionally thin: it assembles the canonical `Language` from
//! `handl-kit-data-canonical` and wires the `plugin-lsp` engine into the
//! `handl-host-tooling` plugin traits.  Any handl-specific behavior that cannot
//! be expressed in `lsp.yaml` is a bug in the data model, not a reason to add
//! logic here.

use handl_host_tooling::lsp::{
    CompletionPlugin, CompletionRequest, CompletionResponse, DocumentSymbolsPlugin,
    DocumentSymbolsRequest, DocumentSymbolsResponse, FoldingRangePlugin, FoldingRangeRequest,
    FoldingRangeResponse, GotoDefPlugin, GotoDefRequest, GotoDefResponse, HoverPlugin,
    HoverRequest, HoverResponse, SignatureHelpPlugin, SignatureHelpRequest, SignatureHelpResponse,
};
use handl_kit_data_canonical::language_from_yaml;
use handl_language::Language;
use handl_plugin_lsp::LspEngine;

/// A thin `Language` + `LspEngine` bundle that implements all host LSP plugin
/// traits.
pub struct BasicLsp {
    lang: Language,
}

impl BasicLsp {
    /// Create the canonical handl LSP plugin.
    ///
    /// # Panics
    ///
    /// Panics if the canonical `Language` fails to assemble — this is a build
    /// failure, not a runtime recoverable error.
    pub fn new() -> Self {
        Self {
            lang: language_from_yaml().expect("canonical language assembles"),
        }
    }
}

impl Default for BasicLsp {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionPlugin for BasicLsp {
    fn run(
        &self,
        request: &CompletionRequest,
    ) -> Result<CompletionResponse, handl_platform::PlatformError> {
        Ok(LspEngine::new(&self.lang).completion(request))
    }
}

impl HoverPlugin for BasicLsp {
    fn run(&self, request: &HoverRequest) -> Result<HoverResponse, handl_platform::PlatformError> {
        Ok(LspEngine::new(&self.lang).hover(request))
    }
}

impl GotoDefPlugin for BasicLsp {
    fn run(
        &self,
        request: &GotoDefRequest,
    ) -> Result<GotoDefResponse, handl_platform::PlatformError> {
        Ok(LspEngine::new(&self.lang).goto(request))
    }
}

impl DocumentSymbolsPlugin for BasicLsp {
    fn run(
        &self,
        request: &DocumentSymbolsRequest,
    ) -> Result<DocumentSymbolsResponse, handl_platform::PlatformError> {
        Ok(LspEngine::new(&self.lang).document_symbols(request))
    }
}

impl SignatureHelpPlugin for BasicLsp {
    fn run(
        &self,
        request: &SignatureHelpRequest,
    ) -> Result<SignatureHelpResponse, handl_platform::PlatformError> {
        Ok(LspEngine::new(&self.lang).signature_help(request))
    }
}

impl FoldingRangePlugin for BasicLsp {
    fn run(
        &self,
        request: &FoldingRangeRequest,
    ) -> Result<FoldingRangeResponse, handl_platform::PlatformError> {
        Ok(LspEngine::new(&self.lang).folding_range(request))
    }
}
