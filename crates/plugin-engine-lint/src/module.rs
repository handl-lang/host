//! Module-level linting (ADR-0097 — Lint Rules as Language Data).
//!
//! The bootstrap Pratt parser parses one top-level statement per call, so a
//! source file is split into statement chunks at bracket-depth zero
//! (respecting strings and `--` comments) and each chunk is parsed
//! individually — the same convention the bootstrap typechecker and test
//! runner use. A [`LintModule`] bundles the source text with the parsed
//! statements; [`ModuleAnalysis`] is the shared per-statement classification
//! (bindings / references / arrow params / imports) the `scope.*` natives
//! consume.
//!
//! Positions come from [`AstNode::span`] (populated by the Pratt parser),
//! offset by each statement's byte offset in the source — no token-stream
//! rescanning.

use std::collections::HashSet;

use handl_language::{Ast, AstNode, AstRef, LangError, Language, LintFix, LintRuleDef, Span};
use handl_plugin_engine_lex_pattern::lex;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_engine_transform_rewrite::condition::matches_condition;
use handl_plugin_ext_diag::split_statements;

use crate::{DiagnosticLevel, LintDiagnostic, LintFixOut};

/// A single parsed top-level statement within a [`LintModule`].
#[derive(Debug)]
pub struct LintStatement {
    /// The trimmed source text of the statement.
    pub text: String,
    /// Byte offset of `text` in the original source.
    pub offset: usize,
    /// The parsed statement arena.
    pub ast: Ast,
    /// The root node of the statement.
    pub root: AstRef,
}

/// A whole module under lint: source text plus its parsed statements.
pub struct LintModule<'a> {
    /// The original source text.
    pub src: &'a str,
    /// The parsed top-level statements, in source order.
    pub statements: Vec<LintStatement>,
}

/// Split `src` into statements, parse each, and bundle into a [`LintModule`].
///
/// Chunks that lex to no tokens are skipped.
///
/// # Errors
///
/// Returns [`LangError`] if any chunk fails to lex or parse.
pub fn parse_module<'a>(lang: &Language, src: &'a str) -> Result<LintModule<'a>, LangError> {
    let mut statements = Vec::new();
    for chunk in split_statements(src) {
        let tokens = lex(lang, &chunk.text)?;
        if tokens.as_slice().is_empty() {
            continue;
        }
        let ast = PrattEngine::new().parse(lang, &ParserRequest::new(tokens))?;
        if ast.is_empty() {
            continue;
        }
        let root = AstRef::new(ast.len().saturating_sub(1));
        statements.push(LintStatement {
            text: chunk.text,
            offset: chunk.offset,
            ast,
            root,
        });
    }
    Ok(LintModule { src, statements })
}

// ---------------------------------------------------------------------------
// Statement splitting
// ---------------------------------------------------------------------------
//
// The splitter is shared across the typechecker, linter, and test runner —
// it lives in `plugin-ext-diag` (`split_statements`, AGENTS.md "Source
// splitting"). Comment-only chunks never reach here: the shared splitter
// drops them.

// ---------------------------------------------------------------------------
// Module analysis (shared input to the `scope.*` natives)
// ---------------------------------------------------------------------------

/// Per-statement classification used by scope-aware native lint rules.
#[derive(Debug, Default)]
pub struct StatementAnalysis {
    /// Whether the statement is an import declaration.
    pub is_import: bool,
    /// The imported name, when the statement is an import.
    pub import_name: Option<String>,
    /// The top-level binding name and its absolute byte span, when the
    /// statement binds a name.
    pub binding: Option<(String, Span)>,
    /// All name references in the statement, with absolute byte spans.
    pub references: Vec<(String, Span)>,
    /// Arrow lambda parameters introduced in the statement.
    pub params: Vec<String>,
}

/// Analysis of a whole [`LintModule`]: one [`StatementAnalysis`] per
/// statement, in source order.
#[derive(Debug, Default)]
pub struct ModuleAnalysis {
    /// Per-statement analysis, aligned with `LintModule::statements`.
    pub statements: Vec<StatementAnalysis>,
}

