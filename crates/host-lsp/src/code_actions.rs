//! `textDocument/codeAction` support for the handl LSP server.
//!
//! Provides quick fixes for common arrow-file issues:
//!
//! * **Wrap in `do { } while` block** — a binding that is not already inside a
//!   `do { … } while` block can be wrapped so that subsequent bindings are
//!   sequenced.
//! * **Add missing `return`** — a match-arm body that is a bare object literal
//!   `{ … }` (without a `return` prefix) gets the prefix inserted.
//! * **Convert `//` to `///` doc comment** — a regular line comment at the top
//!   of the file (before any code) can be promoted to a doc comment.
//! * **Lint quick fixes (ADR-0094)** — diagnostics from the canonical linter
//!   that carry a `Fix` (e.g. `lint.simplify-paren`, matched by the transform
//!   engine's `find_matches`) are offered as quickfix actions whose edit is
//!   the fix's span-preserving text replacement.
//! * **Engine rewrite actions (ADR-0094)** — matched `lint` sites are fixed by
//!   `RewriteEngine::apply_rule` and re-emitted with `PreserveEmitter`,
//!   producing a `WorkspaceEdit` that replaces the whole statement.
//! * **Extract to named arrow** — a complex selection containing `>>>`, `|||`,
//!   or a nested `~>` arrow can be extracted into a named arrow binding.

use std::sync::Arc;

use handl_language::{Ast, AstNode, AstRef, LangError, Language};
use handl_plugin_engine_fmt::PreserveEmitter;
use handl_plugin_engine_lex_pattern::lex;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_engine_transform::native::{NativeRewriteRegistry, RewriteCtx};
use handl_plugin_engine_transform_rewrite::RewriteEngine;
use serde_json::{Value, json};

use handl_kit_canonical::{SpannedToken, is_comment_kind, is_keyword_kind};

use crate::diagnostics::offset_to_position;
use crate::kit_resolver::kit_for_file;

/// The URI used for edits in untitled buffers.
const UNTITLED_URI: &str = "file:///untitled.hnl";

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Return LSP CodeAction JSON objects for the given range and diagnostics.
///
/// `range_start`/`range_end` are byte offsets into `source`.  `diagnostics`
/// is the list of published diagnostics (LSP JSON objects) for the document;
/// their messages are inspected to offer diagnostic-driven quick fixes.
///
/// Only actions applicable to the given range/diagnostics are returned.  If
/// none apply, an empty vector is returned.
pub fn code_actions(
    source: &str,
    range_start: usize,
    range_end: usize,
    diagnostics: &[Value],
) -> Vec<Value> {
    let mut actions = Vec::new();

    // The extract action is purely string-based and needs no lexing.
    actions.extend(extract_to_named_arrow_actions(
        source,
        range_start,
        range_end,
    ));

    // Lex the source for token-based actions.
    let kit = match kit_for_file(None) {
        Ok(k) => k,
        Err(_) => return actions,
    };
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => return actions,
    };

    // Range-based actions for the user's selection / cursor position.
    actions.extend(wrap_in_do_actions(source, &tokens, range_start, range_end));
    actions.extend(add_return_actions(source, &tokens, range_start, range_end));
    actions.extend(convert_comment_actions(
        source,
        &tokens,
        range_start,
        range_end,
    ));

    // Diagnostic-based actions: if a diagnostic message mentions "return",
    // offer the corresponding fix at the diagnostic's location.
    for diag in diagnostics {
        let msg = diag.get("message").and_then(|m| m.as_str()).unwrap_or("");
        if let Some((ds, de)) = diag_range_to_offsets(source, diag) {
            if msg.contains("return") {
                actions.extend(add_return_actions(source, &tokens, ds, de));
            }
        }
    }

    // Transform-engine-driven lint quick fixes (ADR-0094).
    actions.extend(lint_fix_actions(source, range_start, range_end));

    // Rewrite-based code actions (ADR-0094 LSP consumer).
    actions.extend(engine_rewrite_actions(source, range_start, range_end));

    actions
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Convert a byte offset range into an LSP `Range` JSON object.
fn span_to_range(source: &str, start: usize, end: usize) -> Value {
    let (sl, sc) = offset_to_position(source, start);
    let (el, ec) = offset_to_position(source, end);
    json!({
        "start": { "line": sl, "character": sc },
        "end": { "line": el, "character": ec },
    })
}

