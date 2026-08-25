use handl_ast_canonical::Expr;
use handl_expr::{ParseContext, PrimaryFragment};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;

/// Primary fragment that parses array literals.
pub struct PrimaryArrays;

const PRIMARY_ARRAYS_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::BracketOpen];

impl PrimaryArrays {
    /// Creates a new array primary fragment.
    pub fn new() -> Self {
        PrimaryArrays
    }
}

impl PrimaryFragment for PrimaryArrays {
    fn primary_tokens(&self) -> &'static [CanonicalKind] {
        PRIMARY_ARRAYS_TOKENS
    }

    fn try_primary(
        &self,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::BracketOpen {
            return None;
        }
        ctx.consume();

        let mut elements = Vec::new();
        if ctx.consume_if(CanonicalKind::BracketClose) {
            return Some(Ok(Expr::Array(elements)));
        }

        loop {
            if ctx.peek().is_none() {
                if ctx.recover {
                    ctx.error("unterminated array literal");
                    break;
                } else {
                    return Some(Err(PlatformError::Plugin(
                        "unterminated array literal".to_string(),
                    )));
                }
            }

            let expr = match ctx.parse_expr() {
                Ok(e) => e,
                Err(e) => {
                    if ctx.recover {
                        ctx.error(e.to_string());
                        break;
                    } else {
                        return Some(Err(e));
                    }
                }
            };
            elements.push(expr);

            if !ctx.consume_if(CanonicalKind::Comma) {
                if let Err(e) = ctx.expect(CanonicalKind::BracketClose) {
                    if ctx.recover {
                        ctx.error(e.to_string());
                        break;
                    } else {
                        return Some(Err(e));
                    }
                }
                break;
            } else if ctx.consume_if(CanonicalKind::BracketClose) {
                break;
            } else if ctx.peek().is_none() {
                if ctx.recover {
                    ctx.error("unterminated array literal");
                    break;
                } else {
                    return Some(Err(PlatformError::Plugin(
                        "unterminated array literal".to_string(),
                    )));
                }
            }
        }

        Some(Ok(Expr::Array(elements)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_plugin_delimiters::Delimiters;
    use handl_plugin_numbers::Numbers;
    use handl_plugin_primary_values::PrimaryValues;

    #[test]
    fn parses_array() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Numbers::new()));
        p.register_token_rule(Box::new(Delimiters::new()));
        p.register_primary(Box::new(PrimaryArrays::new()));
        p.register_primary(Box::new(PrimaryValues::new()));

        let tokens = p.lex("[1, 2]").unwrap();
        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::Array(vec![
                Expr::Value(NonFunctionValue::new(1.0)),
                Expr::Value(NonFunctionValue::new(2.0)),
            ])
        );
    }

    #[test]
    fn parses_empty_array() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Delimiters::new()));
        p.register_primary(Box::new(PrimaryArrays::new()));

        let tokens = p.lex("[]").unwrap();
        let expr = p.parse(tokens).unwrap();
        assert_eq!(expr, Expr::Array(vec![]));
    }
}
