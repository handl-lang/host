//! Expression formatter stub; canonical formatting implementation pending.

use handl_expr::{Expr, Formatter};
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use handl_vm_values_json::NonFunctionValue;
use serde_json::json;

/// Formatter plugin for individual expressions.
pub struct ExpressionFormatter {
    capabilities: Vec<CapabilityId>,
}

impl ExpressionFormatter {
    /// Creates a new expression formatter.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("fmt.expression")],
        }
    }
}

impl Default for ExpressionFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter for ExpressionFormatter {
    fn format(
        &self,
        _expr: &Expr<NonFunctionValue>,
    ) -> Result<String, handl_platform::PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

impl Plugin for ExpressionFormatter {
    fn name(&self) -> &str {
        "handl-plugin-fmt-expression"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        Some(json!({ "description": "format handl expressions" }))
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}
