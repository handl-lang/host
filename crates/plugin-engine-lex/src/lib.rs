//! # handl-plugin-engine-lex
//!
//! The **abstraction** crate for the lex operation (ADR-0044 §3).
//!
//! Splits `plugin-ext-lexer` into an abstraction (operation marker + trait,
//! no engine) and a future engine crate. This crate defines the `Lexer`
//! operation marker, [`LexerRequest`], and the [`LexerEngine`] trait that
//! engines implement. **No engine code lives here** — the abstraction is the
//! contract; a future `plugin-engine-lex-logos` (or the lexer half of
//! `plugin-engine-parse-pratt`) implements it.
//!
//! Engines consume [`handl_language::Language`] **data** — they read
//! `lang.tokens` to decide how to tokenize, not Rust `Behavior` impls. This is
//! the defining property of "language as data" (ADR-0044 §1).

#![warn(missing_docs)]

use handl_language::{LangError, Language, Source, Tokens};

/// The lex operation marker.
///
/// A zero-sized type identifying the lex stage. It carries no state; its
/// purpose is to be the `Operation` type that pairs with [`LexerRequest`] and
/// to serve as a namespace for the lex abstraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Lexer;

/// Request passed to a [`LexerEngine`].
///
/// Carries the source text to tokenize plus a serializable options map. The
/// options are opaque to this crate — a concrete engine interprets them
/// against the `Language`'s `options` fragment set.
#[derive(Debug, Clone, Default)]
pub struct LexerRequest {
    /// The source text to lex.
    pub source: Source,
    /// Engine-specific options (opaque key/value pairs).
    pub options: serde_json::Value,
}

impl LexerRequest {
    /// Create a new request from a source string with no options.
    pub fn new(source: impl Into<Source>) -> Self {
        Self {
            source: source.into(),
            options: serde_json::Value::Null,
        }
    }

    /// Attach an options value (builder style).
    pub fn with_options(mut self, options: serde_json::Value) -> Self {
        self.options = options;
        self
    }
}

/// The trait a lex engine implements.
///
/// An engine reads `lang.tokens` (the [`handl_language::TokenKindDef`] fragment
/// set) to decide which token kinds exist and how to recognize them, then
/// produces a [`Tokens`] stream. The engine is pure data-driven: it does not
/// call Rust `Behavior` impls.
///
/// # Errors
///
/// Returns [`LangError`] on lexing failure (e.g. an unrecognized character).
pub trait LexerEngine {
    /// Lex `req.source` into a token stream using the language's token
    /// definitions.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on failure.
    fn lex(&self, lang: &Language, req: &LexerRequest) -> Result<Tokens, LangError>;

    /// Produce an owned boxed clone of this engine.
    ///
    /// Used by the registry-based runtime (`handl-runtime`) to obtain an
    /// owned engine from a `&dyn LexerEngine` lent by the `EngineRegistry`
    /// (ADR-0047 §2).
    fn box_clone(&self) -> Box<dyn LexerEngine>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::Source;
    use serde_json::json;

    #[test]
    fn lexer_marker_constructs() {
        let _ = Lexer;
        let l1 = Lexer;
        let l2 = Lexer;
        assert_eq!(l1, l2);
        let l3 = Lexer::default();
        assert_eq!(l1, l3);
    }

    #[test]
    fn lexer_request_new_from_str() {
        let req = LexerRequest::new("1 + 2");
        assert_eq!(req.source.as_str(), "1 + 2");
        assert_eq!(req.options, serde_json::Value::Null);
    }

    #[test]
    fn lexer_request_new_from_source() {
        let src = Source::new("hello");
        let req = LexerRequest::new(src);
        assert_eq!(req.source.as_str(), "hello");
    }

    #[test]
    fn lexer_request_with_options() {
        let req = LexerRequest::new("x").with_options(json!({"skip_ws": true}));
        assert_eq!(req.options, json!({"skip_ws": true}));
    }

    /// A stub engine that returns `NotImplemented`, used to verify trait
    /// dispatch shape.
    #[derive(Clone)]
    struct StubLexer;

    impl LexerEngine for StubLexer {
        fn lex(&self, _lang: &Language, _req: &LexerRequest) -> Result<Tokens, LangError> {
            Err(LangError::NotImplemented)
        }

        fn box_clone(&self) -> Box<dyn LexerEngine> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn trait_dispatch_returns_engine_result() {
        let engine = StubLexer;
        let lang = Language::new();
        let req = LexerRequest::new("1 + 2");
        let result = engine.lex(&lang, &req);
        assert_eq!(result.unwrap_err(), LangError::NotImplemented);
    }

    #[test]
    fn trait_dispatch_via_dyn_reference() {
        let engine: Box<dyn LexerEngine> = Box::new(StubLexer);
        let lang = Language::new();
        let req = LexerRequest::new("test");
        let result = engine.lex(&lang, &req);
        assert!(matches!(result, Err(LangError::NotImplemented)));
    }
}
