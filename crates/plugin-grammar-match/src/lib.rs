use handl_plugin_ast::Expr;
use handl_plugin_lexer::{Token, TokenKind};
use handl_parser::{GrammarRule, ParseError, ParserContext};

/// Token capability required by the Match grammar rule.
pub trait MatchToken: TokenKind {
    fn is_match(&self) -> bool;
}

/// Match grammar rule
#[derive(Debug, Default, Clone, Copy)]
pub struct MatchRule;

impl<K, N> GrammarRule<K, N> for MatchRule
where
    K: MatchToken,
    N: Expr,
{
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_match()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut dyn ParserContext<K, N>,
    ) -> Result<N, ParseError> {
        let subject = ctx.expression(0)?;
        Ok(N::match_(subject))
    }

    fn infix_binding(&self, _kind: &K) -> Option<(u32, u32)> {
        None
    }

    fn infix(
        &self,
        _left: N,
        _token: Token<K>,
        _ctx: &mut dyn ParserContext<K, N>,
        _rbp: u32,
    ) -> Result<N, ParseError> {
        Err(ParseError::UnexpectedToken {
            span: _token.span,
            expected: "Match is prefix-only".into(),
        })
    }
}
