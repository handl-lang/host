//! Canonical `Match` grammar-rule plugin.
//!
//! Provides the `MatchRule` and the `MatchToken` capability trait.
//! `MatchRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the match grammar rule.
pub trait MatchToken: TokenKind {
    /// Returns `true` for the `match` keyword token.
    fn is_match(&self) -> bool;
}

/// Grammar rule that parses `match` expressions: `match subject`.
#[derive(Debug, Default, Clone, Copy)]
pub struct MatchRule;

impl<K: MatchToken> Behavior<Parser<K>> for MatchRule {
    fn kind(&self) -> &str {
        "match"
    }

    fn apply(
        &self,
        _op: &mut <Parser<K> as Operation>::Request,
    ) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: MatchToken> ParserBehavior<K> for MatchRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_match()
    }

    fn prefix(
        &self,
        token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let subject = ctx.expression(0)?;

        let start = token.span.start;
        let end = subject.span.map_or(start, |s| s.end);

        Ok(AstNode {
            kind: "match".into(),
            span: Some(Span { start, end }),
            payload: json!({ "subject": &subject }),
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
        unreachable!("MatchRule is a prefix rule and cannot be used as an infix operator")
    }
}
