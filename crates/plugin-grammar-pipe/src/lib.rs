//! Canonical `Pipe` grammar-rule plugin.
//!
//! Provides the `PipeRule` and the `PipeToken` capability trait.
//! `PipeRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the pipe grammar rule.
pub trait PipeToken: TokenKind {
    /// Returns `true` for the pipe operator token, e.g. `|>`.
    fn is_pipe(&self) -> bool;
}

/// Grammar rule that parses infix pipe syntax: `left |> right`.
#[derive(Debug, Default, Clone, Copy)]
pub struct PipeRule;

impl<K: PipeToken> Behavior<Parser<K>> for PipeRule {
    fn kind(&self) -> &str {
        "pipe"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        // Parser rules do their work through the Pratt callbacks, not `apply`.
        Ok(())
    }
}

impl<K: PipeToken> ParserBehavior<K> for PipeRule {
    fn matches_prefix(&self, _kind: &K) -> bool {
        false
    }

    fn prefix(
        &self,
        _token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        unreachable!("PipeRule is an infix rule and cannot be used as a prefix operator")
    }

    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)> {
        if kind.is_pipe() { Some((30, 31)) } else { None }
    }

    fn infix(
        &self,
        left: AstNode,
        _token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
        rbp: u32,
    ) -> Result<AstNode, PlatformError> {
        let right = ctx.expression(rbp)?;

        let start = left.span.map_or(0, |s| s.start);
        let end = right.span.map_or(start, |s| s.end);

        Ok(AstNode {
            kind: "pipe".into(),
            span: Some(Span { start, end }),
            payload: json!({
                "left": &left,
                "right": &right,
            }),
        })
    }
}
