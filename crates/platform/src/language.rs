use crate::error::PlatformError;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Source {
    pub text: String,
    pub name: Option<String>,
    pub offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenKind(String);

impl TokenKind {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TokenKind {
    fn from(value: &str) -> Self {
        TokenKind(value.to_string())
    }
}

impl From<String> for TokenKind {
    fn from(value: String) -> Self {
        TokenKind(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyntaxNode {
    LineComment { text: String, span: Span },
    BlockComment { text: String, span: Span },
    DocComment { text: String, span: Span },
    Custom { kind: String, payload: Value },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
    pub span: Option<Span>,
}

/// Output of a fully executed `LanguagePipeline`.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineOutput {
    pub formatted: String,
    pub diagnostics: Vec<Diagnostic>,
    pub ast: Vec<SyntaxNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    NotApplicable,
    Diagnostic(Diagnostic),
    Platform(PlatformError),
}

pub type ParseResult<T> = Result<T, ParseError>;

#[derive(Debug, Default)]
pub struct LexerContext {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

impl LexerContext {
    pub fn emit(&mut self, token: Token) {
        self.tokens.push(token);
    }

    pub fn error(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }
}

#[derive(Debug)]
pub struct ParserContext {
    pub tokens: Vec<Token>,
    pub pos: usize,
    pub errors: Vec<ParseError>,
}

impl ParserContext {
    pub fn new(tokens: Vec<Token>) -> Self {
        ParserContext {
            tokens,
            pos: 0,
            errors: Vec::new(),
        }
    }

    pub fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    pub fn consume(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos)?.clone();
        self.pos += 1;
        Some(token)
    }

    pub fn error(&mut self, err: ParseError) {
        self.errors.push(err);
    }
}

#[derive(Debug, Default)]
pub struct FormatContext {
    pub output: String,
}

impl FormatContext {
    pub fn write(&mut self, text: &str) {
        self.output.push_str(text);
    }

    pub fn newline(&mut self) {
        self.output.push('\n');
    }
}

#[derive(Debug, Default)]
pub struct DiagnosticContext {
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticContext {
    pub fn warning(&mut self, message: impl Into<String>, span: Option<Span>) {
        self.diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Warning,
            message: message.into(),
            span,
        });
    }

    pub fn error(&mut self, message: impl Into<String>, span: Option<Span>) {
        self.diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Error,
            message: message.into(),
            span,
        });
    }

    pub fn info(&mut self, message: impl Into<String>, span: Option<Span>) {
        self.diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Info,
            message: message.into(),
            span,
        });
    }
}

pub trait LexerPlugin: Send + Sync {
    fn lex(&self, input: &str, ctx: &mut LexerContext);
}

pub trait ParserPlugin: Send + Sync {
    fn parse(&self, ctx: &mut ParserContext) -> ParseResult<SyntaxNode>;
}

pub trait SyntaxPlugin: Send + Sync {
    fn register(&self, syntax: &mut SyntaxRegistry);
}

pub trait FormatterPlugin: Send + Sync {
    fn format(&self, node: &SyntaxNode, ctx: &mut FormatContext);
}

pub trait DiagnosticsPlugin: Send + Sync {
    fn diagnose(&self, node: &SyntaxNode, ctx: &mut DiagnosticContext);
}

#[derive(Debug, Default)]
pub struct SyntaxRegistry {
    kinds: Vec<String>,
    seen: HashSet<String>,
}

impl SyntaxRegistry {
    pub fn register_node(&mut self, kind: impl Into<String>) {
        let kind = kind.into();
        if self.seen.insert(kind.clone()) {
            self.kinds.push(kind);
        }
    }

    pub fn known_kinds(&self) -> &[String] {
        &self.kinds
    }
}

#[derive(Default)]
pub struct LanguagePipeline {
    lexer_plugins: Vec<Box<dyn LexerPlugin>>,
    parser_plugins: Vec<Box<dyn ParserPlugin>>,
    syntax_registry: SyntaxRegistry,
    formatter_plugins: Vec<Box<dyn FormatterPlugin>>,
    diagnostics_plugins: Vec<Box<dyn DiagnosticsPlugin>>,
}

