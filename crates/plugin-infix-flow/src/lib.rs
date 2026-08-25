use handl_ast_canonical::Expr;
use handl_expr::{InfixFragment, ParseContext, Token};
use handl_lexer_canonical::CanonicalKind;
use handl_vm_values_json::NonFunctionValue;

/// Infix fragment for flow (`>>>`) and compose (`<<<`) arrow combinators.
pub struct Flow;

const FLOW_TOKENS: &'static [CanonicalKind] = &[
    CanonicalKind::Flow,
    CanonicalKind::Compose,
];

impl Flow {
    /// Creates a new flow/compose infix fragment.
    pub fn new() -> Self {
        Flow
    }
}

impl InfixFragment for Flow {
    fn infix_tokens(&self) -> &'static [CanonicalKind] {
        FLOW_TOKENS
    }

    fn binding(&self, token: &Token) -> Option<(u32, u32)> {
        match token.kind {
            CanonicalKind::Flow | CanonicalKind::Compose => Some((60, 61)),
            _ => None,
        }
    }

    fn try_infix(
        &self,
        left: Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, handl_platform::PlatformError>> {
        let token = ctx.peek().cloned()?;
        let name = match token.kind {
            CanonicalKind::Flow => "fn.flow",
            CanonicalKind::Compose => "fn.compose",
            _ => return None,
        };
        ctx.consume();

        let right = match ctx.parse_expr_with_min_bp(61) {
            Ok(r) => r,
            Err(e) => return Some(Err(e)),
        };

        Some(Ok(Expr::call(name, vec![left, right])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;
    use handl_plugin_identifiers::Identifiers;
    use handl_plugin_operators::Operators;
    use handl_plugin_primary_identifiers::PrimaryIdentifiers;

    fn pipeline() -> ExpressionPipeline {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Operators::new()));
        p.register_token_rule(Box::new(Identifiers::new()));
        p.register_primary(Box::new(PrimaryIdentifiers::new()));
        p.register_infix(Box::new(Flow::new()));
        p
    }

    #[test]
    fn flow_forward() {
        let p = pipeline();
        let tokens = p.lex("f >>> g").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("f".to_string()));
        assert_eq!(tokens[1].kind, CanonicalKind::Flow);
        assert_eq!(tokens[2].kind, CanonicalKind::Identifier("g".to_string()));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(expr, Expr::call("fn.flow", vec![Expr::var("f"), Expr::var("g")]));
    }
}
