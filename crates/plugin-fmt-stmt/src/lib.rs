//! `FormatBehavior` contributor for statement formatting.
//!
//! This plugin is currently a no-op contributor to `BasicFormatter` pending
//! a generic way to pass canonical statement data through `FormatRequest`.

use handl_platform::{
    BasicFormatter, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Format, FormatBehavior, FormatRequest, PlatformError,
    Plugin, PluginContext,
};

/// Statement `FormatBehavior` contributor.
#[derive(Debug, Default, Clone, Copy)]
pub struct StatementFormatter;

impl StatementFormatter {
    /// Creates a new statement formatter rule.
    pub fn new() -> Self {
        Self
    }
}

impl Behavior<Format> for StatementFormatter {
    fn kind(&self) -> &str {
        "fmt.statements"
    }

    fn apply(&self, _op: &mut FormatRequest) -> Result<(), PlatformError> {
        // TODO: the concrete `Stmt<NonFunctionValue>` formatting path cannot be
        // cleanly adapted to the generic `FormatRequest`; wire statement nodes
        // through the request once a canonical AST representation is available.
        Ok(())
    }
}

impl FormatBehavior for StatementFormatter {}

impl Plugin for StatementFormatter {
    fn id(&self) -> &str {
        "fmt.statements"
    }

    fn name(&self) -> &str {
        "handl-plugin-fmt-stmt"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        let rule: Box<dyn FormatBehavior> = Box::new(StatementFormatter);
        ctx.contribute::<BasicFormatter>(rule)
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::FormatRequest;

    #[test]
    fn kind_is_fmt_statements() {
        assert_eq!(StatementFormatter::new().kind(), "fmt.statements");
    }

    #[test]
    fn apply_is_noop() {
        use handl_platform::{AstNode, ResolvedOptions};
        let mut req = FormatRequest {
            ast: AstNode {
                kind: "fmt.statements".to_string(),
                span: None,
                payload: serde_json::Value::Null,
            },
            output: String::new(),
            options: ResolvedOptions::new(),
        };
        assert!(StatementFormatter::new().apply(&mut req).is_ok());
    }
}
