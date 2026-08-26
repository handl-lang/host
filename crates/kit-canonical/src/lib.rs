//! Canonical kit as a generic plugin host.
//!
//! `Kit` loads a [`KitManifest`], registers the core canonical plugins, and
//! composes a [`Lexer`], a parser, and a node factory from them. It no longer
//! depends on the concrete `handl-ast-canonical`, `handl-lexer-canonical`, or
//! `handl-parser-canonical` crates.

use std::fmt;
use std::sync::Arc;

use handl_ast::{Expr, Node};
use handl_lexer::{DefaultLexError, Lexer, Span, Token, TokenKind};
use handl_parser::{CompositeGrammar, Grammar, GrammarRule, ParseError, ParserContext};
use handl_platform::PlatformError;
use handl_plugin_ast_call::Call;
use handl_plugin_grammar_call::{CallRule, CallToken};
use handl_plugin_token_number::NumberKind;

pub mod kit_manifest;
pub use kit_manifest::{canonical_manifest, KitManifest};

/// A generic metadata value for kit nodes.
pub type KitMetadata = ();

static UNIT: KitMetadata = ();

/// Composite token kind produced by the kit's lexer.
///
/// Built from `plugin-token-number`'s `NumberKind` plus the call delimiters
/// required by `plugin-grammar-call`'s `CallRule`.
#[derive(Debug, Clone, PartialEq)]
pub enum KitToken {
    /// A numeric literal (provided by `plugin-token-number`).
    Number(NumberKind),
    /// `(`
    CallOpen,
    /// `)`
    CallClose,
    /// `,`
    Comma,
    /// End-of-input sentinel.
    Eof,
}

impl fmt::Display for KitToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KitToken::Number(kind) => kind.fmt(f),
            KitToken::CallOpen => write!(f, "("),
            KitToken::CallClose => write!(f, ")"),
            KitToken::Comma => write!(f, ","),
            KitToken::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for KitToken {
    fn is_eof(&self) -> bool {
        matches!(self, KitToken::Eof)
    }

    fn is_error(&self) -> bool {
        false
    }
}

impl CallToken for KitToken {
    fn is_call_open(&self) -> bool {
        matches!(self, KitToken::CallOpen)
    }

    fn is_call_close(&self) -> bool {
        matches!(self, KitToken::CallClose)
    }

    fn is_comma(&self) -> bool {
        matches!(self, KitToken::Comma)
    }
}

/// Generic AST node produced by the canonical kit.
///
/// Composes the `Call` node shape from `plugin-ast-call` with a built-in
/// numeric literal node. More shapes will be added as additional canonical
/// plugins are created.
#[derive(Debug, Clone, PartialEq)]
pub enum KitNode {
    /// A numeric literal.
    Number(f64),
    /// A function call or application (uses `plugin-ast-call::Call`).
    Call(Call<KitNode, KitMetadata>),
}

impl Node for KitNode {
    type Value = f64;
    type Metadata = KitMetadata;

    fn metadata(&self) -> &Self::Metadata {
        &UNIT
    }

    fn with_metadata(self, _metadata: Self::Metadata) -> Self
    where
        Self: Sized,
    {
        self
    }
}

impl Expr for KitNode {
    fn value(value: Self::Value) -> Self
    where
        Self: Sized,
    {
        KitNode::Number(value)
    }

    fn call<S>(name: S, args: Vec<Self>) -> Self
    where
        Self: Sized,
        S: Into<String>,
    {
        KitNode::Call(Call::new(name, args))
    }

    fn apply(target: Self, args: Vec<Self>) -> Self
    where
        Self: Sized,
    {
        let mut all = vec![target];
        all.extend(args);
        KitNode::Call(Call::new("__apply", all))
    }

    fn array(_elements: Vec<Self>) -> Self
    where
        Self: Sized,
    {
        // No array shape plugin yet; stub as a number.
        KitNode::Number(0.0)
    }

    fn object(_entries: Vec<(String, Self)>) -> Self
    where
        Self: Sized,
    {
        // No object shape plugin yet; stub as a number.
        KitNode::Number(0.0)
    }

    fn index(_target: Self, _index: Self) -> Self
    where
        Self: Sized,
    {
        // No index shape plugin yet; stub as a number.
        KitNode::Number(0.0)
    }
}

/// Built-in grammar rule for parsing numeric literals.
#[derive(Debug, Default, Clone, Copy)]
pub struct NumberRule;

impl GrammarRule<KitToken, KitNode> for NumberRule {
    fn matches_prefix(&self, kind: &KitToken) -> bool {
        matches!(kind, KitToken::Number(NumberKind::Number(_)))
    }

    fn prefix(
        &self,
        token: Token<KitToken>,
        _ctx: &mut dyn ParserContext<KitToken, KitNode>,
    ) -> Result<KitNode, ParseError> {
        match token.kind {
            KitToken::Number(NumberKind::Number(v)) => Ok(KitNode::Number(v)),
            _ => Err(ParseError::UnexpectedToken {
                span: token.span,
                expected: "number literal".into(),
            }),
        }
    }

