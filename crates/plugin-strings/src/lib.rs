use handl_expr::{Span, Token, TokenRule};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;

/// Token rule for string literals.
pub struct Strings;

const STRINGS_FIRST: &'static [char] = &['"', '\''];

impl Strings {
    /// Creates a new string token rule.
    pub fn new() -> Self {
        Strings
    }
}

impl TokenRule for Strings {
    fn name(&self) -> &str {
        "strings"
    }

    fn first_chars(&self) -> &'static [char] {
        STRINGS_FIRST
    }

    fn try_lex(&self, input: &str, start: usize) -> Option<Result<Token, PlatformError>> {
        let rest = &input[start..];
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }

        let quote_len = quote.len_utf8();
        let body = &rest[quote_len..];
        let mut value = String::new();

        for (pos, ch) in body.char_indices() {
            if ch == quote {
                let end = start + quote_len + pos + quote_len;
                return Some(Ok(Token::new(CanonicalKind::String(value), Span::new(start, end))));
            }
            value.push(ch);
        }

        Some(Err(PlatformError::Plugin("unterminated string".to_string())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;

    #[test]
    fn lexes_double_and_single_quotes() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Strings::new()));

        let tokens = p.lex("\"hello\" 'world'").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, CanonicalKind::String("hello".to_string()));
        assert_eq!(tokens[0].span, Span::new(0, 7));
        assert_eq!(tokens[1].kind, CanonicalKind::String("world".to_string()));
        assert_eq!(tokens[1].span, Span::new(8, 15));
    }

    #[test]
    fn reports_unterminated() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Strings::new()));
        assert!(p.lex("\"open").is_err());
    }
}
