//! Canonical `Fanout` grammar-rule plugin (arrow-calculus `&&&`).
//!
//! Provides the `FanoutRule` and the `FanoutToken` capability trait.
//! `FanoutRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.
//!
//! `&&&` is the arrow fanout operator. `f &&& g` desugars at parse time to a
//! `fn.fanout_fn(f, g)` call, which returns a new arrow `x => [f(x), g(x)]`
//! rather than an eager result. This matches Haskell's `Control.Arrow` `&&&`:
//! right-associative, tighter than `>>>`, and composable
//! (`f &&& g &&& h` = `f &&& (g &&& h)`, `(f &&& g) >>> h` works).

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_token_kind::DynKind;
use handl_plugin_token_operator::OperatorKind;
use serde_json::json;

/// Token capability required by the fanout grammar rule.
pub trait FanoutToken: TokenKind {
    /// Returns `true` for the fanout operator token, e.g. `&&&`.
    fn is_fanout(&self) -> bool;
}

/// Grammar rule that parses infix arrow-fanout: `f &&& g`, desugared
/// to a `fn.fanout_fn(f, g)` call node.
#[derive(Debug, Default, Clone, Copy)]
pub struct FanoutRule;

impl<K: FanoutToken> Behavior<Parser<K>> for FanoutRule {
    fn kind(&self) -> &str {
        "fanout"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        // Parser rules do their work through the Pratt callbacks, not `apply`.
        Ok(())
    }
}

impl<K: FanoutToken> ParserBehavior<K> for FanoutRule {
    fn matches_prefix(&self, _kind: &K) -> bool {
        false
    }

    fn prefix(
        &self,
        _token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        unreachable!("FanoutRule is an infix rule and cannot be used as a prefix operator")
    }

    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)> {
        // Right-associative (rbp == lbp) so `f &&& g &&& h` = `f &&& (g &&& h)`.
        // Binding power 25: tighter than `>>>` (20), looser than pipe (30) and
        // arrow function literal `->` (31) so `(x) -> body &&& (y) -> body`
        // parses as `((x) -> body) &&& ((y) -> body)`.
        if kind.is_fanout() { Some((25, 25)) } else { None }
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

        // Desugar `f &&& g` into `fn.fanout_fn(f, g)`.
        Ok(AstNode {
            kind: "call".into(),
            span: Some(Span { start, end }),
            payload: json!({
                "callee": qualified_callee("fn", "fanout_fn"),
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

impl FanoutToken for DynKind {
    fn is_fanout(&self) -> bool {
        self.downcast_ref::<OperatorKind>()
            .map(|k| *k == OperatorKind::Fanout)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_lexer_operator::OperatorToken;
    use handl_plugin_token_kind::{DynFactory, DynKind};

    #[test]
    fn fanout_token_recognizes_triple_amp() {
        let factory = DynFactory::default();
        let kind = factory.operator_kind("&&&").unwrap();
        assert!(kind.is_fanout());
    }

    #[test]
    fn fanout_token_rejects_other_ops() {
        let factory = DynFactory::default();
        let gt = factory.operator_kind(">").unwrap();
        assert!(!gt.is_fanout());
    }

    #[test]
    fn infix_binding_only_for_fanout() {
        let rule = FanoutRule;
        let factory = DynFactory::default();
        let fanout = factory.operator_kind("&&&").unwrap();
        assert_eq!(rule.infix_binding(&fanout), Some((25, 25)));
        let flow = factory.operator_kind(">>>").unwrap();
        assert_eq!(rule.infix_binding(&flow), None);
    }

    #[test]
    fn is_not_prefix() {
        let rule = FanoutRule;
        let factory = DynFactory::default();
        let fanout = factory.operator_kind("&&&").unwrap();
        assert!(!rule.matches_prefix(&fanout));
    }

    #[test]
    fn eof_is_not_fanout() {
        let kind = DynKind(Box::new(OperatorKind::Eof));
        assert!(!kind.is_fanout());
    }
}
