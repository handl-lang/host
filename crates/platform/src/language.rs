//! Language pipeline plugins and syntax types.

use crate::error::PlatformError;
use serde_json::Value;
use std::collections::HashSet;

/// A source input with optional name and offset.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Source {
    /// The source text.
    pub text: String,
    /// Optional source name, e.g. a file path.
    pub name: Option<String>,
    /// Byte offset where this source fragment begins.
    pub offset: usize,
}

/// A span of byte offsets in source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    /// Inclusive start offset.
    pub start: usize,
    /// Exclusive end offset.
    pub end: usize,
}

/// A lexical token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    /// The token's kind.
    pub kind: TokenKind,
    /// The byte span of the token in source.
    pub span: Span,
    /// The exact text of the token.
    pub text: String,
}

/// A named token kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenKind(String);

impl TokenKind {
    /// Returns the token kind as a string slice.
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

/// A node in the concrete syntax tree.
#[derive(Debug, Clone, PartialEq)]
pub enum SyntaxNode {
    /// A line comment.
    LineComment {
        /// The comment text.
        text: String,
        /// The source span.
        span: Span,
    },
    /// A block comment.
    BlockComment {
        /// The comment text.
        text: String,
        /// The source span.
        span: Span,
    },
    /// A documentation comment.
    DocComment {
        /// The comment text.
        text: String,
        /// The source span.
        span: Span,
    },
    /// A custom node produced by a parser plugin.
    Custom {
        /// The syntax kind.
        kind: String,
        /// The node payload.
        payload: Value,
    },
}

/// Severity level of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    /// An error.
    Error,
    /// A warning.
    Warning,
    /// An informational message.
    Info,
}

/// A diagnostic message with optional source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The diagnostic severity.
    pub level: DiagnosticLevel,
    /// The human-readable message.
    pub message: String,
    /// The optional source span.
    pub span: Option<Span>,
}

/// Output of a fully executed `LanguagePipeline`.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineOutput {
    /// Formatted source produced by formatter plugins.
    pub formatted: String,
    /// Diagnostics emitted during lexing, parsing, and diagnosis.
    pub diagnostics: Vec<Diagnostic>,
    /// Concrete syntax tree produced by parser plugins.
    pub ast: Vec<SyntaxNode>,
}

/// A parsing result or failure.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// The parser does not apply to the current token.
    NotApplicable,
    /// A parser-produced diagnostic.
    Diagnostic(Diagnostic),
    /// An underlying platform error.
    Platform(PlatformError),
}

/// Result type for parser plugins.
pub type ParseResult<T> = Result<T, ParseError>;

/// Context accumulated during lexing.
#[derive(Debug, Default)]
pub struct LexerContext {
    /// Tokens emitted by lexer plugins.
    pub tokens: Vec<Token>,
    /// Diagnostics emitted during lexing.
    pub diagnostics: Vec<Diagnostic>,
}

impl LexerContext {
    /// Emits a token into the context.
    pub fn emit(&mut self, token: Token) {
        self.tokens.push(token);
    }

    /// Records a diagnostic.
    pub fn error(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }
}

/// Context maintained while parsing tokens.
#[derive(Debug)]
pub struct ParserContext {
    /// The token stream.
    pub tokens: Vec<Token>,
    /// Current position in the token stream.
    pub pos: usize,
    /// Errors encountered while parsing.
    pub errors: Vec<ParseError>,
}

impl ParserContext {
    /// Creates a new parser context from a token stream.
    pub fn new(tokens: Vec<Token>) -> Self {
        ParserContext {
            tokens,
            pos: 0,
            errors: Vec::new(),
        }
    }

    /// Returns the current token without consuming it.
    pub fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// Consumes and returns the current token.
    pub fn consume(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos)?.clone();
        self.pos += 1;
        Some(token)
    }

    /// Records a parse error.
    pub fn error(&mut self, err: ParseError) {
        self.errors.push(err);
    }
}

/// Context accumulated during formatting.
#[derive(Debug, Default)]
pub struct FormatContext {
    /// The formatted output buffer.
    pub output: String,
}

impl FormatContext {
    /// Appends text to the formatted output.
    pub fn write(&mut self, text: &str) {
        self.output.push_str(text);
    }

    /// Appends a newline to the formatted output.
    pub fn newline(&mut self) {
        self.output.push('\n');
    }
}

/// Context accumulated during diagnostics generation.
#[derive(Debug, Default)]
pub struct DiagnosticContext {
    /// Diagnostics emitted by diagnostics plugins.
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticContext {
    /// Records a warning diagnostic.
    pub fn warning(&mut self, message: impl Into<String>, span: Option<Span>) {
        self.diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Warning,
            message: message.into(),
            span,
        });
    }

    /// Records an error diagnostic.
    pub fn error(&mut self, message: impl Into<String>, span: Option<Span>) {
        self.diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Error,
            message: message.into(),
            span,
        });
    }

    /// Records an info diagnostic.
    pub fn info(&mut self, message: impl Into<String>, span: Option<Span>) {
        self.diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Info,
            message: message.into(),
            span,
        });
    }
}

