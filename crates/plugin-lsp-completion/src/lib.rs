//! Stub completion plugin; full LSP port pending.

use handl_host_tooling::lsp::{CompletionPlugin, CompletionRequest, CompletionResponse};
use handl_platform::PlatformError;

/// A minimal completion plugin.
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicCompletion;

impl BasicCompletion {
    pub fn new() -> Self {
        Self
    }
}

impl CompletionPlugin for BasicCompletion {
    fn run(&self, _request: &CompletionRequest) -> Result<CompletionResponse, PlatformError> {
        Ok(CompletionResponse(Vec::new()))
    }
}
