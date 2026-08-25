use handl_ast_canonical::{Expr, Pattern};
use handl_expr::{ParseContext, PrimaryFragment};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;
use serde_json::json;

/// Primary fragment that parses a canonical `match` expression.
///
/// Builds `fn.match(value, [pattern, function], ...)` where each pattern is
/// a JSON value and each handler is a first-class function whose parameters
/// are the pattern's identifier bindings.
pub struct PrimaryMatch;

const PRIMARY_MATCH_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::Match];

impl PrimaryMatch {
    /// Creates a new `match` primary fragment.
    pub fn new() -> Self {
        PrimaryMatch
    }

    fn capture_body(ctx: &mut ParseContext) -> Result<String, PlatformError> {
        let start = ctx.peek().map(|t| t.span.start).unwrap_or(ctx.span().start);
        ctx.parse_expr()?;
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

    fn parse_pattern(
        ctx: &mut ParseContext,
    ) -> Result<(NonFunctionValue, Vec<String>), PlatformError> {
        let token = ctx
            .peek()
            .cloned()
            .ok_or(PlatformError::Plugin("expected match pattern".to_string()))?;
        match token.kind {
            CanonicalKind::Number(n) => {
                ctx.consume();
                Ok((NonFunctionValue::new(json!(n)), Vec::new()))
            }
            CanonicalKind::String(s) => {
                ctx.consume();
                Ok((NonFunctionValue::new(json!(s)), Vec::new()))
            }
            CanonicalKind::Bool(b) => {
                ctx.consume();
                Ok((NonFunctionValue::new(json!(b)), Vec::new()))
            }
            CanonicalKind::Null => {
                ctx.consume();
                Ok((NonFunctionValue::new(serde_json::Value::Null), Vec::new()))
            }
            CanonicalKind::Identifier(name) => {
                ctx.consume();
                if name == "_" {
                    Ok((NonFunctionValue::new(json!({"type": "wildcard"})), Vec::new()))
                } else {
                    Ok((
                        NonFunctionValue::new(json!({"type": "identifier", "name": name.clone()})),
                        vec![name],
                    ))
                }
            }
            CanonicalKind::BracketOpen => {
                ctx.consume();
                let mut patterns = Vec::new();
                let mut bindings = Vec::new();
                if ctx
                    .peek()
                    .map(|t| t.kind != CanonicalKind::BracketClose)
                    .unwrap_or(false)
                {
                    loop {
                        let (p, mut b) = Self::parse_pattern(ctx)?;
                        patterns.push(p.into_inner());
                        bindings.append(&mut b);
                        if !ctx.consume_if(CanonicalKind::Comma) {
                            break;
                        }
                    }
                }
                ctx.expect(CanonicalKind::BracketClose)?;
                Ok((
                    NonFunctionValue::new(json!({"type": "array", "patterns": patterns})),
                    bindings,
                ))
            }
            CanonicalKind::BlockOpen => {
                ctx.consume();
                let mut entries = Vec::new();
                let mut bindings = Vec::new();
                if ctx
                    .peek()
                    .map(|t| t.kind != CanonicalKind::BlockClose)
                    .unwrap_or(false)
                {
                    loop {
                        let key_token = ctx
                            .peek()
                            .cloned()
                            .ok_or(PlatformError::Plugin(
                                "expected object pattern key".to_string(),
                            ))?;
                        let key = match key_token.kind {
                            CanonicalKind::String(s) | CanonicalKind::Identifier(s) => {
                                ctx.consume();
                                s
                            }
                            _ => {
                                return Err(PlatformError::Plugin(
                                    "expected object pattern key".to_string(),
                                ))
                            }
                        };
                        let (value, mut b) = if ctx.consume_if(CanonicalKind::Colon) {
                            Self::parse_pattern(ctx)?
                        } else {
                            // Shorthand: the key name is also the binding.
                            (
                                NonFunctionValue::new(
                                    json!({"type": "identifier", "name": key.clone()}),
                                ),
                                vec![key.clone()],
                            )
                        };
                        entries.push(json!([key, value.into_inner()]));
                        bindings.append(&mut b);
                        if !ctx.consume_if(CanonicalKind::Comma) {
                            break;
                        }
                    }
                }
                ctx.expect(CanonicalKind::BlockClose)?;
                Ok((
                    NonFunctionValue::new(json!({"type": "object", "entries": entries})),
                    bindings,
                ))
            }
            _ => Err(PlatformError::Plugin(format!(
                "unexpected token {} in match pattern",
                token.kind
            ))),
        }
    }
}

impl PrimaryFragment for PrimaryMatch {
    fn primary_tokens(&self) -> &'static [CanonicalKind] {
        PRIMARY_MATCH_TOKENS
    }

