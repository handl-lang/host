use handl_ast_canonical::Expr;
use handl_expr::{ParseContext, PostfixFragment};
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_vm_values_json::NonFunctionValue;

/// Postfix fragment for field access expressions.
pub struct PostfixField;

const POSTFIX_FIELD_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::Dot];

impl PostfixField {
    /// Creates a new field-access postfix fragment.
    pub fn new() -> Self {
        PostfixField
    }
}

impl PostfixFragment for PostfixField {
    fn postfix_tokens(&self) -> &'static [CanonicalKind] {
        POSTFIX_FIELD_TOKENS
    }

    fn try_postfix(
        &self,
        left: &mut Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<(), handl_platform::PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::Dot {
            return None;
        }
        let old_left = std::mem::replace(left, Expr::Value(NonFunctionValue::default()));
        ctx.consume(); // `.`

        let field = match ctx.peek().cloned() {
            Some(token) => match token.kind {
                CanonicalKind::Identifier(name) => {
                    ctx.consume();
                    name
                }
                _ => {
                    *left = old_left;
                    return Some(Err(handl_platform::PlatformError::Plugin(
                        "expected field name after `.`".to_string(),
                    )))
                }
            },
            None => {
                *left = old_left;
                return Some(Err(handl_platform::PlatformError::Plugin(
                    "expected field name after `.`".to_string(),
                )))
            }
        };

        *left = Expr::field_access(old_left, field);
        Some(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_plugin_primary_identifiers::PrimaryIdentifiers;

    #[test]
    fn field_access() {
        let p = pipeline();
        let tokens = kit.lex("obj.key").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("obj".to_string()));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(expr, Expr::field_access(Expr::Var("obj".to_string()), "key"));
    }
}
