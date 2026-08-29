//! Canonical `let` statement grammar-rule plugin.
//!
//! Provides the `LetStmtRule` and the `LetStmtToken` capability trait.
//! `LetStmtRule` implements `ParserBehavior<K>` for the
//! `handl-plugin-ext-parser` Pratt engine and produces `"stmt.let"` AST nodes.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the let statement grammar rule.
pub trait LetStmtToken: TokenKind {
    /// Returns `true` for the `let` keyword token.
    fn is_let_stmt(&self) -> bool;
    /// If this token can serve as a binding name, returns its text.
    fn as_let_stmt_name(&self) -> Option<&str>;
    /// Returns `true` for the binding assignment token, e.g. `=`.
    fn is_let_stmt_equal(&self) -> bool;
    /// Returns `true` for the statement terminator token, e.g. `;`.
    fn is_let_stmt_semicolon(&self) -> bool;
}

/// Grammar rule that parses `let` statements: `let name = value;`.
#[derive(Debug, Default, Clone, Copy)]
pub struct LetStmtRule;

impl<K: LetStmtToken> Behavior<Parser<K>> for LetStmtRule {
    fn kind(&self) -> &str {
        "stmt.let"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: LetStmtToken> ParserBehavior<K> for LetStmtRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_let_stmt()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let name_tok = ctx.advance()?;
        let name = name_tok
            .kind
            .as_let_stmt_name()
            .ok_or_else(|| {
                PlatformError::Plugin(format!("expected a binding name, got {}", name_tok.kind))
            })?
            .to_string();

        let eq = ctx.advance()?;
        if !eq.kind.is_let_stmt_equal() {
            return Err(PlatformError::Plugin(format!(
                "expected `=`, got {}",
                eq.kind
            )));
        }

        let value = ctx.expression(0)?;

        let semi = ctx.advance()?;
        if !semi.kind.is_let_stmt_semicolon() {
            return Err(PlatformError::Plugin(format!(
                "expected `;`, got {}",
                semi.kind
            )));
        }

        let start = _token.span.start;
        let end = semi.span.end;

        Ok(AstNode {
            kind: "stmt.let".into(),
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
        unreachable!("LetStmtRule is a prefix rule and cannot be used as an infix operator")
    }
}

#[cfg(test)]
mod tests {}
