use handl_plugin_ast::Expr;
use handl_plugin_lexer::{Token, TokenKind};
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

    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)> {
        if kind.is_bracket_open() {
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
        rbp: u32,
    ) -> Result<N, ParseError> {
        let index = ctx.expression(rbp)?;
        let close = ctx.advance()?;
        if !close.kind.is_bracket_close() {
            return Err(ParseError::UnexpectedToken {
                span: close.span,
                expected: "]".into(),
            });
        }
        Ok(N::index(left, index))
    }
}
