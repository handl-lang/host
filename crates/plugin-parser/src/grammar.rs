//! Grammar and grammar-rule abstractions for handl parsers.
//!
//! A [`Grammar`] is a collection of [`GrammarRule`] plugins.  A concrete
//! parser engine (for example the one in `handl-plugin-parser`) uses the
//! [`ParserContext`] interface to ask the current rule for a `nud` (null
//! denotation) or `led` (left denotation) action.

use crate::ParseError;
use handl_plugin_ast::Expr;
use handl_plugin_lexer::{Token, TokenKind};

/// Context passed to grammar rules while parsing.
///
/// Implementing parser engines provide a concrete `ParserContext` that
/// can inspect the token stream, advance, match expected tokens, and
/// recursively parse expressions.
pub trait ParserContext<K, N>
where
    K: TokenKind,
    N: Expr,
{
    /// The tokens being parsed.
    fn tokens(&self) -> &[Token<K>];

    /// The source string.
    fn source(&self) -> &str;

    /// The current position in the token stream.
    fn pos(&self) -> usize;

    /// Returns `true` if all tokens have been consumed.
    fn is_at_end(&self) -> bool;

    /// Returns the current token without consuming it.
    fn peek(&self) -> Option<&Token<K>>;

    /// Returns the kind of the current token, if any.
    fn peek_kind(&self) -> Option<K>;

    /// Consume and return the current token.
    fn advance(&mut self) -> Result<&Token<K>, ParseError>;

    /// Consume the current token only if it matches `kind`.
    fn match_kind(&mut self, kind: K) -> Option<&Token<K>>;

    /// Consume the current token, failing if it does not match `kind`.
    fn expect(&mut self, kind: K, label: &str) -> Result<&Token<K>, ParseError>;

    /// Parse an expression with the given minimum binding power.
    fn expression(&mut self, min_bp: u32) -> Result<N, ParseError>;
}

/// A grammar rule plugin.
///
/// Implementors provide one or more of:
/// - a *nud* (prefix / null denotation) action, via `matches_prefix` and `prefix`,
/// - an *led* (infix / left denotation) action, via `infix_binding` and `infix`.
pub trait GrammarRule<K, N>
where
    K: TokenKind,
    N: Expr,
{
    /// Returns `true` if this rule knows how to parse `kind` as a prefix expression.
    fn matches_prefix(&self, kind: &K) -> bool;

    /// Parse a prefix expression starting with `token`.
    fn prefix(&self, token: Token<K>, ctx: &mut dyn ParserContext<K, N>) -> Result<N, ParseError>;

    /// Returns the left and right binding powers for `kind` as an infix operator, if any.
    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)>;

    /// Parse an infix expression from `left`, the `token` operator, and the right side.
    fn infix(
        &self,
        left: N,
        token: Token<K>,
        ctx: &mut dyn ParserContext<K, N>,
        rbp: u32,
    ) -> Result<N, ParseError>;
}

/// A `Grammar` answers the engine's questions about the current token
/// and drives top-level parsing.
pub trait Grammar<K, N>
where
    K: TokenKind,
    N: Expr,
{
    /// Find the rule responsible for `kind`, if any.
    fn rule_for(&self, kind: &K) -> Option<&dyn GrammarRule<K, N>>;

    /// Find the prefix rule for `kind`, if any.
    fn rule_for_prefix(&self, kind: &K) -> Option<&dyn GrammarRule<K, N>> {
        self.rule_for(kind)
    }

    /// Find the infix rule for `kind`, if any.
    fn rule_for_infix(&self, kind: &K) -> Option<&dyn GrammarRule<K, N>> {
        self.rule_for(kind)
    }

    /// The end-of-input token, if the grammar requires one.
    fn eof(&self) -> Option<K>;

    /// Parse the root of the input using the supplied context.
    fn parse_root(&self, ctx: &mut dyn ParserContext<K, N>) -> Result<N, ParseError>;
}

/// A simple grammar that collects [`GrammarRule`] plugins.
///
/// Plugins are registered with [`CompositeGrammar::register`].  Rules are
/// consulted in registration order when the parser engine asks for the
/// rule for a token.
pub struct CompositeGrammar<K, N>
where
    K: TokenKind,
    N: Expr,
{
    rules: Vec<Box<dyn GrammarRule<K, N>>>,
    eof: Option<K>,
}

impl<K, N> CompositeGrammar<K, N>
where
    K: TokenKind,
    N: Expr,
{
    /// Create an empty grammar.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            eof: None,
        }
    }

    /// Register a grammar rule plugin.
    pub fn register<R>(mut self, rule: R) -> Self
    where
        R: GrammarRule<K, N> + 'static,
    {
        self.rules.push(Box::new(rule));
        self
    }

    /// Set the end-of-input token.
    pub fn eof(mut self, kind: K) -> Self {
        self.eof = Some(kind);
        self
    }
}

impl<K, N> Default for CompositeGrammar<K, N>
where
    K: TokenKind,
    N: Expr,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, N> Grammar<K, N> for CompositeGrammar<K, N>
where
    K: TokenKind,
    N: Expr,
{
    fn rule_for(&self, kind: &K) -> Option<&dyn GrammarRule<K, N>> {
        self.rules
            .iter()
            .find(|r| r.matches_prefix(kind) || r.infix_binding(kind).is_some())
            .map(|r| r.as_ref())
    }

    fn rule_for_prefix(&self, kind: &K) -> Option<&dyn GrammarRule<K, N>> {
        self.rules
            .iter()
            .find(|r| r.matches_prefix(kind))
            .map(|r| r.as_ref())
    }

    fn rule_for_infix(&self, kind: &K) -> Option<&dyn GrammarRule<K, N>> {
        self.rules
            .iter()
            .find(|r| r.infix_binding(kind).is_some())
            .map(|r| r.as_ref())
    }

    fn eof(&self) -> Option<K> {
        self.eof.clone()
    }

    fn parse_root(&self, ctx: &mut dyn ParserContext<K, N>) -> Result<N, ParseError> {
        ctx.expression(0)
    }
}