impl LanguagePipeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_lexer<P: LexerPlugin + 'static>(&mut self, plugin: P) {
        self.lexer_plugins.push(Box::new(plugin));
    }

    pub fn register_parser<P: ParserPlugin + 'static>(&mut self, plugin: P) {
        self.parser_plugins.push(Box::new(plugin));
    }

    pub fn register_syntax<P: SyntaxPlugin + 'static>(&mut self, plugin: P) {
        plugin.register(&mut self.syntax_registry);
    }

    pub fn register_formatter<P: FormatterPlugin + 'static>(&mut self, plugin: P) {
        self.formatter_plugins.push(Box::new(plugin));
    }

    pub fn register_diagnostics<P: DiagnosticsPlugin + 'static>(&mut self, plugin: P) {
        self.diagnostics_plugins.push(Box::new(plugin));
    }

    pub fn known_kinds(&self) -> &[String] {
        self.syntax_registry.known_kinds()
    }

    pub fn run(&self, source: &str) -> (Vec<SyntaxNode>, String, Vec<Diagnostic>) {
        let mut lex_ctx = LexerContext::default();
        for plugin in &self.lexer_plugins {
            plugin.lex(source, &mut lex_ctx);
        }

        let mut parse_ctx = ParserContext::new(lex_ctx.tokens);
        let mut nodes = Vec::new();

        while parse_ctx.peek().is_some() {
            let mut handled = false;
            for plugin in &self.parser_plugins {
                match plugin.parse(&mut parse_ctx) {
                    Ok(node) => {
                        nodes.push(node);
                        handled = true;
                        break;
                    }
                    Err(ParseError::NotApplicable) => continue,
                    Err(err) => {
                        parse_ctx.error(err);
                        break;
                    }
                }
            }
            if !handled && parse_ctx.peek().is_some() {
                parse_ctx.consume();
            }
        }

        let mut fmt_ctx = FormatContext::default();
        for node in &nodes {
            for plugin in &self.formatter_plugins {
                plugin.format(node, &mut fmt_ctx);
            }
        }

        let mut diag_ctx = DiagnosticContext::default();
        for node in &nodes {
            for plugin in &self.diagnostics_plugins {
                plugin.diagnose(node, &mut diag_ctx);
            }
        }

        let mut diagnostics = lex_ctx.diagnostics;
        diagnostics.extend(diag_ctx.diagnostics);
        diagnostics.extend(parse_ctx.errors.into_iter().map(|e| match e {
            ParseError::Diagnostic(d) => d,
            ParseError::NotApplicable => Diagnostic {
                level: DiagnosticLevel::Error,
                message: "not applicable".to_string(),
                span: None,
            },
            ParseError::Platform(p) => Diagnostic {
                level: DiagnosticLevel::Error,
                message: p.to_string(),
                span: None,
            },
        }));

        (nodes, fmt_ctx.output, diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct WordLexer;

    impl LexerPlugin for WordLexer {
        fn lex(&self, input: &str, ctx: &mut LexerContext) {
            let mut offset = 0;
            for word in input.split_whitespace() {
                let start = input[offset..].find(word).map(|i| offset + i).unwrap_or(offset);
                let end = start + word.len();
                offset = end;
                ctx.emit(Token {
                    kind: "WORD".into(),
                    span: Span { start, end },
                    text: word.to_string(),
                });
            }
        }
    }

    struct NumberParser;

    impl ParserPlugin for NumberParser {
        fn parse(&self, ctx: &mut ParserContext) -> ParseResult<SyntaxNode> {
            if let Some(token) = ctx.peek() {
                if token.text == "42" {
                    ctx.consume();
                    Ok(SyntaxNode::Custom {
                        kind: "Number".to_string(),
                        payload: Value::Number(serde_json::Number::from(42)),
                    })
                } else {
                    Err(ParseError::NotApplicable)
                }
            } else {
                Err(ParseError::NotApplicable)
            }
        }
    }

    struct NumberFormatter;

    impl FormatterPlugin for NumberFormatter {
        fn format(&self, node: &SyntaxNode, ctx: &mut FormatContext) {
            if let SyntaxNode::Custom { kind, payload } = node {
                if kind == "Number" {
                    if let Some(n) = payload.as_u64() {
                        ctx.write(&n.to_string());
                        ctx.newline();
                    }
                }
            }
        }
    }

    struct NumberDiagnostics;

    impl DiagnosticsPlugin for NumberDiagnostics {
        fn diagnose(&self, node: &SyntaxNode, ctx: &mut DiagnosticContext) {
            if let SyntaxNode::Custom { kind, .. } = node {
                if kind == "Number" {
                    ctx.info("found number", None);
                }
            }
        }
    }

    struct MySyntax;

    impl SyntaxPlugin for MySyntax {
        fn register(&self, syntax: &mut SyntaxRegistry) {
            syntax.register_node("Number");
            syntax.register_node("Word");
        }
    }

    #[test]
    fn lexer_emits_tokens() {
        let lexer = WordLexer;
        let mut ctx = LexerContext::default();
        lexer.lex("hello world", &mut ctx);
        assert_eq!(ctx.tokens.len(), 2);
        assert_eq!(ctx.tokens[0].text, "hello");
        assert_eq!(ctx.tokens[1].text, "world");
    }

    #[test]
    fn parser_recognizes_token_or_not_applicable() {
        let tokens = vec![
            Token {
                kind: "NUM".into(),
                span: Span { start: 0, end: 2 },
                text: "42".to_string(),
            },
            Token {
                kind: "NUM".into(),
                span: Span { start: 3, end: 5 },
                text: "99".to_string(),
            },
        ];
        let mut ctx = ParserContext::new(tokens);
        let parser = NumberParser;

        assert!(parser.parse(&mut ctx).is_ok());
        assert!(matches!(parser.parse(&mut ctx), Err(ParseError::NotApplicable)));
    }

    #[test]
    fn pipeline_composes_plugins() {
        let mut pipeline = LanguagePipeline::new();
        pipeline.register_lexer(WordLexer);
        pipeline.register_parser(NumberParser);
        pipeline.register_formatter(NumberFormatter);
        pipeline.register_diagnostics(NumberDiagnostics);

        let (nodes, output, diags) = pipeline.run("hello 42 world");

        assert_eq!(nodes.len(), 1);
        assert_eq!(output, "42\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].level, DiagnosticLevel::Info);
    }

    #[test]
    fn syntax_registry_collects_kinds() {
        let mut registry = SyntaxRegistry::default();
        MySyntax.register(&mut registry);

        let kinds = registry.known_kinds();
        assert_eq!(kinds.len(), 2);
        assert!(kinds.contains(&"Number".to_string()));
        assert!(kinds.contains(&"Word".to_string()));
    }
}
