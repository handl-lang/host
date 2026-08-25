use handl_platform::language::{FormatContext, FormatterPlugin, SyntaxNode};

pub struct DocComments;

impl FormatterPlugin for DocComments {
    fn format(&self, node: &SyntaxNode, ctx: &mut FormatContext) {
        if let SyntaxNode::DocComment { text, .. } = node {
            ctx.write("///");
            ctx.write(text);
            ctx.newline();
        }
    }
}
