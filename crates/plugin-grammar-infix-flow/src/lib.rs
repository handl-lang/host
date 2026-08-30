//! Canonical `Flow` grammar-rule plugin (arrow-calculus `>>>`).
//!
//! Provides the `FlowRule` and the `FlowToken` capability trait.
//! `FlowRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.
//!
//! `>>>` is the arrow-composition operator. `f >>> g` desugars at parse time
//! to a `fn.flow(f, g)` call, which returns a composed function (a new arrow)
//! rather than an eager result. This matches Haskell's `Control.Arrow` `>>>`:
//! right-associative, lower precedence than `&&&` (25), and composable
//! (`f >>> g >>> h` = `f >>> (g >>> h)`).

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_token_kind::DynKind;
use handl_plugin_token_operator::OperatorKind;
use serde_json::json;

/// Token capability required by the flow grammar rule.
pub trait FlowToken: TokenKind {
    /// Returns `true` for the flow operator token, e.g. `>>>`.
    fn is_flow(&self) -> bool;
}

/// Grammar rule that parses infix arrow-composition: `f >>> g`, desugared
/// to a `fn.flow(f, g)` call node.
#[derive(Debug, Default, Clone, Copy)]
pub struct FlowRule;

impl<K: FlowToken> Behavior<Parser<K>> for FlowRule {
    fn kind(&self) -> &str {
        "flow"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        // Parser rules do their work through the Pratt callbacks, not `apply`.
        Ok(())
    }
}

impl<K: FlowToken> ParserBehavior<K> for FlowRule {
    fn matches_prefix(&self, _kind: &K) -> bool {
        false
    }

    fn prefix(
        &self,
        _token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        unreachable!("FlowRule is an infix rule and cannot be used as a prefix operator")
    }

    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)> {
        // Right-associative (rbp == lbp) so `f >>> g >>> h` = `f >>> (g >>> h)`.
        // Binding power 20: looser than `&&&` (25) and pipe (30), tighter than
        // assignment (10).
        if kind.is_flow() { Some((20, 20)) } else { None }
    }

    fn infix(
        &self,
        left: AstNode,
        _token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
        rbp: u32,
    ) -> Result<AstNode, PlatformError> {
        let right = ctx.expression(rbp)?;

        let start = left.span.map_or(0, |s| s.start);
        let end = right.span.map_or(start, |s| s.end);

        // Desugar `f >>> g` into `fn.flow(f, g)`.
        Ok(AstNode {
            kind: "call".into(),
            span: Some(Span { start, end }),
            payload: json!({
                "callee": qualified_callee("fn", "flow"),
                "args": [&left, &right],
            }),
        })
    }
}

/// Build a module-qualified callee AST node: `module.func` parses as an
/// `index` node with a `var` target and a string-literal index, which the
/// eval layer resolves via `ctx.call_function("module.func", ...)`.
fn qualified_callee(module: &str, func: &str) -> serde_json::Value {
    json!({
        "kind": "index",
        "span": null,
        "payload": {
            "target": {
                "kind": "var",
                "span": null,
                "payload": { "name": module }
            },
            "index": {
                "kind": "value",
                "span": null,
                "payload": { "value": func }
            }
        }
    })
}

impl FlowToken for DynKind {
    fn is_flow(&self) -> bool {
        self.downcast_ref::<OperatorKind>()
            .map(|k| *k == OperatorKind::Flow)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_lexer::Span;
    use handl_plugin_lexer_operator::OperatorToken;
    use handl_plugin_token_kind::{DynFactory, DynKind};

    #[test]
    fn flow_token_recognizes_triple_gt() {
        let factory = DynFactory::default();
        let kind = factory.operator_kind(">>>").unwrap();
        assert!(kind.is_flow());
    }

    #[test]
    fn flow_token_rejects_other_ops() {
        let factory = DynFactory::default();
        let gt = factory.operator_kind(">").unwrap();
        assert!(!gt.is_flow());
    }

    #[test]
    fn infix_binding_only_for_flow() {
        let rule = FlowRule;
        let factory = DynFactory::default();
        let flow = factory.operator_kind(">>>").unwrap();
        assert_eq!(rule.infix_binding(&flow), Some((20, 20)));
        let gt = factory.operator_kind(">").unwrap();
        assert_eq!(rule.infix_binding(&gt), None);
    }

    #[test]
    fn is_not_prefix() {
        let rule = FlowRule;
        let factory = DynFactory::default();
        let flow = factory.operator_kind(">>>").unwrap();
        assert!(!rule.matches_prefix(&flow));
    }

    #[test]
    fn eof_is_not_flow() {
        let kind = DynKind(Box::new(OperatorKind::Eof));
        assert!(!kind.is_flow());
        // sanity: TokenKind::is_eof wiring
        let _ = Span::new(0, 0);
    }
}
