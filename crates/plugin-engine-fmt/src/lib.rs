//! Data-driven AST formatter engine (ADR-0044 "language as data").
//!
//! The [`FormatEngine`] reads `lang.grammar` (reverse-lookup node kind to
//! token and position), `lang.tokens` (token to label text), and `lang.ast`
//! (child roles), then walks an [`Ast`] and emits surface syntax. It never
//! matches on hardcoded kind strings — all kind-to-syntax mapping comes from
//! `Language` data. Only the rendering strategy (infix / prefix / delimited)
//! is fixed Rust, part of the calculus vocabulary like the Pratt parser's
//! prefix/infix loop.

use std::collections::HashMap;

use handl_language::{
    Ast, AstNode, AstRef, AstShapeRef, ChildSeparator, DelimitedShape, GrammarPosition, GrammarRow,
    LangError, Language,
};
use serde_json::Value;

/// Formatting options for the [`FormatEngine`].
#[derive(Debug, Clone)]
pub struct FormatOptions {
    /// Number of spaces per indentation level.
    pub indent_size: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self { indent_size: 2 }
    }
}

/// A data-driven AST formatter that reads `Language` data to produce surface
/// syntax.
///
/// The engine builds a reverse index from `lang.grammar` (node kind → grammar
/// row) on each [`format`](Self::format) call, then recursively walks the
/// `Ast` arena. Kind→syntax mapping is entirely data-driven; the only fixed
/// logic is the rendering strategy (infix / prefix / delimited / leaf), which
/// is part of the calculus vocabulary.
#[derive(Debug, Clone)]
pub struct FormatEngine {
    options: FormatOptions,
}

impl Default for FormatEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatEngine {
    /// Create a new engine with default options (2-space indent).
    pub fn new() -> Self {
        Self {
            options: FormatOptions::default(),
        }
    }

    /// Create a new engine with the given options.
    pub fn with_options(options: FormatOptions) -> Self {
        Self { options }
    }

    /// Format the subtree rooted at `root` in `ast` using `lang` data.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::engine`] if a referenced child index is out of
    /// bounds or the AST is malformed.
    pub fn format(&self, lang: &Language, ast: &Ast, root: AstRef) -> Result<String, LangError> {
        let index = build_grammar_index(lang);
        let ctx = FormatCtx {
            lang,
            ast,
            index: &index,
            indent_size: self.options.indent_size,
        };
        ctx.format_node(root, 0)
    }
}

/// Reverse index: `child_shape.node_kind` → grammar row.
type GrammarIndex<'a> = HashMap<String, &'a GrammarRow>;

fn build_grammar_index(lang: &Language) -> GrammarIndex<'_> {
    lang.grammar
        .iter()
        .map(|row| (row.child_shape.node_kind.clone(), row))
        .collect()
}

/// Internal formatting context.
struct FormatCtx<'a> {
    lang: &'a Language,
    ast: &'a Ast,
    index: &'a GrammarIndex<'a>,
    indent_size: usize,
}

impl<'a> FormatCtx<'a> {
    fn indent(&self, level: usize) -> String {
        " ".repeat(level * self.indent_size)
    }

