use handl_plugin_lexer::{
    DynToken, DynTokenKind, LexError, LexRule, LexerConfig, Span, TokenKind,
};
use std::fmt;
use std::marker::PhantomData;
use thiserror::Error;

/// Errors produced by the dynamic lexer.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum DynLexError {
    /// No rule matched a non-whitespace character.
    #[error("unexpected character at {span}: {char:?}")]
    UnexpectedChar { span: Span, char: char },

    /// A rule matched but failed to produce a token.
    #[error("rule error at {span}: {message}")]
    RuleFailed { span: Span, message: String },

    /// The lexer has no EOF kind configured.
    #[error("no eof kind configured")]
    NoEofKind,
}

/// Object-safe lexing rule for the dynamic lexer.
///
/// Unlike [`LexRule`], this trait does not require a static `K: TokenKind`
/// generic parameter — rules produce `Box<dyn DynTokenKind>` instead.
pub trait DynLexRule: Send + Sync + 'static {
    /// Returns `true` if this rule matches the input at the given byte position.
    fn matches(&self, input: &str, pos: usize) -> bool;

    /// Lexes a token starting at `pos`. Returns the token kind and its span.
    fn lex(&self, input: &str, pos: usize) -> Result<(Box<dyn DynTokenKind>, Span), DynLexError>;

    /// Rule priority — higher priority rules are tried first.
    fn priority(&self) -> u32;
}

/// A dynamic lexer that assembles rules at runtime and produces
/// `Vec<DynToken>` without a static `K: TokenKind` parameter.
pub struct DynLexer {
    rules: Vec<Box<dyn DynLexRule>>,
    eof_kind: Option<Box<dyn DynTokenKind>>,
}

impl Default for DynLexer {
    fn default() -> Self {
        Self::new()
    }
}

impl DynLexer {
    /// Creates an empty dynamic lexer.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            eof_kind: None,
        }
    }

    /// Adds a lexing rule.
    pub fn with_rule(mut self, rule: Box<dyn DynLexRule>) -> Self {
        self.rules.push(rule);
        self
    }

    /// Sets the EOF token kind emitted at the end of the stream.
    pub fn with_eof_kind(mut self, kind: Box<dyn DynTokenKind>) -> Self {
        self.eof_kind = Some(kind);
        self
    }

    /// Lexes `input` into a token stream.
    ///
    /// Whitespace is skipped. At each position, rules are tried in priority
    /// order (highest first); the first that matches produces a token and
    /// advances the position. If no rule matches, an error is returned.
    /// An EOF token is appended at the end.
    pub fn lex(&self, input: &str) -> Result<Vec<DynToken>, DynLexError> {
        let mut tokens = Vec::new();
        let mut pos = 0;
        let bytes = input.as_bytes();

        // Sort rule indices by priority descending (stable on insertion order).
        let mut order: Vec<usize> = (0..self.rules.len()).collect();
        order.sort_by(|&a, &b| self.rules[b].priority().cmp(&self.rules[a].priority()));

        while pos < bytes.len() {
            // Skip whitespace.
            let rest = &input[pos..];
            let mut chars = rest.char_indices();
            let ch = match chars.next() {
                Some((_, ch)) if ch.is_whitespace() => {
                    pos += ch.len_utf8();
                    continue;
                }
                Some((_, ch)) => ch,
                None => break,
            };

            let mut matched = false;
            for &idx in &order {
                if self.rules[idx].matches(input, pos) {
                    let (kind, span) = self.rules[idx].lex(input, pos)?;
                    if span.end <= pos {
                        return Err(DynLexError::RuleFailed {
                            span,
                            message: "rule did not advance the position".into(),
                        });
                    }
                    tokens.push(DynToken::new(kind, span));
                    pos = span.end;
                    matched = true;
                    break;
                }
            }

            if !matched {
                return Err(DynLexError::UnexpectedChar {
                    span: Span::new(pos, pos + ch.len_utf8()),
                    char: ch,
                });
            }
        }

        let eof = self
            .eof_kind
            .as_ref()
            .ok_or(DynLexError::NoEofKind)?
            .dyn_clone();
        tokens.push(DynToken::new(eof, Span::at(input.len())));
        Ok(tokens)
    }
}

/// Bridge adapter that wraps a static [`LexRule<K, E>`] + its factory and
/// exposes it as a [`DynLexRule`].
///
/// This lets existing rules written for the generic lexer be reused with
/// the dynamic lexer.
pub struct StaticRuleAdapter<R, K, E>
where
    R: LexRule<K, E>,
    K: TokenKind,
    E: LexError,
{
    rule: R,
    factory: K::Factory,
    _e: PhantomData<E>,
}

impl<R, K, E> StaticRuleAdapter<R, K, E>
where
    R: LexRule<K, E>,
    K: TokenKind,
    E: LexError,
{
    /// Creates a new adapter wrapping `rule` with the given `factory`.
    pub fn new(rule: R, factory: K::Factory) -> Self {
        Self {
            rule,
            factory,
            _e: PhantomData,
        }
    }
}

impl<R, K, E> Clone for StaticRuleAdapter<R, K, E>
where
    R: LexRule<K, E> + Clone,
    K: TokenKind,
    E: LexError,
{
    fn clone(&self) -> Self {
        Self {
            rule: self.rule.clone(),
            factory: self.factory.clone(),
            _e: PhantomData,
        }
    }
}

