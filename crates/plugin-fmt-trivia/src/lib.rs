//! `FormatBehavior` contributor for trivia/comment formatting.
//!
//! This plugin is currently a no-op contributor to `BasicFormatter` pending
//! a generic way to pass token-stream data through `FormatRequest`.

use handl_platform::{
    BasicFormatter, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest,
    CapabilityResponse, ContributionContext, Format, FormatBehavior, FormatRequest, PlatformError,
    Plugin, PluginContext,
};

/// Trivia `FormatBehavior` contributor.
#[derive(Debug, Default, Clone, Copy)]
pub struct TriviaFormatter;

impl TriviaFormatter {
    /// Creates a new trivia formatter rule.
    pub fn new() -> Self {
        Self
    }
}

impl Behavior<Format> for TriviaFormatter {
    fn kind(&self) -> &str {
        "fmt.trivia"
    }

    fn apply(&self, _op: &mut FormatRequest) -> Result<(), PlatformError> {
        // TODO: wire the token stream through the request once a canonical
        // representation is available.
        Ok(())
    }
}

impl FormatBehavior for TriviaFormatter {}

impl Plugin for TriviaFormatter {
    fn id(&self) -> &str {
        "fmt.trivia"
    }

    fn name(&self) -> &str {
        "handl-plugin-fmt-trivia"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &[]
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        let rule: Box<dyn FormatBehavior> = Box::new(TriviaFormatter);
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
    fn kind_is_fmt_trivia() {
        assert_eq!(TriviaFormatter::new().kind(), "fmt.trivia");
    }

    #[test]
    fn apply_is_noop() {
        use handl_platform::{AstNode, ResolvedOptions};
        let mut req = FormatRequest {
            ast: AstNode {
                kind: "fmt.trivia".to_string(),
                span: None,
                payload: serde_json::Value::Null,
            },
            output: String::new(),
            options: ResolvedOptions::new(),
        };
        assert!(TriviaFormatter::new().apply(&mut req).is_ok());
    }
}
