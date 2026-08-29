//! [`IdentRule`] — matches identifiers and keywords.
//!
//! This crate owns the identifier lexing logic (`lex_ident`) and the
//! [`IdentToken`] trait that a host's token kind factory implements to
//! produce identifier token kinds.

use handl_plugin_lexer::{
    DefaultLexError, LexError, LexRule, LexerConfig, Span, Token, TokenKind,
};
use std::iter::Peekable;

// ---------------------------------------------------------------------------
// Primitive
// ---------------------------------------------------------------------------

/// Lexes an identifier. The first character is already known to be an
/// identifier start; `is_continue` decides which characters are consumed.
pub fn lex_ident(
    chars: &mut Peekable<std::str::CharIndices<'_>>,
    is_continue: impl Fn(char) -> bool,
) -> Result<(String, Span), DefaultLexError> {
    let (start, _) = chars.peek().copied().unwrap();
    let mut text = String::new();
    let mut end = start;

    while let Some(&(pos, ch)) = chars.peek() {
        if !is_continue(ch) {
            break;
        }
        text.push(ch);
        end = pos + ch.len_utf8();
        chars.next();
    }

    Ok((text, Span::new(start, end)))
}

// ---------------------------------------------------------------------------
// Token trait
// ---------------------------------------------------------------------------

/// Token-kind factory capability for identifiers.
///
/// A host's token kind factory implements this so that [`IdentRule`] can
/// produce identifier tokens. The host decides which characters start and
/// continue an identifier.
pub trait IdentToken<K: TokenKind>: Clone + Send + Sync + 'static {
    /// Returns `true` if `ch` can start an identifier.
    fn is_ident_start(&self, ch: char) -> bool;

    /// Returns `true` if `ch` may appear after the first char of an identifier.
    fn is_ident_continue(&self, ch: char) -> bool;

    /// Builds an identifier token kind from the raw text.
    fn ident_kind(&self, text: String) -> K;

    /// Returns a regex pattern that matches identifiers for this dialect,
    /// e.g. `r"[a-zA-Z_][a-zA-Z0-9_]*"`.
    ///
    /// Override this to provide a dialect-specific identifier regex for grammar
    /// generators. The default returns a basic ASCII identifier pattern.
    fn identifier_regex(&self) -> &str {
        r"[a-zA-Z_][a-zA-Z0-9_]*"
    }
}

// ---------------------------------------------------------------------------
// Rule
// ---------------------------------------------------------------------------

/// Matches identifiers and keywords (identifiers in the keyword table
/// are remapped to their token kind).
#[derive(Clone, Default)]
pub struct IdentRule;

impl<K, E> LexRule<K, E> for IdentRule
where
    K: TokenKind,
    K::Factory: IdentToken<K>,
    E: LexError + From<DefaultLexError>,
{
    fn try_lex(
        &self,
        chars: &mut Peekable<std::str::CharIndices<'_>>,
        config: &LexerConfig<K, E>,
    ) -> Option<Result<Token<K>, E>> {
        match chars.peek() {
            Some(&(_, ch)) if config.factory().is_ident_start(ch) => Some(
                lex_ident(chars, |c| config.factory().is_ident_continue(c))
                    .map(|(text, span)| {
                        let kind = config
                            .lookup_keyword(&text)
                            .cloned()
                            .unwrap_or_else(|| config.factory().ident_kind(text));
                        Token::new(kind, span)
                    })
                    .map_err(E::from),
            ),
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        "ident"
    }

    fn clone_box(&self) -> Box<dyn LexRule<K, E>> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_lexer::{DefaultLexError, LexRule, Span, TokenKind};
    use handl_plugin_lexer_common::{MockFactory, MockKind, empty_config as cfg};

    // Orphan-rule-allowed impl: IdentToken is local to this crate.
    impl IdentToken<MockKind> for MockFactory {
        fn is_ident_start(&self, ch: char) -> bool {
            ch.is_alphabetic() || ch == '_' || ch == '.'
        }

        fn is_ident_continue(&self, ch: char) -> bool {
            ch.is_alphanumeric() || ch == '_' || ch == '.'
        }

        fn ident_kind(&self, text: String) -> MockKind {
            (self.ident_kind_fn())(text)
        }
    }

    const RULE: IdentRule = IdentRule;

    #[test]
    fn ident_rule_matches_identifier() {
        let config = cfg();
        let mut chars = "foo bar".char_indices().peekable();
        let result = IdentRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Ident("foo".into()));
        assert_eq!(token.span, Span::new(0, 3));
    }

    #[test]
    fn ident_rule_matches_namespace_path() {
        let config = cfg();
        let mut chars = "math.sqrt rest".char_indices().peekable();
        let result = IdentRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Ident("math.sqrt".into()));
    }

    #[test]
    fn ident_rule_keyword_remapped() {
        let config = cfg().with_keyword("true", MockKind::Delim('t'));
        let mut chars = "true rest".char_indices().peekable();
        let result = IdentRule.try_lex(&mut chars, &config);
        let token = result.unwrap().unwrap();
        assert_eq!(token.kind, MockKind::Delim('t'));
    }

    #[test]
    fn ident_rule_no_match_digit() {
        let config = cfg();
        let mut chars = "42".char_indices().peekable();
        let result = IdentRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn ident_rule_no_match_symbol() {
        let config = cfg();
        let mut chars = "@@".char_indices().peekable();
        let result = IdentRule.try_lex(&mut chars, &config);
        assert!(result.is_none());
    }

    #[test]
    fn ident_rule_name() {
        let rule: &dyn LexRule<MockKind, DefaultLexError> = &RULE;
        assert_eq!(rule.name(), "ident");
    }

    #[test]
    fn ident_rule_clone_box() {
        let cloned: Box<dyn LexRule<MockKind, DefaultLexError>> = RULE.clone_box();
        assert_eq!(cloned.name(), "ident");
    }
}
