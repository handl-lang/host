//! # handl-plugin-engine-parse-pratt
//!
//! The **Pratt engine** for the parse operation (ADR-0044 §3).
//!
//! This engine reads [`handl_language::Language`] **grammar data** — it looks
//! up [`handl_language::GrammarRow`]s by `(token, position)` and drives a
//! classic top-down operator-precedence (Pratt) loop. It does **not** call
//! `rule.prefix(token, ctx)` on a Rust `ParserBehavior` object; it reads the
//! descriptor (prefix/infix, binding powers, child shape) from data and
//! executes.
//!
//! ## Root node convention
//!
//! The parse produces an [`Ast`] arena. The **root node is the last node
//! pushed** — in a Pratt parser, the outermost expression's node is always
//! the final one appended (children are pushed before their parents). For
//! empty input, the arena is empty and there is no root.
//!
//! ## Payload convention
//!
//! - **Atoms** (arity 0): the payload is the token's interpreted value — a
//!   number if the token kind id contains `"number"`, otherwise the token
//!   text as a string.
//! - **Compound nodes** (arity > 0): the payload is
//!   `{"children": [idx0, idx1, ...]}` where each `idxN` is the arena index
//!   (`AstRef::index()`) of the Nth child. The eval engine uses this to
//!   resolve `arr(proj_child, ...)` references.

#![warn(missing_docs)]

use std::cell::RefCell;

use handl_language::{
    Ast, AstNode, AstRef, ChildSeparator, DelimitedShape, GrammarPosition, LangError, Language,
    LiteralKind, PayloadTokenSpec, Span, Token, Value,
};
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};

/// The Pratt parsing engine.
///
/// Reads `lang.grammar` data exclusively — no Rust `ParserBehavior` impls.
/// The engine owns an [`Ast`] arena via [`RefCell`] (reset at the start of
/// each [`ParserEngine::parse`] call) so that the recursive
/// [`ParserEngine::expression`] entry point can push nodes through `&self`.
///
/// Construct with [`PrattEngine::new`] and pass to any consumer of
/// `&dyn ParserEngine`.
#[derive(Clone)]
pub struct PrattEngine {
    /// The arena being built during the current parse.
    ast: RefCell<Ast>,
}

impl PrattEngine {
    /// Create a new Pratt engine with an empty arena.
    pub fn new() -> Self {
        Self {
            ast: RefCell::new(Ast::new()),
        }
    }

    /// Interpret a token's text into a payload [`Value`], using the
    /// language's token kind definitions' `literal` field for typed dispatch
    /// (ADR-0053).
    ///
    /// Dispatches on `TokenKindDef.literal` (defaulting to [`LiteralKind::Raw`])
    /// — no substring matching on the token kind id. Adding a new literal form
    /// is a new `LiteralKind` variant + a token row, not a parser edit.
    fn atom_payload(&self, lang: &Language, token: &Token) -> Value {
        let kind_def = lang.tokens.get_by_identity(&token.kind);
        let literal = kind_def
            .map(|tkd| tkd.literal.clone())
            .unwrap_or(None)
            .unwrap_or(LiteralKind::Raw);

        match literal {
            LiteralKind::Number => {
                if let Ok(n) = token.text.parse::<i64>() {
                    return Value::from(n);
                }
                if let Ok(f) = token.text.parse::<f64>() {
                    return Value::from(f);
                }
                // Non-numeric text with Number kind: fall back to string.
                Value::from(token.text.as_str())
            }
            LiteralKind::Bool => Value::from(token.text == "true"),
            LiteralKind::Null => Value::Null,
            LiteralKind::String {
                delim_chars,
                escapes,
            } => {
                if token.text.len() >= 2 * delim_chars {
                    let inner = &token.text[delim_chars..token.text.len() - delim_chars];
                    let mut result = inner.to_string();
                    for (from, to) in &escapes {
                        result = result.replace(from, to);
                    }
                    Value::from(result)
                } else {
                    Value::from(token.text.as_str())
                }
            }
            LiteralKind::StripAffix {
                prefix,
                suffix,
                wrap,
            } => {
                let text = token.text.as_str();
                let stripped = if text.starts_with(&prefix) && text.ends_with(&suffix) {
                    let start = prefix.len();
                    let end = text.len().saturating_sub(suffix.len());
                    if start <= end {
                        &text[start..end]
                    } else {
                        ""
                    }
                } else {
                    text
                };
                match wrap {
                    Some((open, close)) => Value::from(format!("{}{}{}", open, stripped, close)),
                    None => Value::from(stripped),
                }
            }
            LiteralKind::Raw => Value::from(token.text.as_str()),
        }
    }

    /// Look up a grammar row by `(token_kind, position)` using the identity
    /// key `token@position`.
    fn lookup_row<'l>(
        lang: &'l Language,
        token_kind: &str,
        position: GrammarPosition,
    ) -> Option<&'l handl_language::GrammarRow> {
        let key = format!("{}@{}", token_kind, position.as_str());
        lang.grammar.get_by_identity(&key)
    }
}

impl Default for PrattEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ParserEngine for PrattEngine {
    fn parse(&self, lang: &Language, req: &ParserRequest) -> Result<Ast, LangError> {
        // Reset the arena for this parse.
        *self.ast.borrow_mut() = Ast::new();

        let raw = req.tokens.as_slice();
        if raw.is_empty() {
            return Ok(Ast::new());
        }

        // ADR-0044: trivia tokens (e.g. `token.comment`, `token.doc_comment`)
        // are flagged in `lang.tokens` data. They carry no syntactic meaning,
        // so the Pratt loop never sees them — the lexer still emits them so
        // tooling (LSP hover, semantic tokens) can surface comments. Filter
        // them out of the token slice before driving the grammar. This is a
        // data-driven decision (lookup in `lang.tokens`), not a hardcoded
        // `kind == "token.comment"` check.
        let filtered: Vec<Token> = filter_trivia(lang, raw);
        if filtered.is_empty() {
            return Ok(Ast::new());
        }
        let tokens = filtered.as_slice();

        // Drive the Pratt loop from a local cursor. We do not use
        // ParserContext here because the engine owns the recursion; the
        // context is for external callers. We call expression directly.
        let mut cursor = Cursor { tokens, pos: 0 };
        let _root = self.expression_inner(lang, &mut cursor, 0)?;

        // Return the built arena. Root = last node pushed.
        Ok(self.ast.borrow().clone())
    }

    fn expression(
        &self,
        ctx: &mut handl_plugin_engine_parse::ParserContext<'_, '_, Self>,
        rbp: u8,
    ) -> Result<AstRef, LangError> {
        // Build a cursor from the context's remaining tokens.
        let tokens = ctx.tokens();
        let pos = ctx.pos();
        let mut cursor = Cursor { tokens, pos };
        let result = self.expression_inner(ctx.lang(), &mut cursor, rbp)?;
        // Sync the context position.
        // We need to advance the context to match the cursor's consumed
        // tokens. Since ParserContext::advance consumes one token, we call
        // it for each token the cursor consumed.
        let consumed = cursor.pos - pos;
        for _ in 0..consumed {
            let _ = ctx.advance()?;
        }
        Ok(result)
    }
}

