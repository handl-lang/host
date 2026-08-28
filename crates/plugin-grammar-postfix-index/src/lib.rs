use handl_ast_canonical::Expr;
use handl_expr::{ParseContext, PostfixFragment};
use handl_lexer_canonical::CanonicalKind;
use handl_vm_values_json::NonFunctionValue;

/// Postfix fragment for index expressions.
pub struct PostfixIndex;

const POSTFIX_INDEX_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::BracketOpen];

impl PostfixIndex {
    /// Creates a new index postfix fragment.
    pub fn new() -> Self {
        PostfixIndex
    }
}

impl PostfixFragment for PostfixIndex {
    fn postfix_tokens(&self) -> &'static [CanonicalKind] {
        POSTFIX_INDEX_TOKENS
    }

    fn try_postfix(
        &self,
        left: &mut Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<(), handl_platform::PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::BracketOpen {
            return None;
        }
        let old_left = std::mem::replace(left, Expr::Value(NonFunctionValue::default()));
        ctx.consume(); // `[`

        let index = match ctx.parse_expr() {
            Ok(i) => i,
            Err(e) => {
                *left = old_left;
                return Some(Err(e));
            }
        };

        if let Err(e) = ctx.expect(CanonicalKind::BracketClose) {
            *left = old_left;
            return Some(Err(e));
        }

        *left = Expr::index(old_left, index);
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
        p.register_postfix(Box::new(PostfixIndex::new()));
        p
    }

    #[test]
    fn index_expression() {
        let p = pipeline();
        let tokens = p.lex("arr[0]").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("arr".to_string()));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::index(
                Expr::Var("arr".to_string()),
                Expr::Value(NonFunctionValue::new(0.0)),
            )
        );
    }
}
