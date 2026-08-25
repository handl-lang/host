use handl_expr::Token;
use handl_lexer_canonical::CanonicalKind;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use serde_json::{json, Value as JsonValue};

/// Formatter plugin that formats trivia comments from a token stream.
pub struct TriviaFormatter {
    capabilities: Vec<CapabilityId>,
}

impl TriviaFormatter {
    /// Creates a new trivia formatter.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("fmt.trivia")],
        }
    }

    /// Formats the `Comment` tokens in `tokens` as source text.
    ///
    /// Line comments are emitted as `// text\n`; block comments as
    /// `/* text */`. The comment style is inferred from the token span.
    pub fn format_comments(&self, tokens: &[Token]) -> String {
        let mut out = String::new();
        for token in tokens {
            if let CanonicalKind::Comment(text) = &token.kind {
                let span_len = token.span.end - token.span.start;
                let body_len = text.len();
                if span_len == body_len + 2 {
                    // Line comment: the span covers `//` + body.
                    out.push_str("//");
                    out.push_str(text);
                    out.push('\n');
                } else {
                    // Block comment: the span covers `/*` + body + `*/`.
                    out.push_str("/*");
                    out.push_str(text);
                    out.push_str("*/");
                }
            }
        }
        out
    }
}

impl Default for TriviaFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TriviaFormatter {
    fn name(&self) -> &str {
        "handl-plugin-fmt-trivia"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        Some(json!({ "description": "format comment trivia from a token stream" }))
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        let tokens = match &request.payload {
            JsonValue::Array(arr) => arr,
            _ => {
                return Err(PlatformError::Plugin(
                    "expected a JSON array of tokens".to_string(),
                ))
            }
        };
        let _ = tokens;
        Err(PlatformError::Plugin(
            "fmt.trivia is not exposed through the capability interface".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_lexer::Span;

    #[test]
    fn format_line_and_block_comments() {
        let tokens = vec![
            Token::new(CanonicalKind::Comment(" hello".to_string()), Span::new(0, 8)),
            Token::new(CanonicalKind::Comment(" world ".to_string()), Span::new(9, 20)),
        ];
        let f = TriviaFormatter::new();
        let formatted = f.format_comments(&tokens);
        assert_eq!(formatted, "// hello\n/* world */");
    }

    #[test]
    fn plugin_reports_capability() {
        let p = TriviaFormatter::new();
        assert!(p.capabilities().iter().any(|c| c.as_str() == "fmt.trivia"));
    }
}
