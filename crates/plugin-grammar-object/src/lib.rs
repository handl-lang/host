use handl_plugin_ast::Expr;
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_parser::{GrammarRule, ParseError, ParserContext};

/// Token capability required by the Object grammar rule.
pub trait ObjectToken: TokenKind {
    fn is_brace_open(&self) -> bool;
    fn is_brace_close(&self) -> bool;
    fn is_comma(&self) -> bool;
    fn is_colon(&self) -> bool;
    fn as_object_key(&self) -> Option<String>;
}

/// Object grammar rule
#[derive(Debug, Default, Clone, Copy)]
pub struct ObjectRule;

impl<K, N> GrammarRule<K, N> for ObjectRule
where
    K: ObjectToken,
    N: Expr,
{
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_brace_open()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut dyn ParserContext<K, N>,
    ) -> Result<N, ParseError> {
        let mut entries = Vec::new();

        if !ctx.peek_kind().as_ref().map_or(true, |k| k.is_brace_close()) {
            loop {
                let key_tok = ctx.advance()?.clone();
                let key = key_tok
                    .kind
                    .as_object_key()
                    .ok_or_else(|| ParseError::UnexpectedToken {
                        span: key_tok.span,
                        expected: "object key".into(),
                    })?;

                let colon = ctx.advance()?;
                if !colon.kind.is_colon() {
                    return Err(ParseError::UnexpectedToken {
                        span: colon.span,
                        expected: "`:`".into(),
                    });
                }

                let value = ctx.expression(0)?;
                entries.push((key, value));

                match ctx.peek_kind() {
                    Some(ref k) if k.is_brace_close() => break,
                    Some(ref k) if k.is_comma() => {
                        ctx.advance()?;
                    }
                    Some(_) | None => break,
                }
            }
        }

        let close = ctx.advance()?;
        if !close.kind.is_brace_close() {
            return Err(ParseError::UnexpectedToken {
                span: close.span,
                expected: "`}`".into(),
            });
        }

        Ok(N::object(entries))
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
            expected: "Object is prefix-only".into(),
        })
    }
}
