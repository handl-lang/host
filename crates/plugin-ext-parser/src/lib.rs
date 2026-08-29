//! Generic Pratt parser rule-acceptor engine for the `handl-platform` `Parser`
//! operation.
//!
//! `BasicParser` is a `RuleAcceptor` that collects [`ParserBehavior`] rules and
//! runs a classic top-down operator-precedence (Pratt) parser over a token
//! stream.  It is intentionally agnostic about the concrete token kind and
//! grammar, so each `plugin-grammar-*` rule can be migrated to the platform
//! `Behavior`/`RuleAcceptor` model independently.

use std::marker::PhantomData;

use handl_platform::{
    AstNode, Behavior, CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse,
    Contribution, Operation, OptionSchema, PlatformError, Plugin, PluginContext, ResolvedOptions,
    RuleAcceptor,
};
use handl_plugin_lexer::{Token, TokenKind};

/// `Parser` operation marker, generic over the concrete token kind `K`.
#[derive(Debug, Clone, Copy)]
pub struct Parser<K: TokenKind>(PhantomData<K>);

impl<K: TokenKind> Operation for Parser<K> {
    type Request = ParserRequest<K>;
}

/// Request passed to `Parser` behaviors.
#[derive(Debug, Clone)]
pub struct ParserRequest<K: TokenKind> {
    /// Tokens to parse.
    pub tokens: Vec<Token<K>>,
    /// Current parse position inside `tokens`.
    pub pos: usize,
    /// Resolved parser options.
    pub options: ResolvedOptions,
    /// Output AST node (behaviors may set this).
    pub ast: AstNode,
}

impl<K: TokenKind> ParserRequest<K> {
    /// Create a new `ParserRequest` from a token stream.
    pub fn new(tokens: Vec<Token<K>>) -> Self {
        Self {
            tokens,
            pos: 0,
            options: ResolvedOptions::new(),
            ast: AstNode {
                kind: "parser.root".into(),
                span: None,
                payload: serde_json::Value::Null,
            },
        }
    }
}

/// Context passed to grammar rules while parsing.
///
/// Wraps the token stream and the current position, and provides the entry
/// point into the Pratt parser loop via [`ParserContext::expression`].
pub struct ParserContext<'a, K: TokenKind> {
    parser: &'a BasicParser<K>,
    tokens: &'a [Token<K>],
    pos: usize,
    options: ResolvedOptions,
}

impl<'a, K: TokenKind> ParserContext<'a, K> {
    /// Create a new `ParserContext` for `parser` over `tokens`.
    pub fn new(
        parser: &'a BasicParser<K>,
        tokens: &'a [Token<K>],
        options: ResolvedOptions,
    ) -> Self {
        Self {
            parser,
            tokens,
            pos: 0,
            options,
        }
    }

    /// Returns the current token without consuming it.
    pub fn peek(&self) -> Option<&Token<K>> {
        self.tokens.get(self.pos)
    }

    /// Consume and return the current token.
    pub fn advance(&mut self) -> Result<Token<K>, PlatformError> {
        match self.tokens.get(self.pos) {
            Some(token) => {
                self.pos += 1;
                Ok(token.clone())
            }
            None => Err(PlatformError::Plugin("unexpected end of input".into())),
        }
    }

    /// Parse an expression with the given minimum right binding power.
    pub fn expression(&mut self, rbp: u32) -> Result<AstNode, PlatformError> {
        self.parser.expression(self, rbp)
    }

    /// The tokens being parsed.
    pub fn tokens(&self) -> &[Token<K>] {
        self.tokens
    }

    /// The current position in the token stream.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Resolved parser options.
    pub fn options(&self) -> &ResolvedOptions {
        &self.options
    }
}

/// Behavior trait for a grammar rule that can participate in the Pratt parser.
///
/// Rules may implement prefix (null denotation) and/or infix (left denotation)
/// actions.  They also implement [`Behavior<Parser<K>>`], which means each rule
/// must provide `kind()` and `apply()`.  The `apply` implementation is usually
/// a no-op for parser grammar rules; the real work is done through the Pratt
/// callbacks.
pub trait ParserBehavior<K: TokenKind>: Behavior<Parser<K>> {
    /// Returns `true` if this rule knows how to parse `kind` as a prefix
    /// expression.
    fn matches_prefix(&self, kind: &K) -> bool;

