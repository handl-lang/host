use handl_ast_canonical::{Expr, Pattern};
use handl_expr::{InfixFragment, ParseContext, Token};
use handl_lexer_canonical::CanonicalKind;
use handl_vm_values_json::NonFunctionValue;

/// Infix fragment for the assignment (`=`) operator.
pub struct Assign;

const ASSIGN_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::Equal];

impl Assign {
    /// Creates a new assignment infix fragment.
    pub fn new() -> Self {
        Assign
    }
}

fn expr_to_pattern<V>(expr: &Expr<V>) -> Result<Pattern, handl_platform::PlatformError> {
    match expr {
        Expr::Var(name) => Ok(Pattern::Identifier(name.clone())),
        Expr::Array(elements) => Ok(Pattern::Array(
            elements
                .iter()
                .map(expr_to_pattern)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Expr::Object(entries) => Ok(Pattern::Object(
            entries
                .iter()
                .map(|(k, v)| expr_to_pattern(v).map(|p| (k.clone(), p)))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        _ => Err(handl_platform::PlatformError::Plugin(
            "invalid assignment target".to_string(),
        )),
    }
}

impl InfixFragment for Assign {
    fn infix_tokens(&self) -> &'static [CanonicalKind] {
        ASSIGN_TOKENS
    }

    fn binding(&self, token: &Token) -> Option<(u32, u32)> {
        match token.kind {
            CanonicalKind::Equal => Some((1, 0)),
            _ => None,
        }
    }

    fn try_infix(
        &self,
        left: Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, handl_platform::PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::Equal {
            return None;
        }
        ctx.consume(); // `=`

        let target = match expr_to_pattern(&left) {
            Ok(p) => p,
            Err(e) => return Some(Err(e)),
        };

        let right = match ctx.parse_expr_with_min_bp(0) {
            Ok(r) => r,
            Err(e) => return Some(Err(e)),
        };

        Some(Ok(Expr::assign(target, right)))
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
        p.register_infix(Box::new(Assign::new()));
        p
    }

    #[test]
    fn simple_assignment() {
        let p = pipeline();
        let tokens = p.lex("a = 1").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("a".to_string()));
        assert_eq!(tokens[1].kind, CanonicalKind::Equal);
        assert_eq!(tokens[2].kind, CanonicalKind::Number(1.0));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::Assign {
                target: Pattern::Identifier("a".to_string()),
                value: Box::new(Expr::Value(NonFunctionValue::new(1.0))),
            }
        );
    }
}