    /// Resolve the children array of a node into `AstRef`s.
    fn children_of(&self, node: &AstNode) -> Vec<AstRef> {
        node.payload
            .get("children")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|i| AstRef::new(i as usize)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Look up a token's label (surface text) from `lang.tokens`.
    fn token_label(&self, token_id: &str) -> Option<&str> {
        self.lang
            .tokens
            .get_by_identity(token_id)
            .and_then(|t| t.label.as_deref())
    }

    /// Recursively format a node at the given indent level.
    fn format_node(&self, r: AstRef, indent: usize) -> Result<String, LangError> {
        let node = self.ast.at(r)?;

        if let Some(row) = self.index.get(&node.kind) {
            return self.format_grammar_node(node, row, indent);
        }
        Ok(self.format_leaf(node))
    }

    /// Format a node that has a grammar row (infix / prefix / delimited).
    fn format_grammar_node(
        &self,
        node: &AstNode,
        row: &GrammarRow,
        indent: usize,
    ) -> Result<String, LangError> {
        let label = self.token_label(&row.token);
        let children = self.children_of(node);
        let shape = &row.child_shape;

        match row.position {
            GrammarPosition::Infix => {
                self.format_infix(node, row, label, &children, indent)
            }
            GrammarPosition::Prefix => {
                if shape.delimited.is_some() {
                    self.format_prefix_delimited(row, label, &children, indent)
                } else if !shape.payload_tokens.is_empty() {
                    self.format_prefix_payload(node, row, label, &children, indent)
                } else if shape.arity == 0 && label.is_none() {
                    // An arity-0 prefix with no label is an atom (e.g. `lit`):
                    // the surface text comes from the payload, not the token.
                    Ok(self.format_leaf(node))
                } else {
                    self.format_prefix_simple(node, row, label, &children, indent)
                }
            }
        }
    }

    /// Format an infix binary node: `lhs op rhs`.
    fn format_infix(
        &self,
        _node: &AstNode,
        row: &GrammarRow,
        label: Option<&str>,
        children: &[AstRef],
        indent: usize,
    ) -> Result<String, LangError> {
        let op = label.unwrap_or("");
        if children.len() == 2 {
            let lhs = self.format_child(children[0], row, 0, true, indent)?;
            let rhs = self.format_child(children[1], row, 1, false, indent)?;
            Ok(format!("{lhs} {op} {rhs}"))
        } else {
            // Fallback: render operator + all children joined.
            let mut parts: Vec<String> = Vec::new();
            for c in children {
                parts.push(self.format_node(*c, indent)?);
            }
            Ok(parts.join(&format!(" {op} ")))
        }
    }

    /// Format a simple prefix node: `op child`.
    fn format_prefix_simple(
        &self,
        _node: &AstNode,
        _row: &GrammarRow,
        label: Option<&str>,
        children: &[AstRef],
        indent: usize,
    ) -> Result<String, LangError> {
        let op = label.unwrap_or("");
        if children.is_empty() {
            return Ok(op.to_string());
        }
        let child = self.format_node(children[0], indent)?;
        Ok(format!("{op}{child}"))
    }

    /// Format a prefix node with payload tokens (e.g. `let x = ...`).
    fn format_prefix_payload(
        &self,
        node: &AstNode,
        row: &GrammarRow,
        label: Option<&str>,
        children: &[AstRef],
        indent: usize,
    ) -> Result<String, LangError> {
        let mut out = String::new();
        if let Some(kw) = label {
            out.push_str(kw);
        }

        // Render payload token values from the node's payload.
        for spec in &row.child_shape.payload_tokens {
            if let Some(field) = &spec.field {
                if let Some(val) = node.payload.get(field) {
                    if !out.is_empty() && !out.ends_with(' ') {
                        out.push(' ');
                    }
                    out.push_str(&value_to_text(val));
                }
            } else if let Some(tok_label) = self.token_label(&spec.token) {
                // A consumed-but-not-stored token (e.g. `=`); render its label.
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(tok_label);
            }
        }

        // Render children with separators.
        let seps = &row.child_shape.child_separators;
        for (i, child) in children.iter().enumerate() {
            if i > 0
                && let Some(sep) = seps.get(i - 1)
                && let Some(sep_label) = self.separator_label(sep)
            {
                out.push(' ');
                out.push_str(&sep_label);
            }
            out.push(' ');
            out.push_str(&self.format_node(*child, indent)?);
        }
        Ok(out.trim_end().to_string())
    }

    /// Format a prefix node with a delimited block (e.g. `if cond { body }`).
    fn format_prefix_delimited(
        &self,
        row: &GrammarRow,
        label: Option<&str>,
        children: &[AstRef],
        indent: usize,
    ) -> Result<String, LangError> {
        let shape = &row.child_shape;
        let delimited = shape.delimited.as_ref().expect("checked by caller");

        let mut out = String::new();

        // When `open_is_operator_token` is true, the prefix token IS the open
        // delimiter (e.g. `{` for block, `(` for paren/call). The open
        // delimiter is rendered by `format_delimited_block`, so we don't
        // render the keyword label here. Non-block children (e.g. the callee
        // in `f(...)`) go before the open delimiter.
        if !delimited.open_is_operator_token
            && let Some(kw) = label
        {
            out.push_str(kw);
        }

        // Non-block children are the first `arity - 1` children (the last
        // child slot is the delimited block). When `flatten` is true, the
        // block elements are spliced directly after the non-block children.
        let non_block_count = shape.arity.saturating_sub(1);
        for i in 0..non_block_count {
            if let Some(child) = children.get(i) {
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(&self.format_node(*child, indent)?);
            }
        }

        // Render the delimited block. Add a separating space only when there
        // is preceding content (keyword / non-block children).
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&self.format_delimited_block(shape, delimited, children, indent)?);
        Ok(out)
    }