/// Filter trivia tokens out of `tokens`.
///
/// A token is trivia when its kind is declared with `trivia: true` in
/// `lang.tokens` (ADR-0044). The decision is data-driven: this performs a
/// `lang.tokens` lookup per token, never a hardcoded kind-string check.
fn filter_trivia(lang: &Language, tokens: &[Token]) -> Vec<Token> {
    tokens
        .iter()
        .filter(|t| !is_trivia(lang, &t.kind))
        .cloned()
        .collect()
}

/// Whether `kind` is a trivia token kind (declared `trivia: true` in
/// `lang.tokens`).
fn is_trivia(lang: &Language, kind: &str) -> bool {
    lang.tokens
        .get_by_identity(kind)
        .map(|def| def.trivia)
        .unwrap_or(false)
}

/// A local cursor over a token slice, used internally by the Pratt loop.
struct Cursor<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Result<Token, LangError> {
        match self.tokens.get(self.pos) {
            Some(t) => {
                self.pos += 1;
                Ok(t.clone())
            }
            None => Err(LangError::engine_at(
                "unexpected end of input".to_string(),
                self.last_span(),
                None::<&str>,
            )),
        }
    }

    /// The byte span of the token at the current cursor position, if any.
    /// Used to attach source locations to parse errors.
    fn current_span(&self) -> Option<Span> {
        self.tokens.get(self.pos).and_then(|t| t.span())
    }

    /// The byte span of the most recently consumed token, falling back to the
    /// current token's span. Used for end-of-input diagnostics.
    fn last_span(&self) -> Option<Span> {
        if self.pos == 0 {
            self.current_span()
        } else {
            self.tokens.get(self.pos - 1).and_then(|t| t.span())
        }
    }
}

impl PrattEngine {
    /// The internal Pratt expression loop, driven entirely by `lang.grammar`
    /// data (ADR-0053). No string literals, no `kind == "..."`, no
    /// `token.kind == "..."` — the parser is a fixed interpreter of grammar
    /// data.
    fn expression_inner(
        &self,
        lang: &Language,
        cursor: &mut Cursor<'_>,
        rbp: u8,
    ) -> Result<AstRef, LangError> {
        let token = cursor.advance()?;

        // 1. Look up the prefix row for this token.
        let prefix_row = Self::lookup_row(lang, &token.kind, GrammarPosition::Prefix)
            .ok_or_else(|| {
                LangError::engine_at(
                    format!("unexpected token: {}", token.kind),
                    token.span(),
                    Some("token".to_string()),
                )
            })?;

        let shape = &prefix_row.child_shape;
        let arity = shape.arity;
        let kind = &shape.node_kind;
        let delim = shape.delimited.as_ref();

        // 2. Build the prefix (null-denotation) node.
        let left = if arity == 0 && delim.is_none() && shape.payload_tokens.is_empty() {
            // Atom: payload is the token's interpreted value (dispatches on
            // LiteralKind — no substring matching).
            let payload = self.atom_payload(lang, &token);
            let node = AstNode::new(kind).with_payload(payload);
            self.ast.borrow_mut().push(node)
        } else {
            // Prefix operator: parse normal children + optional delimited block.
            let prefix_rbp = prefix_row.binding_power.map(|(l, _)| l).unwrap_or(0);
            let mut children: Vec<usize> = Vec::with_capacity(arity);

            // Consume payload tokens (e.g. binding name, `=` sign) before
            // parsing children. Each token is consumed and, when `field` is
            // set, its text is stored in the payload (ADR-0053 §7b).
            let mut payload_fields: serde_json::Map<String, Value> = serde_json::Map::new();
            for spec in &shape.payload_tokens {
                let tok = cursor.advance()?;
                if tok.kind != spec.token {
                    return Err(LangError::engine_at(
                        format!("expected payload token {}, got {}", spec.token, tok.kind),
                        tok.span(),
                        Some("token".to_string()),
                    ));
                }
                if let Some(field) = &spec.field {
                    payload_fields.insert(field.clone(), Value::from(tok.text.as_str()));
                }
            }

            // When delimited is set, the last child slot is the delimited
            // block (flatten just controls whether a container node is emitted
            // or elements are spliced — the slot count is the same).
            let normal = match delim {
                Some(_) => arity.saturating_sub(1),
                None => arity,
            };
            for i in 0..normal {
                let child = if shape.block_children.contains(&i)
                    && cursor
                        .peek()
                        .map(|t| t.kind.as_str() == "token.lbrace")
                        .unwrap_or(false)
                {
                    let block_delim = DelimitedShape {
                        open: "token.lbrace".into(),
                        close: "token.rbrace".into(),
                        separator: None,
                        container_kind: "block".into(),
                        pair: None,
                        open_is_operator_token: false,
                        flatten: false,
                        rest_token: None,
                    };
                    let result = self.parse_delimited_block(lang, cursor, &block_delim)?;
                    let idx = match result {
                        DelimitedResult::Container(idx) => idx,
                        DelimitedResult::Elements(indices) => {
                            if indices.is_empty() {
                                let node = AstNode::new("block")
                                    .with_payload(serde_json::json!({ "children": [] }));
                                self.ast.borrow_mut().push(node).index()
                            } else {
                                indices[0]
                            }
                        }
                    };
                    AstRef::new(idx)
                } else {
                    self.expression_inner(lang, cursor, prefix_rbp)?
                };
                children.push(child.index());
                // Consume separator between children (not after the last
                // normal child — the delimited block follows).
                if i + 1 < normal {
                    if let Some(sep) = shape.child_separators.get(i) {
                        Self::consume_separator(cursor, sep)?;
                    }
                }
            }
            if let Some(d) = delim {
                let result = self.parse_delimited_block(lang, cursor, d)?;
                match result {
                    DelimitedResult::Container(idx) => children.push(idx),
                    DelimitedResult::Elements(indices) => children.extend(indices),
                }
            }
            // Build payload: start with extracted fields, then add children.
            payload_fields.insert(
                "children".to_string(),
                Value::Array(children.iter().map(|i| Value::from(*i)).collect()),
            );
            let payload = Value::Object(payload_fields);
            let node = AstNode::new(kind).with_payload(payload);
            self.ast.borrow_mut().push(node)
        };

        // 3. Infix (left-denotation) loop.
        let mut left = left;
        while let Some(next) = cursor.peek() {
            let infix_row = match Self::lookup_row(lang, &next.kind, GrammarPosition::Infix) {
                Some(row) => row,
                None => break,
            };

            let (lbp, rbp_right) = infix_row.binding_power.ok_or_else(|| {
                LangError::engine_at(
                    format!("infix row {} has no binding power", infix_row.id),
                    next.span(),
                    Some("token".to_string()),
                )
            })?;

            if lbp < rbp {
                break;
            }

            // Consume the operator token.
            let _op = cursor.advance()?;

            let shape = &infix_row.child_shape;
            let infix_kind = &shape.node_kind;
            let infix_arity = shape.arity;
            let delim = shape.delimited.as_ref();

            // normal = arity - 1 (left) - 1 (delimited) when delimited is set.
            // flatten just controls container vs splice — the slot count is
            // the same.
            let normal = match delim {
                Some(_) => infix_arity.saturating_sub(2),
                None => infix_arity.saturating_sub(1),
            };
            let mut children: Vec<usize> = Vec::with_capacity(infix_arity);
            children.push(left.index());
            for i in 0..normal {
                let child_idx = i + 1;
                let child = if shape.block_children.contains(&child_idx)
                    && cursor
                        .peek()
                        .map(|t| t.kind.as_str() == "token.lbrace")
                        .unwrap_or(false)
                {
                    let block_delim = DelimitedShape {
                        open: "token.lbrace".into(),
                        close: "token.rbrace".into(),
                        separator: None,
                        container_kind: "block".into(),
                        pair: None,
                        open_is_operator_token: false,
                        flatten: false,
                        rest_token: None,
                    };
                    let result = self.parse_delimited_block(lang, cursor, &block_delim)?;
                    let idx = match result {
                        DelimitedResult::Container(idx) => idx,
                        DelimitedResult::Elements(indices) => {
                            if indices.is_empty() {
                                let node = AstNode::new("block")
                                    .with_payload(serde_json::json!({ "children": [] }));
                                self.ast.borrow_mut().push(node).index()
                            } else {
                                indices[0]
                            }
                        }
                    };
                    AstRef::new(idx)
                } else {
                    self.expression_inner(lang, cursor, rbp_right)?
                };
                children.push(child.index());
                // Consume separator between children (not after the last).
                if i + 1 < normal {
                    if let Some(sep) = shape.child_separators.get(i) {
                        Self::consume_separator(cursor, sep)?;
                    }
                }
            }
            if let Some(d) = delim {
                let result = self.parse_delimited_block(lang, cursor, d)?;
                match result {
                    DelimitedResult::Container(idx) => children.push(idx),
                    DelimitedResult::Elements(indices) => children.extend(indices),
                }
            }
            let payload = serde_json::json!({ "children": children });
            let node = AstNode::new(infix_kind).with_payload(payload);
            left = self.ast.borrow_mut().push(node);
        }

        Ok(left)
    }

