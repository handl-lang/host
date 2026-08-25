use handl_expr::{Span, Token, TokenRule};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;

/// Token rule for numeric literals.
pub struct Numbers;

const NUMBERS_FIRST: &'static [char] = &[
    '-', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
];

impl Numbers {
    /// Creates a new numeric token rule.
    pub fn new() -> Self {
        Numbers
    }
}

impl TokenRule for Numbers {
    fn name(&self) -> &str {
        "numbers"
    }

    fn first_chars(&self) -> &'static [char] {
        NUMBERS_FIRST
    }

    fn try_lex(&self, input: &str, start: usize) -> Option<Result<Token, PlatformError>> {
        let rest = &input[start..];
        let mut chars = rest.char_indices().peekable();
        let first = chars.peek()?.1;
        if !first.is_ascii_digit() && first != '-' {
            return None;
        }

        if first == '-' {
            chars.next();
            if !matches!(chars.peek(), Some((_, c)) if c.is_ascii_digit()) {
                return None;
            }
        }

        let mut has_dot = false;
        let mut end = start;

        while let Some((pos, ch)) = chars.peek() {
            if ch.is_ascii_digit() {
                end = start + pos + ch.len_utf8();
                chars.next();
            } else if *ch == '.' && !has_dot {
                end = start + pos + 1;
                has_dot = true;
                chars.next();
            } else {
                break;
            }
        }

        let text = &input[start..end];
        if text == "-" {
            return None;
        }

        let value: f64 = match text.parse() {
            Ok(v) => v,
            Err(_) => {
                return Some(Err(PlatformError::Plugin(format!(
                    "invalid number literal: {text}"
                ))));
            }
        };

        Some(Ok(Token::new(CanonicalKind::Number(value), Span::new(start, end))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;

    #[test]
    fn lexes_integer() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Numbers::new()));
        let tokens = p.lex("42").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, CanonicalKind::Number(42.0));
    }

    #[test]
    fn lexes_negative_and_float() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Numbers::new()));
        let tokens = p.lex("-3 1.5").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, CanonicalKind::Number(-3.0));
        assert_eq!(tokens[1].kind, CanonicalKind::Number(1.5));
    }
}
