//! WebIDL backend for the Handl SDK generator (ADR-0113).

use handl_plugin_sdkgen::{SdkgenBackend, SdkgenError, SdkgenModule};
use std::path::Path;

/// WebIDL parser using a placeholder implementation.
pub struct WebIdlBackend;

impl WebIdlBackend {
    /// Create a new WebIDL backend.
    pub fn new() -> Self {
        Self
    }
}

impl SdkgenBackend for WebIdlBackend {
    /// Parse a WebIDL file and produce an `SdkgenModule`.
    fn parse_header(&self, _path: &Path) -> Result<SdkgenModule, SdkgenError> {
        Err(SdkgenError("WebIDL backend is not yet implemented".into()))
    }
}
