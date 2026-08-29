use handl_plugin_ast::Expr;
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_parser::{GrammarRule, ParseError, ParserContext};

/// Token capability required by the Handle grammar rule.
pub trait HandleToken: TokenKind {
    fn is_handle(&self) -> bool;
}

/// Handle grammar rule
#[derive(Debug, Default, Clone, Copy)]
pub struct HandleRule;

impl<K, N> GrammarRule<K, N> for HandleRule
where
    K: HandleToken,
    N: Expr,
{
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_handle()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut dyn ParserContext<K, N>,
    ) -> Result<N, ParseError> {
        let expr = ctx.expression(0)?;
        Ok(N::handle(expr))
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
            expected: "Handle is prefix-only".into(),
        })
    }
}
