//! Canonical `Apply` grammar-rule plugin.
//!
//! Provides the `ApplyRule` and the `ApplyToken` capability trait.
//! `ApplyRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.  It handles parenthesized grouping: `( expr )`.

use handl_platform::{AstNode, Behavior, Operation, PlatformError};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_token_kind::{LexDelimiter, LexKind};

/// Token capability required by the apply grammar rule.
pub trait ApplyToken: TokenKind {
    /// Returns `true` for the opening paren token, e.g. `(`.
    fn is_paren_open(&self) -> bool;
    /// Returns `true` for the closing paren token, e.g. `)`.
    fn is_paren_close(&self) -> bool;
    /// Returns `true` for the argument-separator token, e.g. `,`.
    fn is_comma(&self) -> bool;
}

/// Grammar rule that parses parenthesized grouping: `( expr )`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ApplyRule;

impl<K: ApplyToken> Behavior<Parser<K>> for ApplyRule {
    fn kind(&self) -> &str {
        "apply"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: ApplyToken> ParserBehavior<K> for ApplyRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_paren_open()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let inner = ctx.expression(0)?;
        let close = ctx.advance()?;
        if !close.kind.is_paren_close() {
            return Err(PlatformError::Plugin(format!(
                "expected `)`, got {}",
                close.kind
            )));
        }
        Ok(inner)
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
        unreachable!("ApplyRule is a prefix rule and cannot be used as an infix operator")
    }
}

impl ApplyToken for LexKind {
    fn is_paren_open(&self) -> bool {
        matches!(self, LexKind::Delimiter(LexDelimiter::ParenOpen))
    }

    fn is_paren_close(&self) -> bool {
        matches!(self, LexKind::Delimiter(LexDelimiter::ParenClose))
    }

    fn is_comma(&self) -> bool {
        matches!(self, LexKind::Delimiter(LexDelimiter::Comma))
    }
}
