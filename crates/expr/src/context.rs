use crate::diagnostics::{Diagnostic, DiagnosticLevel};
use crate::pipeline::ExpressionPipeline;
use crate::token::{Span, Token};
use handl_ast_canonical::Expr;
use handl_lexer_canonical::CanonicalKind;
use handl_vm_values_json::NonFunctionValue;

/// Parser state — the remaining token stream, current position, and a
/// back-reference to the pipeline for recursive sub-expression parsing.
pub struct ParseContext<'a> {
    /// The token stream being parsed (including trivia such as comments).
    pub tokens: &'a [Token],
    /// The current index into `tokens`.
    pub pos: usize,
    /// The pipeline driving this parse, used to recurse.
    pipeline: &'a ExpressionPipeline,
    /// The original source string, used for source-level captures.
    source: &'a str,
    /// Diagnostics accumulated during recovery parsing.
    pub diagnostics: Vec<Diagnostic>,
    /// Whether to recover from parse errors and continue with partial trees.
    pub recover: bool,
}

impl<'a> ParseContext<'a> {
    /// Creates a context over `tokens` starting at position 0.
    pub fn new(tokens: &'a [Token], source: &'a str, pipeline: &'a ExpressionPipeline) -> Self {
        ParseContext {
            tokens,
            pos: 0,
            pipeline,
            source,
            diagnostics: Vec::new(),
            recover: false,
        }
    }

    /// Returns the original source string.
    pub fn source(&self) -> &'a str {
        self.source
    }

    /// Returns the source text covered by the half-open byte range `[start, end)`.
    pub fn source_span(&self, start: usize, end: usize) -> &'a str {
        let start = start.min(self.source.len());
        let end = end.min(self.source.len());
        if start > end {
            return "";
        }
        &self.source[start..end]
    }

    /// Returns the span of the current non-trivia token, or the last
    /// consumed token at end of input, or an empty span at the end of the
    /// source if the stream is empty.
    pub fn current_span(&self) -> Span {
        for token in &self.tokens[self.pos..] {
            if !matches!(token.kind, CanonicalKind::Comment(_)) {
                return token.span;
            }
        }
        if self.pos > 0 {
            return self.tokens[self.pos.saturating_sub(1)].span;
        }
        Span::at(self.source.len())
    }

    /// Advances `pos` to the next token of `kind`, without consuming it.
    /// Returns the found token, or `None` if the end of the stream is reached.
    pub fn seek_to(&mut self, kind: CanonicalKind) -> Option<Token> {
        for (idx, token) in self.tokens[self.pos..].iter().enumerate() {
            if token.kind == kind {
                self.pos += idx;
                return Some(token.clone());
            }
        }
        self.pos = self.tokens.len();
        None
    }

    /// Advances `pos` past any trivia tokens.
    pub fn skip_trivia(&mut self) {
        while self.pos < self.tokens.len() {
            if matches!(self.tokens[self.pos].kind, CanonicalKind::Comment(_)) {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Returns the next non-trivia token without consuming it.
    pub fn peek(&mut self) -> Option<&Token> {
        self.skip_trivia();
        self.tokens.get(self.pos)
    }

    /// Returns the next non-trivia token and advances the position.
    pub fn consume(&mut self) -> Option<&Token> {
        self.skip_trivia();
        let token = self.tokens.get(self.pos)?;
        self.pos += 1;
        Some(token)
    }

    /// Returns the token at `offset` ahead of the current position.
    pub fn at(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.pos + offset)
    }

    /// Returns the span of the current non-trivia token, or an empty span at the
    /// current position if there is none.
    pub fn span(&mut self) -> Span {
        self.peek()
            .map(|t| t.span)
            .unwrap_or_else(|| Span::at(self.pos))
    }

    /// Returns `true` when all tokens have been consumed.
    pub fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// Records a diagnostic at the current span.
    pub fn diagnostic(&mut self, level: DiagnosticLevel, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            level,
            message: message.into(),
            span: Some(self.current_span()),
        });
    }

    /// Records an error diagnostic at the current span.
    pub fn error(&mut self, message: impl Into<String>) {
        self.diagnostic(DiagnosticLevel::Error, message);
    }

    /// Advances past tokens until a synchronizing token or end of input.
    pub fn synchronize(&mut self) {
        while let Some(token) = self.peek().cloned() {
            if matches!(
                token.kind,
                CanonicalKind::CallClose
                    | CanonicalKind::BracketClose
                    | CanonicalKind::BlockClose
                    | CanonicalKind::Comma
                    | CanonicalKind::Semicolon
                    | CanonicalKind::Pipe
                    | CanonicalKind::BackwardPipe
                    | CanonicalKind::Eof
            ) {
                return;
            }
            self.consume();
        }
    }

    /// Consumes the next token and returns an error if it is not `kind`.
    pub fn expect(&mut self, kind: CanonicalKind) -> Result<(), handl_platform::PlatformError> {
        if let Some(token) = self.peek().cloned() {
            if token.kind == kind {
                self.pos += 1;
                return Ok(());
            }
            return Err(handl_platform::PlatformError::Plugin(format!(
                "expected {} but found {} at {}",
                kind, token.kind, token.span
            )));
        }
        Err(handl_platform::PlatformError::Plugin(format!(
            "expected {} but found end of input at {}",
            kind,
            self.current_span()
        )))
    }

    /// Consumes the next token if it matches `kind`.
    pub fn consume_if(&mut self, kind: CanonicalKind) -> bool {
        if let Some(token) = self.peek().cloned() {
            if token.kind == kind {
                self.pos += 1;
                return true;
            }
        }
        false
    }

    /// Recursively parses a primary expression.
    pub fn parse_primary(
        &mut self,
    ) -> Result<Expr<NonFunctionValue>, handl_platform::PlatformError> {
        let pipeline = self.pipeline;
        pipeline.parse_primary(self)
    }

    /// Recursively parses a full expression.
    pub fn parse_expr(&mut self) -> Result<Expr<NonFunctionValue>, handl_platform::PlatformError> {
        let pipeline = self.pipeline;
        pipeline.parse_expr(self)
    }

    /// Recursively parses an expression with the given minimum binding power.
    pub fn parse_expr_with_min_bp(
        &mut self,
        min_bp: u32,
    ) -> Result<Expr<NonFunctionValue>, handl_platform::PlatformError> {
        let pipeline = self.pipeline;
        pipeline.parse_expr_with_min_bp(self, min_bp)
    }
}
