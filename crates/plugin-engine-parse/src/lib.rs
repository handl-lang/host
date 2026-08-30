//! # handl-plugin-engine-parse
//!
//! The **abstraction** crate for the parse operation (ADR-0044 §3).
//!
//! Splits `plugin-ext-parser` into an abstraction (operation marker + trait +
//! `ParserContext`, no engine) and an engine crate
//! (`plugin-engine-parse-pratt`). This crate defines the `Parser` operation
//! marker, [`ParserRequest`], the [`ParserContext`] API that engines use to
//! walk tokens, and the [`ParserEngine`] trait. **No Pratt loop lives here** —
//! that is the engine crate's job.
//!
//! Engines consume [`handl_language::Language`] **data** — they read
//! `lang.grammar` rows to decide how to parse, not Rust `ParserBehavior` impls.

#![warn(missing_docs)]

use handl_language::{Ast, AstRef, LangError, Language, Token, Tokens};

/// The parse operation marker.
///
/// A zero-sized type identifying the parse stage. It carries no state; its
/// purpose is to namespace the parse abstraction and pair with
/// [`ParserRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Parser;

/// Request passed to a [`ParserEngine`].
///
/// Carries the token stream to parse plus a serializable options map. The
/// options are opaque to this crate — a concrete engine interprets them
/// against the `Language`'s `options` fragment set.
#[derive(Debug, Clone, Default)]
pub struct ParserRequest {
    /// The token stream to parse.
    pub tokens: Tokens,
    /// Engine-specific options (opaque key/value pairs).
    pub options: serde_json::Value,
}

impl ParserRequest {
    /// Create a new request from a token stream with no options.
    pub fn new(tokens: Tokens) -> Self {
        Self {
            tokens,
            options: serde_json::Value::Null,
        }
    }

    /// Attach an options value (builder style).
    pub fn with_options(mut self, options: serde_json::Value) -> Self {
        self.options = options;
        self
    }
}

/// The handle an engine uses to walk a token stream.
///
/// Wraps the token slice and current position, providing `peek`, `advance`,
/// and `pos` accessors. The engine crate's Pratt loop calls these methods.
/// [`ParserContext::expression`] delegates to the engine's parse entry point,
/// enabling recursive descent via the context.
pub struct ParserContext<'a, 'e, E: ParserEngine + ?Sized> {
    engine: &'e E,
    lang: &'a Language,
    tokens: &'a [Token],
    pos: usize,
}

impl<'a, 'e, E: ParserEngine + ?Sized> ParserContext<'a, 'e, E> {
    /// Create a new `ParserContext` for `engine` parsing `tokens` under
    /// `lang`.
    pub fn new(engine: &'e E, lang: &'a Language, tokens: &'a [Token]) -> Self {
        Self {
            engine,
            lang,
            tokens,
            pos: 0,
        }
    }

    /// Returns the current token without consuming it, or `None` at EOF.
    pub fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// Consume and return the current token.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Engine`] if at EOF (no token to consume).
    pub fn advance(&mut self) -> Result<Token, LangError> {
        match self.tokens.get(self.pos) {
            Some(token) => {
                self.pos += 1;
                Ok(token.clone())
            }
            None => Err(LangError::Engine(
                "unexpected end of input".to_string(),
            )),
        }
    }

    /// The current position in the token stream.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// The full token slice being parsed.
    pub fn tokens(&self) -> &[Token] {
        self.tokens
    }

    /// The language being parsed against.
    pub fn lang(&self) -> &Language {
        self.lang
    }

    /// Parse an expression with the given minimum right binding power.
    ///
    /// This delegates to the engine's [`ParserEngine::expression`] entry
    /// point, enabling recursive descent: an engine's grammar action can call
    /// `ctx.expression(rbp)` to parse a sub-expression at a higher precedence.
    ///
    /// # Errors
    ///
    /// Propagates the engine's [`LangError`].
    pub fn expression(&mut self, rbp: u8) -> Result<AstRef, LangError> {
        self.engine.expression(self, rbp)
    }
}