/// A plugin that tokenizes source text.
pub trait LexerPlugin: Send + Sync {
    /// Tokenizes the input, appending tokens and diagnostics to the context.
    fn lex(&self, input: &str, ctx: &mut LexerContext);
}

/// A plugin that produces syntax nodes from tokens.
pub trait ParserPlugin: Send + Sync {
    /// Parses the current token(s), returning a syntax node when applicable.
    fn parse(&self, ctx: &mut ParserContext) -> ParseResult<SyntaxNode>;
}

/// A plugin that registers custom syntax kinds.
pub trait SyntaxPlugin: Send + Sync {
    /// Registers syntax kinds with the registry.
    fn register(&self, syntax: &mut SyntaxRegistry);
}

/// A plugin that formats a syntax node.
pub trait FormatterPlugin: Send + Sync {
    /// Formats the node, appending output to the context.
    fn format(&self, node: &SyntaxNode, ctx: &mut FormatContext);
}

/// A plugin that diagnoses a syntax node.
pub trait DiagnosticsPlugin: Send + Sync {
    /// Diagnoses the node, appending diagnostics to the context.
    fn diagnose(&self, node: &SyntaxNode, ctx: &mut DiagnosticContext);
}

/// A registry of known syntax node kinds.
#[derive(Debug, Default)]
pub struct SyntaxRegistry {
    kinds: Vec<String>,
    seen: HashSet<String>,
}

impl SyntaxRegistry {
    /// Registers a new syntax kind if not already known.
    pub fn register_node(&mut self, kind: impl Into<String>) {
        let kind = kind.into();
        if self.seen.insert(kind.clone()) {
            self.kinds.push(kind);
        }
    }

    /// Returns all known syntax kinds.
    pub fn known_kinds(&self) -> &[String] {
        &self.kinds
    }
}

/// A composable language pipeline with lexer, parser, formatter, and diagnostics stages.
#[derive(Default)]
pub struct LanguagePipeline {
    lexer_plugins: Vec<Box<dyn LexerPlugin>>,
    parser_plugins: Vec<Box<dyn ParserPlugin>>,
    syntax_registry: SyntaxRegistry,
    formatter_plugins: Vec<Box<dyn FormatterPlugin>>,
    diagnostics_plugins: Vec<Box<dyn DiagnosticsPlugin>>,
}

