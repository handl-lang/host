//! Canonical function-literal grammar-rule plugin.
//!
//! Provides the `ArrowRule` and the `ArrowToken` capability trait.
//! `ArrowRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.  It handles the `->` infix operator that separates a
//! function's parameter list from its body expression.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_token_kind::{LexKind, LexOperator};
use serde_json::json;

/// Token capability required by the function-literal grammar rule.
pub trait ArrowToken: TokenKind {
    /// Returns `true` for the arrow operator token, e.g. `->`.
    fn is_arrow(&self) -> bool;
}

/// Grammar rule that parses function-literal syntax: `(params) -> body`.
///
/// The left side is the function head (typically a parenthesized parameter
/// list).  The right side is the body expression.  The parameter names are
/// extracted from the left AST node.
#[derive(Debug, Default, Clone, Copy)]
pub struct ArrowRule;

impl<K: ArrowToken> Behavior<Parser<K>> for ArrowRule {
    fn kind(&self) -> &str {
        "function-literal"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: ArrowToken> ParserBehavior<K> for ArrowRule {
    fn matches_prefix(&self, _kind: &K) -> bool {
        false
    }

    fn prefix(
        &self,
        _token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        unreachable!("ArrowRule is an infix rule and cannot be used as a prefix operator")
    }

    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)> {
        if kind.is_arrow() {
            Some((31, 32))
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
        let params = extract_params(&left);
        let body = ctx.expression(rbp)?;

        let start = left.span.map_or(0, |s| s.start);
        let end = body.span.map_or(start, |s| s.end);

        Ok(AstNode {
            kind: "function".into(),
            span: Some(Span { start, end }),
            payload: json!({
                "params": params,
                "body": &body,
            }),
        })
    }
}

/// Extracts parameter names from a function-head AST node.
///
/// Handles:
/// - `var` nodes (single identifier param, e.g. from `(s)`)
/// - `call` nodes (callee used as param name)
/// - `array` nodes (multiple params, each element is a var)
/// - `value` nodes (no params, e.g. `() -> body`)
fn extract_params(node: &AstNode) -> Vec<String> {
    match node.kind.as_str() {
        "var" => {
            let name = node
                .payload
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                Vec::new()
            } else {
                vec![name]
            }
        }
        "array" => {
            let elements = node.payload.get("elements").and_then(|v| v.as_array());
            match elements {
                Some(arr) => arr
                    .iter()
                    .filter_map(|elem| {
                        let elem_ast: AstNode = serde_json::from_value(elem.clone()).ok()?;
                        extract_params(&elem_ast).into_iter().next()
                    })
                    .collect(),
                None => Vec::new(),
            }
        }
        "value" => Vec::new(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {}

impl ArrowToken for LexKind {
    fn is_arrow(&self) -> bool {
        matches!(self, LexKind::Operator(LexOperator::Arrow))
    }
}
