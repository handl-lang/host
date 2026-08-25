use handl_expr::{Span, Token, TokenRule};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;

/// Token rule for boolean literals.
pub struct Booleans;

const BOOLEANS_FIRST: &'static [char] = &['t', 'f'];

impl Booleans {
    /// Creates a new boolean token rule.
    pub fn new() -> Self {
        Booleans
    }
}

fn is_word_boundary(ch: Option<char>) -> bool {
    match ch {
        Some(c) if c.is_alphanumeric() || c == '_' => false,
        _ => true,
    }
}

impl TokenRule for Booleans {
    fn name(&self) -> &str {
        "booleans"
    }

    fn first_chars(&self) -> &'static [char] {
        BOOLEANS_FIRST
    }

    fn try_lex(&self, input: &str, start: usize) -> Option<Result<Token, PlatformError>> {
        let rest = &input[start..];

        if rest.starts_with("true") && is_word_boundary(rest.chars().nth(4)) {
            return Some(Ok(Token::new(CanonicalKind::Bool(true), Span::new(start, start + 4))));
        }

        if rest.starts_with("false") && is_word_boundary(rest.chars().nth(5)) {
            return Some(Ok(Token::new(
                CanonicalKind::Bool(false),
                Span::new(start, start + 5),
            )));
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
    fn lexes_booleans() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Booleans::new()));

        let tokens = p.lex("true false").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, CanonicalKind::Bool(true));
        assert_eq!(tokens[1].kind, CanonicalKind::Bool(false));
    }

    #[test]
    fn rejects_prefix_of_identifier() {
        assert!(Booleans::new().try_lex("truest", 0).is_none());
        assert!(Booleans::new().try_lex("true", 0).is_some());
    }
}
