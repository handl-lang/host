use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::Token;
use handl_plugin_lexer_template::TemplateSegment;
use serde_json::{Value, json};

use crate::token::TemplateToken;

/// Grammar rule that parses template string literals.
#[derive(Debug, Default, Clone, Copy)]
pub struct TemplateRule;

impl<K: TemplateToken> Behavior<Parser<K>> for TemplateRule {
    fn kind(&self) -> &str {
        "template"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: TemplateToken> ParserBehavior<K> for TemplateRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.as_template_segments().is_some()
    }

    fn prefix(
        &self,
        token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let segments = token.kind.as_template_segments().ok_or_else(|| {
            PlatformError::Plugin(format!("expected a template token, got {}", token.kind))
        })?;

        let seg_array: Vec<Value> = segments
            .iter()
            .map(|seg| match seg {
                TemplateSegment::Text(text) => json!({
                    "type": "text",
                    "value": text,
                }),
                TemplateSegment::Expr(raw) => json!({
                    "type": "expr",
                    "raw": raw,
                }),
            })
            .collect();

        let start = token.span.start;
        let end = token.span.end;

        Ok(AstNode {
            kind: "template".into(),
            span: Some(Span { start, end }),
            payload: json!({
                "segments": seg_array,
            }),
        })
    }

    fn infix_binding(&self, _kind: &K) -> Option<(u32, u32)> {
        None
    }

    fn infix(
        &self,
        _left: AstNode,
        _token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
        _rbp: u32,
    ) -> Result<AstNode, PlatformError> {
        unreachable!("TemplateRule is a prefix rule and cannot be used as an infix operator")
    }
}
