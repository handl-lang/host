//! Low-level, parameterizable tokenization primitives.
//!
//! Each function consumes characters from a `CharIndices` peekable
//! iterator and produces a raw value + span. These are reusable building
//! blocks for writing [`crate::LexRule`] functions and concrete factory
//! implementations. The concrete factory or token plugin decides which
//! prefixes, delimiters, and character sets to pass to these helpers.

use crate::{DefaultLexError, Span};

/// Returns `true` if the iterator is positioned at the start of `prefix`.
pub fn starts_with(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    prefix: &str,
) -> bool {
    let mut peek = chars.clone();
    for expected in prefix.chars() {
        match peek.next() {
            Some((_, ch)) if ch == expected => {}
            _ => return false,
        }
    }
    true
}

/// Lexes a string literal. The first character is used as the quote,
/// and the literal runs until the matching quote is found.
pub fn lex_string(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> Result<(String, Span), DefaultLexError> {
    let (start, quote) = chars.next().unwrap();
    let mut value = String::new();
    let mut end = start + 1;

    loop {
        match chars.next() {
            None => {
                return Err(DefaultLexError::UnterminatedString {
                    span: Span::new(start, end),
                });
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
                    return Err(DefaultLexError::UnterminatedString {
                        span: Span::new(start, end),
                    });
                }
            },
            Some((_, ch)) => {
                value.push(ch);
            }
        }
    }

    Ok((value, Span::new(start, end)))
}

/// Lexes a decimal number literal with an optional leading `-`.
/// Returns the parsed `f64` value and its span.
pub fn lex_number(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> Result<(f64, Span), DefaultLexError> {
    let (start, _) = chars.peek().copied().unwrap();
    let mut text = String::new();
    let mut end = start;

    if chars.peek().map(|(_, c)| *c) == Some('-') {
        let (pos, ch) = chars.next().unwrap();
        text.push(ch);
        end = pos + 1;
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

    let value: f64 = text.parse().map_err(|_| DefaultLexError::InvalidNumber {
        span: Span::new(start, end),
    })?;

    Ok((value, Span::new(start, end)))
}

/// Lexes an identifier. The first character is already known to be an
/// identifier start; `is_continue` decides which characters are consumed.
pub fn lex_ident(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
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

/// Lexes a line comment with the given `prefix`.
/// Returns the comment body (after `prefix`) and its span.
pub fn lex_line_comment(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    prefix: &str,
) -> (String, Span) {
    let (start, _) = chars.peek().copied().unwrap();
    let mut end = start;

    for _ in 0..prefix.chars().count() {
        let (pos, ch) = chars.next().unwrap();
        end = pos + ch.len_utf8();
    }

    let mut text = String::new();
    while let Some(&(pos, ch)) = chars.peek() {
        if ch == '\n' {
            break;
        }
        text.push(ch);
        end = pos + ch.len_utf8();
        chars.next();
    }

    (text, Span::new(start, end))
}

/// Lexes a block comment with the given `open` and `close` delimiters.
/// Returns the comment text and its span, or an error if unterminated.
pub fn lex_block_comment(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    open: &str,
    close: &str,
) -> Result<(String, Span), DefaultLexError> {
    let (start, _) = chars.peek().copied().unwrap();
    let mut end = start;

    for _expected in open.chars() {
        let (pos, ch) = chars.next().unwrap();
        let _ = ch; // assume caller has verified the prefix
        end = pos + ch.len_utf8();
    }

    let mut close_chars = close.chars();
    let close_first = close_chars.next().unwrap();
    let close_second = close_chars.next().unwrap();

    let mut text = String::new();
    loop {
        match chars.next() {
            None => {
                return Err(DefaultLexError::UnterminatedString {
                    span: Span::new(start, end),
                });
            }
            Some((pos, ch)) if ch == close_first => {
                end = pos + ch.len_utf8();
                match chars.peek() {
                    Some(&(_, c)) if c == close_second => {
                        let (_, c) = chars.next().unwrap();
                        end += c.len_utf8();
                        break;
                    }
                    _ => text.push(ch),
                }
            }
            Some((pos, ch)) => {
                text.push(ch);
                end = pos + ch.len_utf8();
            }
        }
    }

    Ok((text, Span::new(start, end)))
}

/// Tries to lex a run of operator characters from `operator_chars`.
/// Returns the raw operator text and its span, or `None` if no operator starts here.
pub fn lex_operator(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    operator_chars: &str,
) -> Option<(String, Span)> {
    let &(start, first) = chars.peek()?;
    if !operator_chars.contains(first) {
        return None;
    }

    let mut text = String::new();
    let mut end = start;
    while let Some(&(pos, ch)) = chars.peek() {
        if !operator_chars.contains(ch) {
            break;
        }
        text.push(ch);
        end = pos + ch.len_utf8();
        chars.next();
    }

    if text.is_empty() {
        return None;
    }

    Some((text, Span::new(start, end)))
}