    /// Format a delimited block (the last child of a prefix-delimited node, or
    /// a standalone delimited container).
    fn format_delimited_block(
        &self,
        shape: &AstShapeRef,
        delimited: &DelimitedShape,
        children: &[AstRef],
        indent: usize,
    ) -> Result<String, LangError> {
        let open = self.token_label(&delimited.open).unwrap_or("");
        let close = self.token_label(&delimited.close).unwrap_or("");

        // Non-block children count: `arity - 1` (the last slot is the block).
        let non_block_count = shape.arity.saturating_sub(1);

        // Collect the block elements.
        let elements: Vec<AstRef> = if delimited.flatten {
            // Flattened: elements are spliced directly into the parent's
            // children after the non-block children.
            children.iter().skip(non_block_count).copied().collect()
        } else {
            // Non-flattened: the child at index `non_block_count` is the
            // container node whose children are the elements.
            if let Some(&container_ref) = children.get(non_block_count) {
                let container = self.ast.at(container_ref)?;
                self.children_of(container)
            } else {
                Vec::new()
            }
        };

        if elements.is_empty() {
            return Ok(format!("{open} {close}"));
        }

        let sep_label = delimited
            .separator
            .as_deref()
            .and_then(|s| self.token_label(s));

        // Inline rendering: a single element with no separator and no pair
        // shape renders on one line (e.g. `( expr )`).
        if elements.len() == 1 && sep_label.is_none() && delimited.pair.is_none() {
            let rendered = self.format_node(elements[0], indent)?;
            // If the rendered content is multi-line, fall through to the
            // multi-line path below.
            if !rendered.contains('\n') {
                return Ok(format!("{open} {rendered} {close}"));
            }
        }

        let inner_indent = indent + 1;
        let pad = self.indent(inner_indent);

        let mut lines: Vec<String> = Vec::new();
        for elem_ref in &elements {
            let elem = self.ast.at(*elem_ref)?;
            // Handle pair elements (e.g. match arms `pattern : body`).
            if let Some(pair) = &delimited.pair
                && elem.kind == pair.node_kind
            {
                let pair_children = self.children_of(elem);
                let pair_sep = self.token_label(&pair.separator).unwrap_or("");
                if pair_children.len() == 2 {
                    let lhs = self.format_node(pair_children[0], inner_indent)?;
                    let rhs = self.format_node(pair_children[1], inner_indent)?;
                    lines.push(format!("{pad}{lhs} {pair_sep} {rhs}"));
                    continue;
                }
            }
            let rendered = self.format_node(*elem_ref, inner_indent)?;
            // The recursive call already produces correctly-indented output
            // for nested structures; we only need to pad the first line.
            lines.push(format!("{pad}{rendered}"));
        }

        let joined = if let Some(sep) = sep_label {
            lines.join(&format!("{sep}\n"))
        } else {
            lines.join("\n")
        };
        let closing_pad = self.indent(indent);
        Ok(format!("{open}\n{joined}\n{closing_pad}{close}"))
    }

    /// Format a child of an infix node, applying parenthesization when the
    /// child's binding power is lower than the parent's.
    fn format_child(
        &self,
        child: AstRef,
        parent_row: &GrammarRow,
        _slot: usize,
        is_left: bool,
        indent: usize,
    ) -> Result<String, LangError> {
        let rendered = self.format_node(child, indent)?;
        let child_node = self.ast.at(child)?;
        let Some(child_row) = self.index.get(&child_node.kind) else {
            return Ok(rendered);
        };
        // Only parenthesize infix children (not prefix/leaves).
        if child_row.position != GrammarPosition::Infix {
            return Ok(rendered);
        }
        let (parent_l, parent_r) = match parent_row.binding_power {
            Some(bp) => bp,
            None => return Ok(rendered),
        };
        let (child_l, child_r) = match child_row.binding_power {
            Some(bp) => bp,
            None => return Ok(rendered),
        };
        // Left child: parenthesize if child's right bp < parent's left bp.
        // Right child: parenthesize if child's left bp < parent's right bp.
        let needs_parens = if is_left {
            child_r < parent_l
        } else {
            child_l < parent_r
        };
        if needs_parens {
            Ok(format!("({rendered})"))
        } else {
            Ok(rendered)
        }
    }

    /// Resolve a [`ChildSeparator`] to its label text.
    fn separator_label(&self, sep: &ChildSeparator) -> Option<String> {
        let token = match sep {
            ChildSeparator::Required(t) | ChildSeparator::Optional(t) => t,
            ChildSeparator::OneOf(ts) => ts.first()?,
        };
        self.token_label(token).map(|s| s.to_string())
    }

    /// Format a leaf node (no grammar row) from its payload.
    fn format_leaf(&self, node: &AstNode) -> String {
        if let Some(value) = node.payload.get("value") {
            return value_to_text(value);
        }
        if let Some(name) = node.payload.get("name") {
            return value_to_text(name);
        }
        // If the payload itself is a non-object scalar, use it directly.
        if !node.payload.is_object() && !node.payload.is_null() {
            return value_to_text(&node.payload);
        }
        node.kind.clone()
    }
}

/// Convert a JSON [`Value`] to surface text.
fn value_to_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests;
