use handl_platform::language::{SyntaxPlugin, SyntaxRegistry};

pub struct Comments;

impl SyntaxPlugin for Comments {
    fn register(&self, syntax: &mut SyntaxRegistry) {
        syntax.register_node("line_comment");
        syntax.register_node("block_comment");
    }
}
