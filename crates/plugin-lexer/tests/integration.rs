//! Integration tests for the generic lexer infrastructure.
//!
//! These tests exercise the public API of `handl-plugin-lexer` from an external
//! perspective, defining a custom `TokenKind` implementation and verifying
//! that the generic config, rules, and lexer trait work end-to-end.

use handl_plugin_lexer::{
    CommentTokenKind, DefaultLexError, EofFactory, FnRule, LexRule, Lexer, LexerConfig, Span,
    Token, TokenKind,
};
use std::fmt;

// --- Test dialect ---

#[derive(Debug, Clone, PartialEq)]
enum IntegKind {
    Ident(String),
    Num(f64),
    Str(String),
    Eof,
}

impl fmt::Display for IntegKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntegKind::Ident(s) => write!(f, "ident({s})"),
            IntegKind::Num(n) => write!(f, "num({n})"),
            IntegKind::Str(s) => write!(f, "str({s:?})"),

            IntegKind::Eof => write!(f, "eof"),
        }
    }
}

impl TokenKind for IntegKind {
    type Factory = ();

    fn is_eof(&self) -> bool {
        matches!(self, IntegKind::Eof)
    }
    fn is_error(&self) -> bool {
        false
    }
}

impl EofFactory<IntegKind> for () {
    fn eof_kind(&self) -> IntegKind {
        IntegKind::Eof
    }
}

impl CommentTokenKind for IntegKind {
    fn is_comment(&self) -> bool {
        false
    }
}

// --- Local rules (each rule knows how to build its own token kind) ---

#[derive(Clone)]
struct StringRule;

impl LexRule<IntegKind, DefaultLexError> for StringRule {
    fn try_lex(
        &self,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        _config: &LexerConfig<IntegKind, DefaultLexError>,
    ) -> Option<Result<Token<IntegKind>, DefaultLexError>> {
        let (start, quote) = *chars.peek()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        chars.next();
        let mut value = String::new();
        let mut end = start + 1;

        loop {
            match chars.next() {
                None => {
                    return Some(Err(DefaultLexError::UnterminatedString {
                        span: Span::new(start, end),
                    }));
                }
                Some((pos, ch)) if ch == quote => {
                    end = pos + 1;
                    break;
                }
                Some((_, '\\')) => match chars.next() {
                    Some((_, 'n')) => value.push('\n'),
                    Some((_, 't')) => value.push('\t'),
                    Some((_, 'r')) => value.push('\r'),
                    Some((_, '\\')) => value.push('\\'),
                    Some((_, '"')) => value.push('"'),
                    Some((_, '\'')) => value.push('\''),
                    Some((pos, ch)) => {
                        value.push(ch);
                        let _ = pos;
                    }
                    None => {
                        return Some(Err(DefaultLexError::UnterminatedString {
                            span: Span::new(start, end),
                        }));
                    }
                },
                Some((_, ch)) => value.push(ch),
            }
        }

        Some(Ok(Token::new(IntegKind::Str(value), Span::new(start, end))))
    }

    fn name(&self) -> &'static str {
        "string"
    }

    fn clone_box(&self) -> Box<dyn LexRule<IntegKind, DefaultLexError>> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
struct NumberRule;

impl LexRule<IntegKind, DefaultLexError> for NumberRule {
    fn try_lex(
        &self,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        _config: &LexerConfig<IntegKind, DefaultLexError>,
    ) -> Option<Result<Token<IntegKind>, DefaultLexError>> {
        let (start, _) = *chars.peek()?;
        let mut text = String::new();
        let mut end = start;

        if chars.peek().map(|(_, c)| *c) == Some('-') {
            let (pos, ch) = chars.next().unwrap();
            text.push(ch);
            end = pos + 1;
        }

        if !matches!(chars.peek(), Some((_, c)) if c.is_ascii_digit()) {
            return None;
        }

        while let Some(&(pos, ch)) = chars.peek() {
            if ch.is_ascii_digit() || ch == '.' {
                text.push(ch);
                end = pos + ch.len_utf8();
                chars.next();
            } else {
                break;
            }
        }

        if text == "-" {
            return None;
        }

        let value: f64 = match text.parse() {
            Ok(v) => v,
            Err(_) => {
                return Some(Err(DefaultLexError::InvalidNumber {
                    span: Span::new(start, end),
                }));
            }
        };

        Some(Ok(Token::new(IntegKind::Num(value), Span::new(start, end))))
    }

    fn name(&self) -> &'static str {
        "number"
    }

    fn clone_box(&self) -> Box<dyn LexRule<IntegKind, DefaultLexError>> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
struct IdentRule;

impl LexRule<IntegKind, DefaultLexError> for IdentRule {
    fn try_lex(
        &self,
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<IntegKind, DefaultLexError>,
    ) -> Option<Result<Token<IntegKind>, DefaultLexError>> {
        let (start, ch) = *chars.peek()?;
        if !ch.is_alphabetic() && ch != '_' {
            return None;
        }

        let mut text = String::new();
        let mut end = start;
        let mut clone = chars.clone();

        while let Some(&(pos, c)) = clone.peek() {
            if !c.is_alphanumeric() && c != '_' {
                break;
            }
            text.push(c);
            end = pos + c.len_utf8();
            clone.next();
        }

        if text.is_empty() {
            return None;
        }

        *chars = clone;
        let kind = config
            .lookup_keyword(&text)
            .cloned()
            .unwrap_or(IntegKind::Ident(text));
        Some(Ok(Token::new(kind, Span::new(start, end))))
    }

