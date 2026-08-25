use handl_ast_canonical::Expr;
use handl_expr::{InfixFragment, ParseContext, Token};
use handl_lexer_canonical::CanonicalKind;
use handl_vm_values_json::NonFunctionValue;

/// Infix fragment for pipe (`|>`) and backward pipe (`<|`) operators.
pub struct Pipe;

const PIPE_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::Pipe, CanonicalKind::BackwardPipe];

impl Pipe {
    /// Creates a new pipe infix fragment.
    pub fn new() -> Self {
        Pipe
    }
}

impl InfixFragment for Pipe {
    fn infix_tokens(&self) -> &'static [CanonicalKind] {
        PIPE_TOKENS
    }

    fn binding(&self, token: &Token) -> Option<(u32, u32)> {
        match token.kind {
            CanonicalKind::Pipe => Some((40, 41)),
            CanonicalKind::BackwardPipe => Some((40, 39)),
            _ => None,
        }
    }

    fn try_infix(
        &self,
        left: Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, handl_platform::PlatformError>> {
        let token = ctx.peek().cloned()?;
        let rbp = match token.kind {
            CanonicalKind::Pipe => 41,
            CanonicalKind::BackwardPipe => 39,
            _ => return None,
        };
        ctx.consume();

        let right = match ctx.parse_expr_with_min_bp(rbp) {
            Ok(r) => r,
            Err(e) => {
                if ctx.recover {
                    ctx.error(e.to_string());
                    return Some(Ok(left));
                } else {
                    return Some(Err(e));
                }
            }
        };

        let result = match token.kind {
            CanonicalKind::Pipe => Expr::call("fn.pipe", vec![left, right]),
            CanonicalKind::BackwardPipe => {
                Expr::call("fn.apply", vec![left, Expr::array(vec![right])])
            }
            _ => unreachable!(),
        };

        Some(Ok(result))
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
    use handl_plugin_operators::Operators;
    use handl_plugin_primary_identifiers::PrimaryIdentifiers;
    use handl_plugin_primary_values::PrimaryValues;

    fn pipeline() -> ExpressionPipeline {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Operators::new()));
        p.register_token_rule(Box::new(Identifiers::new()));
        p.register_token_rule(Box::new(Numbers::new()));
        p.register_token_rule(Box::new(Delimiters::new()));
        p.register_primary(Box::new(PrimaryIdentifiers::new()));
        p.register_primary(Box::new(PrimaryValues::new()));
        p.register_infix(Box::new(Pipe::new()));
        p
    }

    #[test]
    fn pipe_forward() {
        let p = pipeline();
        let tokens = p.lex("1 |> add").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Number(1.0));
        assert_eq!(tokens[1].kind, CanonicalKind::Pipe);
        assert_eq!(tokens[2].kind, CanonicalKind::Identifier("add".to_string()));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::call(
                "fn.pipe",
                vec![
                    Expr::Value(NonFunctionValue::new(1.0)),
                    Expr::var("add"),
                ],
            )
        );
    }

    #[test]
    fn pipe_left_to_right() {
        let p = pipeline();
        let expr = p.parse(p.lex("1 |> add |> mul").unwrap()).unwrap();
        assert_eq!(expr, Expr::call("fn.pipe", vec![
            Expr::call("fn.pipe", vec![
                Expr::Value(NonFunctionValue::new(1.0)),
                Expr::var("add"),
            ]),
            Expr::var("mul"),
        ]));
    }
}