    fn infix_binding(&self, _kind: &KitToken) -> Option<(u32, u32)> {
        None
    }

    fn infix(
        &self,
        _left: KitNode,
        _token: Token<KitToken>,
        _ctx: &mut dyn ParserContext<KitToken, KitNode>,
        _rbp: u32,
    ) -> Result<KitNode, ParseError> {
        panic!("NumberRule is prefix-only")
    }
}

/// The kit's concrete lexer.
#[derive(Debug, Default, Clone, Copy)]
pub struct CanonicalLexer;

impl Lexer<KitToken, DefaultLexError> for CanonicalLexer {
    fn lex(&self, input: &str) -> Result<Vec<Token<KitToken>>, DefaultLexError> {
        let mut tokens = Vec::new();
        let mut chars = input.char_indices().peekable();

        while let Some(&(start, ch)) = chars.peek() {
            if ch.is_whitespace() {
                chars.next();
                continue;
            }

            if ch == '(' {
                chars.next();
                tokens.push(Token::new(KitToken::CallOpen, Span::new(start, start + 1)));
                continue;
            }

            if ch == ')' {
                chars.next();
                tokens.push(Token::new(KitToken::CallClose, Span::new(start, start + 1)));
                continue;
            }

            if ch == ',' {
                chars.next();
                tokens.push(Token::new(KitToken::Comma, Span::new(start, start + 1)));
                continue;
            }

            if ch.is_ascii_digit() || ch == '-' {
                chars.next();
                let mut end = start + ch.len_utf8();

                if ch == '-' && !chars.peek().map_or(false, |(_, c)| c.is_ascii_digit()) {
                    return Err(DefaultLexError::UnexpectedChar {
                        span: Span::new(start, end),
                        char: '-',
                    });
                }

                let mut has_dot = false;
                while let Some(&(pos, c)) = chars.peek() {
                    if c.is_ascii_digit() {
                        end = pos + c.len_utf8();
                        chars.next();
                    } else if c == '.' && !has_dot {
                        has_dot = true;
                        end = pos + 1;
                        chars.next();
                    } else {
                        break;
                    }
                }

                let text = &input[start..end];
                let value = text.parse::<f64>().map_err(|_| DefaultLexError::InvalidNumber {
                    span: Span::new(start, end),
                })?;

                tokens.push(Token::new(
                    KitToken::Number(NumberKind::Number(value)),
                    Span::new(start, end),
                ));
                continue;
            }

            return Err(DefaultLexError::UnexpectedChar {
                span: Span::new(start, start + ch.len_utf8()),
                char: ch,
            });
        }

        tokens.push(Token::new(KitToken::Eof, Span::at(input.len())));
        Ok(tokens)
    }
}

struct KitParserContext {
    tokens: Vec<Token<KitToken>>,
    source: String,
    pos: usize,
    grammar: Arc<dyn Grammar<KitToken, KitNode>>,
}

impl KitParserContext {
    fn peek_span(&self) -> Span {
        self.peek()
            .map(|t| t.span)
            .unwrap_or_else(|| Span::at(self.tokens.last().map(|t| t.span.end).unwrap_or(0)))
    }
}

impl ParserContext<KitToken, KitNode> for KitParserContext {
    fn tokens(&self) -> &[Token<KitToken>] {
        &self.tokens
    }

    fn source(&self) -> &str {
        &self.source
    }

    fn pos(&self) -> usize {
        self.pos
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token<KitToken>> {
        self.tokens.get(self.pos)
    }

    fn peek_kind(&self) -> Option<KitToken> {
        self.peek().map(|t| t.kind.clone())
    }

    fn advance(&mut self) -> Result<&Token<KitToken>, ParseError> {
        if self.tokens.get(self.pos).is_some() {
            let pos = self.pos;
            self.pos += 1;
            return Ok(&self.tokens[pos]);
        }
        Err(ParseError::UnexpectedEof {
            span: self.peek_span(),
            expected: "a token".into(),
        })
    }

    fn match_kind(&mut self, kind: KitToken) -> Option<&Token<KitToken>> {
        let matches = match self.tokens.get(self.pos) {
            Some(token) => std::mem::discriminant(&token.kind) == std::mem::discriminant(&kind),
            None => false,
        };
        if matches {
            let pos = self.pos;
            self.pos += 1;
            return self.tokens.get(pos);
        }
        None
    }

    fn expect(&mut self, kind: KitToken, label: &str) -> Result<&Token<KitToken>, ParseError> {
        let span = self.peek_span();
        match self.match_kind(kind) {
            Some(token) => Ok(token),
            None => Err(ParseError::UnexpectedToken {
                span,
                expected: label.into(),
            }),
        }
    }

    fn expression(&mut self, min_bp: u32) -> Result<KitNode, ParseError> {
        let token = self.advance()?.clone();
        let grammar = self.grammar.clone();
        let rule = grammar
            .rule_for(&token.kind)
            .ok_or_else(|| ParseError::UnexpectedToken {
                span: token.span,
                expected: "a prefix expression".into(),
            })?;
        let mut left = rule.prefix(token, self)?;

        while let Some(kind) = self.peek_kind() {
            let grammar = self.grammar.clone();
            let rule = match grammar.rule_for(&kind) {
                Some(rule) => rule,
                None => break,
            };
            let (left_bp, right_bp) = match rule.infix_binding(&kind) {
                Some(bp) => bp,
                None => break,
            };
            if left_bp <= min_bp {
                break;
            }
            let op = self.advance()?.clone();
            left = rule.infix(left, op, self, right_bp)?;
        }

        Ok(left)
    }
}

/// The kit's parser, built on `handl_parser::CompositeGrammar`.
pub struct KitParser {
    grammar: Arc<dyn Grammar<KitToken, KitNode>>,
}

impl KitParser {
    fn new(grammar: CompositeGrammar<KitToken, KitNode>) -> Self {
        let grammar: Arc<dyn Grammar<KitToken, KitNode>> = Arc::new(grammar);
        Self { grammar }
    }