impl LanguagePipeline {
    /// Creates an empty pipeline.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a lexer plugin.
    pub fn register_lexer<P: LexerPlugin + 'static>(&mut self, plugin: P) {
        self.lexer_plugins.push(Box::new(plugin));
    }

    /// Registers a parser plugin.
    pub fn register_parser<P: ParserPlugin + 'static>(&mut self, plugin: P) {
        self.parser_plugins.push(Box::new(plugin));
    }

    /// Registers a syntax plugin.
    pub fn register_syntax<P: SyntaxPlugin + 'static>(&mut self, plugin: P) {
        plugin.register(&mut self.syntax_registry);
    }

    /// Registers a formatter plugin.
    pub fn register_formatter<P: FormatterPlugin + 'static>(&mut self, plugin: P) {
        self.formatter_plugins.push(Box::new(plugin));
    }

    /// Registers a diagnostics plugin.
    pub fn register_diagnostics<P: DiagnosticsPlugin + 'static>(&mut self, plugin: P) {
        self.diagnostics_plugins.push(Box::new(plugin));
    }

    /// Returns the syntax kinds registered by syntax plugins.
    pub fn known_kinds(&self) -> &[String] {
        self.syntax_registry.known_kinds()
    }

    /// Runs the full pipeline on a source string.
    pub fn run(&self, source: &str) -> (Vec<SyntaxNode>, String, Vec<Diagnostic>) {
        let mut lex_ctx = LexerContext::default();
        for plugin in &self.lexer_plugins {
            plugin.lex(source, &mut lex_ctx);
        }

        let mut parse_ctx = ParserContext::new(lex_ctx.tokens);
        let mut nodes = Vec::new();

        while parse_ctx.peek().is_some() {
            let mut handled = false;
            let pos_before = parse_ctx.pos;
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
                        if parse_ctx.pos == pos_before {
                            parse_ctx.consume();
                        }
                        handled = true;
                        break;
                    }
                }
            }
            if !handled
                && parse_ctx.peek().is_some()
                && let Some(token) = parse_ctx.consume()
            {
                parse_ctx.error(ParseError::Diagnostic(Diagnostic {
                    level: DiagnosticLevel::Error,
                    message: format!("unhandled token: {}", token.text),
                    span: Some(token.span),
                }));
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
                let start = input[offset..]
                    .find(word)
                    .map(|i| offset + i)
                    .unwrap_or(offset);
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
        assert!(matches!(
            parser.parse(&mut ctx),
            Err(ParseError::NotApplicable)
        ));
    }

    #[test]
    fn pipeline_composes_plugins() {
        let mut pipeline = LanguagePipeline::new();
        pipeline.register_lexer(WordLexer);
        pipeline.register_parser(NumberParser);
        pipeline.register_formatter(NumberFormatter);
        pipeline.register_diagnostics(NumberDiagnostics);

        let (nodes, output, diags) = pipeline.run("42");

        assert_eq!(nodes.len(), 1);
        assert_eq!(output, "42\n");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].level, DiagnosticLevel::Info);
    }

    #[test]
    fn pipeline_reports_unhandled_tokens() {
        let mut pipeline = LanguagePipeline::new();
        pipeline.register_lexer(WordLexer);
        pipeline.register_parser(NumberParser);

        let (nodes, _output, diags) = pipeline.run("hello");

        assert!(nodes.is_empty());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].level, DiagnosticLevel::Error);
        assert!(diags[0].message.contains("unhandled token"));
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

    #[test]
    fn source_and_span_defaults_and_token_kind_from_string() {
        let s = Source {
            text: "hi".into(),
            name: Some("f".into()),
            offset: 0,
        };
        assert_eq!(s.text, "hi");
        assert_eq!(s.offset, 0);
        assert_eq!(Span { start: 0, end: 2 }, Span { start: 0, end: 2 });
        let kind: TokenKind = "KIND".to_string().into();
        assert_eq!(kind.as_str(), "KIND");
    }

    #[test]
    fn lexer_context_emits_and_errors() {
        let mut ctx = LexerContext::default();
        ctx.emit(Token {
            kind: "A".into(),
            span: Span { start: 0, end: 1 },
            text: "a".into(),
        });
        ctx.error(Diagnostic {
            level: DiagnosticLevel::Error,
            message: "bad".into(),
            span: None,
        });
        assert_eq!(ctx.tokens.len(), 1);
        assert_eq!(ctx.diagnostics.len(), 1);
    }

    #[test]
    fn parser_context_peeks_consumes_and_errors() {
        let tokens = vec![Token {
            kind: "A".into(),
            span: Span { start: 0, end: 1 },
            text: "a".into(),
        }];
        let mut ctx = ParserContext::new(tokens);
        assert!(ctx.peek().is_some());
        assert!(ctx.consume().is_some());
        assert!(ctx.peek().is_none());
        ctx.error(ParseError::NotApplicable);
        assert_eq!(ctx.errors.len(), 1);
    }

    #[test]
    fn format_and_diagnostic_contexts() {
        let mut f = FormatContext::default();
        f.write("hi");
        f.newline();
        assert_eq!(f.output, "hi\n");

        let mut d = DiagnosticContext::default();
        d.warning("w", None);
        d.error("e", Some(Span { start: 0, end: 1 }));
        d.info("i", None);
        assert_eq!(d.diagnostics.len(), 3);
    }

    #[test]
    fn language_pipeline_registers_all() {
        let mut pipeline = LanguagePipeline::new();
        pipeline.register_lexer(WordLexer);
        pipeline.register_parser(NumberParser);
        pipeline.register_syntax(MySyntax);
        pipeline.register_formatter(NumberFormatter);
        pipeline.register_diagnostics(NumberDiagnostics);

        let kinds = pipeline.known_kinds();
        assert!(kinds.contains(&"Number".to_string()));
    }

    struct PlatformErrorParser;

    impl ParserPlugin for PlatformErrorParser {
        fn parse(&self, _ctx: &mut ParserContext) -> ParseResult<SyntaxNode> {
            Err(ParseError::Platform(
                crate::error::PlatformError::NotImplemented,
            ))
        }
    }

    struct DiagnosticErrorParser;

    impl ParserPlugin for DiagnosticErrorParser {
        fn parse(&self, _ctx: &mut ParserContext) -> ParseResult<SyntaxNode> {
            Err(ParseError::Diagnostic(Diagnostic {
                level: DiagnosticLevel::Error,
                message: "bad".into(),
                span: None,
            }))
        }
    }

    #[test]
    fn pipeline_handles_platform_and_diagnostic_errors() {
        let mut p1 = LanguagePipeline::new();
        p1.register_lexer(WordLexer);
        p1.register_parser(PlatformErrorParser);
        let (_, _, diags) = p1.run("x");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "not implemented");

        let mut p2 = LanguagePipeline::new();
        p2.register_lexer(WordLexer);
        p2.register_parser(DiagnosticErrorParser);
        let (_, _, diags) = p2.run("x");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, "bad");
    }
}
