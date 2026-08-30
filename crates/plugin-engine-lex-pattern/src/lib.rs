//! # handl-plugin-engine-lex-pattern
//!
//! A **data-driven** lexer engine that reads [`LexerRuleDef`] fragments from a
//! [`Language`] and tokenizes source text (ADR-0044 §1, §3).
//!
//! Unlike the Rust-based `plugin-lexer-*` crates (which encode lexing logic in
//! Rust `try_lex` impls), this engine carries **no Rust lexing logic**: every
//! pattern — literals, character classes, numbers, delimited blocks — is read
//! from the `Language`'s `lexer_rules` fragment set. Swapping the lexer is a
//! data change, not a recompile.
//!
//! ## Algorithm
//!
//! 1. Collect all [`LexerRuleDef`] fragments, sort by `priority` ascending
//!    (lower = tried first). Literals are secondarily sorted by text length
//!    descending so the longest literal wins within the same priority.
//! 2. Iterate through the source text:
//!    - Skip whitespace (spaces, tabs, newlines, carriage returns).
//!    - At each position, try each rule in priority order. The first match
//!      produces a [`Token`] and advances the position.
//!    - If no rule matches, return [`LangError::LexError`] with the position.
//! 3. Return the [`Tokens`] stream.

#![warn(missing_docs)]

use handl_language::{LangError, Language, LexerPattern, LexerRuleDef, Token, Tokens};
use handl_plugin_engine_lex::{LexerEngine, LexerRequest};

/// A data-driven lexer engine that reads [`LexerRuleDef`] fragments from the
/// [`Language`] and tokenizes source text.
#[derive(Debug, Default, Clone, Copy)]
pub struct DataLexerEngine;

impl DataLexerEngine {
    /// Create a new data-driven lexer engine.
    pub fn new() -> Self {
        Self
    }
}

impl LexerEngine for DataLexerEngine {
    fn lex(&self, lang: &Language, req: &LexerRequest) -> Result<Tokens, LangError> {
        lex(lang, req.source.as_str())
    }
}

/// Lex `src` using the [`LexerRuleDef`] fragments in `lang`.
///
/// # Errors
///
/// Returns [`LangError::LexError`] if no rule matches at some position, or if a
/// delimited block is unterminated.
pub fn lex(lang: &Language, src: &str) -> Result<Tokens, LangError> {
    // 1. Collect and sort rules by priority (ascending), with literals sorted
    //    by text length descending within the same priority (longest match
    //    first).
    let mut rules: Vec<&LexerRuleDef> = lang.lexer_rules.iter().collect();
    rules.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| literal_len(b).cmp(&literal_len(a)))
    });

    let bytes = src.as_bytes();
    let mut pos: usize = 0;
    let mut tokens: Vec<Token> = Vec::new();

    while pos < bytes.len() {
        // 2a. Skip whitespace.
        if let Some(ch) = peek_char(src, pos) {
            if is_whitespace(ch) {
                pos += ch.len_utf8();
                continue;
            }
        }

        // 2b. Try each rule in priority order; first match wins.
        let mut matched = false;
        for rule in &rules {
            if let Some((kind, text, next)) = try_rule(rule, src, pos) {
                tokens.push(Token {
                    kind: kind.to_string(),
                    text,
                });
                pos = next;
                matched = true;
                break;
            }
        }

        if !matched {
            return Err(LangError::LexError {
                position: pos,
                message: format!(
                    "no lexer rule matches at {:?}",
                    peek_char(src, pos).unwrap_or('\0')
                ),
            });
        }
    }

    Ok(Tokens(tokens))
}

/// The length (in chars) of a literal rule's text, or 0 for non-literals.
/// Used as a secondary sort key so longer literals are tried first.
fn literal_len(rule: &LexerRuleDef) -> usize {
    match &rule.pattern {
        LexerPattern::Literal { text } => text.chars().count(),
        _ => 0,
    }
}

