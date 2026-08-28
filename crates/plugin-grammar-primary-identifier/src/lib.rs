use handl_ast_canonical::Expr;
use handl_expr::{ParseContext, PrimaryFragment};
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_vm_values_json::NonFunctionValue;

/// Primary fragment that parses bare identifiers into variable expressions.
pub struct PrimaryIdentifiers;

const PRIMARY_IDENTIFIER_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::Identifier(String::new())];

impl PrimaryIdentifiers {
    /// Creates a new identifier primary fragment.
    pub fn new() -> Self {
        PrimaryIdentifiers
    }
}

impl PrimaryFragment for PrimaryIdentifiers {
    fn primary_tokens(&self) -> &'static [CanonicalKind] {
        PRIMARY_IDENTIFIER_TOKENS
    }

    fn try_primary(
        &self,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, handl_platform::PlatformError>> {
        let token = ctx.peek().cloned()?;
        if let CanonicalKind::Identifier(mut name) = token.kind {
            ctx.consume();
            // Join dotted names (`math.sqrt`) into a single variable, matching
            // the canonical parser's namespacing behaviour.
            while ctx.peek().map(|t| t.kind == CanonicalKind::Dot).unwrap_or(false) {
                ctx.consume();
                match ctx.peek().cloned() {
                    Some(token) => match token.kind {
                        CanonicalKind::Identifier(part) => {
                            ctx.consume();
                            name = format!("{name}.{part}");
                        }
                        _ => {
                            return Some(Err(handl_platform::PlatformError::Plugin(
                                "expected identifier after `.`".to_string(),
                            )))
                        }
                    },
                    None => {
                        return Some(Err(handl_platform::PlatformError::Plugin(
                            "expected identifier after `.`".to_string(),
                        )))
                    }
                }
            }
            Some(Ok(Expr::Var(name)))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use handl_kit_canonical::{Kit, CanonicalKind};
#[test]
    fn parses_identifier() {
        let kit = Kit::new();
p.register_primary(Box::new(PrimaryIdentifiers::new()));

        let tokens = kit.lex("foo").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("foo".to_string()));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(expr, Expr::Var("foo".to_string()));
    }
}
