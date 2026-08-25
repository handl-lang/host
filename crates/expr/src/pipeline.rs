use crate::context::ParseContext;
use crate::expr_fragment::{InfixFragment, PostfixFragment, PrimaryFragment};
use crate::token::Token;
use crate::token_rule::TokenRule;
use crate::diagnostics::Diagnostic;
use handl_ast_canonical::Expr;
use handl_lexer_canonical::CanonicalKind;
use handl_vm_values_json::NonFunctionValue;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::mem::Discriminant;

/// A parsed value together with any diagnostics produced during recovery.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseResult<T> {
    pub value: T,
    pub diagnostics: Vec<Diagnostic>,
}

/// A pluggable expression pipeline: token rules feed a Pratt-style parser.
pub struct ExpressionPipeline {
    token_rules: Vec<Box<dyn TokenRule>>,
    token_by_char: HashMap<char, Vec<usize>>,
    token_fallbacks: Vec<usize>,

    primaries: Vec<Box<dyn PrimaryFragment>>,
    primary_by_kind: HashMap<Discriminant<CanonicalKind>, Vec<usize>>,
    primary_fallbacks: Vec<usize>,

    postfixes: Vec<Box<dyn PostfixFragment>>,
    postfix_by_kind: HashMap<Discriminant<CanonicalKind>, Vec<usize>>,
    postfix_fallbacks: Vec<usize>,

    infixes: Vec<Box<dyn InfixFragment>>,
    infix_by_kind: HashMap<Discriminant<CanonicalKind>, Vec<usize>>,
    infix_fallbacks: Vec<usize>,
}

impl Default for ExpressionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl ExpressionPipeline {
    /// Creates an empty pipeline.
    pub fn new() -> Self {
        ExpressionPipeline {
            token_rules: Vec::new(),
            token_by_char: HashMap::new(),
            token_fallbacks: Vec::new(),

            primaries: Vec::new(),
            primary_by_kind: HashMap::new(),
            primary_fallbacks: Vec::new(),

            postfixes: Vec::new(),
            postfix_by_kind: HashMap::new(),
            postfix_fallbacks: Vec::new(),

            infixes: Vec::new(),
            infix_by_kind: HashMap::new(),
            infix_fallbacks: Vec::new(),
        }
    }

    /// Registers a token rule.
    pub fn register_token_rule(&mut self, rule: Box<dyn TokenRule>) {
        let idx = self.token_rules.len();
        let firsts = rule.first_chars();

        if firsts.is_empty() {
            self.token_fallbacks.push(idx);
        } else {
            for &c in firsts {
                self.token_by_char.entry(c).or_default().push(idx);
            }
        }

        self.token_rules.push(rule);
    }

    /// Registers a primary-expression fragment.
    pub fn register_primary(&mut self, fragment: Box<dyn PrimaryFragment>) {
        let idx = self.primaries.len();
        let tokens = fragment.primary_tokens();

        if tokens.is_empty() {
            self.primary_fallbacks.push(idx);
        } else {
            for kind in tokens {
                let d = std::mem::discriminant(kind);
                self.primary_by_kind.entry(d).or_default().push(idx);
            }
        }

        self.primaries.push(fragment);
    }

    /// Registers a postfix-expression fragment.
    pub fn register_postfix(&mut self, fragment: Box<dyn PostfixFragment>) {
        let idx = self.postfixes.len();
        let tokens = fragment.postfix_tokens();

        if tokens.is_empty() {
            self.postfix_fallbacks.push(idx);
        } else {
            for kind in tokens {
                let d = std::mem::discriminant(kind);
                self.postfix_by_kind.entry(d).or_default().push(idx);
            }
        }

        self.postfixes.push(fragment);
    }

    /// Registers an infix-expression fragment.
    pub fn register_infix(&mut self, fragment: Box<dyn InfixFragment>) {
        let idx = self.infixes.len();
        let tokens = fragment.infix_tokens();

        if tokens.is_empty() {
            self.infix_fallbacks.push(idx);
        } else {
            for kind in tokens {
                let d = std::mem::discriminant(kind);
                self.infix_by_kind.entry(d).or_default().push(idx);
            }
        }

        self.infixes.push(fragment);
    }

