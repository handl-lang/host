use handl_lexer_canonical::CanonicalKind;

/// Alias for a token carrying the canonical token kind.
pub type Token = handl_lexer::Token<CanonicalKind>;

/// Alias for a source span.
pub type Span = handl_lexer::Span;
