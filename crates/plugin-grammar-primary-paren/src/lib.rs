use handl_ast_canonical::Expr;
use handl_expr::{ParseContext, PrimaryFragment};
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;

/// Primary fragment that parses parenthesized grouping `( <expr> )`.
///
/// This is pure grouping; function literals with `->` are out of scope.
pub struct PrimaryParens;

const PRIMARY_PARENS_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::CallOpen];

impl PrimaryParens {
    /// Creates a new parenthesized-expression primary fragment.
    pub fn new() -> Self {
        PrimaryParens
    }
}

impl PrimaryFragment for PrimaryParens {
    fn primary_tokens(&self) -> &'static [CanonicalKind] {
        PRIMARY_PARENS_TOKENS
    }

    fn try_primary(
        &self,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::CallOpen {
            return None;
        }
        ctx.consume();

        let inner = match ctx.parse_expr() {
            Ok(expr) => expr,
            Err(e) => return Some(Err(e)),
        };

        if let Err(e) = ctx.expect(CanonicalKind::CallClose) {
            return Some(Err(e));
        }

        Some(Ok(inner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_plugin_primary_values::PrimaryValues;

    #[test]
    fn parses_parenthesized_number() {
        let kit = Kit::new();
p.register_token_rule(Box::new(Delimiters::new()));
        p.register_primary(Box::new(PrimaryParens::new()));
        p.register_primary(Box::new(PrimaryValues::new()));

        let tokens = kit.lex("(42)").unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, CanonicalKind::CallOpen);
        assert_eq!(tokens[1].kind, CanonicalKind::Number(42.0));
        assert_eq!(tokens[2].kind, CanonicalKind::CallClose);

        let expr = p.parse(tokens).unwrap();
        assert_eq!(expr, Expr::Value(NonFunctionValue::new(42.0)));
    }
}