    fn parse(&self, tokens: Vec<Token<KitToken>>) -> Result<KitNode, ParseError> {
        let eof = self.grammar.eof();
        let mut ctx = KitParserContext {
            tokens,
            source: String::new(),
            pos: 0,
            grammar: self.grammar.clone(),
        };
        let root = self.grammar.parse_root(&mut ctx)?;

        if let Some(eof) = eof {
            if ctx.match_kind(eof).is_none() && !ctx.is_at_end() {
                return Err(ParseError::UnexpectedToken {
                    span: ctx.peek_span(),
                    expected: "end of input".into(),
                });
            }
        } else if !ctx.is_at_end() {
            return Err(ParseError::UnexpectedToken {
                span: ctx.peek_span(),
                expected: "end of input".into(),
            });
        }

        Ok(root)
    }
}

/// A curated kit that loads a manifest of plugins and composes a
/// lexer/parser pipeline from them.
pub struct Kit {
    lexer: CanonicalLexer,
    parser: KitParser,
}

impl Kit {
    /// Builds the canonical kit from the default manifest.
    pub fn new() -> Self {
        Self::from_manifest(&canonical_manifest()).expect("canonical manifest should always resolve")
    }

    /// Builds a `Kit` from a `KitManifest` by registering the listed plugins.
    pub fn from_manifest(manifest: &KitManifest) -> Result<Self, PlatformError> {
        let mut has_token_number = false;
        let mut has_ast_call = false;
        let mut has_grammar_call = false;

        for plugin in &manifest.plugins {
            match plugin.as_str() {
                "plugin-token-number" => has_token_number = true,
                "plugin-ast-call" => has_ast_call = true,
                "plugin-grammar-call" => has_grammar_call = true,
                other => {
                    return Err(PlatformError::Plugin(format!(
                        "unknown canonical plugin: {other}"
                    )))
                }
            }
        }

        if !has_token_number || !has_ast_call || !has_grammar_call {
            return Err(PlatformError::Plugin(
                "canonical manifest must include plugin-token-number, plugin-ast-call, and plugin-grammar-call".into(),
            ));
        }

        let grammar = CompositeGrammar::new()
            .register(NumberRule)
            .register(CallRule)
            .eof(KitToken::Eof);

        Ok(Self {
            lexer: CanonicalLexer,
            parser: KitParser::new(grammar),
        })
    }

    /// Lexes `input` into the kit's token stream.
    pub fn lex(&self, input: &str) -> Result<Vec<Token<KitToken>>, DefaultLexError> {
        self.lexer.lex(input)
    }

    /// Parses `input` into a generic [`KitNode`].
    pub fn parse(&self, input: &str) -> Result<KitNode, ParseError> {
        let tokens = self.lexer.lex(input).map_err(|e| ParseError::LexError {
            message: e.to_string(),
        })?;
        self.parser.parse(tokens)
    }

    /// Parses a module. Not yet supported by the minimal canonical plugin set.
    pub fn parse_module(&self, _input: &str) -> Result<Vec<KitNode>, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

impl Default for Kit {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_number() {
        let kit = Kit::new();
        let tokens = kit.lex("42").unwrap();
        assert_eq!(tokens.len(), 2); // number + eof
        assert_eq!(tokens[0].kind, KitToken::Number(NumberKind::Number(42.0)));
    }

    #[test]
    fn parse_number() {
        let kit = Kit::new();
        assert_eq!(kit.parse("1").unwrap(), KitNode::Number(1.0));
    }

    #[test]
    fn parse_call() {
        let kit = Kit::new();
        let node = kit.parse("1(2, 3)").unwrap();
        let expected = KitNode::Call(Call::new(
            "__apply",
            vec![
                KitNode::Number(1.0),
                KitNode::Number(2.0),
                KitNode::Number(3.0),
            ],
        ));
        assert_eq!(node, expected);
    }

    #[test]
    fn parse_module_unsupported() {
        let kit = Kit::new();
        assert!(matches!(kit.parse_module(""), Err(PlatformError::NotImplemented)));
    }
}
