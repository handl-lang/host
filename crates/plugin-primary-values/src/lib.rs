use handl_ast_canonical::Expr;
use handl_expr::{ParseContext, PrimaryFragment};
use handl_lexer_canonical::CanonicalKind;
use handl_vm_values_json::NonFunctionValue;
use serde_json::Value;

/// Primary fragment that parses value literals.
///
/// Handles numbers, strings, booleans, null, and byte literals.
pub struct PrimaryValues;

const PRIMARY_VALUE_TOKENS: &'static [CanonicalKind] = &[
    CanonicalKind::Number(0.0),
    CanonicalKind::String(String::new()),
    CanonicalKind::Bool(false),
    CanonicalKind::Null,
    CanonicalKind::Byte(0),
];

impl PrimaryValues {
    /// Creates a new value-literal primary fragment.
    pub fn new() -> Self {
        PrimaryValues
    }
}

impl PrimaryFragment for PrimaryValues {
    fn primary_tokens(&self) -> &'static [CanonicalKind] {
        PRIMARY_VALUE_TOKENS
    }

    fn try_primary(
        &self,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, handl_platform::PlatformError>> {
        let token = ctx.peek().cloned()?;
        match token.kind {
            CanonicalKind::Number(n) => {
                ctx.consume();
                Some(Ok(Expr::Value(NonFunctionValue::new(Value::from(n)))))
            }
            CanonicalKind::String(s) => {
                ctx.consume();
                Some(Ok(Expr::Value(NonFunctionValue::new(Value::from(s)))))
            }
            CanonicalKind::Bool(b) => {
                ctx.consume();
                Some(Ok(Expr::Value(NonFunctionValue::new(Value::from(b)))))
            }
            CanonicalKind::Byte(b) => {
                ctx.consume();
                Some(Ok(Expr::Value(NonFunctionValue::new(Value::from(b)))))
            }
            CanonicalKind::Null => {
                ctx.consume();
                Some(Ok(Expr::Value(NonFunctionValue::new(Value::Null))))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;
    use handl_plugin_numbers::Numbers;
    use handl_plugin_strings::Strings;

    #[test]
    fn parses_number() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Numbers::new()));
        p.register_primary(Box::new(PrimaryValues::new()));

        let tokens = p.lex("42").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, CanonicalKind::Number(42.0));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(expr, Expr::Value(NonFunctionValue::new(42.0)));
    }

    #[test]
    fn parses_string() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Strings::new()));
        p.register_primary(Box::new(PrimaryValues::new()));

        let tokens = p.lex("\"hi\"").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, CanonicalKind::String("hi".to_string()));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(expr, Expr::Value(NonFunctionValue::new("hi".to_string())));
    }
}