    /// Consume a [`ChildSeparator`] from the cursor (ADR-0053).
    fn consume_separator(cursor: &mut Cursor<'_>, sep: &ChildSeparator) -> Result<(), LangError> {
        match sep {
            ChildSeparator::Required(kind) => {
                let tok = cursor.advance()?;
                if tok.kind != *kind {
                    return Err(LangError::engine_at(
                        format!("expected separator {}, got {}", kind, tok.kind),
                        tok.span(),
                        Some("token".to_string()),
                    ));
                }
            }
            ChildSeparator::Optional(kind) => {
                if let Some(t) = cursor.peek() {
                    if t.kind == *kind {
                        let _ = cursor.advance()?;
                    }
                }
            }
            ChildSeparator::OneOf(kinds) => {
                let tok = cursor.advance()?;
                if !kinds.iter().any(|k| k == &tok.kind) {
                    return Err(LangError::engine_at(
                        format!("expected one of {:?}, got {}", kinds, tok.kind),
                        tok.span(),
                        Some("token".to_string()),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Parse a delimited block (ADR-0053). Consumes the `open` token (unless
    /// `open_is_operator_token` is set, in which case the open was the
    /// prefix/infix token itself, already consumed by the caller), zero or
    /// more elements separated by `separator`, and the `close` token.
    ///
    /// Each element is either a single expression (`pair` is `None`) or a
    /// pair `<expr> <pair sep> <expr>` emitted as a node of
    /// `pair.node_kind` with `children = [lhs, rhs]` (e.g. a match arm
    /// `pattern : body`). When `rest_token` is set and the next token matches
    /// it, a bare expression is parsed instead of a pair.
    ///
    /// When `flatten` is true, no container node is emitted — the element
    /// indices are returned directly for splicing into the parent's children.
    /// When false, a container node of `container_kind` is emitted.
    fn parse_delimited_block(
        &self,
        lang: &Language,
        cursor: &mut Cursor<'_>,
        delim: &DelimitedShape,
    ) -> Result<DelimitedResult, LangError> {
        // Consume the opening delimiter, unless the open IS the operator
        // token (already consumed by the caller).
        if !delim.open_is_operator_token {
            let open = cursor.advance()?;
            if open.kind != delim.open {
                return Err(LangError::engine_at(
                    format!("expected opening delimiter {}, got {}", delim.open, open.kind),
                    open.span(),
                    Some("token".to_string()),
                ));
            }
        }

        // The right binding power for a pair's left operand: one above the
        // pair separator's infix left binding power (so the separator is not
        // consumed by the infix loop). Falls back to a high value if the
        // separator has no infix row.
        let pair_lhs_rbp = delim.pair.as_ref().and_then(|p| {
            if delim.separator.is_none() {
                Some(0u8)
            } else {
                Self::lookup_row(lang, &p.separator, GrammarPosition::Infix)
                    .and_then(|r| r.binding_power.map(|(l, _)| l))
                    .map(|lbp| lbp.saturating_add(1))
                    .or(Some(u8::MAX))
            }
        });

        let mut elements: Vec<usize> = Vec::new();

        // Empty block: `open close` (or just `close` when open_is_operator).
        if let Some(next) = cursor.peek() {
            if next.kind == delim.close {
                let _ = cursor.advance()?;
                return if delim.flatten {
                    Ok(DelimitedResult::Elements(elements))
                } else {
                    let node = AstNode::new(&delim.container_kind)
                        .with_payload(serde_json::json!({ "children": elements }));
                    Ok(DelimitedResult::Container(
                        self.ast.borrow_mut().push(node).index(),
                    ))
                };
            }
        }

        loop {
            // Check for rest/spread token: if the next token matches
            // rest_token, parse a bare expression instead of a pair.
            let is_rest = delim
                .rest_token
                .as_ref()
                .and_then(|rt| cursor.peek().map(|t| t.kind == *rt))
                .unwrap_or(false);

            if is_rest {
                let rest = self.expression_inner(lang, cursor, 0)?;
                elements.push(rest.index());
            } else if let (Some(pair), Some(rbp)) = (delim.pair.as_ref(), pair_lhs_rbp) {
                // Parse the left side with a high enough rbp to stop before
                // the pair separator. Then check if the next token IS the
                // pair separator — if so, parse as a pair; otherwise, treat
                // the left side as a bare expression. This allows `{ expr }`
                // blocks alongside `{ key: val }` objects in the same
                // delimited form (ADR-0074 v0.1).
                let lhs = self.expression_inner(lang, cursor, rbp)?;
                let is_pair_sep = cursor
                    .peek()
                    .map(|t| t.kind == pair.separator)
                    .unwrap_or(false);
                if is_pair_sep {
                    let _ = cursor.advance()?; // consume pair separator
                    let rhs = self.expression_inner(lang, cursor, 0)?;
                    let node = AstNode::new(&pair.node_kind).with_payload(
                        serde_json::json!({ "children": [lhs.index(), rhs.index()] }),
                    );
                    let arm_ref = self.ast.borrow_mut().push(node);
                    elements.push(arm_ref.index());
                } else {
                    // Bare expression (not a pair) — use the lhs directly.
                    elements.push(lhs.index());
                }
            } else {
                let el = self.expression_inner(lang, cursor, 0)?;
                elements.push(el.index());
            }

            // After an element: separator (consume, maybe trailing) or close.
            let next = cursor.peek();
            match next {
                Some(t) if delim.separator.as_deref() == Some(t.kind.as_str()) => {
                    let _ = cursor.advance()?; // consume separator
                                               // Tolerate a trailing separator immediately before close.
                    if let Some(t2) = cursor.peek() {
                        if t2.kind == delim.close {
                            break;
                        }
                    }
                    continue;
                }
                Some(t) if t.kind == delim.close => break,
                None => {
                    return Err(LangError::engine_at(
                        "unexpected end of input in delimited block".to_string(),
                        cursor.last_span(),
                        None::<&str>,
                    ));
                }
                Some(_) if delim.separator.is_none() => {
                    if cursor
                        .peek()
                        .map(|t| {
                            t.kind.as_str() == "token.comma"
                                || t.kind.as_str() == "token.arrow.comma"
                                || t.kind.as_str() == "token.semicolon"
                        })
                        .unwrap_or(false)
                    {
                        let _ = cursor.advance()?;
                        if let Some(t2) = cursor.peek() {
                            if t2.kind == delim.close {
                                break;
                            }
                        }
                    }
                    continue;
                }
                Some(t) => {
                    return Err(LangError::engine_at(
                        format!(
                            "expected separator {} or closing {}, got {}",
                            delim.separator.as_deref().unwrap_or("(none)"),
                            delim.close,
                            t.kind
                        ),
                        t.span(),
                        Some("token".to_string()),
                    ));
                }
            }
        }

        // Consume the closing delimiter.
        let close = cursor.advance()?;
        if close.kind != delim.close {
            return Err(LangError::engine_at(
                format!("expected closing delimiter {}, got {}", delim.close, close.kind),
                close.span(),
                Some("token".to_string()),
            ));
        }

        if delim.flatten {
            Ok(DelimitedResult::Elements(elements))
        } else {
            let node = AstNode::new(&delim.container_kind)
                .with_payload(serde_json::json!({ "children": elements }));
            Ok(DelimitedResult::Container(
                self.ast.borrow_mut().push(node).index(),
            ))
        }
    }
}

/// The result of [`PrattEngine::parse_delimited_block`]: either a container
/// node (when `flatten` is false) or raw element indices for splicing into
/// the parent's children (when `flatten` is true).
enum DelimitedResult {
    /// A container node was emitted (flatten: false). The index is the arena
    /// index of the container node.
    Container(usize),
    /// Elements to splice directly into the parent's children (flatten: true).
    Elements(Vec<usize>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstShapeRef, GrammarPosition, GrammarRow, TokenKindDef, Tokens};
    use handl_plugin_engine_parse::{ParserContext, ParserRequest};
    use serde_json::json;

    /// Build a small arithmetic language: numbers, `+` (infix, lbp 10),
    /// `*` (infix, lbp 20), unary `-` (prefix).
    fn arith_lang() -> Language {
        let mut lang = Language::new();
        // Token kinds
        lang.tokens
            .merge(TokenKindDef::new("token.number").with_literal(LiteralKind::Number))
            .unwrap();
        lang.tokens.merge(TokenKindDef::new("token.plus")).unwrap();
        lang.tokens.merge(TokenKindDef::new("token.star")).unwrap();
        lang.tokens.merge(TokenKindDef::new("token.minus")).unwrap();

        // Grammar: number prefix (atom, arity 0)
        lang.grammar
            .merge(GrammarRow::new(
                "g.num",
                "token.number",
                GrammarPosition::Prefix,
                AstShapeRef::new("lit", 0),
            ))
            .unwrap();
        // Grammar: + infix (lbp 10, rbp 11)
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.plus",
                    "token.plus",
                    GrammarPosition::Infix,
                    AstShapeRef::new("binary.add", 2),
                )
                .with_binding_power(10, 11),
            )
            .unwrap();
        // Grammar: * infix (lbp 20, rbp 21)
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.star",
                    "token.star",
                    GrammarPosition::Infix,
                    AstShapeRef::new("binary.mul", 2),
                )
                .with_binding_power(20, 21),
            )
            .unwrap();
        // Grammar: - prefix (arity 1, rbp 30)
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.neg",
                    "token.minus",
                    GrammarPosition::Prefix,
                    AstShapeRef::new("unary.neg", 1),
                )
                .with_binding_power(30, 0),
            )
            .unwrap();
        lang
    }

    fn tok(kind: &str, text: &str) -> Token {
        Token {
            kind: kind.into(),
            text: text.into(),
            ..Default::default()
        }
    }

    #[test]
    fn empty_input_returns_empty_ast() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens::new());
        let ast = engine.parse(&lang, &req).unwrap();
        assert!(ast.is_empty());
    }

    #[test]
    fn single_atom_parses_to_lit() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![tok("token.number", "42")]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.len(), 1);
        let root = &ast.nodes()[0];
        assert_eq!(root.kind, "lit");
        assert_eq!(root.payload, json!(42));
    }

    #[test]
    fn prefix_only_unary_neg() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.minus", "-"),
            tok("token.number", "5"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.len(), 2);
        // Node 0: lit(5)
        assert_eq!(ast.nodes()[0].kind, "lit");
        assert_eq!(ast.nodes()[0].payload, json!(5));
        // Node 1: unary.neg(children=[0])
        assert_eq!(ast.nodes()[1].kind, "unary.neg");
        assert_eq!(ast.nodes()[1].payload, json!({"children": [0]}));
    }

    #[test]
    fn infix_only_binary_add() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.len(), 3);
        // Node 0: lit(1), Node 1: lit(2), Node 2: binary.add(children=[0,1])
        assert_eq!(ast.nodes()[0].kind, "lit");
        assert_eq!(ast.nodes()[1].kind, "lit");
        assert_eq!(ast.nodes()[2].kind, "binary.add");
        assert_eq!(ast.nodes()[2].payload, json!({"children": [0, 1]}));
    }

    #[test]
    fn both_prefix_and_infix_negation_then_add() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.minus", "-"),
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.len(), 4);
        // Node 0: lit(1), Node 1: unary.neg([0]), Node 2: lit(2), Node 3: binary.add([1,2])
        assert_eq!(ast.nodes()[0].kind, "lit");
        assert_eq!(ast.nodes()[1].kind, "unary.neg");
        assert_eq!(ast.nodes()[2].kind, "lit");
        assert_eq!(ast.nodes()[3].kind, "binary.add");
        assert_eq!(ast.nodes()[3].payload, json!({"children": [1, 2]}));
    }

    #[test]
    fn binding_power_precedence_mul_binds_tighter() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
            tok("token.star", "*"),
            tok("token.number", "3"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Expected: 1 + (2 * 3)
        // Node 0: lit(1), Node 1: lit(2), Node 2: lit(3), Node 3: binary.mul([1,2]), Node 4: binary.add([0,3])
        assert_eq!(ast.len(), 5);
        assert_eq!(ast.nodes()[3].kind, "binary.mul");
        assert_eq!(ast.nodes()[3].payload, json!({"children": [1, 2]}));
        assert_eq!(ast.nodes()[4].kind, "binary.add");
        assert_eq!(ast.nodes()[4].payload, json!({"children": [0, 3]}));
    }

    #[test]
    fn eof_handling_stops_infix_loop() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Should parse cleanly and stop at EOF.
        assert_eq!(ast.len(), 3);
    }

    #[test]
    fn unexpected_token_errors() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        // token.bang has no prefix or infix rule.
        let req = ParserRequest::new(Tokens(vec![tok("token.bang", "!")]));
        let err = engine.parse(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
        let msg = format!("{}", err);
        assert!(msg.contains("unexpected token"));
    }

    #[test]
    fn unexpected_token_after_partial_parse() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        // 1 + ! — the "!" has no prefix rule.
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.bang", "!"),
        ]));
        let err = engine.parse(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn infix_without_binding_power_errors() {
        let engine = PrattEngine::new();
        let mut lang = arith_lang();
        // Add a broken infix row with no binding_power.
        lang.grammar
            .merge(GrammarRow::new(
                "g.broken",
                "token.q",
                GrammarPosition::Infix,
                AstShapeRef::new("binary.q", 2),
            ))
            .unwrap();
        lang.tokens.merge(TokenKindDef::new("token.q")).unwrap();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.q", "q"),
            tok("token.number", "2"),
        ]));
        let err = engine.parse(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
        let msg = format!("{}", err);
        assert!(msg.contains("binding power"));
    }

    #[test]
    fn atom_string_payload_for_non_number() {
        let engine = PrattEngine::new();
        let mut lang = Language::new();
        lang.tokens
            .merge(TokenKindDef::new("token.identifier"))
            .unwrap();
        lang.grammar
            .merge(GrammarRow::new(
                "g.ident",
                "token.identifier",
                GrammarPosition::Prefix,
                AstShapeRef::new("identifier", 0),
            ))
            .unwrap();
        let req = ParserRequest::new(Tokens(vec![tok("token.identifier", "foo")]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.nodes()[0].kind, "identifier");
        assert_eq!(ast.nodes()[0].payload, json!("foo"));
    }

    #[test]
    fn atom_float_payload_for_float_number() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![tok("token.number", "3.14")]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.nodes()[0].payload, json!(3.14));
    }

    #[test]
    fn root_is_last_node() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        let root_idx = ast.len() - 1;
        assert_eq!(ast.nodes()[root_idx].kind, "binary.add");
    }

    #[test]
    fn parser_context_expression_delegates() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let tokens = vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ];
        // Reset arena first.
        *engine.ast.borrow_mut() = Ast::new();
        let mut ctx = ParserContext::new(&engine, &lang, &tokens);
        let root = ctx.expression(0).unwrap();
        assert_eq!(root.index(), 2); // binary.add is node 2
        assert_eq!(ctx.pos(), 3); // consumed all 3 tokens
    }

    #[test]
    fn trait_expression_method_directly() {
        // Call the ParserEngine::expression trait method directly (not via
        // parse) to ensure the trait impl is covered.
        use handl_plugin_engine_parse::ParserEngine as _;
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let tokens = vec![
            tok("token.number", "5"),
            tok("token.star", "*"),
            tok("token.number", "3"),
        ];
        *engine.ast.borrow_mut() = Ast::new();
        let mut ctx = ParserContext::new(&engine, &lang, &tokens);
        // Call the trait method directly.
        let root = PrattEngine::expression(&engine, &mut ctx, 0).unwrap();
        // binary.mul is the last node pushed (index 2).
        assert_eq!(root.index(), 2);
        assert_eq!(ctx.pos(), 3);
    }

    #[test]
    fn atom_payload_token_kind_not_in_tokens_fragment_set() {
        // When a token kind has a grammar row but is NOT registered in
        // lang.tokens, atom_payload's get_by_identity returns None and
        // the LiteralKind::Raw fallback is used (ADR-0053).
        let engine = PrattEngine::new();
        let mut lang = Language::new();
        // Register a grammar row for "token.foo" but do NOT register
        // "token.foo" in lang.tokens.
        lang.grammar
            .merge(GrammarRow::new(
                "g.foo",
                "token.foo",
                GrammarPosition::Prefix,
                AstShapeRef::new("foo", 0),
            ))
            .unwrap();
        let req = ParserRequest::new(Tokens(vec![tok("token.foo", "hello")]));
        let ast = engine.parse(&lang, &req).unwrap();
        // atom_payload falls back to LiteralKind::Raw (verbatim text).
        assert_eq!(ast.nodes()[0].payload, json!("hello"));
    }

    #[test]
    fn trait_dispatch_via_generic() {
        fn run_parse<E: ParserEngine>(
            engine: &E,
            lang: &Language,
            req: &ParserRequest,
        ) -> Result<Ast, LangError> {
            engine.parse(lang, req)
        }
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![tok("token.number", "42")]));
        let ast = run_parse(&engine, &lang, &req).unwrap();
        assert_eq!(ast.len(), 1);
        assert_eq!(ast.nodes()[0].payload, json!(42));
    }

    #[test]
    fn default_constructor() {
        let engine = PrattEngine::default();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![tok("token.number", "1")]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.len(), 1);
    }

    #[test]
    fn multiple_parses_reset_arena() {
        let engine = PrattEngine::new();
        let lang = arith_lang();
        // First parse
        let req1 = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]));
        let ast1 = engine.parse(&lang, &req1).unwrap();
        assert_eq!(ast1.len(), 3);
        // Second parse should not contain nodes from the first.
        let req2 = ParserRequest::new(Tokens(vec![tok("token.number", "99")]));
        let ast2 = engine.parse(&lang, &req2).unwrap();
        assert_eq!(ast2.len(), 1);
        assert_eq!(ast2.nodes()[0].payload, json!(99));
    }

    #[test]
    fn left_associative_chain() {
        // 1 + 2 + 3 should parse as (1 + 2) + 3 (left-assoc).
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
            tok("token.plus", "+"),
            tok("token.number", "3"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Node 0: lit(1), Node 1: lit(2), Node 2: binary.add([0,1]),
        // Node 3: lit(3), Node 4: binary.add([2,3])
        assert_eq!(ast.len(), 5);
        assert_eq!(ast.nodes()[2].kind, "binary.add");
        assert_eq!(ast.nodes()[2].payload, json!({"children": [0, 1]}));
        assert_eq!(ast.nodes()[4].kind, "binary.add");
        assert_eq!(ast.nodes()[4].payload, json!({"children": [2, 3]}));
    }

    #[test]
    fn nested_prefix_in_infix() {
        // -1 + 2 → (-1) + 2
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.minus", "-"),
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Node 0: lit(1), Node 1: unary.neg([0]), Node 2: lit(2), Node 3: binary.add([1,2])
        assert_eq!(ast.len(), 4);
        assert_eq!(ast.nodes()[3].payload, json!({"children": [1, 2]}));
    }

    #[test]
    fn number_token_non_numeric_text_falls_back_to_string() {
        // A "token.number" with text that can't parse as number → string.
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![tok("token.number", "abc")]));
        let ast = engine.parse(&lang, &req).unwrap();
        assert_eq!(ast.nodes()[0].payload, json!("abc"));
    }

    #[test]
    fn prefix_op_at_eof_errors() {
        // Unary minus with no operand → cursor advance at EOF errors.
        let engine = PrattEngine::new();
        let lang = arith_lang();
        let req = ParserRequest::new(Tokens(vec![tok("token.minus", "-")]));
        let err = engine.parse(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn next_token_without_infix_rule_stops() {
        // After parsing an atom, the next token exists but has no infix
        // rule → the infix loop breaks (None => break path).
        let engine = PrattEngine::new();
        let mut lang = arith_lang();
        // Add a token kind with a prefix rule but no infix rule.
        lang.tokens.merge(TokenKindDef::new("token.x")).unwrap();
        lang.grammar
            .merge(GrammarRow::new(
                "g.x",
                "token.x",
                GrammarPosition::Prefix,
                AstShapeRef::new("x", 0),
            ))
            .unwrap();
        // Tokens: number(1) x — after parsing number(1), the next token
        // is "x" which has no infix rule, so parsing stops.
        let req = ParserRequest::new(Tokens(vec![tok("token.number", "1"), tok("token.x", "x")]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Only the number atom is parsed; "x" is left unconsumed.
        assert_eq!(ast.len(), 1);
        assert_eq!(ast.nodes()[0].kind, "lit");
    }

    #[test]
    fn infix_arity_three_parses_extra_operands() {
        // An infix operator with arity 3 parses left + right + one extra.
        let engine = PrattEngine::new();
        let mut lang = arith_lang();
        lang.tokens
            .merge(TokenKindDef::new("token.ternary"))
            .unwrap();
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.ternary",
                    "token.ternary",
                    GrammarPosition::Infix,
                    AstShapeRef::new("ternary", 3),
                )
                .with_binding_power(5, 6),
            )
            .unwrap();
        // Tokens: 1 ? 2 3 — ternary infix with arity 3
        let req = ParserRequest::new(Tokens(vec![
            tok("token.number", "1"),
            tok("token.ternary", "?"),
            tok("token.number", "2"),
            tok("token.number", "3"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Node 0: lit(1), Node 1: lit(2), Node 2: lit(3)
        // Node 3: ternary {children: [0, 1, 2]}
        assert_eq!(ast.len(), 4);
        assert_eq!(ast.nodes()[3].kind, "ternary");
        assert_eq!(ast.nodes()[3].payload, json!({"children": [0, 1, 2]}));
    }

    #[test]
    fn vau_prefix_parses_child_expression() {
        // `vau 1 + 2` — the `vau` keyword is a Prefix operator with arity 1
        // and binding_power [0, 0]. With rbp 0, the single child parses the
        // entire remaining expression (`1 + 2`) as a `binary.add` node.
        // (ADR-0045: vau is a typed fexpr/ArrowApply escape.)
        let engine = PrattEngine::new();
        let mut lang = arith_lang();
        // Register the vau token kind and grammar row (mirrors the canonical
        // `grammar.vau.prefix` row in handl-kit-data-canonical).
        lang.tokens.merge(TokenKindDef::new("token.vau")).unwrap();
        lang.grammar
            .merge(
                GrammarRow::new(
                    "grammar.vau.prefix",
                    "token.vau",
                    GrammarPosition::Prefix,
                    AstShapeRef::new("vau", 1),
                )
                .with_binding_power(0, 0),
            )
            .unwrap();
        // Tokens: vau 1 + 2
        let req = ParserRequest::new(Tokens(vec![
            tok("token.vau", "vau"),
            tok("token.number", "1"),
            tok("token.plus", "+"),
            tok("token.number", "2"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Node 0: lit(1), Node 1: lit(2), Node 2: binary.add([0,1]),
        // Node 3: vau([2]) — root is the last node pushed.
        assert_eq!(ast.len(), 4);
        let root = &ast.nodes()[3];
        assert_eq!(root.kind, "vau");
        assert_eq!(root.payload, json!({"children": [2]}));
        // The single child is the binary.add node.
        let child = &ast.nodes()[2];
        assert_eq!(child.kind, "binary.add");
        assert_eq!(child.payload, json!({"children": [0, 1]}));
    }

    /// Build a tiny language with a delimited-block prefix form modelled on
    /// `match x { p: b, ... }`: numbers, identifiers, a `match` keyword whose
    /// last child is a `{ }` block of comma-separated `pattern : body` arms.
    fn match_lang() -> Language {
        use handl_language::DelimitedShape;
        let mut lang = Language::new();
        lang.tokens
            .merge(TokenKindDef::new("token.number").with_literal(LiteralKind::Number))
            .unwrap();
        for k in [
            "token.identifier",
            "token.keyword.match",
            "token.lbrace",
            "token.rbrace",
            "token.comma",
            "token.colon",
        ] {
            lang.tokens.merge(TokenKindDef::new(k)).unwrap();
        }
        // Atoms.
        lang.grammar
            .merge(GrammarRow::new(
                "g.num",
                "token.number",
                GrammarPosition::Prefix,
                AstShapeRef::new("value", 0),
            ))
            .unwrap();
        lang.grammar
            .merge(GrammarRow::new(
                "g.ident",
                "token.identifier",
                GrammarPosition::Prefix,
                AstShapeRef::new("var", 0),
            ))
            .unwrap();
        // `:` is an infix operator (lbp 12) — the delimited-block pair parser
        // must parse the pattern with rbp 13 so the colon is left for it to
        // consume, rather than being absorbed as a `type-annot` node.
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.colon",
                    "token.colon",
                    GrammarPosition::Infix,
                    AstShapeRef::new("type-annot", 2),
                )
                .with_binding_power(12, 13),
            )
            .unwrap();
        // `match` prefix: arity 2 (scrutinee + arms block), last child
        // delimited.
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.match",
                    "token.keyword.match",
                    GrammarPosition::Prefix,
                    AstShapeRef::new("match", 2).with_delimited(DelimitedShape {
                        open: "token.lbrace".into(),
                        close: "token.rbrace".into(),
                        separator: Some("token.comma".into()),
                        container_kind: "block".into(),
                        pair: Some(handl_language::PairShape {
                            separator: "token.colon".into(),
                            node_kind: "arm".into(),
                        }),
                        open_is_operator_token: false,
                        flatten: false,
                        rest_token: None,
                    }),
                )
                .with_binding_power(0, 0),
            )
            .unwrap();
        lang
    }

    #[test]
    fn delimited_block_match_parses_arms() {
        // `match x { a: 1, b: 2 }`
        let engine = PrattEngine::new();
        let lang = match_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.keyword.match", "match"),
            tok("token.identifier", "x"),
            tok("token.lbrace", "{"),
            tok("token.identifier", "a"),
            tok("token.colon", ":"),
            tok("token.number", "1"),
            tok("token.comma", ","),
            tok("token.identifier", "b"),
            tok("token.colon", ":"),
            tok("token.number", "2"),
            tok("token.rbrace", "}"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // Expected (post-order):
        // 0: var(x), 1: var(a), 2: value(1), 3: arm{[1,2]},
        // 4: var(b), 5: value(2), 6: arm{[4,5]},
        // 7: block{[3,6]}, 8: match{[0,7]}
        assert_eq!(ast.len(), 9);
        assert_eq!(ast.nodes()[0].kind, "var");
        assert_eq!(ast.nodes()[0].payload, json!("x"));
        assert_eq!(ast.nodes()[3].kind, "arm");
        assert_eq!(ast.nodes()[3].payload, json!({"children": [1, 2]}));
        assert_eq!(ast.nodes()[6].kind, "arm");
        assert_eq!(ast.nodes()[6].payload, json!({"children": [4, 5]}));
        assert_eq!(ast.nodes()[7].kind, "block");
        assert_eq!(ast.nodes()[7].payload, json!({"children": [3, 6]}));
        let root = &ast.nodes()[8];
        assert_eq!(root.kind, "match");
        assert_eq!(root.payload, json!({"children": [0, 7]}));
    }

    #[test]
    fn delimited_block_empty_and_trailing_separator() {
        let engine = PrattEngine::new();
        let lang = match_lang();
        // `match x { }` — empty block.
        let req = ParserRequest::new(Tokens(vec![
            tok("token.keyword.match", "match"),
            tok("token.identifier", "x"),
            tok("token.lbrace", "{"),
            tok("token.rbrace", "}"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // 0: var(x), 1: block{[]}, 2: match{[0,1]}
        assert_eq!(ast.len(), 3);
        assert_eq!(ast.nodes()[1].kind, "block");
        assert_eq!(ast.nodes()[1].payload, json!({"children": []}));
        assert_eq!(ast.nodes()[2].kind, "match");
        assert_eq!(ast.nodes()[2].payload, json!({"children": [0, 1]}));

        // `match x { a: 1, }` — trailing separator tolerated.
        let req2 = ParserRequest::new(Tokens(vec![
            tok("token.keyword.match", "match"),
            tok("token.identifier", "x"),
            tok("token.lbrace", "{"),
            tok("token.identifier", "a"),
            tok("token.colon", ":"),
            tok("token.number", "1"),
            tok("token.comma", ","),
            tok("token.rbrace", "}"),
        ]));
        let ast2 = engine.parse(&lang, &req2).unwrap();
        // 0: var(x), 1: var(a), 2: value(1), 3: arm{[1,2]},
        // 4: block{[3]}, 5: match{[0,4]}
        assert_eq!(ast2.len(), 6);
        assert_eq!(ast2.nodes()[4].kind, "block");
        assert_eq!(ast2.nodes()[4].payload, json!({"children": [3]}));
    }

    #[test]
    fn delimited_block_missing_open_errors() {
        let engine = PrattEngine::new();
        let lang = match_lang();
        // `match x a: 1` — scrutinee `x`, then no `{` for the block.
        let req = ParserRequest::new(Tokens(vec![
            tok("token.keyword.match", "match"),
            tok("token.identifier", "x"),
            tok("token.identifier", "a"),
            tok("token.colon", ":"),
            tok("token.number", "1"),
        ]));
        let err = engine.parse(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
        let msg = format!("{}", err);
        assert!(msg.contains("opening delimiter"));
    }

    /// Build a language with array, object, paren, function, and call support
    /// modelled on the canonical kit, for testing delimited-literal parsing.
    /// Uses the ADR-0053 generic delimited-block schema (open_is_operator_token,
    /// flatten, rest_token, pair) instead of hardcoded kind checks.
    fn literal_lang() -> Language {
        let mut lang = Language::new();
        // Token kinds with LiteralKind dispatch (ADR-0053).
        lang.tokens
            .merge(TokenKindDef::new("token.number").with_literal(LiteralKind::Number))
            .unwrap();
        lang.tokens
            .merge(
                TokenKindDef::new("token.string").with_literal(LiteralKind::String {
                    delim_chars: 1,
                    escapes: vec![
                        ("\\n".into(), "\n".into()),
                        ("\\r".into(), "\r".into()),
                        ("\\t".into(), "\t".into()),
                        ("\\\"".into(), "\"".into()),
                        ("\\\\".into(), "\\".into()),
                    ],
                }),
            )
            .unwrap();
        for k in [
            "token.identifier",
            "token.lbracket",
            "token.rbracket",
            "token.lbrace",
            "token.rbrace",
            "token.lparen",
            "token.rparen",
            "token.comma",
            "token.colon",
            "token.arrow",
            "token.plus",
            "token.ellipsis",
        ] {
            lang.tokens.merge(TokenKindDef::new(k)).unwrap();
        }
        // Ellipsis prefix: `...rest` produces a `rest` node with arity 1.
        lang.grammar
            .merge(
                GrammarRow::new(
                    "grammar.ellipsis.prefix",
                    "token.ellipsis",
                    GrammarPosition::Prefix,
                    AstShapeRef::new("rest", 1),
                )
                .with_binding_power(0, 0),
            )
            .unwrap();
        lang.grammar
            .merge(GrammarRow::new(
                "g.num",
                "token.number",
                GrammarPosition::Prefix,
                AstShapeRef::new("value", 0),
            ))
            .unwrap();
        lang.grammar
            .merge(GrammarRow::new(
                "g.str",
                "token.string",
                GrammarPosition::Prefix,
                AstShapeRef::new("value", 0),
            ))
            .unwrap();
        lang.grammar
            .merge(GrammarRow::new(
                "g.ident",
                "token.identifier",
                GrammarPosition::Prefix,
                AstShapeRef::new("var", 0),
            ))
            .unwrap();
        // Array: `[` is both the prefix token and the opening delimiter
        // (open_is_operator_token). Elements are spliced directly into the
        // array node's children (flatten). Supports `...rest` via rest_token.
        lang.grammar
            .merge(GrammarRow::new(
                "g.array",
                "token.lbracket",
                GrammarPosition::Prefix,
                AstShapeRef::new("array", 1).with_delimited(DelimitedShape {
                    open: "token.lbracket".into(),
                    close: "token.rbracket".into(),
                    separator: Some("token.comma".into()),
                    container_kind: "array".into(),
                    pair: None,
                    open_is_operator_token: true,
                    flatten: true,
                    rest_token: Some("token.ellipsis".into()),
                }),
            ))
            .unwrap();
        // Object: `{` is both the prefix token and the opening delimiter.
        // Elements are key:value pairs (entry nodes). Supports `...rest`.
        lang.grammar
            .merge(GrammarRow::new(
                "g.object",
                "token.lbrace",
                GrammarPosition::Prefix,
                AstShapeRef::new("object", 1).with_delimited(DelimitedShape {
                    open: "token.lbrace".into(),
                    close: "token.rbrace".into(),
                    separator: Some("token.comma".into()),
                    container_kind: "object".into(),
                    pair: Some(handl_language::PairShape {
                        separator: "token.colon".into(),
                        node_kind: "entry".into(),
                    }),
                    open_is_operator_token: true,
                    flatten: true,
                    rest_token: Some("token.ellipsis".into()),
                }),
            ))
            .unwrap();
        // Paren: `(` is both the prefix token and the opening delimiter.
        // Elements are single expressions (function params).
        lang.grammar
            .merge(GrammarRow::new(
                "g.paren",
                "token.lparen",
                GrammarPosition::Prefix,
                AstShapeRef::new("paren", 1).with_delimited(DelimitedShape {
                    open: "token.lparen".into(),
                    close: "token.rparen".into(),
                    separator: Some("token.comma".into()),
                    container_kind: "paren".into(),
                    pair: None,
                    open_is_operator_token: true,
                    flatten: true,
                    rest_token: None,
                }),
            ))
            .unwrap();
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.colon",
                    "token.colon",
                    GrammarPosition::Infix,
                    AstShapeRef::new("type-annot", 2),
                )
                .with_binding_power(12, 13),
            )
            .unwrap();
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.arrow",
                    "token.arrow",
                    GrammarPosition::Infix,
                    AstShapeRef::new("function", 2),
                )
                .with_binding_power(31, 32),
            )
            .unwrap();
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.plus",
                    "token.plus",
                    GrammarPosition::Infix,
                    AstShapeRef::new("binary.add", 2),
                )
                .with_binding_power(55, 56),
            )
            .unwrap();
        // Call: `(` is the infix operator token and the opening delimiter
        // (open_is_operator_token). Args are spliced directly into the call
        // node's children (flatten) — no nested args container.
        lang.grammar
            .merge(
                GrammarRow::new(
                    "g.call",
                    "token.lparen",
                    GrammarPosition::Infix,
                    AstShapeRef::new("call", 1).with_delimited(DelimitedShape {
                        open: "token.lparen".into(),
                        close: "token.rparen".into(),
                        separator: Some("token.comma".into()),
                        container_kind: "call".into(),
                        pair: None,
                        open_is_operator_token: true,
                        flatten: true,
                        rest_token: None,
                    }),
                )
                .with_binding_power(90, 91),
            )
            .unwrap();
        lang
    }

    #[test]
    fn array_literal_parses_elements() {
        let engine = PrattEngine::new();
        let lang = literal_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.lbracket", "["),
            tok("token.number", "1"),
            tok("token.comma", ","),
            tok("token.number", "2"),
            tok("token.comma", ","),
            tok("token.number", "3"),
            tok("token.rbracket", "]"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // 0: value(1), 1: value(2), 2: value(3), 3: array{[0,1,2]}
        assert_eq!(ast.len(), 4);
        assert_eq!(ast.nodes()[3].kind, "array");
        assert_eq!(ast.nodes()[3].payload, json!({"children": [0, 1, 2]}));
    }

    #[test]
    fn object_literal_parses_entries() {
        let engine = PrattEngine::new();
        let lang = literal_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.lbrace", "{"),
            tok("token.identifier", "a"),
            tok("token.colon", ":"),
            tok("token.number", "1"),
            tok("token.comma", ","),
            tok("token.identifier", "b"),
            tok("token.colon", ":"),
            tok("token.number", "2"),
            tok("token.rbrace", "}"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // 0: var(a), 1: value(1), 2: entry{[0,1]},
        // 3: var(b), 4: value(2), 5: entry{[3,4]},
        // 6: object{[2,5]}
        assert_eq!(ast.len(), 7);
        assert_eq!(ast.nodes()[2].kind, "entry");
        assert_eq!(ast.nodes()[2].payload, json!({"children": [0, 1]}));
        assert_eq!(ast.nodes()[5].kind, "entry");
        assert_eq!(ast.nodes()[5].payload, json!({"children": [3, 4]}));
        assert_eq!(ast.nodes()[6].kind, "object");
        assert_eq!(ast.nodes()[6].payload, json!({"children": [2, 5]}));
    }

    #[test]
    fn paren_parses_multiple_params() {
        let engine = PrattEngine::new();
        let lang = literal_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.lparen", "("),
            tok("token.identifier", "x"),
            tok("token.comma", ","),
            tok("token.identifier", "y"),
            tok("token.rparen", ")"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // 0: var(x), 1: var(y), 2: paren{[0,1]}
        assert_eq!(ast.len(), 3);
        assert_eq!(ast.nodes()[2].kind, "paren");
        assert_eq!(ast.nodes()[2].payload, json!({"children": [0, 1]}));
    }

    #[test]
    fn function_with_paren_params_parses() {
        let engine = PrattEngine::new();
        let lang = literal_lang();
        // (x, y) -> x + y
        let req = ParserRequest::new(Tokens(vec![
            tok("token.lparen", "("),
            tok("token.identifier", "x"),
            tok("token.comma", ","),
            tok("token.identifier", "y"),
            tok("token.rparen", ")"),
            tok("token.arrow", "~>"),
            tok("token.identifier", "x"),
            tok("token.plus", "+"),
            tok("token.identifier", "y"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // 0: var(x), 1: var(y), 2: paren{[0,1]},
        // 3: var(x), 4: var(y), 5: binary.add{[3,4]},
        // 6: function{[2,5]}
        assert_eq!(ast.len(), 7);
        assert_eq!(ast.nodes()[6].kind, "function");
        let children = ast.nodes()[6]
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(children.len(), 2);
        // children[0] = paren idx, children[1] = body idx
        assert_eq!(
            ast.nodes()[children[0].as_u64().unwrap() as usize].kind,
            "paren"
        );
        assert_eq!(
            ast.nodes()[children[1].as_u64().unwrap() as usize].kind,
            "binary.add"
        );
    }

    #[test]
    fn array_literal_parses_rest_explicit() {
        // [1, ...rest] → array{children:[value(1), rest{[var(rest)]}]}
        // The rest_token field on the array's DelimitedShape tells
        // parse_delimited_block to parse a bare expression when it sees
        // token.ellipsis (ADR-0053).
        let engine = PrattEngine::new();
        let lang = literal_lang();
        let req = ParserRequest::new(Tokens(vec![
            tok("token.lbracket", "["),
            tok("token.number", "1"),
            tok("token.comma", ","),
            tok("token.ellipsis", "..."),
            tok("token.identifier", "rest"),
            tok("token.rbracket", "]"),
        ]));
        let ast = engine.parse(&lang, &req).unwrap();
        // The array node is the last node pushed.
        let array_idx = ast.len() - 1;
        let array_node = &ast.nodes()[array_idx];
        assert_eq!(array_node.kind, "array");
        let children = array_node
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(children.len(), 2);
        // First child: value(1)
        let val_node = &ast.nodes()[children[0].as_u64().unwrap() as usize];
        assert_eq!(val_node.payload, json!(1));
        // Second child: rest node
        let rest_node = &ast.nodes()[children[1].as_u64().unwrap() as usize];
        assert_eq!(rest_node.kind, "rest");
        let rest_children = rest_node
            .payload
            .get("children")
            .unwrap()
            .as_array()
            .unwrap();
        let var_node = &ast.nodes()[rest_children[0].as_u64().unwrap() as usize];
        assert_eq!(var_node.kind, "var");
        assert_eq!(var_node.payload, json!("rest"));
    }

    #[test]
    fn empty_array_and_object_parse() {
        let engine = PrattEngine::new();
        let lang = literal_lang();
        let ast = engine
            .parse(
                &lang,
                &ParserRequest::new(Tokens(vec![
                    tok("token.lbracket", "["),
                    tok("token.rbracket", "]"),
                ])),
            )
            .unwrap();
        assert_eq!(ast.len(), 1);
        assert_eq!(ast.nodes()[0].kind, "array");
        assert_eq!(ast.nodes()[0].payload, json!({"children": []}));

        let ast = engine
            .parse(
                &lang,
                &ParserRequest::new(Tokens(vec![
                    tok("token.lbrace", "{"),
                    tok("token.rbrace", "}"),
                ])),
            )
            .unwrap();
        assert_eq!(ast.len(), 1);
        assert_eq!(ast.nodes()[0].kind, "object");
        assert_eq!(ast.nodes()[0].payload, json!({"children": []}));
    }
}
