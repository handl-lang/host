//! Canonical `Array` grammar-rule plugin.
//!
//! Provides the `ArrayRule` and the `ArrayToken` capability trait.
//! `ArrayRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the array grammar rule.
pub trait ArrayToken: TokenKind {
    /// Returns `true` for the opening bracket token, e.g. `[`.
    fn is_bracket_open(&self) -> bool;
    /// Returns `true` for the closing bracket token, e.g. `]`.
    fn is_bracket_close(&self) -> bool;
    /// Returns `true` for the element-separator token, e.g. `,`.
    fn is_comma(&self) -> bool;
}

/// Grammar rule that parses array literals: `[elem1, elem2, ...]`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ArrayRule;

impl<K: ArrayToken> Behavior<Parser<K>> for ArrayRule {
    fn kind(&self) -> &str {
        "array"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: ArrayToken> ParserBehavior<K> for ArrayRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_bracket_open()
    }

    fn prefix(
        &self,
        token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let mut elements = Vec::new();

        if !ctx.peek().map_or(true, |t| t.kind.is_bracket_close()) {
            loop {
                elements.push(ctx.expression(0)?);

                match ctx.peek() {
                    Some(t) if t.kind.is_bracket_close() => break,
                    Some(t) if t.kind.is_comma() => {
                        ctx.advance()?;
                    }
                    Some(_) | None => break,
                }
            }
        }

        let close = ctx.advance()?;
        if !close.kind.is_bracket_close() {
            return Err(PlatformError::Plugin(format!(
                "expected `]`, got {}",
                close.kind
            )));
        }

        let start = token.span.start;
        let end = close.span.end;

        Ok(AstNode {
            kind: "array".into(),
            span: Some(Span { start, end }),
            payload: json!({ "elements": &elements }),
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
        unreachable!("ArrayRule is a prefix rule and cannot be used as an infix operator")
    }
}
