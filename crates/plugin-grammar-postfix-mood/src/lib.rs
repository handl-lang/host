use handl_ast_canonical::Expr;
use handl_expr::{ParseContext, PostfixFragment};
use handl_kit_canonical::{Kit, CanonicalKind};
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
    fn parse_kind(&self, ctx: &mut ParseContext) -> Result<String, handl_platform::PlatformError> {
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
        left: &mut Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<(), handl_platform::PlatformError>> {
        let kind = match ctx.peek().cloned()?.kind {
            k @ (CanonicalKind::Is
            | CanonicalKind::Must
            | CanonicalKind::May
            | CanonicalKind::Not
            | CanonicalKind::When) => k,
            _ => return None,
        };

        let old_left = std::mem::replace(left, Expr::Value(NonFunctionValue::default()));
        ctx.consume();

        let target_kind = match self.parse_kind(ctx) {
            Ok(k) => k,
            Err(e) => {
                *left = old_left;
                return Some(Err(e));
            }
        };

        let mood_fn = match kind {
            CanonicalKind::Is => "type.is",
            CanonicalKind::Must => "type.must",
            CanonicalKind::May => "type.may",
            CanonicalKind::Not => "type.not",
            CanonicalKind::When => "type.when",
            _ => unreachable!(),
        };

        let default = if matches!(kind, CanonicalKind::When) {
            if let Err(e) = ctx.expect(CanonicalKind::Else) {
                *left = old_left;
                return Some(Err(e));
            }
            match ctx.parse_expr() {
                Ok(default) => Some(default),
                Err(e) => {
                    *left = old_left;
                    return Some(Err(e));
                }
            }
        } else {
            None
        };

        let mut args = vec![
            old_left,
            Expr::Value(NonFunctionValue::new(json!(target_kind))),
        ];
        if let Some(default) = default {
            args.push(default);
        }

        *left = Expr::call(mood_fn, args);
        Some(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_plugin_primary_identifiers::PrimaryIdentifiers;

    #[test]
    fn is_mood() {
        let p = pipeline();
        let tokens = kit.lex("x is int").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("x".to_string()));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::call(
                "type.is",
                vec![
                    Expr::Var("x".to_string()),
                    Expr::Value(NonFunctionValue::new(json!("int"))),
                ],
            )
        );
    }
}
