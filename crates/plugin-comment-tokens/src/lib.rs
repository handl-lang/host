use handl_expr::{Span, Token, TokenRule};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;

/// Token rule that emits comment tokens.
pub struct CommentTokens;

const COMMENT_TOKENS_FIRST: &'static [char] = &['/'];

impl CommentTokens {
    /// Creates a new comment token rule.
    pub fn new() -> Self {
        CommentTokens
    }
}

impl TokenRule for CommentTokens {
    fn name(&self) -> &str {
        "comment-tokens"
    }

    fn first_chars(&self) -> &'static [char] {
        COMMENT_TOKENS_FIRST
    }

    fn try_lex(&self, input: &str, start: usize) -> Option<Result<Token, PlatformError>> {
        let rest = &input[start..];

        if rest.starts_with("//") {
            let text_start = start + 2;
            let end = input[text_start..].find('\n').map_or(input.len(), |n| text_start + n);
            let text = input[text_start..end].to_string();
            return Some(Ok(Token::new(CanonicalKind::Comment(text), Span::new(start, end))));
        }

        if rest.starts_with("/*") {
            let text_start = start + 2;
            let body = &input[text_start..];
            if let Some(n) = body.find("*/") {
                let text = body[..n].to_string();
                let end = text_start + n + 2;
                return Some(Ok(Token::new(CanonicalKind::Comment(text), Span::new(start, end))));
            }
            return Some(Err(PlatformError::Plugin(
                "unterminated block comment".to_string(),
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
    fn lexes_line_and_block_comments() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(CommentTokens::new()));

        let tokens = p.lex("// hello\n/* world */").unwrap();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind, CanonicalKind::Comment(" hello".to_string()));
        assert_eq!(tokens[0].span, Span::new(0, 8));
        assert_eq!(tokens[1].kind, CanonicalKind::Comment(" world ".to_string()));
        assert_eq!(tokens[1].span, Span::new(9, 20));
    }

    #[test]
    fn reports_unterminated_block_comment() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(CommentTokens::new()));
        assert!(p.lex("/* open").is_err());
    }
}
