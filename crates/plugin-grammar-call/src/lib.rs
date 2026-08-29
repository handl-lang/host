//! Canonical `Call` grammar-rule plugin.
//!
//! Provides the `CallRule` and the `CallToken` capability trait.
//! `CallRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the call grammar rule.
pub trait CallToken: TokenKind {
    /// Returns `true` for the token that opens a call, e.g. `(`.
    fn is_call_open(&self) -> bool;

    /// Returns `true` for the token that closes a call, e.g. `)`.
    fn is_call_close(&self) -> bool;

    /// Returns `true` for the argument-separator token, e.g. `,`.
    fn is_comma(&self) -> bool;
}

/// Grammar rule that parses function-call syntax: `callee(arg1, arg2, ...)`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CallRule;

impl<K: CallToken> Behavior<Parser<K>> for CallRule {
    fn kind(&self) -> &str {
        "call"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        // Parser rules do their work through the Pratt callbacks, not `apply`.
        Ok(())
    }
}

impl<K: CallToken> ParserBehavior<K> for CallRule {
    fn matches_prefix(&self, _kind: &K) -> bool {
        false
    }

    fn prefix(
        &self,
        _token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        unreachable!("CallRule is an infix rule and cannot be used as a prefix operator")
    }

    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)> {
        if kind.is_call_open() {
            Some((90, 91))
        } else {
            None
        }
    }

    fn infix(
        &self,
        left: AstNode,
        _token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
        _rbp: u32,
    ) -> Result<AstNode, PlatformError> {
        let mut args = Vec::new();

        if !ctx.peek().map_or(true, |t| t.kind.is_call_close()) {
            loop {
                args.push(ctx.expression(0)?);

                match ctx.peek() {
                    Some(t) if t.kind.is_call_close() => break,
                    Some(t) if t.kind.is_comma() => {
                        ctx.advance()?;
                    }
                    Some(_) | None => break,
                }
            }
        }

        let close = ctx.advance()?;
        if !close.kind.is_call_close() {
            return Err(PlatformError::Plugin(format!(
                "expected `)`, got {}",
                close.kind
            )));
        }

        let start = left.span.map_or(0, |s| s.start);
        let end = close.span.end;

        Ok(AstNode {
            kind: "call".into(),
            span: Some(Span { start, end }),
            payload: json!({
                "callee": &left,
                "args": &args,
            }),
        })
    }
}
