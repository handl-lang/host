use handl_ast_canonical::Expr;
use handl_expr::{ParseContext, PrimaryFragment};
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;

/// Primary fragment that parses `not <primary>` expressions.
pub struct PrimaryNot;

const PRIMARY_NOT_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::Not];

impl PrimaryNot {
    /// Creates a new `not` primary fragment.
    pub fn new() -> Self {
        PrimaryNot
    }
}

impl PrimaryFragment for PrimaryNot {
    fn primary_tokens(&self) -> &'static [CanonicalKind] {
        PRIMARY_NOT_TOKENS
    }

    fn try_primary(
        &self,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::Not {
            return None;
        }
        ctx.consume();

        match ctx.parse_primary() {
            Ok(operand) => Some(Ok(Expr::call("bool.not", vec![operand]))),
            Err(e) => Some(Err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use handl_plugin_primary_values::PrimaryValues;

    #[test]
    fn parses_not() {
        let kit = Kit::new();
p.register_token_rule(Box::new(Booleans::new()));
        p.register_primary(Box::new(PrimaryNot::new()));
        p.register_primary(Box::new(PrimaryValues::new()));

        let tokens = kit.lex("not true").unwrap();
        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::call(
                "bool.not",
                vec![Expr::Value(NonFunctionValue::new(true))]
            )
        );
    }
}