    fn name(&self) -> &'static str {
        "ident"
    }

    fn clone_box(&self) -> Box<dyn LexRule<IntegKind, DefaultLexError>> {
        Box::new(self.clone())
    }
}

// --- A simple lexer using the generic infrastructure ---

struct SimpleLexer {
    config: LexerConfig<IntegKind, DefaultLexError>,
}

impl Default for SimpleLexer {
    fn default() -> Self {
        SimpleLexer {
            config: LexerConfig::new(())
                .with_rule(StringRule)
                .with_rule(NumberRule)
                .with_rule(IdentRule),
        }
    }
}

impl Lexer<IntegKind, DefaultLexError> for SimpleLexer {
    fn lex(&self, input: &str) -> Result<Vec<Token<IntegKind>>, DefaultLexError> {
        let mut tokens = Vec::new();
        let mut chars = input.char_indices().peekable();

        while let Some(&(pos, ch)) = chars.peek() {
            if ch.is_whitespace() {
                chars.next();
                continue;
            }

            let mut matched = false;
            for rule in self.config.rules() {
                if let Some(result) = rule.try_lex(&mut chars, &self.config) {
                    let token = result?;
                    if !token.kind.is_comment() {
                        tokens.push(token);
                    }
                    matched = true;
                    break;
                }
            }

            if !matched {
                chars.next();
                return Err(DefaultLexError::UnexpectedChar {
                    span: Span::at(pos),
                    char: ch,
                });
            }
        }

        tokens.push(Token::new(IntegKind::Eof, Span::at(input.len())));
        Ok(tokens)
    }
}

// --- Tests ---

#[test]
fn lex_basic_tokens() {
    let lexer = SimpleLexer::default();
    let tokens = lexer.lex("foo 42 \"hello\"").unwrap();
    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[0].kind, IntegKind::Ident("foo".into()));
    assert_eq!(tokens[1].kind, IntegKind::Num(42.0));
    assert_eq!(tokens[2].kind, IntegKind::Str("hello".into()));
    assert_eq!(tokens[3].kind, IntegKind::Eof);
}

#[test]
fn lex_unterminated_string_errors() {
    let lexer = SimpleLexer::default();
    let result = lexer.lex("\"oops");
    assert!(result.is_err());
}

#[test]
fn lex_unexpected_char_errors() {
    let lexer = SimpleLexer::default();
    let result = lexer.lex("@@@");
    assert!(result.is_err());
}

#[test]
fn lex_iter_streams_tokens() {
    let lexer = SimpleLexer::default();
    let tokens: Vec<_> = lexer.lex_iter("foo 42").collect();
    assert_eq!(tokens.len(), 3);
    assert!(tokens.iter().all(|t| t.is_ok()));
}

#[test]
fn custom_fn_rule_works() {
    fn bang_rule(
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        _config: &LexerConfig<IntegKind, DefaultLexError>,
    ) -> Option<Result<Token<IntegKind>, DefaultLexError>> {
        match chars.peek() {
            Some(&(_, '!')) => {
                let (pos, _) = chars.next().unwrap();
                Some(Ok(Token::new(IntegKind::Ident("!".into()), Span::at(pos))))
            }
            _ => None,
        }
    }

    let config = LexerConfig::new(())
        .with_rule(FnRule::new("bang", bang_rule))
        .with_rule(StringRule)
        .with_rule(NumberRule)
        .with_rule(IdentRule);

    let lexer = SimpleLexer { config };
    let tokens = lexer.lex("! 42").unwrap();
    assert_eq!(tokens[0].kind, IntegKind::Ident("!".into()));
    assert_eq!(tokens[1].kind, IntegKind::Num(42.0));
}

#[test]
fn config_merge_combines_rules() {
    let base: LexerConfig<IntegKind, DefaultLexError> = LexerConfig::new(())
        .with_rule(StringRule)
        .with_rule(NumberRule);
    let ext = LexerConfig::new(()).with_rule(IdentRule);

    let merged = base.merge(ext);
    assert_eq!(merged.rules().len(), 3);
    assert_eq!(merged.rules()[0].name(), "string");
    assert_eq!(merged.rules()[1].name(), "number");
    assert_eq!(merged.rules()[2].name(), "ident");
}

#[test]
fn config_merge_dedups_by_name() {
    let base: LexerConfig<IntegKind, DefaultLexError> = LexerConfig::new(()).with_rule(StringRule);
    let ext = LexerConfig::new(()).with_rule(StringRule);

    let merged = base.merge(ext);
    assert_eq!(merged.rules().len(), 1);
}

#[test]
fn without_rule_by_name_disables_feature() {
    let config = LexerConfig::new(())
        .with_rule(StringRule)
        .with_rule(NumberRule)
        .with_rule(IdentRule)
        .without_rule_by_name("string");

    let lexer = SimpleLexer { config };
    let result = lexer.lex("\"hello\"");
    assert!(result.is_err());
}