/// Build a single CodeAction JSON object with one text edit.
fn make_action(title: &str, source: &str, start: usize, end: usize, new_text: &str) -> Value {
    json!({
        "title": title,
        "kind": "quickfix",
        "edit": {
            "changes": {
                UNTITLED_URI: [{
                    "range": span_to_range(source, start, end),
                    "newText": new_text,
                }]
            }
        },
        "isPreferred": true,
    })
}

/// Return the byte range `[start, end)` of the line containing `offset`,
/// excluding the trailing newline.
fn line_range(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let start = source[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = source[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(source.len());
    (start, end)
}

/// Whether the cursor / selection touches the line containing `token_offset`.
///
/// For a zero-width cursor (`range_start == range_end`) the cursor must lie
/// within the line.  For a non-empty selection the selection must overlap the
/// line.
fn range_touches_line(
    source: &str,
    range_start: usize,
    range_end: usize,
    token_offset: usize,
) -> bool {
    let (line_start, line_end) = line_range(source, token_offset);
    if range_start == range_end {
        range_start >= line_start && range_start <= line_end
    } else {
        range_start < line_end && range_end > line_start
    }
}

/// Convert an LSP diagnostic's `(line, character)` range into byte offsets.
fn diag_range_to_offsets(source: &str, diag: &Value) -> Option<(usize, usize)> {
    let range = diag.get("range")?;
    let start = range.get("start")?;
    let end = range.get("end")?;
    let sl = start.get("line")?.as_u64()? as u32;
    let sc = start.get("character")?.as_u64()? as u32;
    let el = end.get("line")?.as_u64()? as u32;
    let ec = end.get("character")?.as_u64()? as u32;
    let s = crate::document::position_to_offset(source, sl, sc);
    let e = crate::document::position_to_offset(source, el, ec);
    Some((s, e))
}

// ---------------------------------------------------------------------------
// a. Wrap in `do` block
// ---------------------------------------------------------------------------

/// Whether the byte `offset` is inside a `do { … }` block.
///
/// Walks tokens before `offset`, tracking brace depth.  A `do` keyword
/// followed by `{` pushes the depth onto a stack; the matching `}` pops it.
/// If the stack is non-empty at `offset`, we are inside a do block.
fn is_inside_do_block(tokens: &[SpannedToken], offset: usize) -> bool {
    let mut depth: i32 = 0;
    let mut do_depths: Vec<i32> = Vec::new();
    let mut pending_do = false;
    for t in tokens {
        if t.span.start >= offset {
            break;
        }
        match t.kind.as_str() {
            "token.lbrace" => {
                depth += 1;
                if pending_do {
                    do_depths.push(depth);
                    pending_do = false;
                }
            }
            "token.rbrace" => {
                if do_depths.last() == Some(&depth) {
                    do_depths.pop();
                }
                depth -= 1;
            }
            _ => {
                if is_keyword_kind(t.kind.as_str()) && t.text == "do" {
                    pending_do = true;
                } else {
                    pending_do = false;
                }
            }
        }
    }
    !do_depths.is_empty()
}

/// Offer to wrap a `<-` binding (that is not already inside a `do` block) in
/// `do { … }`.
fn wrap_in_do_actions(
    source: &str,
    tokens: &[SpannedToken],
    range_start: usize,
    range_end: usize,
) -> Vec<Value> {
    let mut actions = Vec::new();
    for i in 0..tokens.len() {
        // `<-` is lexed as two tokens: `token.lt` followed by `token.minus`.
        let t = &tokens[i];
        if !(t.kind == "token.lt" && i + 1 < tokens.len() && tokens[i + 1].kind == "token.minus") {
            continue;
        }
        // The binding must be on the cursor / selection's line.
        if !range_touches_line(source, range_start, range_end, t.span.start) {
            continue;
        }
        // Skip if already inside a do block.
        if is_inside_do_block(tokens, t.span.start) {
            continue;
        }
        // Wrap the entire line in `do { … }`, preserving indentation.
        let (line_start, line_end) = line_range(source, t.span.start);
        let line_text = &source[line_start..line_end];
        let indent_len = line_text.len() - line_text.trim_start().len();
        let indent = &line_text[..indent_len];
        let body = &line_text[indent_len..];
        let new_text = format!("do {{\n{}  {}\n{}}}", indent, body, indent);
        actions.push(make_action(
            "Wrap in `do` block",
            source,
            line_start,
            line_end,
            &new_text,
        ));
        // Only one wrap action per range.
        break;
    }
    actions
}

// ---------------------------------------------------------------------------
// b. Add missing `return`
// ---------------------------------------------------------------------------

/// Offer to insert `return ` before a match-arm object literal `{ … }`
/// that lacks the prefix.
///
/// A `{` is considered a match-arm body when the immediately preceding token
/// is `:` (the arm separator) and the preceding token is not already a
/// `return` keyword.
#[allow(non_snake_case)]
fn add_return_actions(
    source: &str,
    tokens: &[SpannedToken],
    range_start: usize,
    range_end: usize,
) -> Vec<Value> {
    let mut actions = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        if t.kind != "token.lbrace" {
            continue;
        }
        // Must be on the cursor / selection's line.
        if !range_touches_line(source, range_start, range_end, t.span.start) {
            continue;
        }
        let prev1 = if i > 0 { tokens.get(i - 1) } else { None };

        // Already has `return` prefix → skip.
        let has_return =
            prev1.map(|p| is_keyword_kind(p.kind.as_str()) && p.text == "return") == Some(true);
        if has_return {
            continue;
        }

        // Must be a match-arm body: preceded by `:`.
        if prev1.map(|p| p.kind.as_str()) != Some("token.colon") {
            continue;
        }

        actions.push(make_action(
            "Add missing return",
            source,
            t.span.start,
            t.span.start,
            "return ",
        ));
        // Only one per line.
        break;
    }
    actions
}

// ---------------------------------------------------------------------------
// c. Convert `//` to `///` doc comment
// ---------------------------------------------------------------------------

/// Whether the comment at `index` is at the top of the file — i.e. every
/// token before it is also a comment (doc or regular).
fn is_top_of_file_comment(tokens: &[SpannedToken], index: usize) -> bool {
    tokens
        .iter()
        .take(index)
        .all(|t| is_comment_kind(t.kind.as_str()))
}

/// Offer to convert a top-of-file `--` comment into a `---` doc comment.
fn convert_comment_actions(
    source: &str,
    tokens: &[SpannedToken],
    range_start: usize,
    range_end: usize,
) -> Vec<Value> {
    let mut actions = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        // Only regular comments (not already doc comments).
        if t.kind != "token.comment" {
            continue;
        }
        // Must be at the top of the file (before any non-comment code).
        if !is_top_of_file_comment(tokens, i) {
            continue;
        }
        // Must be on the cursor / selection's line.
        if !range_touches_line(source, range_start, range_end, t.span.start) {
            continue;
        }
        // Replace the leading `--` with `---`.
        actions.push(make_action(
            "Convert `--` to `---` doc comment",
            source,
            t.span.start,
            t.span.start + 2,
            "---",
        ));
        break;
    }
    actions
}

