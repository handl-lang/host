use handl_ast::Expr;
use handl_lexer::{Token, TokenKind};
use handl_parser::{GrammarRule, ParseError, ParserContext};

/// Token capability required by the Index grammar rule.
pub trait IndexToken: TokenKind {
    fn is_bracket_open(&self) -> bool;
    fn is_bracket_close(&self) -> bool;
}

/// Index grammar rule
#[derive(Debug, Default, Clone, Copy)]
pub struct IndexRule;

impl<K, N> GrammarRule<K, N> for IndexRule
where
    K: IndexToken,
    N: Expr,
{
    fn matches_prefix(&self, _kind: &K) -> bool {
        false
    }

    fn prefix(
        &self,
        _token: Token<K>,
        _ctx: &mut dyn ParserContext<K, N>,
    ) -> Result<N, ParseError> {
        Err(ParseError::UnexpectedToken {
            span: _token.span,
            expected: "Index prefix not yet implemented".into(),
        })
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
            expected: "Index infix not yet implemented".into(),
        })
    }
}