    fn try_primary(
        &self,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::Match {
            return None;
        }
        ctx.consume();

        let value = match ctx.parse_expr() {
            Ok(expr) => expr,
            Err(e) => return Some(Err(e)),
        };

        if let Err(e) = ctx.expect(CanonicalKind::BlockOpen) {
            return Some(Err(e));
        }

        let mut branches = Vec::new();
        if ctx
            .peek()
            .map(|t| t.kind != CanonicalKind::BlockClose)
            .unwrap_or(false)
        {
            loop {
                let (pattern, bindings) = match Self::parse_pattern(ctx) {
                    Ok(p) => p,
                    Err(e) => return Some(Err(e)),
                };

                if let Err(e) = ctx.expect(CanonicalKind::Arrow) {
                    return Some(Err(e));
                }

                let body = match Self::capture_body(ctx) {
                    Ok(b) => b,
                    Err(e) => return Some(Err(e)),
                };

                let params: Vec<(Pattern, Option<Box<Expr<NonFunctionValue>>>)> = bindings
                    .into_iter()
                    .map(|name| (Pattern::Identifier(name), None))
                    .collect();
                let handler = Expr::function(Vec::new(), params, None, body);

                branches.push(Expr::array(vec![Expr::Value(pattern), handler]));

                if !ctx.consume_if(CanonicalKind::Comma) {
                    break;
                }
            }
        }

        if let Err(e) = ctx.expect(CanonicalKind::BlockClose) {
            return Some(Err(e));
        }

        let mut args = vec![value];
        args.extend(branches);
        Some(Ok(Expr::call("fn.match", args)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;
    use handl_plugin_delimiters::Delimiters;
    use handl_plugin_identifiers::Identifiers;
    use handl_plugin_keywords::Keywords;
    use handl_plugin_numbers::Numbers;
    use handl_plugin_operators::Operators;
    use handl_plugin_primary_identifiers::PrimaryIdentifiers;
    use handl_plugin_primary_values::PrimaryValues;

    #[test]
    fn parses_minimal_match() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Keywords::new()));
        p.register_token_rule(Box::new(Identifiers::new()));
        p.register_token_rule(Box::new(Numbers::new()));
        p.register_token_rule(Box::new(Delimiters::new()));
        p.register_token_rule(Box::new(Operators::new()));

        p.register_primary(Box::new(PrimaryMatch::new()));
        p.register_primary(Box::new(PrimaryValues::new()));
        p.register_primary(Box::new(PrimaryIdentifiers::new()));

        let source = "match x { 1 -> 2 }";
        let tokens = p.lex(source).unwrap();
        assert!(matches!(tokens[0].kind, CanonicalKind::Match));

        let expr = p.parse_with_source(tokens, source).unwrap();
        let expected = Expr::call(
            "fn.match",
            vec![
                Expr::Var("x".to_string()),
                Expr::array(vec![
                    Expr::Value(NonFunctionValue::new(1.0)),
                    Expr::function(Vec::new(), Vec::new(), None, "2"),
                ]),
            ],
        );
        assert_eq!(expr, expected);
    }
}
