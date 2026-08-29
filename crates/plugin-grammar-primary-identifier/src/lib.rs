//! Primary identifier grammar rule for the generic `handl-plugin-ext-parser` Pratt engine.
//!
//! `IdentifierRule` parses identifier tokens into `AstNode`s of kind `"var"`.
//! Dialects opt in by implementing the `IdentToken` capability trait for their
//! token kind.

use handl_platform::{AstNode, Behavior, PlatformError};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_token_kind::LexKind;
use serde_json::json;

/// Token capability required by the primary identifier grammar rule.
pub trait IdentToken: TokenKind {
    /// If this token represents an identifier, return its name.
    fn as_ident(&self) -> Option<&str>;
}

/// Grammar rule that parses primary identifier references.
#[derive(Debug, Default, Clone, Copy)]
pub struct IdentifierRule;

impl IdentifierRule {
    /// Creates a new identifier rule.
    pub fn new() -> Self {
        IdentifierRule
    }
}

impl<K: IdentToken> Behavior<Parser<K>> for IdentifierRule {
    fn kind(&self) -> &str {
        "var"
    }

    fn apply(
        &self,
        _op: &mut <Parser<K> as handl_platform::Operation>::Request,
    ) -> Result<(), PlatformError> {
        // Parser rules do their work through the Pratt callbacks, not `apply`.
        Ok(())
    }
}

impl<K: IdentToken> ParserBehavior<K> for IdentifierRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.as_ident().is_some()
    }

    fn prefix(
        &self,
        token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let name = token
            .kind
            .as_ident()
            .ok_or_else(|| {
                PlatformError::Plugin(format!("expected an identifier, got {}", token.kind))
            })?
            .to_string();

        Ok(AstNode {
            kind: "var".into(),
            span: Some(handl_platform::Span {
                start: token.span.start,
                end: token.span.end,
            }),
            payload: json!({"name": name}),
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
        unreachable!("IdentifierRule is a prefix rule and cannot be used as an infix operator")
    }
}

impl IdentToken for LexKind {
    fn as_ident(&self) -> Option<&str> {
        match self {
            LexKind::Identifier(name) => Some(name.as_str()),
            _ => None,
        }
    }
}
