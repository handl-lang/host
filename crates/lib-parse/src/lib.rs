//! # handl-lib-parse
//!
//! Generic, tree-shakeable parsing framework for the handl runtime.
//!
//! Mirrors `handl-lib-serialize` but for one-to-one string-to-value parsing.

/// Shared error for all parser failures.
#[derive(Debug, thiserror::Error)]
#[error("parse error: {reason}")]
pub struct ParseError {
    /// Human-readable description of the failure.
    pub reason: String,
}

impl ParseError {
    /// Create a new parse error.
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

/// A parser that converts an `input` of type `I` into an output of type `O`.
pub trait Parser<I: ?Sized, O> {
    /// Parse `input` into `O`.
    fn parse(&self, input: &I) -> Result<O, ParseError>;
}

/// Parse `input` using the given `parser`.
pub fn parse<I: ?Sized, O, P: Parser<I, O>>(input: &I, parser: P) -> Result<O, ParseError> {
    parser.parse(input)
}
