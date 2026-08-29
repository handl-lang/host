use handl_plugin_ast::Expr;
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_parser::{GrammarRule, ParseError, ParserContext};

/// Token capability required by the Array grammar rule.
pub trait ArrayToken: TokenKind {
    fn is_bracket_open(&self) -> bool;
    fn is_bracket_close(&self) -> bool;
    fn is_comma(&self) -> bool;
}

/// Array grammar rule
#[derive(Debug, Default, Clone, Copy)]
pub struct ArrayRule;

impl<K, N> GrammarRule<K, N> for ArrayRule
where
    K: ArrayToken,
    N: Expr,
{
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_bracket_open()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut dyn ParserContext<K, N>,
    ) -> Result<N, ParseError> {
        let mut elements = Vec::new();

        if !ctx.peek_kind().as_ref().map_or(true, |k| k.is_bracket_close()) {
            loop {
                elements.push(ctx.expression(0)?);

                match ctx.peek_kind() {
                    Some(ref k) if k.is_bracket_close() => break,
                    Some(ref k) if k.is_comma() => {
                        ctx.advance()?;
                    }
                    Some(_) | None => break,
                }
            }
        }

        let close = ctx.advance()?;
        if !close.kind.is_bracket_close() {
            return Err(ParseError::UnexpectedToken {
                span: close.span,
                expected: "]".into(),
            });
        }

        Ok(N::array(elements))
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
            expected: "Array is prefix-only".into(),
        })
    }
}