impl ModuleAnalysis {
    /// Walk every statement in `module`, classifying name occurrences by
    /// role (binding target / arrow param / reference) using `Language` data
    /// — `target`+`value` roles mark bindings, `params`+`body` roles mark
    /// arrow lambdas, `pattern_role: "leaf"` + `subject` roles identify name
    /// leaves (ADR-0081).
    pub fn build(lang: &Language, module: &LintModule) -> Self {
        // Kinds produced by `import` statements: grammar rows whose token is
        // the `import` keyword.
        let import_kinds: HashSet<String> = lang
            .grammar
            .iter()
            .filter(|row| Some(row.token.as_str()) == lang.token_kind_for_label("import"))
            .map(|row| row.child_shape.node_kind.clone())
            .collect();

        let mut statements = Vec::with_capacity(module.statements.len());
        for stmt in &module.statements {
            let mut occs: Vec<VarOcc> = Vec::new();
            walk_vars(lang, &stmt.ast, stmt.root, &mut occs);

            let root_node = stmt.ast.at(stmt.root).ok();
            let is_import = root_node
                .map(|n| import_kinds.contains(&n.kind))
                .unwrap_or(false);
            let import_name = if is_import {
                root_node.and_then(|n| first_child_var(lang, &stmt.ast, n))
            } else {
                None
            };

            let binding = occs
                .iter()
                .find(|o| o.role == VarRole::Target)
                .map(|o| (o.name.clone(), absolute(o.span, stmt.offset)));
            let references = occs
                .iter()
                .filter(|o| o.role == VarRole::Reference)
                .map(|o| (o.name.clone(), absolute(o.span, stmt.offset)))
                .collect();
            let params = occs
                .iter()
                .filter(|o| o.role == VarRole::Param)
                .map(|o| o.name.clone())
                .collect();

            statements.push(StatementAnalysis {
                is_import,
                import_name,
                binding,
                references,
                params,
            });
        }
        Self { statements }
    }
}

/// Offset a statement-local span into absolute source coordinates.
pub(crate) fn absolute(span: Option<Span>, offset: usize) -> Span {
    match span {
        Some(s) => Span {
            start: s.start + offset,
            end: s.end + offset,
        },
        None => Span::default(),
    }
}

/// The role a name occurrence plays in the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VarRole {
    /// A binding target (child at the `target` role).
    Target,
    /// An arrow lambda parameter (child at the `params` role).
    Param,
    /// A name reference.
    Reference,
}

/// A name occurrence collected during the walk.
#[derive(Debug)]
struct VarOcc {
    name: String,
    role: VarRole,
    /// Statement-local span of the name leaf.
    span: Option<Span>,
}