    /// Lexes `input` into a token stream using the registered token rules.
    pub fn lex(&self, input: &str) -> Result<Vec<Token>, handl_platform::PlatformError> {
        let mut tokens = Vec::new();
        let mut pos = 0;
        let len = input.len();

        while pos < len {
            // Skip ASCII whitespace.
            if input[pos..].starts_with(|c: char| c.is_ascii_whitespace()) {
                let ch = input[pos..].chars().next().unwrap();
                pos += ch.len_utf8();
                continue;
            }

            let first = input[pos..].chars().next().unwrap();

            let mut candidates: Vec<usize> = self
                .token_by_char
                .get(&first)
                .map(|v| v.clone())
                .unwrap_or_else(|| (0..self.token_rules.len()).collect());

            if !self.token_fallbacks.is_empty() {
                candidates.extend(&self.token_fallbacks);
                candidates.sort();
            }

            let mut matched = false;
            for &idx in &candidates {
                match self.token_rules[idx].try_lex(input, pos) {
                    None => continue,
                    Some(Err(e)) => return Err(e),
                    Some(Ok(token)) => {
                        pos = token.span.end;
                        tokens.push(token);
                        matched = true;
                        break;
                    }
                }
            }

            if !matched {
                return Err(handl_platform::PlatformError::Plugin(format!(
                    "unexpected character '{}' at {}",
                    first, pos
                )));
            }
        }

        Ok(tokens)
    }

    /// Parses `tokens` into an expression using the registered fragments.
    pub fn parse(&self, tokens: Vec<Token>) -> Result<Expr<NonFunctionValue>, handl_platform::PlatformError> {
        self.parse_with_source(tokens, "")
    }

    /// Parses `tokens` into an expression with the original `source` text.
    pub fn parse_with_source(
        &self,
        tokens: Vec<Token>,
        source: &str,
    ) -> Result<Expr<NonFunctionValue>, handl_platform::PlatformError> {
        let mut ctx = ParseContext::new(&tokens, source, self);
        let expr = self.parse_expression(&mut ctx, 0)?;

        if let Some(token) = ctx.peek().cloned() {
            return Err(handl_platform::PlatformError::Plugin(format!(
                "unexpected token {} at {}",
                token.kind, token.span
            )));
        }

        Ok(expr)
    }

    /// Parses `tokens` into a `ParseResult`, recovering from errors.
    pub fn parse_with_recovery(
        &self,
        tokens: Vec<Token>,
        source: &str,
    ) -> ParseResult<Expr<NonFunctionValue>> {
        let mut ctx = ParseContext::new(&tokens, source, self);
        ctx.recover = true;
        let expr = self.parse_expression(&mut ctx, 0).unwrap_or_else(|_| {
            ctx.error("could not parse expression");
            Expr::Value(NonFunctionValue::new(JsonValue::Null))
        });

        while let Some(token) = ctx.peek().cloned() {
            ctx.error(format!("unexpected token {} at {}", token.kind, token.span));
            ctx.consume();
        }

        ParseResult {
            value: expr,
            diagnostics: ctx.diagnostics,
        }
    }

    /// Lexes and parses `input`, recovering from errors.
    pub fn parse_recovered(&self, input: &str) -> ParseResult<Expr<NonFunctionValue>> {
        match self.lex(input) {
            Ok(tokens) => self.parse_with_recovery(tokens, input),
            Err(e) => ParseResult {
                value: Expr::Value(NonFunctionValue::new(JsonValue::Null)),
                diagnostics: vec![Diagnostic {
                    level: crate::diagnostics::DiagnosticLevel::Error,
                    message: e.to_string(),
                    span: Some(crate::token::Span::at(input.len())),
                }],
            },
        }
    }

