//! Canonical `Arrow` grammar-rule plugin.
//!
//! Provides the `ArrowRule` and the `ArrowToken` capability trait.
//! `ArrowRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine. It handles function-literal syntax: `(params) -> body` and
//! `(params): RetType -> body`, while falling back to parenthesized grouping
//! `(expr)` when the content is not a parameter list.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the arrow grammar rule.
pub trait ArrowToken: TokenKind {
    /// Returns `true` for the opening paren token, e.g. `(`.
    fn is_paren_open(&self) -> bool;
    /// Returns `true` for the closing paren token, e.g. `)`.
    fn is_paren_close(&self) -> bool;
    /// Returns `true` for the argument-separator token, e.g. `,`.
    fn is_comma(&self) -> bool;
    /// Returns `true` for the colon delimiter token, e.g. `:`.
    fn is_colon(&self) -> bool;
    /// Returns `true` for the arrow operator token, e.g. `->`.
    fn is_arrow(&self) -> bool;
    /// If this token represents an identifier, return its text.
    fn ident_text(&self) -> Option<&str>;
}

/// Grammar rule that parses function-literal syntax: `(params) -> body` or
/// `(params): RetType -> body`. Falls back to parenthesized grouping `(expr)`
/// when the content inside the parens is not a parameter list.
#[derive(Debug, Default, Clone, Copy)]
pub struct ArrowRule;

impl<K: ArrowToken> Behavior<Parser<K>> for ArrowRule {
    fn kind(&self) -> &str {
        "arrow"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: ArrowToken> ParserBehavior<K> for ArrowRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_paren_open()
    }

    fn prefix(
        &self,
        _token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let saved = ctx.pos();

        // Try to parse as a function literal.
        if let Ok(node) = try_parse_function_literal(ctx) {
            return Ok(node);
        }

        // Restore and parse as a simple grouping expression.
        ctx.set_pos(saved);
        let inner = ctx.expression(0)?;
        let close = ctx.advance()?;
        if !close.kind.is_paren_close() {
            return Err(PlatformError::Plugin(format!(
                "expected `)`, got {}",
                close.kind
            )));
        }
        Ok(inner)
    }

    fn infix_binding(&self, _kind: &K) -> Option<(u32, u32)> {
        None
    }

    fn infix(
        &self,
        _left: AstNode,
        _token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
        _rbp: u32,
    ) -> Result<AstNode, PlatformError> {
        unreachable!("ArrowRule is a prefix rule and cannot be used as an infix operator")
    }
}

