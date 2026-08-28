//! Typecheck plugin stub; concrete canonical typecheck implementation pending.

use handl_expr::Stmt;
use handl_platform::{
    CapabilityId, CapabilityRequest, CapabilityResponse, PlatformError, Plugin, PluginContext,
};
use handl_runtime::{TypecheckedOutput, TypecheckPlugin};
use handl_vm_values_json::NonFunctionValue;
use serde_json::json;

/// A `TypecheckPlugin` stub that always reports success.
pub struct TypecheckCore {
    capabilities: Vec<CapabilityId>,
}

impl TypecheckCore {
    /// Creates a new `TypecheckCore` plugin.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("typecheck")],
        }
    }
}

impl Default for TypecheckCore {
    fn default() -> Self {
        Self::new()
    }
}

impl TypecheckPlugin for TypecheckCore {
    fn name(&self) -> &str {
        "typecheck-core"
    }

    fn typecheck(
        &self,
        _stmts: &[Stmt<NonFunctionValue>],
    ) -> Result<TypecheckedOutput, PlatformError> {
        Ok(TypecheckedOutput { success: true })
    }
}

impl Plugin for TypecheckCore {
    fn name(&self) -> &str {
        "typecheck-core"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(
        &self,
        _capability: &CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        Some(json!({}))
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}
