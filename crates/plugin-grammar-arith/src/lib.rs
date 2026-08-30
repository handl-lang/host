//! Canonical `Arith` grammar-rule plugin — arithmetic operator sugars.
//!
//! Provides the `ArithRule` and the `ArithToken` capability trait.
//! `ArithRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.
//!
//! The arithmetic operators `+`, `-`, `*`, `/`, and `**` are desugared at
//! parse time into `math.*` calls, which the `math` library module evaluates:
//!
//! | sugar      | desugar            |
//! |------------|--------------------|
//! | `a + b`    | `math.add(a, b)`   |
//! | `a - b`    | `math.sub(a, b)`   |
//! | `a * b`    | `math.mul(a, b)`   |
//! | `a / b`    | `math.div(a, b)`   |
//! | `a ** b`   | `math.pow(a, b)`   |
//!
//! Precedence (all below postfix call/index at 90, above comparison at 50):
//!
//! | operator | associativity | binding power |
//! |----------|---------------|---------------|
//! | `**`     | right         | (70, 70)      |
//! | `*` `/`  | left          | (60, 61)      |
//! | `+` `-`  | left          | (55, 56)      |
//!
//! `**` is right-associative (`2 ** 3 ** 2` = `2 ** (3 ** 2)`), matching
//! mathematical convention. `*`, `/`, `+`, `-` are left-associative.
//!
//! ## Note on `//` (floor division)
//!
//! `//` cannot be used as a floor-division operator because the line-comment
//! lexer rule (`//` prefix) is registered before the operator lexer rule, so
//! `//` always lexes as a line comment. Use `math.floor_div(a, b)` directly.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_token_kind::DynKind;
use handl_plugin_token_operator::OperatorKind;
use serde_json::json;

/// Token capability required by the arithmetic grammar rule.
pub trait ArithToken: TokenKind {
    /// Returns the `(math_func, left_bp, right_bp)` triple for this token, or
    /// `None` if it is not an arithmetic operator. `left_bp == right_bp`
    /// indicates right-associativity; `right_bp == left_bp + 1` indicates
    /// left-associativity.
    fn arith_op(&self) -> Option<(&'static str, u32, u32)>;
}

/// Grammar rule that parses infix arithmetic operators and desugars them to
/// `math.*` calls.
#[derive(Debug, Default, Clone, Copy)]
pub struct ArithRule;

impl<K: ArithToken> Behavior<Parser<K>> for ArithRule {
    fn kind(&self) -> &str {
        "arith"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: ArithToken> ParserBehavior<K> for ArithRule {
    fn matches_prefix(&self, _kind: &K) -> bool {
        false
    }

    fn prefix(
        &self,
        _token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        unreachable!("ArithRule is an infix rule and cannot be used as a prefix operator")
    }

    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)> {
        kind.arith_op().map(|(_, lbp, rbp)| (lbp, rbp))
    }

    fn infix(
        &self,
        left: AstNode,
        token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
        rbp: u32,
    ) -> Result<AstNode, PlatformError> {
        let (func, _, _) = token.kind.arith_op().ok_or_else(|| {
            PlatformError::Plugin(format!(
                "ArithRule: token {} is not an arithmetic operator",
                token.kind
            ))
        })?;
        let right = ctx.expression(rbp)?;

        let start = left.span.map_or(0, |s| s.start);
        let end = right.span.map_or(start, |s| s.end);

        // Desugar `a <op> b` into `math.<func>(a, b)`.
        Ok(AstNode {
            kind: "call".into(),
            span: Some(Span { start, end }),
            payload: json!({
                "callee": qualified_callee("math", func),
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

impl ArithToken for DynKind {
    fn arith_op(&self) -> Option<(&'static str, u32, u32)> {
        let op = self.downcast_ref::<OperatorKind>()?;
        match op {
            OperatorKind::Plus => Some(("add", 55, 56)),
            OperatorKind::Minus => Some(("sub", 55, 56)),
            OperatorKind::Star => Some(("mul", 60, 61)),
            OperatorKind::Slash => Some(("div", 60, 61)),
            OperatorKind::Pow => Some(("pow", 70, 70)),
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
        assert_eq!(f.operator_kind("+").unwrap().arith_op(), Some(("add", 55, 56)));
        assert_eq!(f.operator_kind("-").unwrap().arith_op(), Some(("sub", 55, 56)));
        assert_eq!(f.operator_kind("*").unwrap().arith_op(), Some(("mul", 60, 61)));
        assert_eq!(f.operator_kind("/").unwrap().arith_op(), Some(("div", 60, 61)));
        assert_eq!(f.operator_kind("**").unwrap().arith_op(), Some(("pow", 70, 70)));
    }

    #[test]
    fn rejects_non_arithmetic_ops() {
        let f = factory();
        assert_eq!(f.operator_kind("==").and_then(|k| k.arith_op()), None);
        assert_eq!(f.operator_kind(">>>").and_then(|k| k.arith_op()), None);
        assert_eq!(f.operator_kind("=").and_then(|k| k.arith_op()), None);
    }

    #[test]
    fn pow_is_right_associative() {
        let f = factory();
        let pow = f.operator_kind("**").unwrap();
        let (_, lbp, rbp) = pow.arith_op().unwrap();
        assert_eq!(lbp, rbp, "** should be right-associative");
    }

    #[test]
    fn mul_div_add_sub_are_left_associative() {
        let f = factory();
        for text in ["+", "-", "*", "/"] {
            let kind = f.operator_kind(text).unwrap();
            let (_, lbp, rbp) = kind.arith_op().unwrap();
            assert_eq!(rbp, lbp + 1, "{text} should be left-associative");
        }
    }

    #[test]
    fn infix_binding_for_all_arith_ops() {
        let rule = ArithRule;
        let f = factory();
        for text in ["+", "-", "*", "/", "**"] {
            let kind = f.operator_kind(text).unwrap();
            assert!(rule.infix_binding(&kind).is_some(), "{text}");
        }
    }

    #[test]
    fn is_not_prefix() {
        let rule = ArithRule;
        let f = factory();
        let plus = f.operator_kind("+").unwrap();
        assert!(!rule.matches_prefix(&plus));
    }
}
