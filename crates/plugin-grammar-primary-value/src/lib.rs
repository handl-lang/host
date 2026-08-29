//! Primary value grammar rule for the generic `handl-plugin-ext-parser` Pratt engine.
//!
//! `ValueRule` parses literal tokens (numbers, strings, booleans, and `null`)
//! into `AstNode`s of kind `"value"`.  Dialects opt in by implementing the
//! `ValueToken` capability trait for their token kind.

use handl_platform::{AstNode, Behavior, PlatformError};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::{json, Value};

/// Token capability required by the primary value grammar rule.
pub trait ValueToken: TokenKind {
    /// If this token represents a primary value, return its JSON form.
    fn as_value(&self) -> Option<Value>;
}

/// Grammar rule that parses primary literal values.
#[derive(Debug, Default, Clone, Copy)]
pub struct ValueRule;

impl ValueRule {
    /// Creates a new value rule.
    pub fn new() -> Self {
        ValueRule
    }
}

impl<K: ValueToken> Behavior<Parser<K>> for ValueRule {
    fn kind(&self) -> &str {
        "value"
    }

    fn apply(&self, _op: &mut <Parser<K> as handl_platform::Operation>::Request) -> Result<(), PlatformError> {
        // Parser rules do their work through the Pratt callbacks, not `apply`.
        Ok(())
    }
}

impl<K: ValueToken> ParserBehavior<K> for ValueRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.as_value().is_some()
    }

    fn prefix(
        &self,
        token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let value = token
            .kind
            .as_value()
            .ok_or_else(|| PlatformError::Plugin(format!("expected a primary value, got {}", token.kind)))?;

        Ok(AstNode {
            kind: "value".into(),
            span: Some(handl_platform::Span {
                start: token.span.start,
                end: token.span.end,
            }),
            payload: json!({"value": value}),
        })
    }

    fn infix_binding(&self, _kind: &K) -> Option<(u32, u32)> {
        None
    }

    fn infix(
        &self,
        _left: AstNode,
        token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
        _rbp: u32,
    ) -> Result<AstNode, PlatformError> {
        Err(PlatformError::Plugin(format!(
            "token {} cannot be used as an infix operator",
            token.kind
        )))
    }
}
