use handl_expr::{Span, Token, TokenRule};
use handl_lexer_canonical::CanonicalKind;
use handl_platform::PlatformError;

/// Token rule for multi-character operators.
pub struct Operators;

const OPERATORS_FIRST: &'static [char] = &['|', '<', '>', '&', '?', '.', '-'];

impl Operators {
    /// Creates a new operators token rule.
    pub fn new() -> Self {
        Operators
    }
}

const OPERATORS: &[(&str, CanonicalKind)] = &[
    ("|||", CanonicalKind::Parallel),
    ("&&&", CanonicalKind::Fanout),
    (">>>", CanonicalKind::Flow),
    ("<<<", CanonicalKind::Compose),
    ("...", CanonicalKind::Spread),
    ("..", CanonicalKind::Range),
    ("|>", CanonicalKind::Pipe),
    ("<|", CanonicalKind::BackwardPipe),
    ("->", CanonicalKind::Arrow),
];

impl TokenRule for Operators {
    fn name(&self) -> &str {
        "operators"
    }

    fn first_chars(&self) -> &'static [char] {
        OPERATORS_FIRST
    }

    fn try_lex(&self, input: &str, start: usize) -> Option<Result<Token, PlatformError>> {
        let rest = &input[start..];

        for (text, kind) in OPERATORS {
            if rest.starts_with(text) {
                let end = start + text.len();
                return Some(Ok(Token::new(kind.clone(), Span::new(start, end))));
            }
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
    fn lexes_operators() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Operators::new()));

        let tokens = p.lex("|> <| >>> <<< &&& ||| .. ... ->").unwrap();
        assert_eq!(tokens.len(), 9);
        assert_eq!(tokens[0].kind, CanonicalKind::Pipe);
        assert_eq!(tokens[1].kind, CanonicalKind::BackwardPipe);
        assert_eq!(tokens[8].kind, CanonicalKind::Arrow);
    }

    #[test]
    fn prefers_longest_operator() {
        let mut p = ExpressionPipeline::new();
        p.register_token_rule(Box::new(Operators::new()));

        let tokens = p.lex("...").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, CanonicalKind::Spread);
    }
}
