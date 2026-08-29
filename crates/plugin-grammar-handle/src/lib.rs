//! Canonical `Handle` grammar-rule plugin.
//!
//! Provides the `HandleRule` and the `HandleToken` capability trait.
//! `HandleRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the handle grammar rule.
pub trait HandleToken: TokenKind {
    /// Returns `true` for the `handle` keyword token.
    fn is_handle(&self) -> bool;
}

/// Grammar rule that parses `handle` expressions: `handle expr`.
#[derive(Debug, Default, Clone, Copy)]
pub struct HandleRule;

impl<K: HandleToken> Behavior<Parser<K>> for HandleRule {
    fn kind(&self) -> &str {
        "handle"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: HandleToken> ParserBehavior<K> for HandleRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_handle()
    }

    fn prefix(
        &self,
        token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let expr = ctx.expression(0)?;

        let start = token.span.start;
        let end = expr.span.map_or(start, |s| s.end);

        Ok(AstNode {
            kind: "handle".into(),
            span: Some(Span { start, end }),
            payload: json!({ "expr": &expr }),
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
        unreachable!("HandleRule is a prefix rule and cannot be used as an infix operator")
    }
}
