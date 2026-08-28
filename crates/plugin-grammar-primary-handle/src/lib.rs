use handl_ast_canonical::{Expr, HandlerClause};
use handl_expr::{ParseContext, PrimaryFragment};
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;

/// Primary fragment that parses a `handle` expression.
///
/// Supports the canonical form `handle(op(params...) as cont: body, ...)`
/// plus the minimal `handle op as body` form for kit compatibility.
pub struct PrimaryHandle;

const PRIMARY_HANDLE_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::Handle];

impl PrimaryHandle {
    /// Creates a new `handle` primary fragment.
    pub fn new() -> Self {
        PrimaryHandle
    }

    fn parse_handler_clause(
        ctx: &mut ParseContext,
    ) -> Result<HandlerClause<NonFunctionValue>, PlatformError> {
        let op_name = match ctx.peek().cloned() {
            Some(token) => match token.kind {
                CanonicalKind::String(s) => {
                    ctx.consume();
                    s
                }
                _ => {
                    return Err(PlatformError::Plugin(
                        "expected handler operation name string".to_string(),
                    ))
                }
            },
            None => {
                return Err(PlatformError::Plugin(
                    "expected handler operation name string".to_string(),
                ))
            }
        };

        ctx.expect(CanonicalKind::CallOpen)?;

        let mut params = Vec::new();
        if ctx
            .peek()
            .map(|t| t.kind != CanonicalKind::CallClose)
            .unwrap_or(false)
        {
            loop {
                let name = match ctx.peek().cloned() {
                    Some(token) => match token.kind {
                        CanonicalKind::Identifier(n) => {
                            ctx.consume();
                            n
                        }
                        _ => {
                            return Err(PlatformError::Plugin(
                                "expected handler parameter name".to_string(),
                            ))
                        }
                    },
                    None => {
                        return Err(PlatformError::Plugin(
                            "expected handler parameter name".to_string(),
                        ))
                    }
                };
                params.push(name);
                if !ctx.consume_if(CanonicalKind::Comma) {
                    break;
                }
            }
        }

        ctx.expect(CanonicalKind::CallClose)?;
        ctx.expect(CanonicalKind::As)?;

        let continuation_name = match ctx.peek().cloned() {
            Some(token) => match token.kind {
                CanonicalKind::Identifier(n) => {
                    ctx.consume();
                    n
                }
                _ => {
                    return Err(PlatformError::Plugin(
                        "expected continuation name".to_string(),
                    ))
                }
            },
            None => {
                return Err(PlatformError::Plugin(
                    "expected continuation name".to_string(),
                ))
            }
        };

        ctx.expect(CanonicalKind::Colon)?;

        let body = ctx.parse_expr()?;

        Ok(HandlerClause {
            op_name,
            params,
            continuation_name,
            body,
        })
    }
}

impl PrimaryFragment for PrimaryHandle {
    fn primary_tokens(&self) -> &'static [CanonicalKind] {
        PRIMARY_HANDLE_TOKENS
    }

    fn try_primary(
        &self,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::Handle {
            return None;
        }
        ctx.consume();

        // Canonical form: handle("op"(p) as k: body, ...)
        if ctx.consume_if(CanonicalKind::CallOpen) {
            let mut handlers = Vec::new();
            handlers.push(match Self::parse_handler_clause(ctx) {
                Ok(c) => c,
                Err(e) => return Some(Err(e)),
            });
            while ctx.consume_if(CanonicalKind::Comma) {
                handlers.push(match Self::parse_handler_clause(ctx) {
                    Ok(c) => c,
                    Err(e) => return Some(Err(e)),
                });
            }
            if let Err(e) = ctx.expect(CanonicalKind::CallClose) {
                return Some(Err(e));
            }
            let body = handlers.last().unwrap().body.clone();
            return Some(Ok(Expr::handle(body, handlers)));
        }

        // Minimal fallback: handle op as body
        let op_name = match ctx.peek().cloned() {
            Some(token) => match token.kind {
                CanonicalKind::Identifier(name) => {
                    ctx.consume();
                    name
                }
                _ => {
                    return Some(Err(PlatformError::Plugin(
                        "expected handler operation name".to_string(),
                    )))
                }
            },
            None => {
                return Some(Err(PlatformError::Plugin(
                    "expected handler operation name".to_string(),
                )))
            }
        };

        if let Err(e) = ctx.expect(CanonicalKind::As) {
            return Some(Err(e));
        }

        let body = match ctx.parse_expr() {
            Ok(expr) => expr,
            Err(e) => return Some(Err(e)),
        };

        let clause = HandlerClause {
            op_name,
            params: Vec::new(),
            continuation_name: String::new(),
            body: body.clone(),
        };

        Some(Ok(Expr::handle(body, vec![clause])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use handl_plugin_postfix_calls::PostfixCalls;
    use handl_plugin_primary_identifiers::PrimaryIdentifiers;
    use handl_plugin_primary_parens::PrimaryParens;
    use handl_plugin_primary_values::PrimaryValues;

    #[test]
    fn parses_minimal_handle() {
        let kit = Kit::new();
p.register_token_rule(Box::new(Identifiers::new()));
p.register_token_rule(Box::new(Delimiters::new()));

        p.register_primary(Box::new(PrimaryHandle::new()));
        p.register_primary(Box::new(PrimaryParens::new()));
        p.register_primary(Box::new(PrimaryValues::new()));
        p.register_primary(Box::new(PrimaryIdentifiers::new()));
        p.register_postfix(Box::new(PostfixCalls::new()));

        let tokens = kit.lex("handle add as add").unwrap();
        assert!(matches!(tokens[0].kind, CanonicalKind::Handle));

        let expr = p.parse(tokens).unwrap();
        let expected = Expr::handle(
            Expr::Var("add".to_string()),
            vec![HandlerClause {
                op_name: "add".to_string(),
                params: Vec::new(),
                continuation_name: String::new(),
                body: Expr::Var("add".to_string()),
            }],
        );
        assert_eq!(expr, expected);
    }

    #[test]
    fn parses_full_handle() {
        let kit = Kit::new();
p.register_token_rule(Box::new(Identifiers::new()));
p.register_token_rule(Box::new(Delimiters::new()));
p.register_primary(Box::new(PrimaryHandle::new()));
        p.register_primary(Box::new(PrimaryParens::new()));
        p.register_primary(Box::new(PrimaryValues::new()));
        p.register_primary(Box::new(PrimaryIdentifiers::new()));
        p.register_postfix(Box::new(PostfixCalls::new()));

        let source = r#"handle("fetch"(url) as k: k(42))"#;
        let tokens = kit.lex(source).unwrap();
        let expr = p.parse(tokens).unwrap();
        let expected = Expr::handle(
            Expr::call("k".to_string(), vec![Expr::Value(NonFunctionValue::new(42.0))]),
            vec![HandlerClause {
                op_name: "fetch".to_string(),
                params: vec!["url".to_string()],
                continuation_name: "k".to_string(),
                body: Expr::call("k".to_string(), vec![Expr::Value(NonFunctionValue::new(42.0))]),
            }],
        );
        assert_eq!(expr, expected);
    }
}
