use handl_ast_canonical::Expr;
use handl_expr::{InfixFragment, ParseContext, Token};
use handl_lexer_canonical::CanonicalKind;
use handl_vm_values_json::NonFunctionValue;

/// Infix fragment for the ternary (`? :`) operator.
pub struct Ternary;

const TERNARY_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::Question];

impl Ternary {
    /// Creates a new ternary infix fragment.
    pub fn new() -> Self {
        Ternary
    }
}

impl InfixFragment for Ternary {
    fn infix_tokens(&self) -> &'static [CanonicalKind] {
        TERNARY_TOKENS
    }

    fn binding(&self, token: &Token) -> Option<(u32, u32)> {
        match token.kind {
            CanonicalKind::Question => Some((30, 29)),
            _ => None,
        }
    }

    fn try_infix(
        &self,
        left: &mut Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<(), handl_platform::PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::Question {
            return None;
        }
        let old_left = std::mem::replace(left, Expr::Value(NonFunctionValue::default()));
        ctx.consume(); // `?`

        let then_ = match ctx.parse_expr_with_min_bp(29) {
            Ok(e) => e,
            Err(e) => {
                *left = old_left;
                return Some(Err(e));
            }
        };

        if let Err(e) = ctx.expect(CanonicalKind::Colon) {
            *left = old_left;
            return Some(Err(e));
        }

        let else_ = match ctx.parse_expr_with_min_bp(29) {
            Ok(e) => e,
            Err(e) => {
                *left = old_left;
                return Some(Err(e));
            }
        };

        *left = Expr::call("if", vec![old_left, then_, else_]);
        Some(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;
    use handl_plugin_delimiters::Delimiters;
    use handl_plugin_identifiers::Identifiers;
    use handl_plugin_numbers::Numbers;
    use handl_plugin_primary_identifiers::PrimaryIdentifiers;
    use handl_plugin_primary_values::PrimaryValues;

    fn pipeline() -> ExpressionPipeline {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Identifiers::new()));
        p.register_token_rule(Box::new(Numbers::new()));
        p.register_token_rule(Box::new(Delimiters::new()));
        p.register_primary(Box::new(PrimaryIdentifiers::new()));
        p.register_primary(Box::new(PrimaryValues::new()));
        p.register_infix(Box::new(Ternary::new()));
        p
    }

    #[test]
    fn ternary_expression() {
        let p = pipeline();
        let tokens = p.lex("x ? 1 : 2").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("x".to_string()));
        assert_eq!(tokens[1].kind, CanonicalKind::Question);
        assert_eq!(tokens[2].kind, CanonicalKind::Number(1.0));
        assert_eq!(tokens[3].kind, CanonicalKind::Colon);
        assert_eq!(tokens[4].kind, CanonicalKind::Number(2.0));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::call(
                "if",
                vec![
                    Expr::var("x"),
                    Expr::Value(NonFunctionValue::new(1.0)),
                    Expr::Value(NonFunctionValue::new(2.0)),
                ],
            )
        );
    }
}
