use handl_expr::{Span, Token, TokenRule};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;

/// Token rule for canonical keywords.
pub struct Keywords;

impl Keywords {
    /// Creates a new keyword token rule.
    pub fn new() -> Self {
        Keywords
    }
}

fn is_word_boundary(ch: Option<char>) -> bool {
    match ch {
        Some(c) if c.is_alphanumeric() || c == '_' => false,
        _ => true,
    }
}

impl TokenRule for Keywords {
    fn name(&self) -> &str {
        "keywords"
    }

    fn try_lex(&self, input: &str, start: usize) -> Option<Result<Token, PlatformError>> {
        let rest = &input[start..];

        let try_keyword = |text: &str, kind: CanonicalKind| -> Option<Result<Token, PlatformError>> {
            if rest.starts_with(text) && is_word_boundary(rest.chars().nth(text.len())) {
                Some(Ok(Token::new(kind, Span::new(start, start + text.len()))))
            } else {
                None
            }
        };

        try_keyword("handle", CanonicalKind::Handle)
            .or_else(|| try_keyword("typeof", CanonicalKind::TypeOf))
            .or_else(|| try_keyword("as", CanonicalKind::As))
            .or_else(|| try_keyword("is", CanonicalKind::Is))
            .or_else(|| try_keyword("must", CanonicalKind::Must))
            .or_else(|| try_keyword("may", CanonicalKind::May))
            .or_else(|| try_keyword("not", CanonicalKind::Not))
            .or_else(|| try_keyword("when", CanonicalKind::When))
            .or_else(|| try_keyword("else", CanonicalKind::Else))
            .or_else(|| try_keyword("match", CanonicalKind::Match))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_expr::ExpressionPipeline;
    use handl_lexer_canonical::CanonicalKind;

    #[test]
    fn lexes_keywords() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Keywords::new()));

        let tokens = p.lex("handle as is").unwrap();
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, CanonicalKind::Handle);
        assert_eq!(tokens[1].kind, CanonicalKind::As);
        assert_eq!(tokens[2].kind, CanonicalKind::Is);
    }

    #[test]
    fn rejects_prefix_of_identifier() {
        assert!(Keywords::new().try_lex("handlebar", 0).is_none());
        assert!(Keywords::new().try_lex("match", 0).is_some());
    }
}
