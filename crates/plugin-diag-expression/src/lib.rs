//! Diagnostics plugin stub; concrete rule implementation pending migration.

use handl_expr::{Diagnostic, Diagnostics};
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use serde_json::json;

/// Diagnostics plugin that currently reports no diagnostics.
pub struct ExpressionDiagnostics {
    capabilities: Vec<CapabilityId>,
}

impl ExpressionDiagnostics {
    /// Creates a new expression diagnostics plugin.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("diag.expression")],
        }
    }
}

impl Default for ExpressionDiagnostics {
    fn default() -> Self {
        Self::new()
    }
}

impl Diagnostics for ExpressionDiagnostics {
    fn diagnose<N>(&self, _expr: &N) -> Vec<Diagnostic> {
        Vec::new()
    }
}

impl Plugin for ExpressionDiagnostics {
    fn name(&self) -> &str {
        "handl-plugin-diag-expression"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        Some(json!({ "description": "expression diagnostics" }))
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}
