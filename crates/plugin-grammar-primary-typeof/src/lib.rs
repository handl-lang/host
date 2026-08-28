use handl_ast_canonical::Expr;
use handl_expr::{ParseContext, PrimaryFragment};
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;

/// Primary fragment that parses `typeof <primary>` expressions.
pub struct PrimaryTypeOf;

const PRIMARY_TYPEOF_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::TypeOf];

impl PrimaryTypeOf {
    /// Creates a new `typeof` primary fragment.
    pub fn new() -> Self {
        PrimaryTypeOf
    }
}

impl PrimaryFragment for PrimaryTypeOf {
    fn primary_tokens(&self) -> &'static [CanonicalKind] {
        PRIMARY_TYPEOF_TOKENS
    }

    fn try_primary(
        &self,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::TypeOf {
            return None;
        }
        ctx.consume();

        match ctx.parse_primary() {
            Ok(operand) => Some(Ok(Expr::call("type.typeof", vec![operand]))),
            Err(e) => Some(Err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use handl_plugin_primary_values::PrimaryValues;

    #[test]
    fn parses_typeof() {
        let kit = Kit::new();
p.register_token_rule(Box::new(Numbers::new()));
        p.register_primary(Box::new(PrimaryTypeOf::new()));
        p.register_primary(Box::new(PrimaryValues::new()));

        let tokens = kit.lex("typeof 42").unwrap();
        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::call(
                "type.typeof",
                vec![Expr::Value(NonFunctionValue::new(42.0))]
            )
        );
    }
}
