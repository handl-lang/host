use handl_ast_canonical::{Expr, Pattern};
use handl_expr::{ParseContext, PostfixFragment};
use handl_lexer_canonical::CanonicalKind;
use handl_vm_values_json::NonFunctionValue;

/// Postfix fragment for first-class function literals.
pub struct FunctionLiterals;

const FUNCTION_LITERAL_TOKENS: &'static [CanonicalKind] = &[
    CanonicalKind::CallOpen,
    CanonicalKind::LessThan,
];

impl FunctionLiterals {
    /// Creates a new function-literal postfix fragment.
    pub fn new() -> Self {
        FunctionLiterals
    }

    fn parse_generic_params(
        &self,
        ctx: &mut ParseContext,
    ) -> Result<Vec<String>, handl_platform::PlatformError> {
        ctx.expect(CanonicalKind::LessThan)?;
        let mut params = Vec::new();

        if ctx.peek().map(|t| t.kind != CanonicalKind::GreaterThan).unwrap_or(false) {
            let name = match ctx.peek().cloned() {
                Some(token) => match token.kind {
                    CanonicalKind::Identifier(n) => {
                        ctx.consume();
                        n
                    }
                    _ => {
                        return Err(handl_platform::PlatformError::Plugin(
                            "expected generic parameter name".to_string(),
                        ))
                    }
                },
                None => {
                    return Err(handl_platform::PlatformError::Plugin(
                        "expected generic parameter name".to_string(),
                    ))
                }
            };
            params.push(name);

            while ctx.consume_if(CanonicalKind::Comma) {
                let name = match ctx.peek().cloned() {
                    Some(token) => match token.kind {
                        CanonicalKind::Identifier(n) => {
                            ctx.consume();
                            n
                        }
                        _ => {
                            return Err(handl_platform::PlatformError::Plugin(
                                "expected generic parameter name".to_string(),
                            ))
                        }
                    },
                    None => {
                        return Err(handl_platform::PlatformError::Plugin(
                            "expected generic parameter name".to_string(),
                        ))
                    }
                };
                params.push(name);
            }
        }

        ctx.expect(CanonicalKind::GreaterThan)?;
        Ok(params)
    }

    fn parse_pattern(
        &self,
        ctx: &mut ParseContext,
    ) -> Result<Pattern, handl_platform::PlatformError> {
        match ctx.peek().cloned() {
            Some(token) => match token.kind {
                CanonicalKind::Identifier(name) => {
                    ctx.consume();
                    Ok(Pattern::Identifier(name))
                }
                _ => Err(handl_platform::PlatformError::Plugin(
                    "expected parameter name".to_string(),
                )),
            },
            None => Err(handl_platform::PlatformError::Plugin(
                "expected parameter name".to_string(),
            )),
        }
    }

    fn parse_param_list(
        &self,
        ctx: &mut ParseContext,
    ) -> Result<Vec<(Pattern, Option<Box<Expr<NonFunctionValue>>>)>, handl_platform::PlatformError>
    {
        let mut params = Vec::new();

        if ctx.peek().map(|t| t.kind != CanonicalKind::CallClose).unwrap_or(false) {
            let pat = self.parse_pattern(ctx)?;
            let ty = if ctx.consume_if(CanonicalKind::Colon) {
                Some(Box::new(ctx.parse_expr()?))
            } else {
                None
            };
            params.push((pat, ty));

            while ctx.consume_if(CanonicalKind::Comma) {
                let pat = self.parse_pattern(ctx)?;
                let ty = if ctx.consume_if(CanonicalKind::Colon) {
                    Some(Box::new(ctx.parse_expr()?))
                } else {
                    None
                };
                params.push((pat, ty));
            }
        }

        ctx.expect(CanonicalKind::CallClose)?;
        Ok(params)
    }

    fn capture_body(
        &self,
        ctx: &mut ParseContext,
    ) -> Result<String, handl_platform::PlatformError> {
        let start = ctx.peek().map(|t| t.span.start).unwrap_or(ctx.span().start);
        let _ = ctx.parse_expr()?;
        let end = if ctx.pos > 0 {
            ctx.tokens[ctx.pos - 1].span.end
        } else {
            start
        };
        let text = ctx.source_span(start, end);
        if text.is_empty() {
            Ok(ctx.source().to_string())
        } else {
            Ok(text.to_string())
        }
    }
}

impl PostfixFragment for FunctionLiterals {
    fn postfix_tokens(&self) -> &'static [CanonicalKind] {
        FUNCTION_LITERAL_TOKENS
    }

    fn try_postfix(
        &self,
        left: Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, handl_platform::PlatformError>> {
        let name = match left {
            Expr::Var(name) => name,
            _ => return None,
        };

        let has_generics = ctx.peek().map(|t| t.kind == CanonicalKind::LessThan).unwrap_or(false);
        if !has_generics && ctx.peek().map(|t| t.kind != CanonicalKind::CallOpen).unwrap_or(true) {
            return None;
        }

        let type_params = if has_generics {
            match self.parse_generic_params(ctx) {
                Ok(p) => p,
                Err(e) => return Some(Err(e)),
            }
        } else {
            Vec::new()
        };

        if let Err(e) = ctx.expect(CanonicalKind::CallOpen) {
            return Some(Err(e));
        }

        let params = match self.parse_param_list(ctx) {
            Ok(p) => p,
            Err(e) => return Some(Err(e)),
        };

        let ret = if ctx.consume_if(CanonicalKind::Colon) {
            match ctx.parse_expr() {
                Ok(r) => Some(Box::new(r)),
                Err(e) => return Some(Err(e)),
            }
        } else {
            None
        };

        if let Err(e) = ctx.expect(CanonicalKind::Arrow) {
            return Some(Err(e));
        }

        let body = match self.capture_body(ctx) {
            Ok(b) => b,
            Err(e) => return Some(Err(e)),
        };

        Some(Ok(Expr::assign(
            Pattern::Identifier(name),
            Expr::function(type_params, params, ret, body),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;
    use handl_plugin_delimiters::Delimiters;
    use handl_plugin_identifiers::Identifiers;
    use handl_plugin_operators::Operators;
    use handl_plugin_primary_identifiers::PrimaryIdentifiers;

    fn pipeline() -> ExpressionPipeline {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Identifiers::new()));
        p.register_token_rule(Box::new(Delimiters::new()));
        p.register_token_rule(Box::new(Operators::new()));
        p.register_primary(Box::new(PrimaryIdentifiers::new()));
        p.register_postfix(Box::new(FunctionLiterals::new()));
        p
    }

    #[test]
    fn named_function() {
        let p = pipeline();
        let source = "add(a) -> a";
        let tokens = p.lex(source).unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("add".to_string()));

        let expr = p.parse_with_source(tokens, source).unwrap();
        assert_eq!(
            expr,
            Expr::assign(
                Pattern::Identifier("add".to_string()),
                Expr::function(
                    Vec::new(),
                    vec![(Pattern::Identifier("a".to_string()), None)],
                    None,
                    "a",
                ),
            )
        );
    }
}