/// Try a single rule at `pos`. Returns `(token_kind, consumed_text, next_pos)`
/// on match.
fn try_rule<'r>(
    rule: &'r LexerRuleDef,
    src: &str,
    pos: usize,
) -> Option<(&'r str, String, usize)> {
    match &rule.pattern {
        LexerPattern::Literal { text } => {
            if src[pos..].starts_with(text.as_str()) {
                let next = pos + text.len();
                Some((rule.token_kind.as_str(), text.clone(), next))
            } else {
                None
            }
        }
        LexerPattern::CharClass {
            start_chars,
            continue_chars,
        } => {
            let ch = peek_char(src, pos)?;
            if !char_matches(ch, start_chars) {
                return None;
            }
            let start_byte = pos;
            let mut cur = pos + ch.len_utf8();
            while let Some(next_ch) = peek_char(src, cur) {
                if char_matches(next_ch, continue_chars) {
                    cur += next_ch.len_utf8();
                } else {
                    break;
                }
            }
            let text = src[start_byte..cur].to_string();
            Some((rule.token_kind.as_str(), text, cur))
        }
        LexerPattern::Number => try_number(rule, src, pos),
        LexerPattern::Delimited {
            open,
            close,
            escape,
            interpolation: _,
            close_at_eof,
        } => try_delimited(rule, open, close, *escape, *close_at_eof, src, pos),
    }
}

/// Try to match a number literal: optional leading `-`, digits, optional `.`
/// + digits.
fn try_number<'r>(
    rule: &'r LexerRuleDef,
    src: &str,
    pos: usize,
) -> Option<(&'r str, String, usize)> {
    let mut cur = pos;
    // Optional leading '-' — but only if followed by a digit.
    if peek_char(src, cur) == Some('-') {
        if let Some(d) = peek_char(src, cur + 1) {
            if !d.is_ascii_digit() {
                return None;
            }
        } else {
            return None;
        }
        cur += 1;
    }
    // One or more digits.
    let digit_start = cur;
    while let Some(ch) = peek_char(src, cur) {
        if ch.is_ascii_digit() {
            cur += ch.len_utf8();
        } else {
            break;
        }
    }
    if cur == digit_start {
        return None;
    }
    // Optional '.' + digits.
    if peek_char(src, cur) == Some('.') {
        let after_dot = cur + 1;
        let frac_start = after_dot;
        let mut frac = after_dot;
        while let Some(ch) = peek_char(src, frac) {
            if ch.is_ascii_digit() {
                frac += ch.len_utf8();
            } else {
                break;
            }
        }
        if frac > frac_start {
            cur = frac;
        }
    }
    let text = src[pos..cur].to_string();
    Some((rule.token_kind.as_str(), text, cur))
}

/// Try to match a delimited block (string, comment, template).
fn try_delimited<'r>(
    rule: &'r LexerRuleDef,
    open: &str,
    close: &str,
    escape: bool,
    close_at_eof: bool,
    src: &str,
    pos: usize,
) -> Option<(&'r str, String, usize)> {
    if !src[pos..].starts_with(open) {
        return None;
    }
    let mut cur = pos + open.len();
    while cur < src.len() {
        // Check for close delimiter.
        if src[cur..].starts_with(close) {
            cur += close.len();
            let text = src[pos..cur].to_string();
            return Some((rule.token_kind.as_str(), text, cur));
        }
        // Escape sequence: skip the next char.
        if escape && peek_char(src, cur) == Some('\\') {
            cur += 1;
            if let Some(esc_ch) = peek_char(src, cur) {
                cur += esc_ch.len_utf8();
            }
            continue;
        }
        // Otherwise consume one char.
        if let Some(ch) = peek_char(src, cur) {
            cur += ch.len_utf8();
        } else {
            break;
        }
    }
    // Reached EOF.
    if close_at_eof {
        let text = src[pos..cur].to_string();
        Some((rule.token_kind.as_str(), text, cur))
    } else {
        // Unterminated — signal failure by returning None so the engine
        // reports a lex error at this position.
        None
    }
}

