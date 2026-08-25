use handl_platform::language::{DiagnosticContext, DiagnosticsPlugin, SyntaxNode};

pub struct Comments;

impl DiagnosticsPlugin for Comments {
    fn diagnose(&self, node: &SyntaxNode, ctx: &mut DiagnosticContext) {
        if let SyntaxNode::BlockComment { text, span } = node {
            if text.contains("\r\n") {
                ctx.warning("block comment contains CRLF line endings", Some(*span));
            }
        }
    }
}
