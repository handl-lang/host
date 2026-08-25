use handl_platform::language::{FormatContext, FormatterPlugin, SyntaxNode};

pub struct Comments;

impl FormatterPlugin for Comments {
    fn format(&self, node: &SyntaxNode, ctx: &mut FormatContext) {
        match node {
            SyntaxNode::LineComment { text, .. } => {
                ctx.write("//");
                ctx.write(text);
                ctx.newline();
            }
            SyntaxNode::BlockComment { text, .. } => {
                ctx.write("/*");
                ctx.write(text);
                ctx.write("*/");
            }
            _ => {}
        }
    }
}
