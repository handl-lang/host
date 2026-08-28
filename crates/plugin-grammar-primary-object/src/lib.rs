use handl_ast_canonical::Expr;
use handl_expr::{ParseContext, PrimaryFragment};
use handl_kit_canonical::{Kit, CanonicalKind};
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;

/// Primary fragment that parses object literals.
pub struct PrimaryObjects;

const PRIMARY_OBJECTS_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::BlockOpen];

impl PrimaryObjects {
    /// Creates a new object primary fragment.
    pub fn new() -> Self {
        PrimaryObjects
    }
}

impl PrimaryFragment for PrimaryObjects {
    fn primary_tokens(&self) -> &'static [CanonicalKind] {
        PRIMARY_OBJECTS_TOKENS
    }

    fn try_primary(
        &self,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::BlockOpen {
            return None;
        }
        ctx.consume();

        let mut entries = Vec::new();
        if ctx.consume_if(CanonicalKind::BlockClose) {
            return Some(Ok(Expr::Object(entries)));
        }

        loop {
            let token = ctx.peek().cloned()?;
            let key = match token.kind {
                CanonicalKind::String(s) | CanonicalKind::Identifier(s) => {
                    ctx.consume();
                    s
                }
                _ => {
                    return Some(Err(PlatformError::Plugin(
                        "object key expected".to_string(),
                    )))
                }
            };

            let value = if ctx.consume_if(CanonicalKind::Colon) {
                match ctx.parse_expr() {
                    Ok(e) => e,
                    Err(e) => return Some(Err(e)),
                }
            } else {
                Expr::Var(key.clone())
            };

            entries.push((key, value));

            if !ctx.consume_if(CanonicalKind::Comma) {
                if let Err(e) = ctx.expect(CanonicalKind::BlockClose) {
                    return Some(Err(e));
                }
                break;
            } else if ctx.consume_if(CanonicalKind::BlockClose) {
                break;
            }
        }

        Some(Ok(Expr::Object(entries)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use handl_plugin_primary_identifiers::PrimaryIdentifiers;
    use handl_plugin_primary_values::PrimaryValues;
#[test]
    fn parses_object() {
        let kit = Kit::new();
p.register_token_rule(Box::new(Strings::new()));
p.register_token_rule(Box::new(Delimiters::new()));
        p.register_primary(Box::new(PrimaryObjects::new()));
        p.register_primary(Box::new(PrimaryValues::new()));
        p.register_primary(Box::new(PrimaryIdentifiers::new()));

        let tokens = kit.lex("{ \"a\": 1 }").unwrap();
        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::Object(vec![(
                "a".to_string(),
                Expr::Value(NonFunctionValue::new(1.0))
            )])
        );
    }

    #[test]
    fn parses_empty_object() {
        let kit = Kit::new();
p.register_primary(Box::new(PrimaryObjects::new()));

        let tokens = kit.lex("{}").unwrap();
        let expr = p.parse(tokens).unwrap();
        assert_eq!(expr, Expr::Object(vec![]));
    }
}
