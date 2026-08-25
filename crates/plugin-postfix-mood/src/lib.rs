use handl_ast_canonical::Expr;
use handl_expr::{ParseContext, PostfixFragment};
use handl_lexer_canonical::CanonicalKind;
use handl_vm_values_json::NonFunctionValue;
use serde_json::json;

/// Postfix fragment for mood operators (`is`, `must`, `may`, `not`, `when`).
pub struct PostfixMood;

const POSTFIX_MOOD_TOKENS: &'static [CanonicalKind] = &[
    CanonicalKind::Is,
    CanonicalKind::Must,
    CanonicalKind::May,
    CanonicalKind::Not,
    CanonicalKind::When,
];

impl PostfixMood {
    /// Creates a new mood postfix fragment.
    pub fn new() -> Self {
        PostfixMood
    }
}

impl PostfixMood {
    fn parse_kind(
        &self,
        ctx: &mut ParseContext,
    ) -> Result<String, handl_platform::PlatformError> {
        let first = match ctx.peek().cloned() {
            Some(token) => match token.kind {
                CanonicalKind::Identifier(name) => {
                    ctx.consume();
                    name
                }
                _ => {
                    return Err(handl_platform::PlatformError::Plugin(
                        "expected kind identifier".to_string(),
                    ))
                }
            },
            None => {
                return Err(handl_platform::PlatformError::Plugin(
                    "expected kind identifier".to_string(),
                ))
            }
        };

        let mut parts = vec![first];
        while ctx.consume_if(CanonicalKind::Dot) {
            let part = match ctx.peek().cloned() {
                Some(token) => match token.kind {
                    CanonicalKind::Identifier(name) => {
                        ctx.consume();
                        name
                    }
                    _ => {
                        return Err(handl_platform::PlatformError::Plugin(
                            "expected identifier after `.`".to_string(),
                        ))
                    }
                },
                None => {
                    return Err(handl_platform::PlatformError::Plugin(
                        "expected identifier after `.`".to_string(),
                    ))
                }
            };
            parts.push(part);
        }

        Ok(parts.join("."))
    }
}

impl PostfixFragment for PostfixMood {
    fn postfix_tokens(&self) -> &'static [CanonicalKind] {
        POSTFIX_MOOD_TOKENS
    }

    fn try_postfix(
        &self,
        left: Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, handl_platform::PlatformError>> {
        let kind = match ctx.peek().cloned()?.kind {
            k @ (CanonicalKind::Is
            | CanonicalKind::Must
            | CanonicalKind::May
            | CanonicalKind::Not
            | CanonicalKind::When) => k,
            _ => return None,
        };

        ctx.consume();

        let target_kind = match self.parse_kind(ctx) {
            Ok(k) => k,
            Err(e) => return Some(Err(e)),
        };

        let mood_fn = match kind {
            CanonicalKind::Is => "mood.int",
            CanonicalKind::Must => "mood.norm",
            CanonicalKind::May => "mood.pot",
            CanonicalKind::Not => "mood.neg",
            CanonicalKind::When => "mood.cnd",
            _ => unreachable!(),
        };

        let mut args = vec![left, Expr::Value(NonFunctionValue::new(json!(target_kind)))];

        if matches!(kind, CanonicalKind::When) {
            if let Err(e) = ctx.expect(CanonicalKind::Else) {
                return Some(Err(e));
            }
            match ctx.parse_expr() {
                Ok(default) => args.push(default),
                Err(e) => return Some(Err(e)),
            }
        }

        Some(Ok(Expr::call(mood_fn, args)))
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
    use handl_plugin_operators::Operators;
    use handl_plugin_primary_identifiers::PrimaryIdentifiers;

    fn pipeline() -> ExpressionPipeline {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Keywords::new()));
        p.register_token_rule(Box::new(Identifiers::new()));
        p.register_token_rule(Box::new(Delimiters::new()));
        p.register_token_rule(Box::new(Operators::new()));
        p.register_primary(Box::new(PrimaryIdentifiers::new()));
        p.register_postfix(Box::new(PostfixMood::new()));
        p
    }

    #[test]
    fn is_mood() {
        let p = pipeline();
        let tokens = p.lex("x is int").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("x".to_string()));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::call(
                "mood.int",
                vec![
                    Expr::Var("x".to_string()),
                    Expr::Value(NonFunctionValue::new(json!("int"))),
                ],
            )
        );
    }
}
