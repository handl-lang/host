//! Canonical `Cmp` grammar-rule plugin — comparison operator sugars.
//!
//! Provides the `CmpRule` and the `CmpToken` capability trait. `CmpRule`
//! implements `ParserBehavior<K>` for the `handl-plugin-ext-parser` Pratt
//! engine.
//!
//! The comparison operators `==`, `!=`, `<`, `<=`, `>`, `>=`, and `<=>`
//! (spaceship) are desugared at parse time into `cmp.*` calls, which the
//! `cmp` library module evaluates:
//!
//! | sugar    | desugar          |
//! |----------|------------------|
//! | `a == b` | `cmp.eq(a, b)`   |
//! | `a != b` | `cmp.ne(a, b)`   |
//! | `a < b`  | `cmp.lt(a, b)`   |
//! | `a <= b` | `cmp.le(a, b)`   |
//! | `a > b`  | `cmp.gt(a, b)`   |
//! | `a >= b` | `cmp.ge(a, b)`   |
//! | `a <=> b`| `cmp.compare(a, b)` |
//!
//! All are left-associative at binding power 50 — below postfix call/index
//! (90) and above arrow fanout `&&&` (40), pipe `|>` (30), and arrow
//! composition `>>>` (20). Chaining (`a < b < c`) parses as
//! `(a < b) < c`; the comparison semantics make that a likely type error,
//! which is left to the typechecker/evaluator to report.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_token_kind::DynKind;
use handl_plugin_token_operator::OperatorKind;
use serde_json::json;

/// Token capability required by the comparison grammar rule.
pub trait CmpToken: TokenKind {
    /// Returns the `cmp` function name this token desugars to, or `None`.
    fn cmp_func(&self) -> Option<&'static str>;
}

/// Grammar rule that parses infix comparison operators and desugars them to
/// `cmp.*` calls.
#[derive(Debug, Default, Clone, Copy)]
pub struct CmpRule;

impl<K: CmpToken> Behavior<Parser<K>> for CmpRule {
    fn kind(&self) -> &str {
        "cmp"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        // Parser rules do their work through the Pratt callbacks, not `apply`.
        Ok(())
    }
}

impl<K: CmpToken> ParserBehavior<K> for CmpRule {
    fn matches_prefix(&self, _kind: &K) -> bool {
        false
    }

    fn prefix(
        &self,
        _token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        unreachable!("CmpRule is an infix rule and cannot be used as a prefix operator")
    }

    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)> {
        // Left-associative (rbp == lbp + 1) so `a == b == c` = `(a == b) == c`.
        if kind.cmp_func().is_some() {
            Some((50, 51))
        } else {
            None
        }
    }

    fn infix(
        &self,
        left: AstNode,
        token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
        rbp: u32,
    ) -> Result<AstNode, PlatformError> {
        let func = token.kind.cmp_func().ok_or_else(|| {
            PlatformError::Plugin(format!(
                "CmpRule: token {} is not a comparison operator",
                token.kind
            ))
        })?;
        let right = ctx.expression(rbp)?;

        let start = left.span.map_or(0, |s| s.start);
        let end = right.span.map_or(start, |s| s.end);

        // Desugar `a <op> b` into `cmp.<func>(a, b)`.
        Ok(AstNode {
            kind: "call".into(),
            span: Some(Span { start, end }),
            payload: json!({
                "callee": qualified_callee("cmp", func),
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

impl CmpToken for DynKind {
    fn cmp_func(&self) -> Option<&'static str> {
        let op = self.downcast_ref::<OperatorKind>()?;
        match op {
            OperatorKind::EqualEqual => Some("eq"),
            OperatorKind::NotEqual => Some("ne"),
            OperatorKind::LessThan => Some("lt"),
            OperatorKind::LessEqual => Some("le"),
            OperatorKind::GreaterThan => Some("gt"),
            OperatorKind::GreaterEqual => Some("ge"),
            OperatorKind::Spaceship => Some("compare"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_lexer_operator::OperatorToken;
    use handl_plugin_token_kind::{DynFactory, DynKind};

    fn factory() -> DynFactory {
        DynFactory::default()
    }

    #[test]
    fn maps_each_operator() {
        let f = factory();
        assert_eq!(f.operator_kind("==").unwrap().cmp_func(), Some("eq"));
        assert_eq!(f.operator_kind("!=").unwrap().cmp_func(), Some("ne"));
        assert_eq!(f.operator_kind("<").unwrap().cmp_func(), Some("lt"));
        assert_eq!(f.operator_kind("<=").unwrap().cmp_func(), Some("le"));
        assert_eq!(f.operator_kind(">").unwrap().cmp_func(), Some("gt"));
        assert_eq!(f.operator_kind(">=").unwrap().cmp_func(), Some("ge"));
        assert_eq!(f.operator_kind("<=>").unwrap().cmp_func(), Some("compare"));
    }

    #[test]
    fn rejects_non_comparison_ops() {
        let f = factory();
        assert_eq!(f.operator_kind("+").and_then(|k| k.cmp_func()), None);
        assert_eq!(f.operator_kind(">>>").and_then(|k| k.cmp_func()), None);
        assert_eq!(f.operator_kind("&&&").and_then(|k| k.cmp_func()), None);
        // `=` is assignment, not comparison.
        assert_eq!(f.operator_kind("=").and_then(|k| k.cmp_func()), None);
    }

    #[test]
    fn infix_binding_for_all_comparison_ops() {
        let rule = CmpRule;
        let f = factory();
        for text in ["==", "!=", "<", "<=", ">", ">=", "<=>"] {
            let kind = f.operator_kind(text).unwrap();
            assert_eq!(rule.infix_binding(&kind), Some((50, 51)), "{text}");
        }
    }

    #[test]
    fn infix_binding_none_for_other_ops() {
        let rule = CmpRule;
        let f = factory();
        let plus = f.operator_kind("+");
        assert!(plus.is_none() || rule.infix_binding(&plus.unwrap()).is_none());
        let flow = f.operator_kind(">>>").unwrap();
        assert_eq!(rule.infix_binding(&flow), None);
    }

    #[test]
    fn is_not_prefix() {
        let rule = CmpRule;
        let f = factory();
        let eq = f.operator_kind("==").unwrap();
        assert!(!rule.matches_prefix(&eq));
    }
}
