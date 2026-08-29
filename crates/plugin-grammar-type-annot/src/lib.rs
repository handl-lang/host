//! Canonical `TypeAnnot` grammar-rule plugin.
//!
//! Provides the `TypeAnnotRule` and the `TypeAnnotToken` capability trait.
//! `TypeAnnotRule` implements `ParserBehavior<K>` for the `handl-plugin-ext-parser`
//! Pratt engine.

use handl_platform::{AstNode, Behavior, Operation, PlatformError, Span};
use handl_plugin_ext_parser::{Parser, ParserBehavior, ParserContext};
use handl_plugin_lexer::{Token, TokenKind};
use handl_plugin_token_kind::{LexDelimiter, LexKind, LexOperator};
use serde_json::json;

/// Token capability required by the type-annotation grammar rule.
pub trait TypeAnnotToken: TokenKind {
    /// Returns `true` for the colon delimiter token, e.g. `:`.
    fn is_colon(&self) -> bool;

    /// Returns `true` for the equal operator token, e.g. `=`.
    fn is_equal(&self) -> bool;

    /// If this token represents an identifier, return its text.
    fn ident_text(&self) -> Option<&str>;
}

/// Grammar rule that parses typed assignments: `name: type = value`.
///
/// This is an infix rule triggered by the `:` delimiter. It parses the
/// type name (an identifier), expects `=`, then parses the value
/// expression. Produces a `stmt.let` AST node with an optional `type` field.
#[derive(Debug, Default, Clone, Copy)]
pub struct TypeAnnotRule;

impl<K: TypeAnnotToken> Behavior<Parser<K>> for TypeAnnotRule {
    fn kind(&self) -> &str {
        "type-annot"
    }

    fn apply(&self, _op: &mut <Parser<K> as Operation>::Request) -> Result<(), PlatformError> {
        Ok(())
    }
}

impl<K: TypeAnnotToken> ParserBehavior<K> for TypeAnnotRule {
    fn matches_prefix(&self, _kind: &K) -> bool {
        false
    }

    fn prefix(
        &self,
        _token: Token<K>,
        _ctx: &mut ParserContext<'_, K>,
    ) -> Result<AstNode, PlatformError> {
        unreachable!("TypeAnnotRule is an infix rule and cannot be used as a prefix operator")
    }

    fn infix_binding(&self, kind: &K) -> Option<(u32, u32)> {
        if kind.is_colon() {
            Some((15, 16))
        } else {
            None
        }
    }

    fn infix(
        &self,
        left: AstNode,
        _token: Token<K>,
        ctx: &mut ParserContext<'_, K>,
        _rbp: u32,
    ) -> Result<AstNode, PlatformError> {
        let name = left
            .payload
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                PlatformError::Plugin(format!("expected identifier before ':', got {:?}", left.kind))
            })?;

        let type_tok = ctx.advance()?;
        let type_name = type_tok
            .kind
            .ident_text()
            .ok_or_else(|| {
                PlatformError::Plugin(format!(
                    "expected type name after ':', got {}",
                    type_tok.kind
                ))
            })?
            .to_string();

        let eq = ctx.advance()?;
        if !eq.kind.is_equal() {
            return Err(PlatformError::Plugin(format!(
                "expected '=' after type annotation, got {}",
                eq.kind
            )));
        }

        let value = ctx.expression(0)?;

        let start = left.span.map_or(0, |s| s.start);
        let end = value.span.map_or(start, |s| s.end);

        Ok(AstNode {
            kind: "stmt.let".into(),
            span: Some(Span { start, end }),
            payload: json!({
                "name": name,
                "type": type_name,
                "value": &value,
            }),
        })
    }
}

impl TypeAnnotToken for LexKind {
    fn is_colon(&self) -> bool {
        matches!(self, LexKind::Delimiter(LexDelimiter::Colon))
    }

    fn is_equal(&self) -> bool {
        matches!(self, LexKind::Operator(LexOperator::Equal))
    }

    fn ident_text(&self) -> Option<&str> {
        match self {
            LexKind::Identifier(s) => Some(s.as_str()),
            _ => None,
        }
    }
}
