use handl_plugin_ast::{Expr, Stmt};
use handl_plugin_lexer::{Token, TokenKind};
use handl_parser::{GrammarRule, ParseError, ParserContext};

/// Token capability required for the Let grammar rule.
pub trait LetToken: TokenKind {
    fn is_let(&self) -> bool;
    fn as_let_name(&self) -> Option<&str>;
    fn is_let_equal(&self) -> bool;
    fn is_let_semicolon(&self) -> bool;
}

/// Let grammar rule
#[derive(Debug, Default, Clone, Copy)]
pub struct LetRule;

impl<K, N> GrammarRule<K, N> for LetRule
where
    K: LetToken,
    N: Expr + Stmt<Expr = N>,
{
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_let()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut dyn ParserContext<K, N>,
    ) -> Result<N, ParseError> {
        let name_tok = ctx.advance()?.clone();
        let name = name_tok
            .kind
            .as_let_name()
            .ok_or_else(|| ParseError::UnexpectedToken {
                span: name_tok.span,
                expected: "binding name".into(),
            })?;

        let eq = ctx.advance()?;
        if !eq.kind.is_let_equal() {
            return Err(ParseError::UnexpectedToken {
                span: eq.span,
                expected: "`=`".into(),
            });
        }

        let value = ctx.expression(0)?;

        let semi = ctx.advance()?;
        if !semi.kind.is_let_semicolon() {
            return Err(ParseError::UnexpectedToken {
                span: semi.span,
                expected: "`;`".into(),
            });
        }

        Ok(N::let_(name, value))
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
            expected: "Let is prefix-only".into(),
        })
    }
}
