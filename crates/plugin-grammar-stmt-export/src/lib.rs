//! Canonical `export` statement grammar-rule plugin.
//!
//! Provides the `ExportStmtRule` and the `ExportStmtToken` capability trait.
//! `ExportStmtRule` implements `ParserBehavior<K>` for the
//! `handl-plugin-ext-parser` Pratt engine and produces `"stmt.export"` AST nodes.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_token_kind::{LexDelimiter, LexKind, LexKeyword};
use serde_json::json;

/// Token capability required by the export statement grammar rule.
pub trait ExportStmtToken: TokenKind {
    /// Returns `true` for the `export` keyword token.
    fn is_export_stmt(&self) -> bool;
    /// Returns `true` for the statement terminator token, e.g. `;`.
    fn is_export_stmt_semicolon(&self) -> bool;
}

/// Grammar rule that parses `export` statements: `export expr`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ExportStmtRule;

impl<K: ExportStmtToken> Behavior<Parser<K>> for ExportStmtRule {
    fn kind(&self) -> &str {
        "stmt.export"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: ExportStmtToken> ParserBehavior<K> for ExportStmtRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_export_stmt()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let value = ctx.expression(0)?;

        let mut end = value.span.map(|s| s.end).unwrap_or(_token.span.end);

        if let Some(t) = ctx.peek() {
            if t.kind.is_export_stmt_semicolon() {
                let semi = ctx.advance()?;
                end = semi.span.end;
            }
        }

        let start = _token.span.start;

        Ok(AstNode {
            kind: "stmt.export".into(),
            span: Some(Span { start, end }),
            payload: json!({
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
        unreachable!("ExportStmtRule is a prefix rule and cannot be used as an infix operator")
    }
}

impl ExportStmtToken for LexKind {
    fn is_export_stmt(&self) -> bool {
        matches!(self, LexKind::Keyword(LexKeyword::Export))
    }

    fn is_export_stmt_semicolon(&self) -> bool {
        matches!(self, LexKind::Delimiter(LexDelimiter::Semicolon))
    }
}

#[cfg(test)]
mod tests {}
