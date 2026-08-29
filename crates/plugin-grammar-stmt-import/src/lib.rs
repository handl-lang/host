//! Canonical `import` statement grammar-rule plugin.
//!
//! Provides the `ImportStmtRule` and the `ImportStmtToken` capability trait.
//! `ImportStmtRule` implements `ParserBehavior<K>` for the
//! `handl-plugin-ext-parser` Pratt engine and produces `"stmt.import"` AST nodes.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the import statement grammar rule.
pub trait ImportStmtToken: TokenKind {
    /// Returns `true` for the `import` keyword token.
    fn is_import_stmt(&self) -> bool;
    /// Returns `true` for the statement terminator token, e.g. `;`.
    fn is_import_stmt_semicolon(&self) -> bool;
}

/// Grammar rule that parses `import` statements: `import source` or
/// `import "source"`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ImportStmtRule;

impl<K: ImportStmtToken> Behavior<Parser<K>> for ImportStmtRule {
    fn kind(&self) -> &str {
        "stmt.import"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: ImportStmtToken> ParserBehavior<K> for ImportStmtRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_import_stmt()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let source = ctx.expression(0)?;

        let mut end = source.span.map(|s| s.end).unwrap_or(_token.span.end);

        if let Some(t) = ctx.peek() {
            if t.kind.is_import_stmt_semicolon() {
                let semi = ctx.advance()?;
                end = semi.span.end;
            }
        }

        let start = _token.span.start;

        Ok(AstNode {
            kind: "stmt.import".into(),
            span: Some(Span { start, end }),
            payload: json!({
                "source": &source,
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
        unreachable!("ImportStmtRule is a prefix rule and cannot be used as an infix operator")
    }
}

#[cfg(test)]
mod tests {}
