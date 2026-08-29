use handl_plugin_ast::Expr;
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_parser::{GrammarRule, ParseError, ParserContext};

/// Token capability required by the Apply grammar rule.
pub trait ApplyToken: TokenKind {
    fn is_paren_open(&self) -> bool;
    fn is_paren_close(&self) -> bool;
    fn is_comma(&self) -> bool;
}

/// Apply grammar rule
#[derive(Debug, Default, Clone, Copy)]
pub struct ApplyRule;

impl<K, N> GrammarRule<K, N> for ApplyRule
where
    K: ApplyToken,
    N: Expr,
{
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_paren_open()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut dyn ParserContext<K, N>,
    ) -> Result<N, ParseError> {
        // Parenthesized grouping or unit: `( expr )`.
        let inner = ctx.expression(0)?;
        let close = ctx.advance()?;
        if !close.kind.is_paren_close() {
            return Err(ParseError::UnexpectedToken {
                span: close.span,
                expected: "`)`".into(),
            });
        }
        Ok(inner)
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
            expected: "Apply is a prefix rule".into(),
        })
    }
}
