//! Canonical `Index` grammar-rule plugin.
//!
//! Provides the `IndexRule` and the `IndexToken` capability trait.
//! `IndexRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the index grammar rule.
pub trait IndexToken: TokenKind {
    /// Returns `true` for the token that opens an index, e.g. `[`.
    fn is_bracket_open(&self) -> bool;
    /// Returns `true` for the token that closes an index, e.g. `]`.
    fn is_bracket_close(&self) -> bool;
    /// Returns `true` for the dot delimiter token, e.g. `.`.
    fn is_dot(&self) -> bool;
    /// If this token represents an identifier, return its text.
    fn ident_text(&self) -> Option<&str>;
}

/// Grammar rule that parses postfix index syntax: `target[index]`.
#[derive(Debug, Default, Clone, Copy)]
pub struct IndexRule;

impl<K: IndexToken> Behavior<Parser<K>> for IndexRule {
    fn kind(&self) -> &str {
        "index"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        // Parser rules do their work through the Pratt callbacks, not `apply`.
        Ok(())
    }
}

impl<K: IndexToken> ParserBehavior<K> for IndexRule {
    fn matches_prefix(&self, _kind: &K) -> bool {
        false
    }

    fn prefix(
        &self,
        _token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        unreachable!("IndexRule is an infix rule and cannot be used as a prefix operator")
    }

    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)> {
        if kind.is_bracket_open() || kind.is_dot() {
            Some((90, 91))
        } else {
            None
        }
    }

    fn infix(
        &self,
        left: AstNode,
        token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
        rbp: u32,
    ) -> Result<AstNode, PlatformError> {
        if token.kind.is_dot() {
            let field = ctx.advance()?;
            let field_name = field.kind.ident_text().ok_or_else(|| {
                PlatformError::Plugin(format!(
                    "expected identifier after '.', got {}",
                    field.kind
                ))
            })?;

            let start = left.span.map_or(0, |s| s.start);
            let end = field.span.end;

            Ok(AstNode {
                kind: "index".into(),
                span: Some(Span { start, end }),
                payload: json!({
                    "target": &left,
                    "index": {
                        "kind": "value",
                        "span": null,
                        "payload": {
                            "value": field_name
                        }
                    },
                }),
            })
        } else {
            let index = ctx.expression(rbp)?;

            let close = ctx.advance()?;
            if !close.kind.is_bracket_close() {
                return Err(PlatformError::Plugin(format!(
                    "expected `]`, got {}",
                    close.kind
                )));
            }

            let start = left.span.map_or(0, |s| s.start);
            let end = close.span.end;

            Ok(AstNode {
                kind: "index".into(),
                span: Some(Span { start, end }),
                payload: json!({
                    "target": &left,
                    "index": &index,
                }),
            })
        }
    }
}
