use handl_ast_canonical::Expr;
use handl_expr::{InfixFragment, ParseContext, Token};
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_vm_values_json::NonFunctionValue;

/// Infix fragment for flow (`>>>`) and compose (`<<<`) arrow combinators.
pub struct Flow;

const FLOW_TOKENS: &'static [CanonicalKind] = &[
    CanonicalKind::Flow,
    CanonicalKind::Compose,
];

impl Flow {
    /// Creates a new flow/compose infix fragment.
    pub fn new() -> Self {
        Flow
    }
}

impl InfixFragment for Flow {
    fn infix_tokens(&self) -> &'static [CanonicalKind] {
        FLOW_TOKENS
    }

    fn binding(&self, token: &Token) -> Option<(u32, u32)> {
        match token.kind {
            CanonicalKind::Flow | CanonicalKind::Compose => Some((60, 61)),
            _ => None,
        }
    }

    fn try_infix(
        &self,
        left: &mut Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<(), handl_platform::PlatformError>> {
        let token = ctx.peek().cloned()?;
        let name = match token.kind {
            CanonicalKind::Flow => "fn.flow",
            CanonicalKind::Compose => "fn.compose",
            _ => return None,
        };
        let old_left = std::mem::replace(left, Expr::Value(NonFunctionValue::default()));
        ctx.consume();

        let right = match ctx.parse_expr_with_min_bp(61) {
            Ok(r) => r,
            Err(e) => {
                *left = old_left;
                return Some(Err(e));
            }
        };

        *left = Expr::call(name, vec![old_left, right]);
        Some(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_plugin_primary_identifiers::PrimaryIdentifiers;

    #[test]
    fn flow_forward() {
        let p = pipeline();
        let tokens = kit.lex("f >>> g").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("f".to_string()));
        assert_eq!(tokens[1].kind, CanonicalKind::Flow);
        assert_eq!(tokens[2].kind, CanonicalKind::Identifier("g".to_string()));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(expr, Expr::call("fn.flow", vec![Expr::var("f"), Expr::var("g")]));
    }
}