// ---------------------------------------------------------------------------
// e. Lint quick fixes via the transform engine (ADR-0094)
// ---------------------------------------------------------------------------

/// Map a lint rule name to a user-facing code-action title.
///
/// The lint pass matches are produced by `RewriteEngine::find_matches` inside
/// `plugin-ext-lint` — this is only a label lookup, not pattern matching.
fn lint_action_title(rule: &str, message: &str) -> String {
    match rule {
        "simplify-paren" => "Remove redundant parentheses".to_string(),
        _ => format!("Fix: {}", message),
    }
}

/// Offer quick fixes for lint diagnostics that carry a [`Fix`].
///
/// Runs `handl_plugin_ext_lint::lint_source` over the document — the
/// `simplify-paren` rule (and any future `lint`-pass rules) is matched by the
/// ADR-0094 transform engine's `find_matches`, so these actions are genuinely
/// engine-driven: find pattern → show diff (the `edit`) → apply at node.
///
/// The match comes from `find_matches`; the edit is a span→text replacement
/// using `AstNode::span` (the `Fix` span that `lint_source` now derives from
/// the matched `paren` and inner-atom node spans).  Only diagnostics whose
/// fix span intersects the requested range are offered.
fn lint_fix_actions(source: &str, range_start: usize, range_end: usize) -> Vec<Value> {
    if source.trim().is_empty() {
        return Vec::new();
    }
    let diags = match handl_plugin_ext_lint::lint_source(source) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut actions = Vec::new();
    for diag in diags {
        let Some(fix) = &diag.fix else {
            continue;
        };
        // Offer the action only when the cursor / selection touches the fix
        // span.  A zero-width cursor must lie inside the span.
        let fs = fix.span.start;
        let fe = fix.span.end;
        let touches = if range_start == range_end {
            range_start >= fs && range_start <= fe
        } else {
            range_start < fe && range_end > fs
        };
        if !touches {
            continue;
        }
        actions.push(json!({
            "title": lint_action_title(diag.rule, &diag.message),
            "kind": "quickfix",
            "diagnostics": [{
                "range": span_to_range(source, diag.span.start, diag.span.end),
                "severity": match diag.severity {
                    handl_plugin_ext_diag::Severity::Error => 1,
                    handl_plugin_ext_diag::Severity::Warning => 2,
                    _ => 3,
                },
                "source": "handl-lint",
                "message": diag.message,
            }],
            "edit": {
                "changes": {
                    UNTITLED_URI: [{
                        "range": span_to_range(source, fs, fe),
                        "newText": fix.replacement,
                    }]
                }
            },
        }));
    }
    actions
}