/// Peek the char at byte offset `pos` in `src`.
fn peek_char(src: &str, pos: usize) -> Option<char> {
    src[pos..].chars().next()
}

/// Whether `ch` is whitespace (skipped by the lexer).
fn is_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r')
}

/// Check whether `ch` matches the character class `class`.
///
/// The class string supports ranges (`a-z`, `A-Z`, `0-9`) and single chars.
/// For example, `"a-zA-Z_"` matches any ASCII letter or underscore.
pub fn char_matches(ch: char, class: &str) -> bool {
    let chars: Vec<char> = class.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // Range: `X-Y` (three chars: start, '-', end).
        if i + 2 < chars.len() && chars[i + 1] == '-' {
            if ch >= chars[i] && ch <= chars[i + 2] {
                return true;
            }
            i += 3;
        } else {
            if ch == chars[i] {
                return true;
            }
            i += 1;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{Language, LexerPattern, LexerRuleDef};

    /// Build a small language with the rules needed for the unit tests.
    fn test_lang() -> Language {
        let mut lang = Language::new();
        // Numbers.
        lang.lexer_rules
            .merge(LexerRuleDef::new(
                "lex.number",
                "token.number",
                LexerPattern::Number,
                40,
            ))
            .unwrap();
        // Operators.
        lang.lexer_rules
            .merge(LexerRuleDef::new(
                "lex.eqeq",
                "token.eqeq",
                LexerPattern::Literal { text: "==".into() },
                10,
            ))
            .unwrap();
        lang.lexer_rules
            .merge(LexerRuleDef::new(
                "lex.plus",
                "token.plus",
                LexerPattern::Literal { text: "+".into() },
                20,
            ))
            .unwrap();
        // Identifiers.
        lang.lexer_rules
            .merge(LexerRuleDef::new(
                "lex.identifier",
                "token.identifier",
                LexerPattern::CharClass {
                    start_chars: "a-zA-Z_".into(),
                    continue_chars: "a-zA-Z0-9_".into(),
                },
                50,
            ))
            .unwrap();
        // Keyword (tried before identifiers).
        lang.lexer_rules
            .merge(LexerRuleDef::new(
                "lex.keyword.let",
                "token.keyword.let",
                LexerPattern::Literal { text: "let".into() },
                15,
            ))
            .unwrap();
        // Delimiters.
        lang.lexer_rules
            .merge(LexerRuleDef::new(
                "lex.lparen",
                "token.lparen",
                LexerPattern::Literal { text: "(".into() },
                30,
            ))
            .unwrap();
        lang.lexer_rules
            .merge(LexerRuleDef::new(
                "lex.rparen",
                "token.rparen",
                LexerPattern::Literal { text: ")".into() },
                30,
            ))
            .unwrap();
        lang.lexer_rules
            .merge(LexerRuleDef::new(
                "lex.comma",
                "token.comma",
                LexerPattern::Literal { text: ",".into() },
                30,
            ))
            .unwrap();
        // String.
        lang.lexer_rules
            .merge(LexerRuleDef::new(
                "lex.string",
                "token.string",
                LexerPattern::Delimited {
                    open: "\"".into(),
                    close: "\"".into(),
                    escape: true,
                    interpolation: false,
                    close_at_eof: false,
                },
                40,
            ))
            .unwrap();
        lang
    }

    fn tok(kind: &str, text: &str) -> Token {
        Token {
            kind: kind.into(),
            text: text.into(),
        }
    }

    #[test]
    fn lex_simple_expression() {
        let lang = test_lang();
        let toks = lex(&lang, "1 + 2").unwrap();
        assert_eq!(
            toks.0,
            vec![
                tok("token.number", "1"),
                tok("token.plus", "+"),
                tok("token.number", "2"),
            ]
        );
    }

    #[test]
    fn lex_identifiers() {
        let lang = test_lang();
        let toks = lex(&lang, "foo bar").unwrap();
        assert_eq!(
            toks.0,
            vec![
                tok("token.identifier", "foo"),
                tok("token.identifier", "bar"),
            ]
        );
    }

    #[test]
    fn lex_string_with_escapes() {
        let lang = test_lang();
        let toks = lex(&lang, "\"hello\"").unwrap();
        assert_eq!(toks.0, vec![tok("token.string", "\"hello\"")]);
    }

    #[test]
    fn lex_string_with_escaped_quote() {
        let lang = test_lang();
        let toks = lex(&lang, r#""he\"llo""#).unwrap();
        assert_eq!(toks.0, vec![tok("token.string", r#""he\"llo""#)]);
    }

    #[test]
    fn lex_keyword_vs_identifier() {
        let lang = test_lang();
        let toks = lex(&lang, "let x").unwrap();
        assert_eq!(
            toks.0,
            vec![
                tok("token.keyword.let", "let"),
                tok("token.identifier", "x"),
            ]
        );
    }

    #[test]
    fn lex_operator_longest_match() {
        let lang = test_lang();
        let toks = lex(&lang, "a == b").unwrap();
        assert_eq!(
            toks.0,
            vec![
                tok("token.identifier", "a"),
                tok("token.eqeq", "=="),
                tok("token.identifier", "b"),
            ]
        );
    }

    #[test]
    fn lex_delimiters() {
        let lang = test_lang();
        let toks = lex(&lang, "(a, b)").unwrap();
        assert_eq!(
            toks.0,
            vec![
                tok("token.lparen", "("),
                tok("token.identifier", "a"),
                tok("token.comma", ","),
                tok("token.identifier", "b"),
                tok("token.rparen", ")"),
            ]
        );
    }

    #[test]
    fn whitespace_is_skipped() {
        let lang = test_lang();
        let toks = lex(&lang, "  1\t\n+  2  ").unwrap();
        assert_eq!(
            toks.0,
            vec![
                tok("token.number", "1"),
                tok("token.plus", "+"),
                tok("token.number", "2"),
            ]
        );
    }

    #[test]
    fn unknown_character_returns_error() {
        let lang = test_lang();
        let err = lex(&lang, "@").unwrap_err();
        assert!(matches!(err, LangError::LexError { position: 0, .. }));
    }

    #[test]
    fn unterminated_string_returns_error() {
        let lang = test_lang();
        let err = lex(&lang, "\"oops").unwrap_err();
        assert!(matches!(err, LangError::LexError { .. }));
    }

    #[test]
    fn lex_decimal_number() {
        let lang = test_lang();
        let toks = lex(&lang, "3.14").unwrap();
        assert_eq!(toks.0, vec![tok("token.number", "3.14")]);
    }

    #[test]
    fn lex_negative_number() {
        let lang = test_lang();
        let toks = lex(&lang, "-42").unwrap();
        assert_eq!(toks.0, vec![tok("token.number", "-42")]);
    }

    #[test]
    fn lex_empty_source() {
        let lang = test_lang();
        let toks = lex(&lang, "").unwrap();
        assert!(toks.0.is_empty());
    }

    #[test]
    fn char_matches_ranges() {
        assert!(char_matches('a', "a-zA-Z_"));
        assert!(char_matches('Z', "a-zA-Z_"));
        assert!(char_matches('_', "a-zA-Z_"));
        assert!(!char_matches('0', "a-zA-Z_"));
        assert!(char_matches('0', "a-zA-Z0-9_"));
        assert!(char_matches('9', "a-zA-Z0-9_"));
        assert!(!char_matches('-', "a-zA-Z0-9_"));
    }

    #[test]
    fn engine_trait_dispatch() {
        let lang = test_lang();
        let engine = DataLexerEngine::new();
        let req = LexerRequest::new("1 + 2");
        let toks = engine.lex(&lang, &req).unwrap();
        assert_eq!(toks.0.len(), 3);
    }
}
