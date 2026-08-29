//! Canonical `Call` grammar-rule plugin.
//!
//! Provides the `CallRule` and the `CallToken` capability trait.
//! `CallRule` implements `handl_parser::GrammarRule`, the base capability
//! trait from the generic parser crate.

use handl_plugin_ast::Expr;
use handl_plugin_lexer::{Token, TokenKind};
use handl_parser::{GrammarRule, ParseError, ParserContext};

/// Token capability required by the call grammar rule.
pub trait CallToken: TokenKind {
    /// Returns `true` for the token that opens a call, e.g. `(`.
    fn is_call_open(&self) -> bool;

    /// Returns `true` for the token that closes a call, e.g. `)`.
    fn is_call_close(&self) -> bool;

    /// Returns `true` for the argument-separator token, e.g. `,`.
    fn is_comma(&self) -> bool;
}

/// Grammar rule that parses function-call syntax: `callee(arg1, arg2, ...)`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CallRule;

impl<K, N> GrammarRule<K, N> for CallRule
where
    K: CallToken,
    N: Expr,
{
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_call_open()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut dyn ParserContext<K, N>,
    ) -> Result<N, ParseError> {
        // Parenthesized grouping: `( expr )`.
        let inner = ctx.expression(0)?;
        let close = ctx.advance()?;
        if !close.kind.is_call_close() {
            return Err(ParseError::UnexpectedToken {
                span: close.span,
                expected: "`)`".into(),
            });
        }
        Ok(inner)
    }

    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)> {
        if kind.is_call_open() {
            Some((90, 91))
        } else {
            None
        }
    }

    fn infix(
        &self,
        left: N,
        _token: Token<K>,
        ctx: &mut dyn ParserContext<K, N>,
        _rbp: u32,
    ) -> Result<N, ParseError> {
        let mut args = Vec::new();

        if !ctx.peek_kind().as_ref().map_or(true, |k| k.is_call_close()) {
            loop {
                args.push(ctx.expression(0)?);

                match ctx.peek_kind() {
                    Some(ref k) if k.is_call_close() => break,
                    Some(ref k) if k.is_comma() => {
                        ctx.advance()?;
                    }
                    Some(_) | None => break,
                }
            }
        }

        let close = ctx.advance()?;
        if !close.kind.is_call_close() {
            return Err(ParseError::UnexpectedToken {
                span: close.span,
                expected: "`)`".into(),
            });
        }

        Ok(N::apply(left, args))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_ast::Node;
    use handl_plugin_lexer::{EofFactory, Span, Token};
    use std::fmt;

    #[derive(Debug, Clone, PartialEq)]
    enum TestToken {
        Ident(String),
        CallOpen,
        CallClose,
        Comma,
        Eof,
    }

    impl fmt::Display for TestToken {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                TestToken::Ident(s) => write!(f, "ident({s})"),

                TestToken::CallOpen => write!(f, "("),
                TestToken::CallClose => write!(f, ")"),
                TestToken::Comma => write!(f, ","),
                TestToken::Eof => write!(f, "eof"),
            }
        }
    }

    #[derive(Debug, Clone)]
    struct TestTokenFactory;

    impl EofFactory<TestToken> for TestTokenFactory {
        fn eof_kind(&self) -> TestToken {
            TestToken::Eof
        }
    }

    impl TokenKind for TestToken {
        type Factory = TestTokenFactory;

        fn is_eof(&self) -> bool {
            matches!(self, TestToken::Eof)
        }

        fn is_error(&self) -> bool {
            false
        }
    }

    impl CallToken for TestToken {
        fn is_call_open(&self) -> bool {
            matches!(self, TestToken::CallOpen)
        }

        fn is_call_close(&self) -> bool {
            matches!(self, TestToken::CallClose)
        }

        fn is_comma(&self) -> bool {
            matches!(self, TestToken::Comma)
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    enum TestExpr {
        Var(String),
        Lit(f64),
        Apply { target: Box<TestExpr>, args: Vec<TestExpr> },
    }

    impl Node for TestExpr {
        type Value = f64;
        type Metadata = ();

        fn metadata(&self) -> &Self::Metadata {
            &()
        }

        fn with_metadata(self, _metadata: Self::Metadata) -> Self
        where
            Self: Sized,
        {
            self
        }
    }

    impl Expr for TestExpr {
        fn value(value: <Self as Node>::Value) -> Self
        where
            Self: Sized,
        {
            TestExpr::Lit(value)
        }

        fn call<S>(name: S, args: Vec<Self>) -> Self
        where
            Self: Sized,
            S: Into<String>,
        {
            TestExpr::Apply {
                target: Box::new(TestExpr::Var(name.into())),
                args,
            }
        }

        fn apply(target: Self, args: Vec<Self>) -> Self
        where
            Self: Sized,
        {
            TestExpr::Apply {
                target: Box::new(target),
                args,
            }
        }

        fn array(_elements: Vec<Self>) -> Self
        where
            Self: Sized,
        {
            TestExpr::Lit(0.0)
        }

        fn object(_entries: Vec<(String, Self)>) -> Self
        where
            Self: Sized,
        {
            TestExpr::Lit(0.0)
        }

        fn index(_target: Self, _index: Self) -> Self
        where
            Self: Sized,
        {
            TestExpr::Lit(0.0)
        }

        fn pipe(_left: Self, _right: Self) -> Self
        where
            Self: Sized,
        {
            TestExpr::Lit(0.0)
        }

        fn assign(_target: Self, _value: Self) -> Self
        where
            Self: Sized,
        {
            TestExpr::Lit(0.0)
        }

        fn match_(_subject: Self) -> Self
        where
            Self: Sized,
        {
            TestExpr::Lit(0.0)
        }

        fn handle(_expr: Self) -> Self
        where
            Self: Sized,
        {
            TestExpr::Lit(0.0)
        }
    }

    #[test]
    fn call_open_is_infix() {
        let rule: &dyn GrammarRule<TestToken, TestExpr> = &CallRule;
        let open = Token::new(TestToken::CallOpen, Span::new(0, 1));
        let ident = Token::new(TestToken::Ident("foo".into()), Span::new(0, 3));

        assert!(rule.matches_prefix(&open.kind));
        assert_eq!(rule.infix_binding(&open.kind), Some((90, 91)));
        assert_eq!(rule.infix_binding(&ident.kind), None);
    }

    #[test]
    fn token_kinds() {
        assert!(TestToken::CallOpen.is_call_open());
        assert!(TestToken::CallClose.is_call_close());
        assert!(TestToken::Comma.is_comma());
        assert!(TestToken::Eof.is_eof());
    }
}
