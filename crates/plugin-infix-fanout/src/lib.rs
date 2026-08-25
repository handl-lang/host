use handl_ast_canonical::{Expr, Pattern};
use handl_expr::{InfixFragment, ParseContext, Token};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;
use serde_json;

/// Infix fragment for the fanout (`&&&`) arrow combinator.
pub struct Fanout;

const FANOUT_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::Fanout, CanonicalKind::Parallel];

impl Fanout {
    /// Creates a new fanout infix fragment.
    pub fn new() -> Self {
        Fanout
    }
}

fn pattern_to_canonical(pattern: &Pattern) -> Result<String, PlatformError> {
    match pattern {
        Pattern::Identifier(name) => Ok(name.clone()),
        Pattern::Array(elements) => {
            let inner = elements
                .iter()
                .map(pattern_to_canonical)
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            Ok(format!("[{}]", inner))
        }
        Pattern::Object(entries) => {
            let inner = entries
                .iter()
                .map(|(k, p)| {
                    let p = pattern_to_canonical(p)?;
                    Ok(format!("{}: {}", k, p))
                })
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            Ok(format!("{{{}}}", inner))
        }
        Pattern::Rest(name) => Ok(format!("...{}", name)),
    }
}

fn expr_to_canonical(expr: &Expr<NonFunctionValue>) -> Result<String, PlatformError> {
    match expr {
        Expr::Value(v) => Ok(serde_json::to_string(v.inner()).unwrap_or_else(|_| "null".to_string())),
        Expr::Var(name) => Ok(name.clone()),
        Expr::Call { name, args, .. } => {
            let rendered = args
                .iter()
                .map(expr_to_canonical)
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            Ok(format!("{}({})", name, rendered))
        }
        Expr::Apply { target, args } => {
            let target = expr_to_canonical(target)?;
            let rendered = args
                .iter()
                .map(expr_to_canonical)
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            Ok(format!("fn.apply({}, [{}])", target, rendered))
        }
        Expr::FieldAccess { target, field } => {
            let target = expr_to_canonical(target)?;
            Ok(format!("{}.{}", target, field))
        }
        Expr::Index { target, index } => {
            let target = expr_to_canonical(target)?;
            let index = expr_to_canonical(index)?;
            Ok(format!("{}[{}]", target, index))
        }
        Expr::Array(elements) => {
            let rendered = elements
                .iter()
                .map(expr_to_canonical)
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            Ok(format!("[{}]", rendered))
        }
        Expr::Object(entries) => {
            let rendered = entries
                .iter()
                .map(|(k, v)| {
                    let key = serde_json::to_string(k).unwrap_or_else(|_| "\"\"".to_string());
                    let value = expr_to_canonical(v)?;
                    Ok(format!("{}: {}", key, value))
                })
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            Ok(format!("{{{}}}", rendered))
        }
        Expr::Function {
            type_params,
            params,
            ret,
            body,
        } => {
            let params = params
                .iter()
                .map(|(p, _)| pattern_to_canonical(p))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            let ret = match ret {
                Some(r) => {
                    let r = expr_to_canonical(r)?;
                    format!(": {}", r)
                }
                None => String::new(),
            };
            let type_params = if type_params.is_empty() {
                String::new()
            } else {
                format!("<{}>", type_params.join(", "))
            };
            Ok(format!("{}({}){} -> {}", type_params, params, ret, body))
        }
        _ => Err(PlatformError::Plugin(
            "unsupported expression in fanout".to_string(),
        )),
    }
}

impl InfixFragment for Fanout {
    fn infix_tokens(&self) -> &'static [CanonicalKind] {
        FANOUT_TOKENS
    }

    fn binding(&self, token: &Token) -> Option<(u32, u32)> {
        match token.kind {
            CanonicalKind::Fanout => Some((55, 56)),
            _ => None,
        }
    }

    fn try_infix(
        &self,
        left: Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::Fanout {
            return None;
        }
        ctx.consume();

        let right = match ctx.parse_expr_with_min_bp(56) {
            Ok(r) => r,
            Err(e) => return Some(Err(e)),
        };

        let left_src = match expr_to_canonical(&left) {
            Ok(s) => s,
            Err(e) => return Some(Err(e)),
        };
        let right_src = match expr_to_canonical(&right) {
            Ok(s) => s,
            Err(e) => return Some(Err(e)),
        };

        let body = format!(
            "[fn.apply({}, [__x]), fn.apply({}, [__x])]",
            left_src, right_src
        );

        Some(Ok(Expr::function(
            Vec::new(),
            vec![(Pattern::Identifier("__x".to_string()), None)],
            None,
            body,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;
    use handl_plugin_identifiers::Identifiers;
    use handl_plugin_operators::Operators;
    use handl_plugin_primary_identifiers::PrimaryIdentifiers;

    fn pipeline() -> ExpressionPipeline {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Operators::new()));
        p.register_token_rule(Box::new(Identifiers::new()));
        p.register_primary(Box::new(PrimaryIdentifiers::new()));
        p.register_infix(Box::new(Fanout::new()));
        p
    }

    #[test]
    fn fanout_combinator() {
        let p = pipeline();
        let tokens = p.lex("f &&& g").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("f".to_string()));
        assert_eq!(tokens[1].kind, CanonicalKind::Fanout);
        assert_eq!(tokens[2].kind, CanonicalKind::Identifier("g".to_string()));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::function(
                Vec::new(),
                vec![(Pattern::Identifier("__x".to_string()), None)],
                None,
                "[fn.apply(f, [__x]), fn.apply(g, [__x])]".to_string(),
            )
        );
    }
}