// ---------------------------------------------------------------------------
// f. Rewrite-based quick fixes via apply_rule + PreserveEmitter (ADR-0094)
// ---------------------------------------------------------------------------

/// A single top-level statement within the source text.
struct StatementInfo<'a> {
    /// Trimmed statement source.
    source: &'a str,
    /// Byte offset of `source` in the original document.
    start: usize,
    /// Byte offset one past the end of `source` in the original document.
    end: usize,
}

/// Native rewrite registry used by the LSP `lsp-rewrite` pass.
///
/// The `remove-paren` native drops a `paren` node, returning its single
/// `body` child unchanged.
fn lsp_natives() -> NativeRewriteRegistry {
    let mut reg = NativeRewriteRegistry::new();
    reg.register(
        "remove-paren",
        Box::new(|ctx: &mut RewriteCtx<'_>| {
            let new_idx = ctx.remap_child(0).ok_or_else(|| {
                LangError::engine("remove-paren: paren body child not remapped".to_string())
            })?;
            Ok(AstRef::new(new_idx))
        }),
    );
    reg
}

/// Map a `lint` pass rule to the `lsp-rewrite` pass rule that actually
/// performs the AST rewrite.
fn rewrite_rule_id(rule: &str) -> Option<String> {
    rule.strip_prefix("lint.")
        .map(|suffix| format!("lsp-rewrite.{suffix}"))
}

/// User-facing title for an engine-rewrite code action.
fn rewrite_action_title(rule: &str) -> String {
    match rule {
        "lint.simplify-paren" => "Apply rewrite: Remove redundant parentheses".to_string(),
        _ => format!("Apply rewrite: {rule}"),
    }
}

/// Offer quick fixes that run `apply_rule` on a matched `lint` site and
/// re-emit the transformed statement with `PreserveEmitter`.
///
/// Statements are split and parsed per-statement, their parser spans are
/// offset back into the original `source`, and the per-statement emitted
/// source becomes a `WorkspaceEdit` replacing that statement's trimmed span.
fn engine_rewrite_actions(source: &str, range_start: usize, range_end: usize) -> Vec<Value> {
    let lang = match handl_plugin_ext_lint::lint_language() {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };
    let engine = RewriteEngine::new().with_native_registry(Arc::new(lsp_natives()));
    let mut actions = Vec::new();

    for info in split_statements(source) {
        if info.source.is_empty() {
            continue;
        }
        let mut ast = match parse_chunk(lang, info.source) {
            Ok(a) => a,
            Err(_) => continue,
        };
        if ast.is_empty() {
            continue;
        }
        if offset_spans(&mut ast, info.start).is_err() {
            continue;
        }
        let root = AstRef::new(ast.len() - 1);
        let matches = engine.find_matches(lang, &ast, root, "lint");
        for m in matches {
            let Some(node) = ast.get(m.at) else { continue; };
            let Some(span) = node.span else { continue; };
            if !spans_intersect(range_start, range_end, span.start, span.end) {
                continue;
            }
            let Some(rewrite_id) = rewrite_rule_id(&m.rule_id) else { continue; };
            let out = match engine.apply_rule(lang, &ast, m.at, &rewrite_id) {
                Ok(o) => o,
                Err(_) => continue,
            };
            if out.trace.rewrites.is_empty() {
                continue;
            }
            if check_emittable(lang, &out.ast, out.root).is_err() {
                continue;
            }
            let changed: Vec<AstRef> = out.trace.rewrites.iter().map(|r| r.to).collect();
            let emitter = match PreserveEmitter::new(source, &out.ast, out.root, &changed) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let emitted = match emitter.format(lang, &out.ast, out.root) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let title = rewrite_action_title(&m.rule_id);
            actions.push(json!({
                "title": title,
                "kind": "quickfix",
                "edit": {
                    "changes": {
                        UNTITLED_URI: [{
                            "range": span_to_range(source, info.start, info.end),
                            "newText": emitted,
                        }]
                    }
                },
            }));
        }
    }
    actions
}

