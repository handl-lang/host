use handl_platform::language::{ParseError, ParseResult, ParserContext, ParserPlugin, SyntaxNode};

pub struct Comments;

impl ParserPlugin for Comments {
    fn parse(&self, ctx: &mut ParserContext) -> ParseResult<SyntaxNode> {
        match ctx.peek() {
            Some(token) if token.kind.as_str() == "line_comment" => {
                let token = ctx.consume().unwrap();
                Ok(SyntaxNode::LineComment {
                    text: token.text,
                    span: token.span,
                })
            }
            Some(token) if token.kind.as_str() == "block_comment" => {
                let token = ctx.consume().unwrap();
                Ok(SyntaxNode::BlockComment {
                    text: token.text,
                    span: token.span,
                })
            }
            _ => Err(ParseError::NotApplicable),
        }
    }
}
