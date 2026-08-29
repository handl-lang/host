//! Canonical `Let` grammar-rule plugin.
//!
//! Provides the `LetRule` and the `LetToken` capability trait.
//! `LetRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the let grammar rule.
pub trait LetToken: TokenKind {
    /// Returns `true` for the `let` keyword token.
    fn is_let(&self) -> bool;
    /// If this token can serve as a binding name, returns its text.
    fn as_let_name(&self) -> Option<&str>;
    /// Returns `true` for the binding assignment token, e.g. `=`.
    fn is_let_equal(&self) -> bool;
    /// Returns `true` for the statement terminator token, e.g. `;`.
    fn is_let_semicolon(&self) -> bool;
}

/// Grammar rule that parses `let` bindings: `let name = value;`.
#[derive(Debug, Default, Clone, Copy)]
pub struct LetRule;

impl<K: LetToken> Behavior<Parser<K>> for LetRule {
    fn kind(&self) -> &str {
        "let"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        // Parser rules do their work through the Pratt callbacks, not `apply`.
        Ok(())
    }
}

impl<K: LetToken> ParserBehavior<K> for LetRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_let()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let name_tok = ctx.advance()?;
        let name = name_tok
            .kind
            .as_let_name()
            .ok_or_else(|| {
                PlatformError::Plugin(format!("expected a binding name, got {}", name_tok.kind))
            })?
            .to_string();

        let eq = ctx.advance()?;
        if !eq.kind.is_let_equal() {
            return Err(PlatformError::Plugin(format!(
                "expected `=`, got {}",
                eq.kind
            )));
        }

        let value = ctx.expression(0)?;

        let semi = ctx.advance()?;
        if !semi.kind.is_let_semicolon() {
            return Err(PlatformError::Plugin(format!(
                "expected `;`, got {}",
                semi.kind
            )));
        }

        let start = _token.span.start;
        let end = semi.span.end;

        Ok(AstNode {
            kind: "let".into(),
            span: Some(Span { start, end }),
            payload: json!({
                "name": name,
                "value": &value,
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
        unreachable!("LetRule is a prefix rule and cannot be used as an infix operator")
    }
}