    /// Parses an expression starting at the current position of `ctx`.
    pub fn parse_expr(&self, ctx: &mut ParseContext) -> Result<Expr<NonFunctionValue>, handl_platform::PlatformError> {
        self.parse_expr_with_min_bp(ctx, 0)
    }

    /// Parses an expression with a minimum binding power.
    pub fn parse_expr_with_min_bp(
        &self,
        ctx: &mut ParseContext,
        min_binding: u32,
    ) -> Result<Expr<NonFunctionValue>, handl_platform::PlatformError> {
        self.parse_expression(ctx, min_binding)
    }

    /// Parses a primary expression starting at the current position of `ctx`.
    pub fn parse_primary(&self, ctx: &mut ParseContext) -> Result<Expr<NonFunctionValue>, handl_platform::PlatformError> {
        let Some(token) = ctx.peek().cloned() else {
            return Err(handl_platform::PlatformError::Plugin(format!(
                "expected primary but found end of input at {}",
                ctx.current_span()
            )));
        };

        let d = std::mem::discriminant(&token.kind);
        let mut candidates: Vec<usize> = self
            .primary_by_kind
            .get(&d)
            .map(|v| v.clone())
            .unwrap_or_else(|| (0..self.primaries.len()).collect());

        if !self.primary_fallbacks.is_empty() {
            candidates.extend(&self.primary_fallbacks);
            candidates.sort();
        }

        for &idx in &candidates {
            if let Some(result) = self.primaries[idx].try_primary(ctx) {
                return result;
            }
        }

        Err(handl_platform::PlatformError::Plugin(format!(
            "expected primary but found {} at {}",
            token.kind, token.span
        )))
    }

    fn parse_expression(
        &self,
        ctx: &mut ParseContext,
        min_binding: u32,
    ) -> Result<Expr<NonFunctionValue>, handl_platform::PlatformError> {
        // Parse a primary.
        let mut left = self.parse_primary(ctx)?;

        // Apply postfixes as long as they match.
        while let Some(token) = ctx.peek().cloned() {
            let d = std::mem::discriminant(&token.kind);

            let mut advanced = false;
            let mut candidates: Vec<usize> = self
                .postfix_by_kind
                .get(&d)
                .map(|v| v.clone())
                .unwrap_or_else(|| (0..self.postfixes.len()).collect());

            if !self.postfix_fallbacks.is_empty() {
                candidates.extend(&self.postfix_fallbacks);
                candidates.sort();
            }

            for &idx in &candidates {
                if let Some(result) = self.postfixes[idx].try_postfix(&mut left, ctx) {
                    match result {
                        Ok(()) => {
                            advanced = true;
                            break;
                        }
                        Err(e) => {
                            if ctx.recover {
                                ctx.error(e.to_string());
                                break;
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
            }
            if !advanced {
                break;
            }
        }

        // Apply infixes with precedence >= min_binding.
        while let Some(token) = ctx.peek().cloned() {
            let d = std::mem::discriminant(&token.kind);

            let mut candidates: Vec<usize> = self
                .infix_by_kind
                .get(&d)
                .map(|v| v.clone())
                .unwrap_or_else(|| (0..self.infixes.len()).collect());

            if !self.infix_fallbacks.is_empty() {
                candidates.extend(&self.infix_fallbacks);
                candidates.sort();
            }

            let mut best: Option<(usize, u32)> = None;
            for &idx in &candidates {
                if let Some((lbp, _)) = self.infixes[idx].binding(&token) {
                    if lbp >= min_binding {
                        if best.map_or(true, |(_, bl)| lbp > bl) {
                            best = Some((idx, lbp));
                        }
                    }
                }
            }

            let idx = match best {
                Some((idx, _)) => idx,
                None => break,
            };

            match self.infixes[idx].try_infix(&mut left, ctx) {
                None => break,
                Some(Ok(())) => {}
                Some(Err(e)) => {
                    if ctx.recover {
                        ctx.error(e.to_string());
                        break;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Ok(left)
    }
}
