//! `LintBehavior` contributor for expression diagnostics.
//!
//! This plugin is currently a no-op contributor to `BasicLinter` pending a
//! generic way to pass expression data through `LintRequest`.

use handl_platform::{
    BasicLinter, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    ContributionContext, Lint, LintBehavior, LintRequest, PlatformError, Plugin, PluginContext,
};

/// Expression `LintBehavior` contributor.
#[derive(Debug, Default, Clone, Copy)]
pub struct ExpressionDiagnostics;

impl ExpressionDiagnostics {
    /// Creates a new expression diagnostics rule.
    pub fn new() -> Self {
        Self
    }
}

impl Behavior<Lint> for ExpressionDiagnostics {
    fn kind(&self) -> &str {
        "lint.expression"
    }

    fn apply(&self, _op: &mut LintRequest) -> Result<(), PlatformError> {
        // TODO: wire expression-specific diagnostics through the generic
        // `LintRequest` once an expression payload is available.
        Ok(())
    }
}

impl LintBehavior for ExpressionDiagnostics {}

impl Plugin for ExpressionDiagnostics {
    fn id(&self) -> &str {
        "lint.expression"
    }

    fn name(&self) -> &str {
        "handl-plugin-diag-expression"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        let rule: Box<dyn LintBehavior> = Box::new(ExpressionDiagnostics);
        ctx.contribute::<BasicLinter>(rule)
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}
