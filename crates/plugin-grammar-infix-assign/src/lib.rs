use handl_ast_canonical::{Expr, Pattern};
use handl_expr::{InfixFragment, ParseContext, Token};
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_vm_values_json::NonFunctionValue;

/// Infix fragment for the assignment (`=`) operator.
pub struct Assign;

const ASSIGN_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::Equal];

impl Assign {
    /// Creates a new assignment infix fragment.
    pub fn new() -> Self {
        Assign
    }
}

fn expr_to_pattern<V>(expr: &Expr<V>) -> Result<Pattern, handl_platform::PlatformError> {
    match expr {
        Expr::Var(name) => Ok(Pattern::Identifier(name.clone())),
        Expr::Array(elements) => Ok(Pattern::Array(
            elements
                .iter()
                .map(expr_to_pattern)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Expr::Object(entries) => Ok(Pattern::Object(
            entries
                .iter()
                .map(|(k, v)| expr_to_pattern(v).map(|p| (k.clone(), p)))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        _ => Err(handl_platform::PlatformError::Plugin(
            "invalid assignment target".to_string(),
        )),
    }
}

impl InfixFragment for Assign {
    fn infix_tokens(&self) -> &'static [CanonicalKind] {
        ASSIGN_TOKENS
    }

    fn binding(&self, token: &Token) -> Option<(u32, u32)> {
        match token.kind {
            CanonicalKind::Equal => Some((1, 0)),
            _ => None,
        }
    }

    fn try_infix(
        &self,
        left: &mut Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<(), handl_platform::PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::Equal {
            return None;
        }
        let old_left = std::mem::replace(left, Expr::Value(NonFunctionValue::default()));
        ctx.consume(); // `=`

        let target = match expr_to_pattern(&old_left) {
            Ok(p) => p,
            Err(e) => {
                *left = old_left;
                return Some(Err(e));
            }
        };

        let right = match ctx.parse_expr_with_min_bp(0) {
            Ok(r) => r,
            Err(e) => {
                *left = old_left;
                return Some(Err(e));
            }
        };

        *left = Expr::assign(target, right);
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
    fn simple_assignment() {
        let p = pipeline();
        let tokens = kit.lex("a = 1").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("a".to_string()));
        assert_eq!(tokens[1].kind, CanonicalKind::Equal);
        assert_eq!(tokens[2].kind, CanonicalKind::Number(1.0));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::Assign {
                target: Pattern::Identifier("a".to_string()),
                value: Box::new(Expr::Value(NonFunctionValue::new(1.0))),
            }
        );
    }
}