/// Recursive AST walk collecting name occurrences. Mirrors the canonical
/// linter's walk: binding targets are recorded (not recursed — a target is
/// not a reference), arrow params are recorded and only the body is
/// recursed, name leaves are references, everything else recurses.
fn walk_vars(lang: &Language, ast: &Ast, node_ref: AstRef, occs: &mut Vec<VarOcc>) {
    let node = match ast.at(node_ref) {
        Ok(n) => n,
        Err(_) => return,
    };

    // Binding: a node with `target` + `value` roles.
    let target_idx = lang.child_role_index(&node.kind, "target").ok().flatten();
    let value_idx = lang.child_role_index(&node.kind, "value").ok().flatten();
    if let (Some(ti), Some(vi)) = (target_idx, value_idx) {
        let children = child_indices(node);
        if let Some(&tref) = children.get(ti)
            && let Some((name, span)) = name_at(lang, ast, AstRef::new(tref))
        {
            occs.push(VarOcc {
                name,
                role: VarRole::Target,
                span,
            });
        }
        if let Some(&vref) = children.get(vi) {
            walk_vars(lang, ast, AstRef::new(vref), occs);
        }
        return;
    }

    // Arrow lambda: a node with `params` + `body` roles.
    let params_idx = lang.child_role_index(&node.kind, "params").ok().flatten();
    let body_idx = lang.child_role_index(&node.kind, "body").ok().flatten();
    if let (Some(pi), Some(bi)) = (params_idx, body_idx) {
        let children = child_indices(node);
        if let Some(&pref) = children.get(pi)
            && let Some((name, span)) = name_at(lang, ast, AstRef::new(pref))
        {
            occs.push(VarOcc {
                name,
                role: VarRole::Param,
                span,
            });
        }
        if let Some(&bref) = children.get(bi) {
            walk_vars(lang, ast, AstRef::new(bref), occs);
        }
        return;
    }

    // Pattern-binder form (`pattern` + `body` roles): the `pattern` child
    // introduces a local name for the `body`, while any other children (e.g.
    // the `collection` of a `for` loop) are still checked in the outer scope.
    // Identified data-drivenly — covers `for` loops and `match` arms.
    let pattern_idx = lang.child_role_index(&node.kind, "pattern").ok().flatten();
    let body_idx = lang.child_role_index(&node.kind, "body").ok().flatten();
    if let (Some(pi), Some(_bi)) = (pattern_idx, body_idx) {
        let children = child_indices(node);
        if let Some(&pref) = children.get(pi)
            && let Some((name, span)) = name_at(lang, ast, AstRef::new(pref))
        {
            occs.push(VarOcc {
                name,
                role: VarRole::Param,
                span,
            });
        }
        for (pos, &c) in children.iter().enumerate() {
            if pos == pi {
                continue;
            }
            walk_vars(lang, ast, AstRef::new(c), occs);
        }
        return;
    }

    // Name leaf: a reference.
    if let Some((name, span)) = name_at(lang, ast, node_ref) {
        occs.push(VarOcc {
            name,
            role: VarRole::Reference,
            span,
        });
        return;
    }

    for &c in &child_indices(node) {
        walk_vars(lang, ast, AstRef::new(c), occs);
    }
}

/// Extract the `payload["children"]` arena indices from a node.
pub(crate) fn child_indices(node: &AstNode) -> Vec<usize> {
    node.payload
        .get("children")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default()
}

/// The name carried by a parameter/name node — a bare `var`
/// (`pattern_role: "leaf"`) or a node with a `subject` role wrapping one —
/// plus the span of the innermost `var` leaf (so `x :: T` highlights `x`).
///
/// Resolved through `Language::param_name` (ADR-0081), never `kind ==`.
fn name_at(lang: &Language, ast: &Ast, r: AstRef) -> Option<(String, Option<Span>)> {
    let node = ast.at(r).ok()?;
    let is_name = lang
        .ast
        .get_by_identity(&node.kind)
        .and_then(|s| s.pattern_role.as_deref())
        == Some("leaf")
        || lang
            .child_role_index(&node.kind, "subject")
            .ok()
            .flatten()
            .is_some();
    if !is_name {
        return None;
    }
    let name = lang.param_name(ast, node).ok()?;
    Some((name, leaf_span(lang, ast, node)))
}

/// Descend `subject` roles until a leaf `var` (string payload) is reached and
/// return its span — the span of the *name*, not the whole annotation.
fn leaf_span(lang: &Language, ast: &Ast, node: &AstNode) -> Option<Span> {
    if node.payload.is_string() {
        return node.span;
    }
    if let Ok(Some(idx)) = lang.child_role_index(&node.kind, "subject") {
        let children = child_indices(node);
        if let Some(&c) = children.get(idx)
            && let Ok(inner) = ast.at(AstRef::new(c))
        {
            return leaf_span(lang, ast, inner);
        }
    }
    node.span
}

