use handl_platform::language::{DiagnosticContext, DiagnosticsPlugin, SyntaxNode};

pub struct DocComments;

impl DiagnosticsPlugin for DocComments {
    fn diagnose(&self, node: &SyntaxNode, ctx: &mut DiagnosticContext) {
        if let SyntaxNode::DocComment { text, span } = node {
            if text.trim().is_empty() {
                ctx.warning("empty doc comment", Some(*span));
            }
        }
    }
}
