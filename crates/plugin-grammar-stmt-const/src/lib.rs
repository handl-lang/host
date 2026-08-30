//! Canonical `const` statement grammar-rule plugin.
//!
//! **Deprecated:** `const` is redundant in handl's pure fragment — all
//! bindings are immutable by default (see `plugin-eval-let`). Use `let`
//! instead. This crate is kept for parser backward-compatibility but the
//! `stmt.const` AST kind has no eval rule; evaluating a `const` binding
//! will produce a "not implemented" error.
//!
//! Provides the `ConstStmtRule` and the `ConstStmtToken` capability trait.
//! `ConstStmtRule` implements `ParserBehavior<K>` for the
//! `handl-plugin-ext-parser` Pratt engine and produces `"stmt.const"` AST nodes.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the const statement grammar rule.
pub trait ConstStmtToken: TokenKind {
    /// Returns `true` for the `const` keyword token.
    fn is_const_stmt(&self) -> bool;
    /// If this token can serve as a binding name, returns its text.
    fn as_const_stmt_name(&self) -> Option<&str>;
    /// Returns `true` for the binding assignment token, e.g. `=`.
    fn is_const_stmt_equal(&self) -> bool;
    /// Returns `true` for the statement terminator token, e.g. `;`.
    fn is_const_stmt_semicolon(&self) -> bool;
}

/// Grammar rule that parses `const` statements: `const name = value;`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ConstStmtRule;

impl<K: ConstStmtToken> Behavior<Parser<K>> for ConstStmtRule {
    fn kind(&self) -> &str {
        "stmt.const"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: ConstStmtToken> ParserBehavior<K> for ConstStmtRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_const_stmt()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let name_tok = ctx.advance()?;
        let name = name_tok
            .kind
            .as_const_stmt_name()
            .ok_or_else(|| {
                PlatformError::Plugin(format!("expected a binding name, got {}", name_tok.kind))
            })?
            .to_string();

        let eq = ctx.advance()?;
        if !eq.kind.is_const_stmt_equal() {
            return Err(PlatformError::Plugin(format!(
                "expected `=`, got {}",
                eq.kind
            )));
        }

        let value = ctx.expression(0)?;

        let semi = ctx.advance()?;
        if !semi.kind.is_const_stmt_semicolon() {
            return Err(PlatformError::Plugin(format!(
                "expected `;`, got {}",
                semi.kind
            )));
        }

        let start = _token.span.start;
        let end = semi.span.end;

        Ok(AstNode {
            kind: "stmt.const".into(),
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
        unreachable!("ConstStmtRule is a prefix rule and cannot be used as an infix operator")
    }
}

#[cfg(test)]
mod tests {}
