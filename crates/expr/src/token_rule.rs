use crate::token::Token;

/// A plugin-provided rule for turning source text into a token.
pub trait TokenRule: Send + Sync {
    /// Returns the rule name for diagnostics.
    fn name(&self) -> &str;

    /// Returns the characters this rule can start a token with.
    ///
    /// An empty slice means the rule should be tried at every position
    /// as a fallback.
    fn first_chars(&self) -> &'static [char] {
        &[]
    }

    /// Attempts to lex a token at `start` in `input`.
    ///
    /// Returns `None` if this rule does not match, or `Some(Ok(token))`
    /// / `Some(Err(...))` if it matched successfully or failed.
    fn try_lex(
        &self,
        input: &str,
        start: usize,
    ) -> Option<Result<Token, handl_platform::PlatformError>>;
}