/// The name of the first parameter-shaped child of a node (used for
/// import/export names).
fn first_child_var(lang: &Language, ast: &Ast, node: &AstNode) -> Option<String> {
    for &idx in &child_indices(node) {
        if let Some((name, _)) = name_at(lang, ast, AstRef::new(idx)) {
            return Some(name);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// `Match` rule evaluation
// ---------------------------------------------------------------------------

/// Evaluate a `Match`-checked [`LintRuleDef`] over a statement, appending a
/// [`LintDiagnostic`] for every node of `node_kind` satisfying `when`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn eval_match_rule(
    lang: &Language,
    stmt: &LintStatement,
    src: &str,
    rule: &LintRuleDef,
    node_kind: &str,
    when: Option<&handl_language::TransformCondition>,
    level: DiagnosticLevel,
    diags: &mut Vec<LintDiagnostic>,
) {
    walk_nodes(&stmt.ast, stmt.root, &mut |node| {
        if node.kind != node_kind {
            return;
        }
        if let Some(cond) = when
            && !matches_condition(lang, &stmt.ast, node, cond)
        {
            return;
        }
        let span = node.span.map(|s| absolute(Some(s), stmt.offset));
        let message = render_match_message(&rule.message, lang, &stmt.ast, node, stmt, src);
        let fix = rule
            .fix
            .as_ref()
            .and_then(|f| compute_fix(f, lang, &stmt.ast, node, stmt, src, span));
        diags.push(LintDiagnostic {
            level,
            message,
            span,
            rule: rule.name.clone(),
            fix,
        });
    });
}

/// Visit every node in the subtree rooted at `r` (pre-order).
fn walk_nodes(ast: &Ast, r: AstRef, f: &mut impl FnMut(&AstNode)) {
    let Ok(node) = ast.at(r) else { return };
    f(node);
    for &c in &child_indices(node) {
        walk_nodes(ast, AstRef::new(c), f);
    }
}

/// Resolve a child by named `role` (ADR-0081) with positional `index`
/// fallback, returning the child node.
fn resolve_child<'a>(
    lang: &Language,
    ast: &'a Ast,
    node: &AstNode,
    index: Option<usize>,
    role: Option<&str>,
) -> Option<&'a AstNode> {
    let pos = role
        .and_then(|r| lang.child_role_index(&node.kind, r).ok().flatten())
        .or(index)?;
    let children = child_indices(node);
    let &cref = children.get(pos)?;
    ast.at(AstRef::new(cref)).ok()
}

/// Compute a declarative [`LintFix`] for a matched node.
fn compute_fix(
    fix: &LintFix,
    lang: &Language,
    ast: &Ast,
    node: &AstNode,
    stmt: &LintStatement,
    src: &str,
    node_span: Option<Span>,
) -> Option<LintFixOut> {
    match fix {
        LintFix::Unwrap { index, role } => {
            let child = resolve_child(lang, ast, node, *index, role.as_deref())?;
            let child_span = child.span?;
            let start = child_span.start + stmt.offset;
            let end = child_span.end + stmt.offset;
            let replacement = src.get(start..end)?.to_string();
            let span = node_span?;
            Some(LintFixOut {
                start: span.start,
                end: span.end,
                replacement,
            })
        }
    }
}

/// Render a `Match` rule's message template. Supported placeholders:
/// `{source}` — the matched node's source text; `{source:<role>}` — the
/// source text of the child occupying `role`; `{payload}` — the node's
/// payload (a string payload renders bare, anything else as compact JSON).
fn render_match_message(
    template: &str,
    lang: &Language,
    ast: &Ast,
    node: &AstNode,
    stmt: &LintStatement,
    src: &str,
) -> String {
    let mut out = template.to_string();
    if let Some(span) = node.span {
        let abs = absolute(Some(span), stmt.offset);
        if let Some(text) = src.get(abs.start..abs.end) {
            out = out.replace("{source}", text);
        }
    }
    // `{source:<role>}` placeholders.
    while let Some(start) = out.find("{source:") {
        let Some(end) = out[start..].find('}') else {
            break;
        };
        let end = start + end;
        let role = &out[start + 8..end];
        let replacement = resolve_child(lang, ast, node, None, Some(role))
            .and_then(|c| c.span)
            .map(|s| absolute(Some(s), stmt.offset))
            .and_then(|s| src.get(s.start..s.end))
            .unwrap_or("")
            .to_string();
        out.replace_range(start..=end, &replacement);
    }
    if out.contains("{payload}") {
        let payload = match &node.payload {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        out = out.replace("{payload}", &payload);
    }
    out
}