// ---------------------------------------------------------------------------
// Source splitting and statement parsing
// ---------------------------------------------------------------------------

fn spans_intersect(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start < b_end && a_end > b_start
}

fn split_statements(src: &str) -> Vec<StatementInfo<'_>> {
    let bytes = src.as_bytes();
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut depth: i32 = 0;
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];

        if b == b'"' {
            i += 1;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => {
                        i += 2;
                        continue;
                    }
                    b'"' => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
            continue;
        }

        if b == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            start = i;
            continue;
        }

        match b {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ => {}
        }

        if depth == 0 && (b == b'\n' || b == b';') {
            push_chunk(src, &mut chunks, start, i);
            i += 1;
            start = i;
            continue;
        }

        i += 1;
    }

    push_chunk(src, &mut chunks, start, src.len());
    chunks
}

fn push_chunk<'a>(src: &'a str, chunks: &mut Vec<StatementInfo<'a>>, start: usize, end: usize) {
    let slice = &src[start..end];
    let lead = slice.len() - slice.trim_start().len();
    let trimmed = slice.trim();
    if !trimmed.is_empty() {
        chunks.push(StatementInfo {
            source: trimmed,
            start: start + lead,
            end: start + lead + trimmed.len(),
        });
    }
}

fn parse_chunk(lang: &Language, text: &str) -> Result<Ast, LangError> {
    let tokens = lex(lang, text)?;
    if tokens.as_slice().is_empty() {
        return Ok(Ast::new());
    }
    let req = ParserRequest::new(tokens);
    PrattEngine::new().parse(lang, &req)
}

fn offset_spans(ast: &mut Ast, delta: usize) -> Result<(), LangError> {
    for i in 0..ast.len() {
        let r = AstRef::new(i);
        let node = ast
            .get_mut(r)
            .ok_or(LangError::AstIndexOutOfBounds { index: i })?;
        if let Some(span) = node.span.as_mut() {
            span.start += delta;
            span.end += delta;
        }
    }
    Ok(())
}

fn check_emittable(lang: &Language, ast: &Ast, root: AstRef) -> Result<(), Vec<String>> {
    let grammar_kinds: std::collections::HashSet<String> =
        lang.grammar.iter().map(|r| r.child_shape.node_kind.clone()).collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = vec![root.index()];
    let mut bad = Vec::new();
    while let Some(i) = stack.pop() {
        if !seen.insert(i) {
            continue;
        }
        let Some(node) = ast.get(AstRef::new(i)) else {
            continue;
        };
        if node.span.is_none()
            && !grammar_kinds.contains(&node.kind)
            && !has_leaf_payload(node)
        {
            bad.push(node.kind.clone());
        }
        if let Some(children) = node.payload.get("children").and_then(|c| c.as_array()) {
            for c in children {
                if let Some(idx) = c.as_u64() {
                    stack.push(idx as usize);
                }
            }
        }
    }
    if bad.is_empty() {
        Ok(())
    } else {
        bad.sort();
        bad.dedup();
        Err(bad)
    }
}

fn has_leaf_payload(node: &AstNode) -> bool {
    if node.payload.get("value").is_some() || node.payload.get("name").is_some() {
        return true;
    }
    !node.payload.is_object() && !node.payload.is_null()
}

// ---------------------------------------------------------------------------
// d. Extract to named arrow
// ---------------------------------------------------------------------------

/// Offer to extract a complex selection into a named arrow binding by
/// prepending `name = ` to the selection.
///
/// A selection is "complex" when it contains `>>>`, `|||`, or a nested
/// `~>` expression.  This action requires a non-empty selection.
fn extract_to_named_arrow_actions(
    source: &str,
    range_start: usize,
    range_end: usize,
) -> Vec<Value> {
    if source.is_empty() {
        return Vec::new();
    }
    let mut start = range_start.min(source.len());
    let mut end = range_end.min(source.len());
    if start > end {
        std::mem::swap(&mut start, &mut end);
    }
    if start == end {
        return Vec::new();
    }
    // Ensure char boundaries.
    while start < source.len() && !source.is_char_boundary(start) {
        start += 1;
    }
    while end < source.len() && !source.is_char_boundary(end) {
        end += 1;
    }
    if start >= end {
        return Vec::new();
    }
    let selection = &source[start..end];
    if selection.contains(">>>") || selection.contains("|||") || selection.contains("~>") {
        vec![make_action(
            "Extract to named arrow",
            source,
            start,
            start,
            "name = ",
        )]
    } else {
        Vec::new()
    }
}
