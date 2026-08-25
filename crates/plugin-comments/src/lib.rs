pub mod lexer;
pub mod parser;
pub mod syntax;
pub mod formatter;
pub mod diagnostics;

use handl_platform::{
    language::LanguagePipeline,
    manifest::{PluginManifest, TargetCompatibility, TargetSupportLevel},
};

pub struct CommentsPlugin;

impl CommentsPlugin {
    pub fn new() -> Self {
        CommentsPlugin
    }

    pub fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: "comments".to_string(),
            version: "0.1.0".to_string(),
            provides: vec![
                "syntax.comment".into(),
                "lexer.comment".into(),
                "parser.comment".into(),
                "formatter.comment".into(),
                "diagnostics.comment".into(),
            ],
            requires: Vec::new(),
            optional: Vec::new(),
            extends: Vec::new(),
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
        pipeline.register_lexer(lexer::Comments);
        pipeline.register_parser(parser::Comments);
        pipeline.register_syntax(syntax::Comments);
        pipeline.register_formatter(formatter::Comments);
        pipeline.register_diagnostics(diagnostics::Comments);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::language::{
        DiagnosticLevel, LanguagePipeline, LexerPlugin, ParserPlugin, Span, SyntaxNode, Token,
    };

    #[test]
    fn lexer_emits_comment_tokens() {
        let mut ctx = handl_platform::language::LexerContext::default();
        lexer::Comments.lex("// hello\n/* world */", &mut ctx);
        assert_eq!(ctx.tokens.len(), 2);
        assert_eq!(ctx.tokens[0].kind.as_str(), "line_comment");
        assert_eq!(ctx.tokens[0].text, " hello");
        assert_eq!(ctx.tokens[1].kind.as_str(), "block_comment");
        assert_eq!(ctx.tokens[1].text, " world ");
    }

    #[test]
    fn parser_recognizes_comments() {
        let tokens = vec![
            Token {
                kind: "line_comment".into(),
                span: Span { start: 0, end: 8 },
                text: " hello".to_string(),
            },
            Token {
                kind: "block_comment".into(),
                span: Span { start: 9, end: 20 },
                text: " world ".to_string(),
            },
        ];
        let mut ctx = handl_platform::language::ParserContext::new(tokens);
        assert!(matches!(
            parser::Comments.parse(&mut ctx),
            Ok(SyntaxNode::LineComment { .. })
        ));
        assert!(matches!(
            parser::Comments.parse(&mut ctx),
            Ok(SyntaxNode::BlockComment { .. })
        ));
    }

    #[test]
    fn pipeline_parses_formats_and_diagnoses_comments() {
        let mut pipeline = LanguagePipeline::new();
        CommentsPlugin::register_all(&mut pipeline);

        let (nodes, output, diags) = pipeline.run("// hello\n/* world */");
        assert_eq!(nodes.len(), 2);
        assert!(matches!(nodes[0], SyntaxNode::LineComment { .. }));
        assert!(matches!(nodes[1], SyntaxNode::BlockComment { .. }));
        assert_eq!(output, "// hello\n/* world */");
        assert!(diags.is_empty());
    }

    #[test]
    fn pipeline_warns_on_crlf_in_block_comment() {
        let mut pipeline = LanguagePipeline::new();
        CommentsPlugin::register_all(&mut pipeline);

        let (nodes, _output, diags) = pipeline.run("/* line1\r\nline2 */");
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0], SyntaxNode::BlockComment { .. }));
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].level, DiagnosticLevel::Warning);
        assert!(diags[0].message.contains("CRLF"));
    }

    #[test]
    fn comments_not_recognized_without_plugin() {
        let pipeline = LanguagePipeline::new();
        let (nodes, _output, _diags) = pipeline.run("// hello\n/* world */");
        assert!(nodes.is_empty());
    }
}
