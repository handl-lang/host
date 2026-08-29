//! Stub goto-definition plugin; full LSP port pending.

use handl_host_tooling::lsp::{GotoDefPlugin, GotoDefRequest, GotoDefResponse};
use handl_platform::PlatformError;

/// A minimal goto-definition plugin.
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicGotoDef;

impl BasicGotoDef {
    pub fn new() -> Self {
        Self
    }
}

impl GotoDefPlugin for BasicGotoDef {
    fn run(&self, _request: &GotoDefRequest) -> Result<GotoDefResponse, PlatformError> {
        Ok(GotoDefResponse(None))
    }
}
