use handl_ast_canonical::{Expr, Pattern};
use handl_expr::{ParseContext, PostfixFragment};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;
use handl_vm_values_json::NonFunctionValue;
use serde_json;
use std::sync::atomic::{AtomicUsize, Ordering};

static PLACEHOLDER_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Postfix fragment for function calls and apply expressions.
pub struct PostfixCalls;

const POSTFIX_CALLS_TOKENS: &'static [CanonicalKind] = &[CanonicalKind::CallOpen];

impl PostfixCalls {
    /// Creates a new call postfix fragment.
    pub fn new() -> Self {
        PostfixCalls
    }

    /// Returns `true` if the `(` at the current position is a function
    /// literal introducer (i.e. a matching `)` is followed by `->`).
    fn arrow_follows_call(ctx: &ParseContext) -> bool {
        let mut depth = 0usize;
        for (i, token) in ctx.tokens[ctx.pos..].iter().enumerate() {
            match token.kind {
                CanonicalKind::CallOpen => depth += 1,
                CanonicalKind::CallClose => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return ctx
                            .tokens
                            .get(ctx.pos + i + 1)
                            .map(|t| t.kind == CanonicalKind::Arrow)
                            .unwrap_or(false);
                    }
                }
                _ => {}
            }
        }
        false
    }
}

fn has_placeholder(expr: &Expr<NonFunctionValue>) -> bool {
    match expr {
        Expr::Var(name) if name == "_" => true,
        Expr::Call { args, .. } | Expr::Apply { args, .. } | Expr::Array(args) => {
            args.iter().any(has_placeholder)
        }
        Expr::FieldAccess { target, .. } | Expr::Index { target, .. } => has_placeholder(target),
        Expr::Object(entries) => entries.iter().any(|(_, v)| has_placeholder(v)),
        _ => false,
    }
}

fn rename_placeholders(
    expr: &mut Expr<NonFunctionValue>,
    params: &mut Vec<String>,
    nonce: usize,
) {
    match expr {
        Expr::Var(name) if name == "_" => {
            let new_name = format!("__p{}_{}", params.len(), nonce);
            *name = new_name.clone();
            params.push(new_name);
        }
        Expr::Call { args, .. } | Expr::Apply { args, .. } | Expr::Array(args) => {
            for a in args.iter_mut() {
                rename_placeholders(a, params, nonce);
            }
        }
        Expr::FieldAccess { target, .. } | Expr::Index { target, .. } => {
            rename_placeholders(&mut **target, params, nonce);
        }
        Expr::Object(entries) => {
            for (_, v) in entries.iter_mut() {
                rename_placeholders(v, params, nonce);
            }
        }
        _ => {}
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
            "unsupported expression in partial application".to_string(),
        )),
    }
}

fn make_partial(mut expr: Expr<NonFunctionValue>) -> Result<Expr<NonFunctionValue>, PlatformError> {
    let nonce = PLACEHOLDER_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut params = Vec::new();
    rename_placeholders(&mut expr, &mut params, nonce);
    let body = expr_to_canonical(&expr)?;
    let patterns = params
        .into_iter()
        .map(|name| (Pattern::Identifier(name), None))
        .collect();
    Ok(Expr::function(Vec::new(), patterns, None, body))
}

impl PostfixFragment for PostfixCalls {
    fn postfix_tokens(&self) -> &'static [CanonicalKind] {
        POSTFIX_CALLS_TOKENS
    }

    fn try_postfix(
        &self,
        left: Expr<NonFunctionValue>,
        ctx: &mut ParseContext,
    ) -> Option<Result<Expr<NonFunctionValue>, PlatformError>> {
        if ctx.peek()?.kind != CanonicalKind::CallOpen {
            return None;
        }
        if Self::arrow_follows_call(ctx) {
            return None;
        }
        ctx.consume(); // `(`

        let mut args = Vec::new();
        if ctx.peek().map(|t| t.kind != CanonicalKind::CallClose).unwrap_or(false) {
            match ctx.parse_expr() {
                Ok(arg) => args.push(arg),
                Err(e) => return Some(Err(e)),
            }
            while ctx.consume_if(CanonicalKind::Comma) {
                if ctx.peek().map(|t| t.kind == CanonicalKind::CallClose).unwrap_or(false) {
                    if ctx.recover {
                        ctx.error("trailing comma in call argument list");
                    } else {
                        return Some(Err(PlatformError::Plugin(
                            "trailing comma in call argument list".to_string(),
                        )));
                    }
                    break;
                }
                match ctx.parse_expr() {
                    Ok(arg) => args.push(arg),
                    Err(e) => {
                        if ctx.recover {
                            ctx.error(e.to_string());
                            break;
                        } else {
                            return Some(Err(e));
                        }
                    }
                }
            }
        }

        if let Err(e) = ctx.expect(CanonicalKind::CallClose) {
            if ctx.recover {
                ctx.error(e.to_string());
            } else {
                return Some(Err(e));
            }
        }

        let result = match left {
            Expr::Var(name) => Expr::call(name, args),
            _ => Expr::apply(left, args),
        };

        if has_placeholder(&result) {
            match make_partial(result) {
                Ok(f) => Some(Ok(f)),
                Err(e) => Some(Err(e)),
            }
        } else {
            Some(Ok(result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;
    use handl_plugin_delimiters::Delimiters;
    use handl_plugin_identifiers::Identifiers;
    use handl_plugin_numbers::Numbers;
    use handl_plugin_primary_identifiers::PrimaryIdentifiers;
    use handl_plugin_primary_parens::PrimaryParens;
    use handl_plugin_primary_values::PrimaryValues;

    fn pipeline() -> ExpressionPipeline {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Identifiers::new()));
        p.register_token_rule(Box::new(Numbers::new()));
        p.register_token_rule(Box::new(Delimiters::new()));
        p.register_primary(Box::new(PrimaryIdentifiers::new()));
        p.register_primary(Box::new(PrimaryValues::new()));
        p.register_primary(Box::new(PrimaryParens::new()));
        p.register_postfix(Box::new(PostfixCalls::new()));
        p
    }

    #[test]
    fn named_call() {
        let p = pipeline();
        let tokens = p.lex("add(1, 2)").unwrap();
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("add".to_string()));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::call(
                "add",
                vec![
                    Expr::Value(NonFunctionValue::new(1.0)),
                    Expr::Value(NonFunctionValue::new(2.0)),
                ],
            )
        );
    }

    #[test]
    fn apply_non_var() {
        let p = pipeline();
        let tokens = p.lex("(1)(2)").unwrap();

        let expr = p.parse(tokens).unwrap();
        assert_eq!(
            expr,
            Expr::apply(
                Expr::Value(NonFunctionValue::new(1.0)),
                vec![Expr::Value(NonFunctionValue::new(2.0))],
            )
        );
    }

    #[test]
    fn partial_application() {
        let p = pipeline();
        let expr = p.parse(p.lex("add(_, 1)").unwrap()).unwrap();
        assert_eq!(
            expr,
            Expr::function(
                Vec::new(),
                vec![(Pattern::Identifier("__p0_0".to_string()), None)],
                None,
                "add(__p0_0, 1.0)",
            )
        );
    }
}
