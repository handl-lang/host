use handl_ast_canonical::Expr;
use handl_expr::{InfixFragment, ParseContext, Token};
use handl_kit_canonical::{Kit, CanonicalKind};
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
        left: &mut Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<(), handl_platform::PlatformError>> {
        let token = ctx.peek().cloned()?;
        let rbp = match token.kind {
            CanonicalKind::Pipe => 41,
            CanonicalKind::BackwardPipe => 39,
            _ => return None,
        };
        let old_left = std::mem::replace(left, Expr::Value(NonFunctionValue::default()));
        ctx.consume();

        let right = match ctx.parse_expr_with_min_bp(rbp) {
            Ok(r) => r,
            Err(e) => {
                *left = old_left;
                if ctx.recover {
                    ctx.error(e.to_string());
                    return Some(Ok(()));
                } else {
                    return Some(Err(e));
                }
            }
        };

        *left = match token.kind {
            CanonicalKind::Pipe => Expr::call("fn.pipe", vec![old_left, right]),
            CanonicalKind::BackwardPipe => {
                Expr::call("fn.apply", vec![old_left, Expr::array(vec![right])])
            }
            _ => {
                *left = old_left;
                return None;
            }
        };

        Some(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_plugin_primary_identifiers::PrimaryIdentifiers;
    use handl_plugin_primary_values::PrimaryValues;

    #[test]
    fn pipe_forward() {
        let p = pipeline();
        let tokens = kit.lex("1 |> add").unwrap();
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
        let expr = p.parse(kit.lex("1 |> add |> mul").unwrap()).unwrap();
        assert_eq!(expr, Expr::call("fn.pipe", vec![
            Expr::call("fn.pipe", vec![
                Expr::Value(NonFunctionValue::new(1.0)),
                Expr::var("add"),
            ]),
            Expr::var("mul"),
        ]));
    }
}
