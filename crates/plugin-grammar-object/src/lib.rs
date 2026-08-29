//! Canonical `Object` grammar-rule plugin.
//!
//! Provides the `ObjectRule` and the `ObjectToken` capability trait.
//! `ObjectRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_token_kind::{LexDelimiter, LexKind};
use serde_json::{Value, json};

/// Token capability required by the object grammar rule.
pub trait ObjectToken: TokenKind {
    /// Returns `true` for the opening brace token, e.g. `{`.
    fn is_brace_open(&self) -> bool;
    /// Returns `true` for the closing brace token, e.g. `}`.
    fn is_brace_close(&self) -> bool;
    /// Returns `true` for the entry-separator token, e.g. `,`.
    fn is_comma(&self) -> bool;
    /// Returns `true` for the key-value separator token, e.g. `:`.
    fn is_colon(&self) -> bool;
    /// If this token can serve as an object key, returns its text.
    fn as_object_key(&self) -> Option<String>;
}

/// Grammar rule that parses object literals: `{key: value, ...}`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ObjectRule;

impl<K: ObjectToken> Behavior<Parser<K>> for ObjectRule {
    fn kind(&self) -> &str {
        "object"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: ObjectToken> ParserBehavior<K> for ObjectRule {
    fn matches_prefix(&self, kind: &K) -> bool {
        kind.is_brace_open()
    }

    fn prefix(
        &self,
        token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        let mut entries: Vec<(String, AstNode)> = Vec::new();

        if !ctx.peek().map_or(true, |t| t.kind.is_brace_close()) {
            loop {
                let key_tok = ctx.advance()?;
                let key = key_tok.kind.as_object_key().ok_or_else(|| {
                    PlatformError::Plugin(format!("expected an object key, got {}", key_tok.kind))
                })?;

                let colon = ctx.advance()?;
                if !colon.kind.is_colon() {
                    return Err(PlatformError::Plugin(format!(
                        "expected `:`, got {}",
                        colon.kind
                    )));
                }

                let value = ctx.expression(0)?;
                entries.push((key, value));

                match ctx.peek() {
                    Some(t) if t.kind.is_brace_close() => break,
                    Some(t) if t.kind.is_comma() => {
                        ctx.advance()?;
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

        let start = token.span.start;
        let end = close.span.end;

        let entries_json: Vec<Value> = entries
            .iter()
            .map(|(k, v)| json!({"key": k, "value": v}))
            .collect();

        Ok(AstNode {
            kind: "object".into(),
            span: Some(Span { start, end }),
            payload: json!({ "entries": entries_json }),
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
        unreachable!("ObjectRule is a prefix rule and cannot be used as an infix operator")
    }
}

impl ObjectToken for LexKind {
    fn is_brace_open(&self) -> bool {
        matches!(self, LexKind::Delimiter(LexDelimiter::BraceOpen))
    }

    fn is_brace_close(&self) -> bool {
        matches!(self, LexKind::Delimiter(LexDelimiter::BraceClose))
    }

    fn is_comma(&self) -> bool {
        matches!(self, LexKind::Delimiter(LexDelimiter::Comma))
    }

    fn is_colon(&self) -> bool {
        matches!(self, LexKind::Delimiter(LexDelimiter::Colon))
    }

    fn as_object_key(&self) -> Option<String> {
        match self {
            LexKind::Identifier(name) => Some(name.clone()),
            LexKind::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}
