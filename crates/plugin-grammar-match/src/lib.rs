//! Canonical `Match` grammar-rule plugin.
//!
//! Provides the `MatchRule` and the `MatchToken` capability trait.
//! `MatchRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use serde_json::json;

/// Token capability required by the match grammar rule.
pub trait MatchToken: TokenKind {
    /// Returns `true` for the `match` keyword token.
    fn is_match(&self) -> bool;
    /// Returns `true` for the opening brace token, e.g. `{`.
    fn is_brace_open(&self) -> bool;
    /// Returns `true` for the closing brace token, e.g. `}`.
    fn is_brace_close(&self) -> bool;
    /// Returns `true` for the arm-separator token, e.g. `,`.
    fn is_comma(&self) -> bool;
    /// Returns `true` for the pattern-body separator token, e.g. `:`.
    fn is_colon(&self) -> bool;
}

/// Grammar rule that parses `match` expressions: `match subject { arms }`.
///
/// Each arm is `pattern : body` or `pattern : guard : body` (where a guard
/// is a boolean expression). Arms are separated by `,`. A trailing comma is
/// permitted.
#[derive(Debug, Default, Clone, Copy)]
pub struct MatchRule;

impl<K: MatchToken> Behavior<Parser<K>> for MatchRule {
    fn kind(&self) -> &str {
        "match"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: MatchToken> ParserBehavior<K> for MatchRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_match()
    }

    fn prefix(
        &self,
        token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let subject = ctx.expression(0)?;

        let mut arms: Vec<serde_json::Value> = Vec::new();
        let mut block_end = subject.span.map_or(token.span.start, |s| s.end);

        // Optional brace-delimited arm block: `{ arm (, arm)* ,? }`.
        if ctx.peek().map_or(false, |t| t.kind.is_brace_open()) {
            ctx.advance()?; // consume `{`

            if !ctx.peek().map_or(true, |t| t.kind.is_brace_close()) {
                loop {
                    // Parse the pattern with rbp 16 so the `:` type-annotation
                    // rule (lbp 15) does not consume the arm's pattern-body
                    // separator. Higher-binding operators (`.`, call) still
                    // bind, so `cmp.eq` etc. parse as expected.
                    let pattern = ctx.expression(16)?;

                    let colon = ctx.advance()?;
                    if !colon.kind.is_colon() {
                        return Err(PlatformError::Plugin(format!(
                            "expected `:` after match pattern, got {}",
                            colon.kind
                        )));
                    }

                    // The next expression is either the guard or the body.
                    // A guard is present only if another `:` follows it.
                    // Parse with rbp 16 so a `:` inside the guard (e.g. a
                    // type annotation on a sub-expression) does not consume
                    // the guard-body separator.
                    let first = ctx.expression(16)?;
                    let (guard, body) = if ctx.peek().map_or(false, |t| t.kind.is_colon()) {
                        ctx.advance()?; // consume the guard-body `:`
                        let body = ctx.expression(0)?;
                        (Some(first), body)
                    } else {
                        (None, first)
                    };

                    arms.push(json!({
                        "pattern": &pattern,
                        "guard": guard.as_ref().map(|g| serde_json::to_value(g).unwrap_or(serde_json::Value::Null)).unwrap_or(serde_json::Value::Null),
                        "body": &body,
                    }));

                    match ctx.peek() {
                        Some(t) if t.kind.is_brace_close() => break,
                        Some(t) if t.kind.is_comma() => {
                            ctx.advance()?;
                            // Allow a trailing comma.
                            if ctx.peek().map_or(false, |t| t.kind.is_brace_close()) {
                                break;
                            }
                        }
                        Some(_) | None => break,
                    }
                }
            }

            let close = ctx.advance()?;
            if !close.kind.is_brace_close() {
                return Err(PlatformError::Plugin(format!(
                    "expected `}}`, got {}",
                    close.kind
                )));
            }
            block_end = close.span.end;
        }

        let start = token.span.start;

        Ok(AstNode {
            kind: "match".into(),
            span: Some(Span { start, end: block_end }),
            payload: json!({ "subject": &subject, "arms": arms }),
        })
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
        unreachable!("MatchRule is a prefix rule and cannot be used as an infix operator")
    }
}

