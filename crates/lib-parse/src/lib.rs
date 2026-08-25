//! # handl-lib-parse
//!
//! Generic, tree-shakeable parsing framework for the handl runtime.
//!
//! Mirrors `handl-lib-serialize` but for one-to-one string-to-value parsing.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

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

/// A parser that converts a string into a `serde_json::Value`.
pub trait DynParser: Send + Sync {
    /// Parse `input` into a `Value`.
    fn parse(&self, input: &str) -> Result<Value, ParseError>;
}

/// A registry of named dynamic parsers.
#[derive(Default)]
pub struct ParserRegistry {
    parsers: HashMap<String, Arc<dyn DynParser>>,
}

impl ParserRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
        }
    }

    /// Registers a parser under a name.
    pub fn with(mut self, name: impl Into<String>, parser: impl DynParser + 'static) -> Self {
        self.parsers.insert(name.into(), Arc::new(parser));
        self
    }

    /// Returns the parser registered under `name`, if any.
    pub fn get(&self, name: &str) -> Option<&dyn DynParser> {
        self.parsers.get(name).map(|p| p.as_ref())
    }
}
