//! Canonical `Assign` grammar-rule plugin.
//!
//! Provides the `AssignRule` and the `AssignToken` capability trait.
//! `AssignRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the assign grammar rule.
pub trait AssignToken: TokenKind {
    /// Returns `true` for the assignment operator token, e.g. `=`.
    fn is_equal(&self) -> bool;
}

/// Grammar rule that parses infix assignment: `target = value`.
#[derive(Debug, Default, Clone, Copy)]
pub struct AssignRule;

impl<K: AssignToken> Behavior<Parser<K>> for AssignRule {
    fn kind(&self) -> &str {
        "assign"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: AssignToken> ParserBehavior<K> for AssignRule {
    fn matches_prefix(&self, _kind: &K) -> bool {
        false
    }

    fn prefix(
        &self,
        _token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        unreachable!("AssignRule is an infix rule and cannot be used as a prefix operator")
    }

    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)> {
        if kind.is_equal() {
            Some((10, 11))
        } else {
            None
        }
    }

    fn infix(
        &self,
        left: AstNode,
        _token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
        rbp: u32,
    ) -> Result<AstNode, PlatformError> {
        let value = ctx.expression(rbp)?;

        let start = left.span.map_or(0, |s| s.start);
        let end = value.span.map_or(start, |s| s.end);

        Ok(AstNode {
            kind: "assign".into(),
            span: Some(Span { start, end }),
            payload: json!({
                "target": &left,
                "value": &value,
            }),
        })
    }
}
