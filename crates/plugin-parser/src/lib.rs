//! # handl-plugin-parser
//!
//! Parsing abstraction for handl expressions and modules.
//!
//! Defines the generic [`Parser`] trait and the [`ParseError`] type.
//! Concrete parser engines live in `plugin-ext-parser` and the
//! `plugin-grammar-*` crates implement `ParserBehavior` rules.
//!
//! ## Architectural rule
//!
//! > **The parser consumes tokens and produces generic [`Node`](handl_plugin_ast::Node)s.**
//!
//! The parser is the bridge between the lexer and the AST. It depends on
//! `handl-plugin-lexer` (for `Token`/`TokenKind`) and `handl-plugin-ast` (for the
//! `Node`/`Expr` capabilities), but not on the VM or any concrete node
//! type. This keeps the parsing pipeline independent of evaluation
//! strategy.

use handl_plugin_ast::Node;
use handl_plugin_lexer::{LexError, Lexer, Token, TokenKind};

mod error;

pub use error::ParseError;

/// A parser that converts a token stream into an AST node.
///
/// `K` is the concrete token kind, `N` is the node type the parser
/// produces, and `E` is the lexer error type.
pub trait Parser<K, N, E>: Send + Sync
where
    K: TokenKind,
    N: Node,
    E: LexError,
{
    /// Parse a token stream into a node.
    fn parse(&self, tokens: Vec<Token<K>>) -> Result<N, ParseError>;

    /// Convenience: lex and parse in one call.
    fn parse_text(&self, input: &str, lexer: &dyn Lexer<K, E>) -> Result<N, ParseError> {
        let tokens = lexer.lex(input).map_err(|e| ParseError::LexError {
            message: e.to_string(),
        })?;
        self.parse(tokens)
    }
}