/// Attempts to parse a function literal: `(params) -> body` or
/// `(params): RetType -> body`. Returns `Err` if the content is not a
/// parameter list, so the caller can fall back to grouping.
fn try_parse_function_literal<K: ArrowToken>(
    ctx: &mut ParserContext<'_, K>,
) -> Result<AstNode, PlatformError> {
    // We are positioned right after the opening `(`.
    let mut params: Vec<String> = Vec::new();

    // Handle empty parameter list: `() -> body`
    if let Some(t) = ctx.peek() {
        if t.kind.is_paren_close() {
            ctx.advance()?; // consume `)`
            // Check for `: RetType ->` or `->`
            let return_type = try_parse_return_type(ctx)?;
            if let Some(rt) = return_type {
                // Expect `->`
                let arrow = ctx.advance()?;
                if !arrow.kind.is_arrow() {
                    return Err(PlatformError::Plugin(
                        "expected `->` after return type in function literal".into(),
                    ));
                }
                let body = ctx.expression(0)?;
                return Ok(build_function_ast(params, Some(rt), body));
            }
            // Check for `->`
            if let Some(t) = ctx.peek() {
                if t.kind.is_arrow() {
                    ctx.advance()?; // consume `->`
                    let body = ctx.expression(0)?;
                    return Ok(build_function_ast(params, None, body));
                }
            }
            // Not a function literal — just `()` which is not valid grouping either
            return Err(PlatformError::Plugin(
                "empty parens not recognized as function literal or grouping".into(),
            ));
        }
    }

    // Parse parameter list: ident (`: TypeExpr)? (`,` ident (`: TypeExpr)?)*
    loop {
        let token = ctx.advance()?;
        let name = token
            .kind
            .ident_text()
            .ok_or_else(|| {
                PlatformError::Plugin(format!(
                    "expected parameter name, got {}",
                    token.kind
                ))
            })?
            .to_string();
        params.push(name);

        // Optional `: TypeExpr` — consume type tokens until `,` or `)`
        if let Some(t) = ctx.peek() {
            if t.kind.is_colon() {
                ctx.advance()?; // consume `:`
                // Consume type expression tokens until we hit `,` or `)`
                consume_type_expr(ctx)?;
            }
        }

        // Check for `,` or `)`
        match ctx.peek() {
            Some(t) if t.kind.is_comma() => {
                ctx.advance()?; // consume `,`
            }
            Some(t) if t.kind.is_paren_close() => {
                ctx.advance()?; // consume `)`
                break;
            }
            _ => {
                return Err(PlatformError::Plugin(
                    "expected `,` or `)` in parameter list".into(),
                ));
            }
        }
    }

    // After `)`, check for `: RetType ->` or just `->`
    let return_type = try_parse_return_type(ctx)?;

    // Expect `->`
    let arrow = ctx.advance()?;
    if !arrow.kind.is_arrow() {
        return Err(PlatformError::Plugin(format!(
            "expected `->` in function literal, got {}",
            arrow.kind
        )));
    }

    let body = ctx.expression(0)?;
    Ok(build_function_ast(params, return_type, body))
}

/// Attempts to parse a return type annotation `: RetType` after `)`.
/// Returns `Ok(Some(type_string))` if found, `Ok(None)` if the next token
/// is not `:`, or `Err` if `:` is found but no valid type follows.
fn try_parse_return_type<K: ArrowToken>(
    ctx: &mut ParserContext<'_, K>,
) -> Result<Option<String>, PlatformError> {
    match ctx.peek() {
        Some(t) if t.kind.is_colon() => {
            ctx.advance()?; // consume `:`
            // Consume type expression tokens until we hit `->`
            let type_str = consume_type_expr_until_arrow(ctx)?;
            Ok(Some(type_str))
        }
        _ => Ok(None),
    }
}

/// Consumes tokens forming a type expression until a `,` or `)` is encountered.
fn consume_type_expr<K: ArrowToken>(
    ctx: &mut ParserContext<'_, K>,
) -> Result<(), PlatformError> {
    loop {
        match ctx.peek() {
            Some(t) if t.kind.is_comma() || t.kind.is_paren_close() => break,
            Some(t) if t.kind.is_eof() => break,
            None => break,
            Some(_) => {
                ctx.advance()?;
            }
        }
    }
    Ok(())
}

/// Consumes tokens forming a type expression until `->` is encountered.
/// Returns the consumed tokens as a string.
fn consume_type_expr_until_arrow<K: ArrowToken>(
    ctx: &mut ParserContext<'_, K>,
) -> Result<String, PlatformError> {
    let mut parts: Vec<String> = Vec::new();
    loop {
        match ctx.peek() {
            Some(t) if t.kind.is_arrow() => break,
            Some(t) if t.kind.is_eof() => {
                return Err(PlatformError::Plugin(
                    "expected `->` after return type".into(),
                ));
            }
            None => {
                return Err(PlatformError::Plugin(
                    "expected `->` after return type".into(),
                ));
            }
            Some(t) => {
                parts.push(format!("{}", t.kind));
                ctx.advance()?;
            }
        }
    }
    Ok(parts.join(" "))
}

fn build_function_ast(params: Vec<String>, return_type: Option<String>, body: AstNode) -> AstNode {
    let payload = if let Some(rt) = return_type {
        json!({
            "params": params,
            "return_type": rt,
            "body": &body,
        })
    } else {
        json!({
            "params": params,
            "body": &body,
        })
    };

    AstNode {
        kind: "function".into(),
        span: Some(Span { start: 0, end: 0 }),
        payload,
    }
}