impl<R, K, E> DynLexRule for StaticRuleAdapter<R, K, E>
where
    R: LexRule<K, E> + Clone + Send + Sync + 'static,
    K: TokenKind + Clone + Send + Sync + 'static,
    E: LexError + fmt::Display,
    K::Factory: Clone + Send + Sync + 'static,
{
    fn matches(&self, input: &str, pos: usize) -> bool {
        let mut chars = input[pos..].char_indices().peekable();
        let config = LexerConfig::<K, E>::new(self.factory.clone());
        matches!(
            self.rule.try_lex(&mut chars, &config),
            Some(Ok(_))
        )
    }

    fn lex(&self, input: &str, pos: usize) -> Result<(Box<dyn DynTokenKind>, Span), DynLexError> {
        let mut chars = input[pos..].char_indices().peekable();
        let config = LexerConfig::<K, E>::new(self.factory.clone());
        match self.rule.try_lex(&mut chars, &config) {
            None => Err(DynLexError::RuleFailed {
                span: Span::at(pos),
                message: "rule did not match".into(),
            }),
            Some(Ok(token)) => {
                let span = Span::new(token.span.start + pos, token.span.end + pos);
                Ok((Box::new(token.kind), span))
            }
            Some(Err(e)) => Err(DynLexError::RuleFailed {
                span: Span::at(pos),
                message: e.to_string(),
            }),
        }
    }

    fn priority(&self) -> u32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_lexer::{EofFactory, FnRule};

    #[derive(Debug, Clone, PartialEq)]
    enum Kind {
        Word(String),
        Eof,
    }

    impl fmt::Display for Kind {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Kind::Word(s) => write!(f, "word({s})"),
                Kind::Eof => write!(f, "eof"),
            }
        }
    }

    impl TokenKind for Kind {
        type Factory = WordFactory;
        fn is_eof(&self) -> bool {
            matches!(self, Kind::Eof)
        }
        fn is_error(&self) -> bool {
            false
        }
    }

    #[derive(Clone)]
    struct WordFactory;

    impl EofFactory<Kind> for WordFactory {
        fn eof_kind(&self) -> Kind {
            Kind::Eof
        }
    }

    fn word_rule(
        chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
        _cfg: &LexerConfig<Kind, handl_plugin_lexer::DefaultLexError>,
    ) -> Option<Result<handl_plugin_lexer::Token<Kind>, handl_plugin_lexer::DefaultLexError>> {
        let (start, first) = chars.peek().copied()?;
        if !first.is_alphabetic() {
            return None;
        }
        let mut text = String::new();
        let mut end = start;
        while let Some((i, ch)) = chars.peek().copied() {
            if ch.is_alphanumeric() {
                text.push(ch);
                end = i + ch.len_utf8();
                chars.next();
            } else {
                break;
            }
        }
        Some(Ok(handl_plugin_lexer::Token::new(
            Kind::Word(text),
            Span::new(start, end),
        )))
    }

    fn make_rule() -> FnRule<Kind, handl_plugin_lexer::DefaultLexError> {
        FnRule::new("word", word_rule)
    }

    #[test]
    fn lexes_via_static_adapter() {
        let rule: Box<dyn DynLexRule> = Box::new(StaticRuleAdapter::new(make_rule(), WordFactory));
        let lexer = DynLexer::new()
            .with_rule(rule)
            .with_eof_kind(Box::new(Kind::Eof));

        let tokens = lexer.lex("hello world").unwrap();
        assert_eq!(tokens.len(), 3);
        assert!(matches!(
            tokens[2].kind.as_any().downcast_ref::<Kind>(),
            Some(Kind::Eof)
        ));
        assert_eq!(tokens[0].span, Span::new(0, 5));
        assert_eq!(tokens[1].span, Span::new(6, 11));
    }

    #[test]
    fn errors_on_unexpected_char() {
        let rule: Box<dyn DynLexRule> = Box::new(StaticRuleAdapter::new(make_rule(), WordFactory));
        let lexer = DynLexer::new()
            .with_rule(rule)
            .with_eof_kind(Box::new(Kind::Eof));

        let err = lexer.lex("hello @ world").unwrap_err();
        assert!(matches!(err, DynLexError::UnexpectedChar { .. }));
    }

    #[test]
    fn priority_orders_rules() {
        struct Hi;
        impl DynLexRule for Hi {
            fn matches(&self, _input: &str, _pos: usize) -> bool {
                true
            }
            fn lex(&self, input: &str, pos: usize) -> Result<(Box<dyn DynTokenKind>, Span), DynLexError> {
                let ch = input[pos..].chars().next().unwrap();
                Ok((Box::new(Kind::Word("hi".into())), Span::new(pos, pos + ch.len_utf8())))
            }
            fn priority(&self) -> u32 {
                10
            }
        }
        struct Lo;
        impl DynLexRule for Lo {
            fn matches(&self, _input: &str, _pos: usize) -> bool {
                true
            }
            fn lex(&self, input: &str, pos: usize) -> Result<(Box<dyn DynTokenKind>, Span), DynLexError> {
                let ch = input[pos..].chars().next().unwrap();
                Ok((Box::new(Kind::Word("lo".into())), Span::new(pos, pos + ch.len_utf8())))
            }
            fn priority(&self) -> u32 {
                1
            }
        }

        let lexer = DynLexer::new()
            .with_rule(Box::new(Lo))
            .with_rule(Box::new(Hi))
            .with_eof_kind(Box::new(Kind::Eof));
        let tokens = lexer.lex("ab").unwrap();
        assert!(matches!(
            tokens[0].kind.as_any().downcast_ref::<Kind>(),
            Some(Kind::Word(s)) if s == "hi"
        ));
    }
}
