use handl_expr::{Span, Token, TokenRule};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;

/// Token rule for the null literal.
pub struct Null;

const NULL_FIRST: &'static [char] = &['n'];

impl Null {
    /// Creates a new null token rule.
    pub fn new() -> Self {
        Null
    }
}

fn is_word_boundary(ch: Option<char>) -> bool {
    match ch {
        Some(c) if c.is_alphanumeric() || c == '_' => false,
        _ => true,
    }
}

impl TokenRule for Null {
    fn name(&self) -> &str {
        "null"
    }

    fn first_chars(&self) -> &'static [char] {
        NULL_FIRST
    }

    fn try_lex(&self, input: &str, start: usize) -> Option<Result<Token, PlatformError>> {
        let rest = &input[start..];

        if rest.starts_with("null") && is_word_boundary(rest.chars().nth(4)) {
            return Some(Ok(Token::new(CanonicalKind::Null, Span::new(start, start + 4))));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;

    #[test]
    fn lexes_null() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Null::new()));

        let tokens = p.lex("null").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, CanonicalKind::Null);
        assert_eq!(tokens[0].span, Span::new(0, 4));
    }

    #[test]
    fn rejects_nullish_identifier() {
        assert!(Null::new().try_lex("nullify", 0).is_none());
    }
}
