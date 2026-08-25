use handl_platform::language::{SyntaxPlugin, SyntaxRegistry};

pub struct DocComments;

impl SyntaxPlugin for DocComments {
    fn register(&self, syntax: &mut SyntaxRegistry) {
        syntax.register_node("doc_comment");
    }
}
