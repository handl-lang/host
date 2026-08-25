use handl_expr::{Span, Token, TokenRule};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;

/// Token rule for single-character delimiters.
pub struct Delimiters;

const DELIMITERS_FIRST: &'static [char] = &[
    '(', ')', '{', '}', '[', ']', ',', ':', ';', '=', '.', '<', '>', '?',
];

impl Delimiters {
    /// Creates a new delimiters token rule.
    pub fn new() -> Self {
        Delimiters
    }
}

impl TokenRule for Delimiters {
    fn name(&self) -> &str {
        "delimiters"
    }

    fn first_chars(&self) -> &'static [char] {
        DELIMITERS_FIRST
    }

    fn try_lex(&self, input: &str, start: usize) -> Option<Result<Token, PlatformError>> {
        let rest = &input[start..];
        let ch = rest.chars().next()?;

        let kind = match ch {
            '(' => CanonicalKind::CallOpen,
            ')' => CanonicalKind::CallClose,
            '{' => CanonicalKind::BlockOpen,
            '}' => CanonicalKind::BlockClose,
            '[' => CanonicalKind::BracketOpen,
            ']' => CanonicalKind::BracketClose,
            ',' => CanonicalKind::Comma,
            ':' => CanonicalKind::Colon,
            ';' => CanonicalKind::Semicolon,
            '=' => CanonicalKind::Equal,
            '.' => CanonicalKind::Dot,
            '<' => CanonicalKind::LessThan,
            '>' => CanonicalKind::GreaterThan,
            '?' => CanonicalKind::Question,
            _ => return None,
        };

        let end = start + ch.len_utf8();
        Some(Ok(Token::new(kind, Span::new(start, end))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;

    #[test]
    fn lexes_delimiters() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Delimiters::new()));

        let tokens = p.lex("(){}[]:,.<>?").unwrap();
        assert_eq!(tokens.len(), 12);
        assert_eq!(tokens[0].kind, CanonicalKind::CallOpen);
        assert_eq!(tokens[1].kind, CanonicalKind::CallClose);
        assert_eq!(tokens[11].kind, CanonicalKind::Question);
    }

    #[test]
    fn rejects_non_delimiter() {
        assert!(Delimiters::new().try_lex("abc", 0).is_none());
    }
}
