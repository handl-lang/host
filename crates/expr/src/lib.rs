//! # handl-expr
//!
//! Composable expression-plugin bridge for the handl platform.
//!
//! This crate provides a small, pluggable pipeline for lexing and
//! parsing handl expressions through fragments without a canonical
//! prefix.

pub mod context;
pub mod diagnostics;
pub mod expr_fragment;
pub mod formatter;
pub mod pipeline;
pub mod statement;
pub mod token;
pub mod token_rule;

pub use context::ParseContext;
pub use diagnostics::{Diagnostic, DiagnosticLevel, Diagnostics};
pub use expr_fragment::{InfixFragment, PostfixFragment, PrimaryFragment};
pub use formatter::Formatter;
pub use pipeline::{ExpressionPipeline, ParseResult};
pub use statement::{expr_to_pattern, ModuleOutput, ModulePipeline, StatementFragment, Stmt, StmtKind};
pub use token::{Span, Token};
pub use token_rule::TokenRule;

#[cfg(test)]
mod tests {
    use super::*;
    use context::ParseContext;
    use expr_fragment::PrimaryFragment;
    use handl_ast_canonical::Expr;
    use handl_lexer::Span;
    use handl_lexer_canonical::CanonicalKind;
    use handl_vm_values_json::NonFunctionValue;
    use token::Token;
    use token_rule::TokenRule;

    struct NumberRule;

    impl TokenRule for NumberRule {
        fn name(&self) -> &str {
            "number"
        }

        fn try_lex(
            &self,
            input: &str,
            start: usize,
        ) -> Option<Result<Token, handl_platform::PlatformError>> {
            let rest = &input[start..];
            let digits: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();

            if digits.is_empty() {
                return None;
            }

            let value: f64 = digits.parse().ok()?;
            let end = start + digits.len();
            Some(Ok(Token::new(CanonicalKind::Number(value), Span::new(start, end))))
        }
    }

    struct StringRule;

    impl TokenRule for StringRule {
        fn name(&self) -> &str {
            "string"
        }

        fn try_lex(
            &self,
            input: &str,
            start: usize,
        ) -> Option<Result<Token, handl_platform::PlatformError>> {
            if !input[start..].starts_with('"') {
                return None;
            }

            let rest = &input[start + 1..];
            let mut chars = rest.chars().enumerate();
            let mut value = String::new();

            while let Some((i, c)) = chars.next() {
                if c == '"' {
                    let end = start + i + 2;
                    return Some(Ok(Token::new(
                        CanonicalKind::String(value),
                        Span::new(start, end),
                    )));
                }
                value.push(c);
            }

            Some(Err(handl_platform::PlatformError::Plugin(
                "unterminated string".to_string(),
            )))
        }
    }

    struct NumberPrimary;

    impl PrimaryFragment for NumberPrimary {
        fn try_primary(
            &self,
            ctx: &mut ParseContext,
        ) -> Option<Result<Expr<NonFunctionValue>, handl_platform::PlatformError>> {
            let token = ctx.peek().cloned()?;
            if let CanonicalKind::Number(n) = token.kind {
                ctx.consume();
                Some(Ok(Expr::Value(NonFunctionValue::new(n))))
            } else {
                None
            }
        }
    }

    struct StringPrimary;

    impl PrimaryFragment for StringPrimary {
        fn try_primary(
            &self,
            ctx: &mut ParseContext,
        ) -> Option<Result<Expr<NonFunctionValue>, handl_platform::PlatformError>> {
            let token = ctx.peek().cloned()?;
            if let CanonicalKind::String(s) = token.kind {
                ctx.consume();
                Some(Ok(Expr::Value(NonFunctionValue::new(s))))
            } else {
                None
            }
        }
    }

    fn pipeline() -> ExpressionPipeline {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(NumberRule));
        p.register_token_rule(Box::new(StringRule));
        p.register_primary(Box::new(NumberPrimary));
        p.register_primary(Box::new(StringPrimary));
        p
    }

    #[test]
    fn number_literal() {
        let p = pipeline();
        let tokens = p.lex("42").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, CanonicalKind::Number(42.0));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(expr, Expr::Value(NonFunctionValue::new(42.0)));
    }

    #[test]
    fn string_literal() {
        let p = pipeline();
        let tokens = p.lex("\"hi\"").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, CanonicalKind::String("hi".to_string()));

        let expr = p.parse(tokens).unwrap();
        assert_eq!(expr, Expr::Value(NonFunctionValue::new("hi".to_string())));
    }
}
