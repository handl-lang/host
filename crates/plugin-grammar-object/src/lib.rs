use handl_ast::Expr;
use handl_lexer::{Token, TokenKind};
use handl_parser::{GrammarRule, ParseError, ParserContext};

/// Token capability required by the Object grammar rule.
pub trait ObjectToken: TokenKind {
    fn is_brace_open(&self) -> bool;
    fn is_brace_close(&self) -> bool;
    fn is_comma(&self) -> bool;
    fn is_colon(&self) -> bool;
}

/// Object grammar rule
#[derive(Debug, Default, Clone, Copy)]
pub struct ObjectRule;

impl<K, N> GrammarRule<K, N> for ObjectRule
where
    K: ObjectToken,
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
            expected: "Object prefix not yet implemented".into(),
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
            expected: "Object infix not yet implemented".into(),
        })
    }
}