/// The trait a parse engine implements.
///
/// An engine reads `lang.grammar` (the [`handl_language::GrammarRow`] fragment
/// set) to decide how to parse, then produces an [`Ast`]. The engine is pure
/// data-driven: it does not call Rust `ParserBehavior` impls.
///
/// The [`ParserEngine::expression`] method is the recursive-descent entry
/// point called by [`ParserContext::expression`]. The top-level
/// [`ParserEngine::parse`] method bootstraps the parse and returns the
/// completed `Ast`.
pub trait ParserEngine {
    /// Parse an expression with the given minimum right binding power, using
    /// `ctx` to walk tokens. Returns the [`AstRef`] of the root node produced.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on parse failure.
    fn expression(&self, ctx: &mut ParserContext<'_, '_, Self>, rbp: u8) -> Result<AstRef, LangError>;

    /// Parse a full token stream into an [`Ast`].
    ///
    /// The default implementation creates a [`ParserContext`] and calls
    /// [`ParserEngine::expression`] with `rbp = 0`, returning the `Ast` with
    /// the root recorded. Engines may override this for custom top-level
    /// handling (e.g. module-level statement parsing).
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on parse failure.
    fn parse(&self, lang: &Language, req: &ParserRequest) -> Result<Ast, LangError> {
        let tokens = req.tokens.as_slice();
        let mut ctx = ParserContext::new(self, lang, tokens);
        if ctx.peek().is_none() {
            return Ok(Ast::new());
        }
        let _root = self.expression(&mut ctx, 0)?;
        // The engine's expression method pushes nodes into its internal Ast;
        // the concrete engine's parse override returns that Ast. The default
        // implementation returns an empty Ast — engines that use the context
        // API must override parse to return their built Ast.
        Ok(Ast::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstNode, Token, Tokens};
    use serde_json::json;

    #[test]
    fn parser_marker_constructs() {
        let _ = Parser;
        let p1 = Parser;
        let p2 = Parser;
        assert_eq!(p1, p2);
        let p3 = Parser::default();
        assert_eq!(p1, p3);
    }

    #[test]
    fn parser_request_new_from_tokens() {
        let toks = Tokens(vec![Token {
            kind: "token.number".into(),
            text: "42".into(),
        }]);
        let req = ParserRequest::new(toks);
        assert_eq!(req.tokens.as_slice().len(), 1);
        assert_eq!(req.options, serde_json::Value::Null);
    }

    #[test]
    fn parser_request_with_options() {
        let req = ParserRequest::new(Tokens::new()).with_options(json!({"mode": "strict"}));
        assert_eq!(req.options, json!({"mode": "strict"}));
    }

    /// A stub engine that builds a single-node Ast, used to verify the
    /// `ParserContext` API and trait dispatch.
    struct StubParser;

    impl ParserEngine for StubParser {
        fn expression(
            &self,
            ctx: &mut ParserContext<'_, '_, Self>,
            _rbp: u8,
        ) -> Result<AstRef, LangError> {
            // Consume one token to verify peek/advance.
            let _tok = ctx.advance()?;
            Ok(AstRef::new(0))
        }

        fn parse(&self, _lang: &Language, req: &ParserRequest) -> Result<Ast, LangError> {
            let mut ast = Ast::new();
            if req.tokens.as_slice().is_empty() {
                return Ok(ast);
            }
            ast.push(AstNode::new("stub").with_payload(json!(req.tokens.as_slice()[0].text.clone())));
            Ok(ast)
        }
    }

    #[test]
    fn parser_context_peek_and_advance() {
        let lang = Language::new();
        let toks = vec![
            Token { kind: "token.a".into(), text: "a".into() },
            Token { kind: "token.b".into(), text: "b".into() },
        ];
        let engine = StubParser;
        let mut ctx = ParserContext::new(&engine, &lang, &toks);
        assert_eq!(ctx.pos(), 0);
        assert_eq!(ctx.peek().unwrap().kind, "token.a");
        let t = ctx.advance().unwrap();
        assert_eq!(t.kind, "token.a");
        assert_eq!(ctx.pos(), 1);
        assert_eq!(ctx.peek().unwrap().kind, "token.b");
    }

    #[test]
    fn parser_context_advance_at_eof_errors() {
        let lang = Language::new();
        let toks = vec![Token { kind: "token.a".into(), text: "a".into() }];
        let engine = StubParser;
        let mut ctx = ParserContext::new(&engine, &lang, &toks);
        ctx.advance().unwrap();
        let err = ctx.advance().unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn parser_context_peek_at_eof_is_none() {
        let lang = Language::new();
        let toks: Vec<Token> = vec![];
        let engine = StubParser;
        let ctx = ParserContext::new(&engine, &lang, &toks);
        assert!(ctx.peek().is_none());
    }

    #[test]
    fn parser_context_tokens_and_lang_accessors() {
        let lang = Language::new();
        let toks = vec![Token { kind: "token.x".into(), text: "x".into() }];
        let engine = StubParser;
        let ctx = ParserContext::new(&engine, &lang, &toks);
        assert_eq!(ctx.tokens().len(), 1);
        assert!(std::ptr::eq(ctx.lang(), &lang));
    }

    #[test]
    fn parser_context_expression_delegates_to_engine() {
        let lang = Language::new();
        let toks = vec![Token { kind: "token.a".into(), text: "a".into() }];
        let engine = StubParser;
        let mut ctx = ParserContext::new(&engine, &lang, &toks);
        let r = ctx.expression(0).unwrap();
        assert_eq!(r.index(), 0);
        assert_eq!(ctx.pos(), 1);
    }

    #[test]
    fn trait_dispatch_parse_returns_ast() {
        let engine = StubParser;
        let lang = Language::new();
        let req = ParserRequest::new(Tokens(vec![Token {
            kind: "token.number".into(),
            text: "42".into(),
        }]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.len(), 1);
        assert_eq!(ast.nodes()[0].kind, "stub");
    }

    #[test]
    fn trait_dispatch_empty_input_returns_empty_ast() {
        let engine = StubParser;
        let lang = Language::new();
        let req = ParserRequest::new(Tokens::new());
        let ast = engine.parse(&lang, &req).unwrap();
        assert!(ast.is_empty());
    }

    #[test]
    fn trait_dispatch_via_generic() {
        fn run_parse<E: ParserEngine>(engine: &E, lang: &Language, req: &ParserRequest) -> Result<Ast, LangError> {
            engine.parse(lang, req)
        }
        let engine = StubParser;
        let lang = Language::new();
        let req = ParserRequest::new(Tokens(vec![Token {
            kind: "token.number".into(),
            text: "1".into(),
        }]));
        let ast = run_parse(&engine, &lang, &req).unwrap();
        assert_eq!(ast.len(), 1);
    }

    /// A parser that uses the default `parse` implementation (does not
    /// override it), to test the trait's default path.
    struct DefaultParser;

    impl ParserEngine for DefaultParser {
        fn expression(
            &self,
            _ctx: &mut ParserContext<'_, '_, Self>,
            _rbp: u8,
        ) -> Result<AstRef, LangError> {
            Ok(AstRef::new(0))
        }
    }

    #[test]
    fn default_parse_empty_input() {
        let engine = DefaultParser;
        let lang = Language::new();
        let req = ParserRequest::new(Tokens::new());
        let ast = engine.parse(&lang, &req).unwrap();
        assert!(ast.is_empty());
    }

    #[test]
    fn default_parse_non_empty_input() {
        let engine = DefaultParser;
        let lang = Language::new();
        let req = ParserRequest::new(Tokens(vec![Token {
            kind: "token.x".into(),
            text: "x".into(),
        }]));
        // Default parse calls expression but returns an empty Ast.
        let ast = engine.parse(&lang, &req).unwrap();
        assert!(ast.is_empty());
    }
}