    /// Parse a prefix expression starting with `token`.
    fn prefix(
        &self,
        token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError>;

    /// Returns the left and right binding powers for `kind` as an infix
    /// operator, if any.
    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)>;

    /// Parse an infix expression from `left`, the `token` operator, and the
    /// right side.  `rbp` is the right binding power to use when parsing the
    /// right operand.
    fn infix(
        &self,
        left: AstNode,
        token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
        rbp: u32,
    ) -> Result<AstNode, PlatformError>;
}

impl<K: TokenKind> Contribution for Box<dyn ParserBehavior<K>> {
    fn id(&self) -> &str {
        self.kind()
    }
}

/// Basic `RuleAcceptor` plugin for the `Parser` operation.
pub struct BasicParser<K: TokenKind> {
    /// Capabilities provided by this plugin.
    capabilities: Vec<CapabilityId>,
    /// Registered parsing rules.
    pub rules: Vec<Box<dyn ParserBehavior<K>>>,
    /// Option schemas contributed for the parser.
    pub option_schemas: Vec<OptionSchema>,
    _marker: PhantomData<K>,
}

impl<K: TokenKind> BasicParser<K> {
    /// Create a new `BasicParser` with no rules.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from("parse")],
            rules: Vec::new(),
            option_schemas: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Parse a token stream into a single [`AstNode`] using the registered
    /// grammar rules.
    pub fn parse(&self, tokens: &[Token<K>]) -> Result<AstNode, PlatformError> {
        let mut ctx = ParserContext::new(self, tokens, ResolvedOptions::new());
        ctx.expression(0)
    }

    /// Parse a token stream into a sequence of top-level [`AstNode`]s (a
    /// module). Each statement is parsed by calling `expression(0)`; the
    /// Pratt loop stops at EOF or when no infix rule matches the next token.
    pub fn parse_module(&self, tokens: &[Token<K>]) -> Result<Vec<AstNode>, PlatformError> {
        let mut ctx = ParserContext::new(self, tokens, ResolvedOptions::new());
        let mut stmts = Vec::new();
        while let Some(t) = ctx.peek() {
            if t.kind.is_eof() {
                break;
            }
            let stmt = ctx.expression(0)?;
            stmts.push(stmt);
        }
        Ok(stmts)
    }

    fn rule_for_prefix(&self, kind: &K) -> Option<&dyn ParserBehavior<K>> {
        self.rules
            .iter()
            .find(|r| r.matches_prefix(kind))
            .map(|r| r.as_ref())
    }

    fn rule_for_infix(&self, kind: &K) -> Option<&dyn ParserBehavior<K>> {
        self.rules
            .iter()
            .find(|r| r.infix_binding(kind).is_some())
            .map(|r| r.as_ref())
    }

    fn expression(
        &self,
        ctx: &mut ParserContext<'_, K>,
        rbp: u32,
    ) -> Result<AstNode, PlatformError> {
        let token = ctx.advance()?;
        let rule = self
            .rule_for_prefix(&token.kind)
            .ok_or_else(|| PlatformError::Plugin(format!("unexpected token: {}", token.kind)))?;

        let mut left = rule.prefix(token, ctx)?;

        while let Some(t) = ctx.peek() {
            if t.kind.is_eof() {
                break;
            }

            let rule = match self.rule_for_infix(&t.kind) {
                Some(rule) => rule,
                None => break,
            };

            let (lbp, rbp2) = rule
                .infix_binding(&t.kind)
                .expect("rule_for_infix returned a rule without a binding");

            if lbp < rbp {
                break;
            }

            let op = ctx.advance()?;
            left = rule.infix(left, op, ctx, rbp2)?;
        }

        Ok(left)
    }
}

impl<K: TokenKind> Default for BasicParser<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: TokenKind> Plugin for BasicParser<K> {
    fn id(&self) -> &str {
        "parser.basic"
    }

    fn name(&self) -> &str {
        "Basic parser"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        (capability.as_str() == "parse").then(|| {
            serde_json::json!({
                "command": "parse",
                "description": "Parse a token stream into an AST"
            })
        })
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != "parse" {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }

        Err(PlatformError::NotImplemented)
    }
}

impl<K: TokenKind> RuleAcceptor for BasicParser<K> {
    type Rule = Box<dyn ParserBehavior<K>>;

    fn accept(&mut self, rule: Self::Rule) -> Result<(), PlatformError> {
        self.rules.push(rule);
        Ok(())
    }
}
