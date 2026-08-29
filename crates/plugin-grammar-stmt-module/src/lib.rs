//! Canonical `module` statement grammar-rule plugin.
//!
//! Provides the `ModuleStmtRule` and the `ModuleStmtToken` capability trait.
//! `ModuleStmtRule` implements `ParserBehavior<K>` for the
//! `handl-plugin-ext-parser` Pratt engine and produces `"stmt.module"` AST nodes.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the module statement grammar rule.
pub trait ModuleStmtToken: TokenKind {
    /// Returns `true` for the `module` keyword token.
    fn is_module_stmt(&self) -> bool;
    /// If this token can serve as a module name, returns its text.
    fn as_module_stmt_name(&self) -> Option<&str>;
    /// Returns `true` for the binding assignment token, e.g. `=`.
    fn is_module_stmt_equal(&self) -> bool;
    /// Returns `true` for the statement terminator token, e.g. `;`.
    fn is_module_stmt_semicolon(&self) -> bool;
}

/// Grammar rule that parses `module` statements: `module name` or
/// `module name = expr`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ModuleStmtRule;

impl<K: ModuleStmtToken> Behavior<Parser<K>> for ModuleStmtRule {
    fn kind(&self) -> &str {
        "stmt.module"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: ModuleStmtToken> ParserBehavior<K> for ModuleStmtRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_module_stmt()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let name_tok = ctx.advance()?;
        let name = name_tok
            .kind
            .as_module_stmt_name()
            .ok_or_else(|| {
                PlatformError::Plugin(format!("expected a module name, got {}", name_tok.kind))
            })?
            .to_string();

        let mut value: Option<AstNode> = None;

        if let Some(t) = ctx.peek() {
            if t.kind.is_module_stmt_equal() {
                ctx.advance()?;
                value = Some(ctx.expression(0)?);
            }
        }

        let mut end = value
            .as_ref()
            .and_then(|v| v.span)
            .map(|s| s.end)
            .unwrap_or(name_tok.span.end);

        if let Some(t) = ctx.peek() {
            if t.kind.is_module_stmt_semicolon() {
                let semi = ctx.advance()?;
                end = semi.span.end;
            }
        }

        let start = _token.span.start;

        let payload = match &value {
            Some(v) => json!({
                "name": name,
                "value": v,
            }),
            None => json!({
                "name": name,
            }),
        };

        Ok(AstNode {
            kind: "stmt.module".into(),
            span: Some(Span { start, end }),
            payload,
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
        unreachable!("ModuleStmtRule is a prefix rule and cannot be used as an infix operator")
    }
}

#[cfg(test)]
mod tests {}
