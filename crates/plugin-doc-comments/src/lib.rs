pub mod diagnostics;
pub mod formatter;
pub mod parser;
pub mod syntax;

use handl_platform::{
    language::LanguagePipeline,
    manifest::{ExtendsDeclaration, PluginManifest, TargetCompatibility, TargetSupportLevel},
    Dependency,
};

pub struct DocCommentsPlugin;

impl DocCommentsPlugin {
    pub fn new() -> Self {
        DocCommentsPlugin
    }

    pub fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: "doc-comments".to_string(),
            version: "0.1.0".to_string(),
            provides: vec!["documentation.syntax".into()],
            requires: vec![
                Dependency::plugin("comments", None),
                Dependency::capability("parser.comment", None),
            ],
            optional: Vec::new(),
            extends: vec![ExtendsDeclaration {
                plugin: "comments".into(),
                version: None,
                extension_point: Some("syntax.comment".into()),
            }],
            conflicts: Vec::new(),
            replaces: Vec::new(),
            targets: vec![TargetCompatibility {
                target: "native".to_string(),
                level: TargetSupportLevel::Supported,
            }],
            exclusive: Vec::new(),
            metadata: serde_json::Value::default(),
        }
    }

    pub fn register_all(pipeline: &mut LanguagePipeline) {
        pipeline.register_syntax(syntax::DocComments);
        pipeline.register_parser(parser::DocComments);
        pipeline.register_formatter(formatter::DocComments);
        pipeline.register_diagnostics(diagnostics::DocComments);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::language::{DiagnosticLevel, LanguagePipeline, SyntaxNode};
    use handl_plugin_comments::CommentsPlugin;

    #[test]
    fn manifest_describes_plugin() {
        let manifest = DocCommentsPlugin::new().manifest();
        assert_eq!(manifest.id, "doc-comments");
        assert_eq!(manifest.version, "0.1.0");
        assert!(manifest.provides.contains(&"documentation.syntax".into()));
    }

    #[test]
    fn doc_comments_are_parsed_before_regular_comments() {
        let mut pipeline = LanguagePipeline::new();
        DocCommentsPlugin::register_all(&mut pipeline);
        CommentsPlugin::register_all(&mut pipeline);

        let (nodes, output, diags) = pipeline.run("// hello\n/// world\n");
        assert_eq!(nodes.len(), 2);
        assert!(matches!(nodes[0], SyntaxNode::LineComment { .. }));
        assert!(matches!(nodes[1], SyntaxNode::DocComment { .. }));
        assert_eq!(output, "// hello\n/// world\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn doc_comments_format_back_to_source() {
        let mut pipeline = LanguagePipeline::new();
        DocCommentsPlugin::register_all(&mut pipeline);
        CommentsPlugin::register_all(&mut pipeline);

        let (_, output, _) = pipeline.run("/// documentation\n");
        assert_eq!(output, "/// documentation\n");
    }

    #[test]
    fn without_doc_comments_triple_slash_is_line_comment() {
        let mut pipeline = LanguagePipeline::new();
        CommentsPlugin::register_all(&mut pipeline);

        let (nodes, output, _) = pipeline.run("/// hello\n");
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0], SyntaxNode::LineComment { .. }));
        assert_eq!(output, "/// hello\n");
    }

    #[test]
    fn empty_doc_comment_warns() {
        let mut pipeline = LanguagePipeline::new();
        DocCommentsPlugin::register_all(&mut pipeline);
        CommentsPlugin::register_all(&mut pipeline);

        let (_, _, diags) = pipeline.run("///\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].level, DiagnosticLevel::Warning);
        assert!(diags[0].message.contains("empty doc comment"));
    }
}
