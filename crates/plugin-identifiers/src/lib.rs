use handl_expr::{Span, Token, TokenRule};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;

/// Token rule for identifiers.
pub struct Identifiers;

impl Identifiers {
    /// Creates a new identifier token rule.
    pub fn new() -> Self {
        Identifiers
    }
}

impl TokenRule for Identifiers {
    fn name(&self) -> &str {
        "identifiers"
    }

    fn try_lex(&self, input: &str, start: usize) -> Option<Result<Token, PlatformError>> {
        let rest = &input[start..];
        let mut chars = rest.char_indices().peekable();
        let first = chars.peek()?.1;

        if !first.is_alphabetic() && first != '_' {
            return None;
        }

        let mut text = String::new();
        let mut end = start;

        while let Some((pos, ch)) = chars.peek() {
            if !ch.is_alphanumeric() && *ch != '_' {
                break;
            }
            text.push(*ch);
            end = start + pos + ch.len_utf8();
            chars.next();
        }

        Some(Ok(Token::new(CanonicalKind::Identifier(text), Span::new(start, end))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;

    #[test]
    fn lexes_plain_identifier() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Identifiers::new()));

        let tokens = p.lex("add").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("add".to_string()));
        assert_eq!(tokens[0].span, Span::new(0, 3));
    }

    #[test]
    fn lexes_identifier_with_digits_and_underscore() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Identifiers::new()));

        let tokens = p.lex("x1_2").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, CanonicalKind::Identifier("x1_2".to_string()));
    }
}
