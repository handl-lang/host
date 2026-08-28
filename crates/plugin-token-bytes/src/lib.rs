use handl_expr::{Span, Token, TokenRule};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;

/// Token rule for byte literals (`0xNN`).
pub struct Bytes;

const BYTES_FIRST: &'static [char] = &['0'];

impl Bytes {
    /// Creates a new byte token rule.
    pub fn new() -> Self {
        Bytes
    }
}

fn is_hex(ch: char) -> bool {
    ch.is_ascii_digit() || (ch >= 'a' && ch <= 'f') || (ch >= 'A' && ch <= 'F')
}

impl TokenRule for Bytes {
    fn name(&self) -> &str {
        "bytes"
    }

    fn first_chars(&self) -> &'static [char] {
        BYTES_FIRST
    }

    fn try_lex(&self, input: &str, start: usize) -> Option<Result<Token, PlatformError>> {
        let rest = &input[start..];

        if !rest.starts_with("0x") {
            return None;
        }

        if rest.len() < 4 {
            return Some(Err(PlatformError::Plugin(
                "incomplete byte literal".to_string(),
            )));
        }

        let digits = &rest[2..4];
        if !digits.chars().all(is_hex) {
            return Some(Err(PlatformError::Plugin(
                "invalid byte literal".to_string(),
            )));
        }

        if let Some(ch) = rest.chars().nth(4) {
            if is_hex(ch) {
                return Some(Err(PlatformError::Plugin(
                    "byte literal must be exactly two hex digits".to_string(),
                )));
            }
        }

        let value = u8::from_str_radix(digits, 16).ok()?;
        Some(Ok(Token::new(CanonicalKind::Byte(value), Span::new(start, start + 4))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;

    #[test]
    fn lexes_byte() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Bytes::new()));

        let tokens = p.lex("0x1a").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, CanonicalKind::Byte(0x1a));
        assert_eq!(tokens[0].span, Span::new(0, 4));
    }

    #[test]
    fn rejects_malformed_byte() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Bytes::new()));
        assert!(p.lex("0xGG").is_err());
    }
}
