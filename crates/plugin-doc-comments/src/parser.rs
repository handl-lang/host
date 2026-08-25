use handl_platform::language::{ParseError, ParseResult, ParserContext, ParserPlugin, SyntaxNode};

pub struct DocComments;

impl ParserPlugin for DocComments {
    fn parse(&self, ctx: &mut ParserContext) -> ParseResult<SyntaxNode> {
        match ctx.peek() {
            Some(token) if token.kind.as_str() == "line_comment" && token.text.starts_with('/') => {
                let token = ctx.consume().unwrap();
                Ok(SyntaxNode::DocComment {
                    text: token.text[1..].to_string(),
                    span: token.span,
                })
            }
            _ => Err(ParseError::NotApplicable),
        }
    }
}
