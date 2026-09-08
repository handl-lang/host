//! `handl-plugin-ext-lint` — canonical bootstrap linter for the v0.1 arrow
//! surface syntax (ADR-0087).
//!
//! Re-exports the platform `Lint` operation machinery ([`BasicLinter`], the
//! generic rule/capability registry) and adds a **canonical rule set** over the
//! v0.1 surface AST (`handl-language` AST via `plugin-engine-parse`), running
//! on the crate's existing generic rule/capability registry.
//!
//! # Canonical rule set
//!
//! All rules are v0.1-surface-specific and hardcoded here (the *machinery* —
//! registry, severity, fix application — is the generic substrate re-exported
//! above, so hosts can contribute additional namespaced rules):
//!
//! | Rule | Severity | Fixable | Description |
//! |------|----------|---------|-------------|
//! | `unused-import` | Warning | yes | import projection never referenced |
//! | `unbound-wire` | Error | no | reference to a name not bound in scope |
//! | `shadowed-binding` | Warning | no | re-binding a name already in scope |
//! | `dead-branch` | Warning | no | `choice`/`if` arm guarded by a constant |
//! | `naming-convention` | Warning | yes | bindings `snake_case`, arrows `kebab-case` |
//! | `missing-module-doc` | Info | no | module lacks a leading doc comment |
//! | `fmt` | Warning | yes | file is not canonical per ADR-0086 |
//! | `simplify-paren` | Warning | yes | redundant `paren` around a single atom (ADR-0094) |
//!
//! Diagnostics use the shared types from `plugin-ext-diag` (ADR-0091). See
//! [`lint_source`], [`check_source`], and [`fix_source`] for the public API.

pub use handl_platform::{BasicLinter, Lint, LintBehavior, LintRequest, RuleAcceptor};

use std::collections::HashSet;
use std::sync::OnceLock;

use handl_language::{Ast, AstNode, AstRef, LangError, Language, TransformDef};
use handl_plugin_engine_lex_pattern::lex;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_engine_transform_rewrite::RewriteEngine;
use handl_plugin_ext_diag::{Diagnostic, Fix, Severity, Span, apply_fixes_to_fixpoint};
use handl_plugin_ext_fmt::{check_source as fmt_check_source, format_source as fmt_format_source};

/// ADR-0094: embedded `lint` pass transform rules used as pattern matchers.
const LINT_TRANSFORMS_YAML: &str = include_str!("../data/lint_transforms.yaml");

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Error from the linter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintError {
    /// Source failed to parse.
    Parse(String),
}

impl std::fmt::Display for LintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintError::Parse(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for LintError {}

impl From<LangError> for LintError {
    fn from(err: LangError) -> Self {
        LintError::Parse(err.to_string())
    }
}

/// Lint a handl source file with the canonical rule set.
///
/// Parses the source and runs all canonical lint rules over the v0.1
/// surface AST. Returns diagnostics sorted by span (byte offset).
///
/// # Errors
///
/// Returns [`LintError::Parse`] if the source cannot be parsed.
pub fn lint_source(src: &str) -> Result<Vec<Diagnostic>, LintError> {
    let lang = lint_language()?;
    let chunks = split_statements(src);
    let info = LangInfo::build(lang);

    // Parse every top-level statement chunk.
    let mut parsed: Vec<ParsedChunk> = Vec::with_capacity(chunks.len());
    for chunk in &chunks {
        let ast = parse_chunk(lang, &chunk.text)?;
        if ast.is_empty() {
            continue;
        }
        let root_idx = ast.len() - 1;
        let idents = identifier_positions(lang, &chunk.text, chunk.offset);
        let bools = bool_positions(lang, &chunk.text, chunk.offset);
        let analysis = analyze_chunk(lang, &info, &ast, AstRef::new(root_idx), &idents, &bools);
        parsed.push(ParsedChunk {
            chunk: chunk.clone(),
            ast,
            analysis,
        });
    }

    let mut diags = Vec::new();

    // Source-level rules.
    diags.extend(rule_missing_module_doc(src));
    diags.extend(rule_fmt(src));

    // AST-level rules.
    diags.extend(rule_unused_import(src, &parsed));
    diags.extend(rule_naming_convention(&parsed));
    diags.extend(rule_shadowed_binding(&parsed));
    diags.extend(rule_unbound_wire(&parsed));
    diags.extend(rule_simplify_paren(src, &parsed, lang));

    // dead-branch diagnostics are collected during analysis.
    for p in &parsed {
        diags.extend(p.analysis.dead_branch_diags.clone());
    }

    // Sort by byte offset (span start), stable.
    diags.sort_by_key(|d| d.span.start);
    Ok(diags)
}

/// Check if a source file is lint-clean (no Error diagnostics).
///
/// # Errors
///
/// Returns [`LintError`] if the source cannot be parsed.
pub fn check_source(src: &str) -> Result<bool, LintError> {
    let diags = lint_source(src)?;
    Ok(!diags.iter().any(|d| d.severity == Severity::Error))
}

/// Apply fixable diagnostics to source text.
///
/// Applies fixes in span order, re-parses, and re-runs lint to a fixpoint
/// (max 5 iterations, then reports unfixable).
///
/// # Errors
///
/// Returns [`LintError::Parse`] if the source cannot be parsed after fixing.
pub fn fix_source(src: &str) -> Result<String, LintError> {
    let fixed = apply_fixes_to_fixpoint(src, compute_fixes, 5)
        .map_err(|e| LintError::Parse(e.to_string()))?;
    // Verify the fixed source still parses.
    let _ = lint_source(&fixed)?;
    Ok(fixed)
}

/// Compute the next batch of fixes for the fixpoint loop.
///
/// The `fmt` fix spans the whole file and would overlap every other fix, so
/// when it is present it is applied alone; otherwise the remaining fixable
/// diagnostics (non-overlapping among themselves) are returned.
fn compute_fixes(src: &str) -> Vec<Fix> {
    let diags = match lint_source(src) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    if let Some(fmt_fix) = diags
        .iter()
        .find(|d| d.rule == "fmt")
        .and_then(|d| d.fix.clone())
    {
        return vec![fmt_fix];
    }
    diags
        .iter()
        .filter(|d| d.rule != "fmt")
        .filter_map(|d| d.fix.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Internal: language assembly + parsing
// ---------------------------------------------------------------------------

/// Assemble the lint language once: canonical data plus the embedded
/// ADR-0094 `lint` pass transform rules.
pub fn lint_language() -> Result<&'static Language, LintError> {
    static LANG: OnceLock<Language> = OnceLock::new();
    let lang = LANG.get_or_init(|| {
        let mut lang = handl_kit_data_canonical::assemble()
            .expect("canonical language data must assemble")
            .lang()
            .clone();
        let rules: Vec<TransformDef> = serde_yaml::from_str(LINT_TRANSFORMS_YAML)
            .expect("lint transforms must be valid YAML");
        for rule in rules {
            lang.transforms
                .merge(rule)
                .expect("lint transforms must merge cleanly");
        }
        lang
    });
    Ok(lang)
}

/// Parse a single statement chunk into an [`Ast`] using the Pratt parser.
fn parse_chunk(lang: &Language, text: &str) -> Result<Ast, LintError> {
    let tokens = lex(lang, text)?;
    if tokens.as_slice().is_empty() {
        return Ok(Ast::new());
    }
    let req = ParserRequest::new(tokens);
    let ast = PrattEngine::new().parse(lang, &req)?;
    Ok(ast)
}

// ---------------------------------------------------------------------------
// Internal: statement splitting (mirrors the bootstrap typechecker / test
// runner — the v0.1 Pratt parser parses one statement per call).
// ---------------------------------------------------------------------------

/// A top-level statement chunk: its trimmed text and byte offset in the
/// original source.
#[derive(Clone, Debug)]
struct Chunk {
    text: String,
    offset: usize,
}

/// Split `src` into top-level statement chunks.
///
/// Splits at bracket-depth zero on newlines or semicolons, respecting string
/// literals and `--` comments.
fn split_statements(src: &str) -> Vec<Chunk> {
    let bytes = src.as_bytes();
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut depth: i32 = 0;
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];

        // String literal — skip to the closing quote (respecting escapes).
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

        // Comment: `--` (or `---`) to end of line.
        if b == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            start = i;
            continue;
        }

        // Bracket nesting.
        match b {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ => {}
        }

        // Statement separators at depth zero.
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

/// Push a trimmed chunk from `src[start..end]` if it is non-empty.
fn push_chunk(src: &str, chunks: &mut Vec<Chunk>, start: usize, end: usize) {
    let slice = &src[start..end];
    let lead = slice.len() - slice.trim_start().len();
    let trimmed = slice.trim();
    if !trimmed.is_empty() {
        chunks.push(Chunk {
            text: trimmed.to_string(),
            offset: start + lead,
        });
    }
}

// ---------------------------------------------------------------------------
// Internal: token positions (the Pratt AST carries no spans, so byte
// positions are recovered by scanning the token stream).
// ---------------------------------------------------------------------------

/// A token's absolute byte range in the original source.
#[derive(Clone, Copy, Debug)]
struct TokenPos {
    start: usize,
    end: usize,
}

/// Compute the byte positions of all `token.identifier` tokens in a chunk,
/// as absolute offsets in the original source.
fn identifier_positions(lang: &Language, chunk_text: &str, chunk_offset: usize) -> Vec<TokenPos> {
    token_positions(lang, chunk_text, chunk_offset, "token.identifier")
}

/// Compute the byte positions of all `token.bool` tokens in a chunk.
fn bool_positions(lang: &Language, chunk_text: &str, chunk_offset: usize) -> Vec<TokenPos> {
    token_positions(lang, chunk_text, chunk_offset, "token.bool")
}

/// Compute the byte positions of all tokens of `kind` in a chunk by scanning
/// the source forward, skipping whitespace (mirrors the formatter's approach).
fn token_positions(
    lang: &Language,
    chunk_text: &str,
    chunk_offset: usize,
    kind: &str,
) -> Vec<TokenPos> {
    let tokens = match lex(lang, chunk_text) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let bytes = chunk_text.as_bytes();
    let mut pos = 0usize;
    let mut out = Vec::new();
    for token in tokens.as_slice() {
        // Skip whitespace.
        while pos < bytes.len() {
            let ch = match chunk_text[pos..].chars().next() {
                Some(c) => c,
                None => break,
            };
            if ch.is_whitespace() {
                pos += ch.len_utf8();
            } else {
                break;
            }
        }
        let start = pos;
        pos += token.text.len();
        if token.kind == kind {
            out.push(TokenPos {
                start: chunk_offset + start,
                end: chunk_offset + pos,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Internal: language classification (data-driven, ADR-0081)
// ---------------------------------------------------------------------------

/// Precomputed classification table mapping node kinds to their structural
/// roles, built from `Language` data (grammar + ast shapes).
struct LangInfo {
    /// Node kinds produced by `import` statements (grammar rows whose token is
    /// `token.keyword.import`).
    import_kinds: HashSet<String>,
    /// Node kinds produced by `export` statements (reserved for future
    /// export-aware rules).
    #[allow(dead_code)]
    export_kinds: HashSet<String>,
    /// Node kinds representing conditional forms (arity 3 with structured
    /// child separators — `if`, `?:` ternary).
    conditional_kinds: HashSet<String>,
}

impl LangInfo {
    fn build(lang: &Language) -> Self {
        let mut import_kinds = HashSet::new();
        let mut export_kinds = HashSet::new();
        let mut conditional_kinds = HashSet::new();

        for row in lang.grammar.iter() {
            let kind = &row.child_shape.node_kind;
            if row.token == "token.keyword.import" {
                import_kinds.insert(kind.clone());
            }
            if row.token == "token.keyword.export" {
                export_kinds.insert(kind.clone());
            }
            // Conditionals: arity 3 with at least one child separator (then/else
            // for `if`, colon for `?:`). `for` has arity 3 but no separators.
            if row.child_shape.arity == 3 && !row.child_shape.child_separators.is_empty() {
                conditional_kinds.insert(kind.clone());
            }
        }

        Self {
            import_kinds,
            export_kinds,
            conditional_kinds,
        }
    }

    fn is_import(&self, kind: &str) -> bool {
        self.import_kinds.contains(kind)
    }

    #[allow(dead_code)]
    fn is_export(&self, kind: &str) -> bool {
        self.export_kinds.contains(kind)
    }

    fn is_conditional(&self, kind: &str) -> bool {
        self.conditional_kinds.contains(kind)
    }
}

// ---------------------------------------------------------------------------
// Internal: per-chunk AST analysis
// ---------------------------------------------------------------------------

/// The role a `var` node plays in the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VarRole {
    /// A binding target (`assign` LHS).
    Target,
    /// An arrow lambda parameter (`function` param).
    Param,
    /// A name reference.
    Reference,
}

/// A `var` occurrence collected during the walk, with its resolved position.
#[derive(Clone, Debug)]
struct VarOcc {
    name: String,
    role: VarRole,
    pos: TokenPos,
}

/// Analysis result for a single top-level statement chunk.
struct ChunkAnalysis {
    /// Whether the chunk is an import declaration.
    is_import: bool,
    /// The imported/exported name (for import/export chunks).
    import_name: Option<String>,
    /// The top-level binding name and position (for binding chunks).
    binding_name: Option<(String, TokenPos)>,
    /// All name references in the chunk (excluding targets and params).
    references: Vec<(String, TokenPos)>,
    /// Arrow lambda parameters introduced in the chunk (in-scope bindings).
    arrow_params: Vec<String>,
    /// Diagnostics emitted locally during the walk (dead-branch).
    dead_branch_diags: Vec<Diagnostic>,
}

/// A parsed chunk bundled with its analysis.
struct ParsedChunk {
    chunk: Chunk,
    ast: Ast,
    analysis: ChunkAnalysis,
}

/// Walk the AST of a chunk, collecting var occurrences (classified by role),
/// arrow params, and dead-branch diagnostics. Var positions are resolved by
/// zipping the in-order var list with the identifier token positions (AST
/// walk order matches source order for the v0.1 surface).
fn analyze_chunk(
    lang: &Language,
    info: &LangInfo,
    ast: &Ast,
    root: AstRef,
    idents: &[TokenPos],
    bools: &[TokenPos],
) -> ChunkAnalysis {
    let mut slots: Vec<VarSlot> = Vec::new();
    let mut dead_diags = Vec::new();
    let mut bool_cursor = 0usize;

    walk(
        lang,
        info,
        ast,
        root,
        &mut slots,
        &mut dead_diags,
        &mut bool_cursor,
        bools,
    );

    // Zip var slots with identifier positions (walk order == source order).
    let mut occs: Vec<VarOcc> = Vec::with_capacity(slots.len());
    for (i, slot) in slots.iter().enumerate() {
        let pos = idents
            .get(i)
            .copied()
            .unwrap_or(TokenPos { start: 0, end: 0 });
        occs.push(VarOcc {
            name: slot.name.clone(),
            role: slot.role,
            pos,
        });
    }

    let root_node = ast.at(root).expect("root exists");
    let is_import = info.is_import(&root_node.kind);

    // Import name: the single child is a `var` carrying the name.
    let import_name = if is_import {
        first_child_var_name(lang, ast, root_node)
    } else {
        None
    };

    // Top-level binding name (the first Target occurrence).
    let binding_name = occs
        .iter()
        .find(|o| o.role == VarRole::Target)
        .map(|o| (o.name.clone(), o.pos));

    let references = occs
        .iter()
        .filter(|o| o.role == VarRole::Reference)
        .map(|o| (o.name.clone(), o.pos))
        .collect();
    let arrow_params = occs
        .iter()
        .filter(|o| o.role == VarRole::Param)
        .map(|o| o.name.clone())
        .collect();

    ChunkAnalysis {
        is_import,
        import_name,
        binding_name,
        references,
        arrow_params,
        dead_branch_diags: dead_diags,
    }
}

/// A pending var slot awaiting position resolution.
struct VarSlot {
    name: String,
    role: VarRole,
}

/// Recursive AST walk collecting var slots and dead-branch diagnostics.
#[allow(clippy::too_many_arguments)]
fn walk(
    lang: &Language,
    info: &LangInfo,
    ast: &Ast,
    node_ref: AstRef,
    slots: &mut Vec<VarSlot>,
    dead_diags: &mut Vec<Diagnostic>,
    bool_cursor: &mut usize,
    bools: &[TokenPos],
) {
    let node = match ast.at(node_ref) {
        Ok(n) => n,
        Err(_) => return,
    };

    // Binding: a node with `target` + `value` roles (data-driven — any shape
    // declaring these roles is a binding, not just `assign`).
    let target_idx = lang.child_role_index(&node.kind, "target").ok().flatten();
    let value_idx = lang.child_role_index(&node.kind, "value").ok().flatten();
    if let (Some(ti), Some(vi)) = (target_idx, value_idx) {
        let children = child_indices(node);
        if let Some(&tref) = children.get(ti)
            && let Some(name) = var_name_at(lang, ast, AstRef::new(tref))
        {
            slots.push(VarSlot {
                name,
                role: VarRole::Target,
            });
        }
        if let Some(&vref) = children.get(vi) {
            walk(
                lang,
                info,
                ast,
                AstRef::new(vref),
                slots,
                dead_diags,
                bool_cursor,
                bools,
            );
        }
        return;
    }

    // Arrow lambda: a node with `params` + `body` roles.
    let params_idx = lang.child_role_index(&node.kind, "params").ok().flatten();
    let body_idx = lang.child_role_index(&node.kind, "body").ok().flatten();
    if let (Some(pi), Some(bi)) = (params_idx, body_idx) {
        let children = child_indices(node);
        if let Some(&pref) = children.get(pi)
            && let Some(name) = var_name_at(lang, ast, AstRef::new(pref))
        {
            slots.push(VarSlot {
                name,
                role: VarRole::Param,
            });
        }
        if let Some(&bref) = children.get(bi) {
            walk(
                lang,
                info,
                ast,
                AstRef::new(bref),
                slots,
                dead_diags,
                bool_cursor,
                bools,
            );
        }
        return;
    }

    // Conditional: arity-3 form with a condition (child 0) and branches.
    if info.is_conditional(&node.kind) {
        let children = child_indices(node);
        if let Some(&cond_ref) = children.first()
            && let Ok(cond) = ast.at(AstRef::new(cond_ref))
            && let Some(val) = cond.payload.as_bool()
        {
            // Constant guard — emit a dead-branch diagnostic.
            let pos = bools
                .get(*bool_cursor)
                .copied()
                .unwrap_or(TokenPos { start: 0, end: 0 });
            *bool_cursor += 1;
            let span = Span::from_byte_range(pos.start, pos.end, "");
            let (msg, dead_idx) = if val {
                // `true` guard: subsequent arms (the else branch, child 2)
                // are dead.
                (
                    "branch guarded by constant `true` is always taken; subsequent arms are dead",
                    2,
                )
            } else {
                // `false` guard: this arm (the then branch, child 1) is dead.
                ("branch guarded by constant `false` is never taken", 1)
            };
            if children.len() > dead_idx {
                let _ = ast.at(AstRef::new(children[dead_idx]));
            }
            dead_diags.push(Diagnostic::new("dead-branch", Severity::Warning, span, msg));
            // Do not recurse into the constant condition (no vars).
        } else if let Some(&cond_ref) = children.first() {
            walk(
                lang,
                info,
                ast,
                AstRef::new(cond_ref),
                slots,
                dead_diags,
                bool_cursor,
                bools,
            );
        }
        // Recurse into the branches.
        for (i, &c) in children.iter().enumerate() {
            if i == 0 {
                continue; // condition already handled
            }
            walk(
                lang,
                info,
                ast,
                AstRef::new(c),
                slots,
                dead_diags,
                bool_cursor,
                bools,
            );
        }
        return;
    }

    // Var: a name reference.
    if let Some(name) = var_name_at(lang, ast, node_ref) {
        slots.push(VarSlot {
            name,
            role: VarRole::Reference,
        });
        return;
    }

    // Default: recurse into all children.
    for &c in &child_indices(node) {
        walk(
            lang,
            info,
            ast,
            AstRef::new(c),
            slots,
            dead_diags,
            bool_cursor,
            bools,
        );
    }
}

/// Extract the `payload["children"]` arena indices from a node.
fn child_indices(node: &AstNode) -> Vec<usize> {
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

/// The name carried by a parameter node: a bare `var` or a `type-annot`
/// wrapping one. Resolved through `Language::param_name` using the
/// `pattern_role: leaf` / `subject` role metadata (ADR-0081).
fn var_name_at(lang: &Language, ast: &Ast, r: AstRef) -> Option<String> {
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
    lang.param_name(ast, node).ok()
}

/// The name of the first parameter child of a node (used for import/export names).
fn first_child_var_name(lang: &Language, ast: &Ast, node: &AstNode) -> Option<String> {
    for &idx in &child_indices(node) {
        if let Some(name) = var_name_at(lang, ast, AstRef::new(idx)) {
            return Some(name);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Internal: naming helpers
// ---------------------------------------------------------------------------

/// Whether `name` is `snake_case` (lowercase letters, digits, underscores;
/// not starting with a digit; not empty).
fn is_snake_case(name: &str) -> bool {
    if name.is_empty() || name == "_" {
        return true;
    }
    let mut has_alpha = false;
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_digit() {
            if i == 0 {
                return false;
            }
            continue;
        }
        if c == '_' {
            continue;
        }
        if c.is_ascii_lowercase() {
            has_alpha = true;
            continue;
        }
        return false;
    }
    has_alpha || name.chars().all(|c| c == '_' || c.is_ascii_digit())
}

/// Convert a name to `snake_case` (insert `_` before uppercase letters, then
/// lowercase; collapse non-alphanumeric runs to `_`).
fn to_snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_alphanumeric() {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

/// `missing-module-doc` (Info) — a module lacks a leading doc comment.
fn rule_missing_module_doc(src: &str) -> Vec<Diagnostic> {
    let trimmed = src.trim_start();
    if trimmed.is_empty() {
        // Empty module — no doc required.
        return Vec::new();
    }
    if trimmed.starts_with("--") {
        return Vec::new();
    }
    let span = Span::from_byte_range(0, 0, src);
    vec![Diagnostic::new(
        "missing-module-doc",
        Severity::Info,
        span,
        "module lacks a leading doc comment",
    )]
}

/// `fmt` (Warning, fixable) — file is not canonical per ADR-0086.
fn rule_fmt(src: &str) -> Vec<Diagnostic> {
    if src.trim().is_empty() {
        return Vec::new();
    }
    match fmt_check_source(src) {
        Ok(true) => Vec::new(),
        Ok(false) => {
            let span = Span::from_byte_range(0, src.len(), src);
            let replacement = fmt_format_source(src).unwrap_or_default();
            let diag = Diagnostic::new(
                "fmt",
                Severity::Warning,
                span,
                "file is not in canonical format",
            )
            .with_fix(Fix { span, replacement });
            vec![diag]
        }
        // Parse errors are reported by the parse stage; don't double-report.
        Err(_) => Vec::new(),
    }
}

/// `unused-import` (Warning, fixable) — import projection never referenced.
fn rule_unused_import(src: &str, parsed: &[ParsedChunk]) -> Vec<Diagnostic> {
    // Collect all referenced names across non-import chunks.
    let mut referenced: HashSet<String> = HashSet::new();
    for p in parsed {
        if p.analysis.is_import {
            continue;
        }
        for (name, _) in &p.analysis.references {
            referenced.insert(name.clone());
        }
        // Binding targets are not references, but an import whose name matches
        // a binding target is still "used" only if referenced. Targets don't
        // count as references to the import.
    }

    let mut diags = Vec::new();
    for p in parsed {
        if !p.analysis.is_import {
            continue;
        }
        let Some(name) = &p.analysis.import_name else {
            continue;
        };
        if referenced.contains(name) {
            continue;
        }
        // Unused — emit a Warning with a Fix removing the import line,
        // consuming the trailing newline so no blank line remains.
        let stmt_end = p.chunk.offset + p.chunk.text.len();
        let line_end = consume_line_end(src, stmt_end);
        let span = Span::from_byte_range(p.chunk.offset, line_end, src);
        let diag = Diagnostic::new(
            "unused-import",
            Severity::Warning,
            span,
            format!("import '{}' is never referenced", name),
        )
        .with_fix(Fix {
            span,
            replacement: String::new(),
        });
        diags.push(diag);
    }
    diags
}

/// `naming-convention` (Warning, fixable) — bindings are `snake_case`.
fn rule_naming_convention(parsed: &[ParsedChunk]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for p in parsed {
        let Some((name, pos)) = &p.analysis.binding_name else {
            continue;
        };
        if is_snake_case(name) {
            continue;
        }
        let span = Span::from_byte_range(pos.start, pos.end, "");
        let renamed = to_snake_case(name);
        let diag = Diagnostic::new(
            "naming-convention",
            Severity::Warning,
            span,
            format!("binding '{}' should be snake_case", name),
        )
        .with_fix(Fix {
            span,
            replacement: renamed,
        });
        diags.push(diag);
    }
    diags
}

/// `shadowed-binding` (Warning) — re-binding a name already in scope.
fn rule_shadowed_binding(parsed: &[ParsedChunk]) -> Vec<Diagnostic> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut diags = Vec::new();
    for p in parsed {
        // Imports introduce names into scope.
        if p.analysis.is_import {
            if let Some(name) = &p.analysis.import_name {
                seen.insert(name.clone());
            }
            continue;
        }
        let Some((name, pos)) = &p.analysis.binding_name else {
            continue;
        };
        if seen.contains(name) {
            let span = Span::from_byte_range(pos.start, pos.end, "");
            diags.push(Diagnostic::new(
                "shadowed-binding",
                Severity::Warning,
                span,
                format!("binding '{}' shadows an existing binding", name),
            ));
        }
        seen.insert(name.clone());
    }
    diags
}

/// `unbound-wire` (Error) — reference to a name not bound in scope.
fn rule_unbound_wire(parsed: &[ParsedChunk]) -> Vec<Diagnostic> {
    // Global scope: imports + all top-level bindings.
    let mut global_scope: HashSet<String> = HashSet::new();
    global_scope.insert("_".to_string()); // wildcard
    for p in parsed {
        if let Some(name) = &p.analysis.import_name {
            global_scope.insert(name.clone());
        }
        if let Some((name, _)) = &p.analysis.binding_name {
            global_scope.insert(name.clone());
        }
    }

    let mut diags = Vec::new();
    for p in parsed {
        // Per-chunk scope adds this chunk's arrow params.
        let mut scope = global_scope.clone();
        for param in &p.analysis.arrow_params {
            scope.insert(param.clone());
        }
        for (name, pos) in &p.analysis.references {
            if scope.contains(name) {
                continue;
            }
            let span = Span::from_byte_range(pos.start, pos.end, "");
            diags.push(Diagnostic::new(
                "unbound-wire",
                Severity::Error,
                span,
                format!("reference to unbound name '{}'", name),
            ));
        }
    }
    diags
}

/// `simplify-paren` (Warning, fixable) — a `paren` node around a single
/// `var`/`value` atom.  The match is found via the ADR-0094 `lint` pass
/// `find_matches`; the source fix is a span-preserving text deletion that
/// replaces the `paren` node span with the source text of its single child.
fn rule_simplify_paren(src: &str, parsed: &[ParsedChunk], lang: &Language) -> Vec<Diagnostic> {
    let engine = RewriteEngine::new();
    let mut diags = Vec::new();
    for p in parsed {
        if p.chunk.text.is_empty() {
            continue;
        }
        let root = AstRef::new(p.ast.len().saturating_sub(1));
        let matches = engine.find_matches(lang, &p.ast, root, "lint");
        for m in matches {
            if m.rule_id != "lint.simplify-paren" {
                continue;
            }
            let Some((span, replacement, inner_text)) =
                paren_text_fix(src, p.chunk.offset, &p.ast, m.at)
            else {
                continue;
            };
            diags.push(
                Diagnostic::new(
                    "simplify-paren",
                    Severity::Warning,
                    span,
                    format!("redundant parentheses around '{}'", inner_text),
                )
                .with_fix(Fix { span, replacement }),
            );
        }
    }
    diags
}

/// Compute a text fix for a matched `paren` node using `AstNode::span`.
///
/// `chunk_offset` is the byte offset of the trimmed chunk in `src`.  Spans
/// produced by the Pratt parser are relative to the chunk, so this offset is
/// added to produce absolute source ranges.
fn paren_text_fix(
    src: &str,
    chunk_offset: usize,
    ast: &Ast,
    paren_ref: AstRef,
) -> Option<(Span, String, String)> {
    let paren = ast.at(paren_ref).ok()?;
    let paren_span = paren.span?;
    let children = child_indices(&paren);
    if children.len() != 1 {
        return None;
    }
    let child = ast.at(AstRef::new(children[0])).ok()?;
    let child_span = child.span?;

    let outer_start = chunk_offset + paren_span.start;
    let outer_end = chunk_offset + paren_span.end;
    let inner_start = chunk_offset + child_span.start;
    let inner_end = chunk_offset + child_span.end;

    let replacement = src.get(inner_start..inner_end)?.to_string();
    let span = Span::from_byte_range(outer_start, outer_end, src);
    Some((span, replacement.clone(), replacement))
}

// ---------------------------------------------------------------------------
// Internal: source span helpers
// ---------------------------------------------------------------------------

/// From `from` (just past the last non-whitespace char of a statement),
/// skip trailing whitespace and consume one trailing newline, returning the
/// byte offset just past the newline (or the source end). This lets a fix
/// remove the whole line without leaving a blank line behind.
fn consume_line_end(src: &str, from: usize) -> usize {
    let bytes = src.as_bytes();
    let mut i = from.min(bytes.len());
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'\n' {
        i += 1;
    }
    i
}

// ---------------------------------------------------------------------------
// Existing platform tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{
        Behavior, CapabilityId, CapabilityRequest, KitRegistry, Lint, LintRequest, PlatformError,
        Plugin, PluginContext, PluginRegistry,
    };

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = BasicLinter::new();
        let meta = plugin.metadata(&CapabilityId::from("lint")).unwrap();
        assert_eq!(meta["command"], "lint");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_diagnostics_when_rule_matches() {
        struct Warn;

        impl Behavior<Lint> for Warn {
            fn kind(&self) -> &str {
                "unused"
            }

            fn apply(&self, op: &mut LintRequest) -> Result<(), PlatformError> {
                op.diagnostics.push("unused variable".to_string());
                Ok(())
            }
        }

        impl LintBehavior for Warn {}

        let mut linter = BasicLinter::new();
        linter.accept(Box::new(Warn)).unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = linter
            .run(
                &CapabilityRequest {
                    capability: "lint".into(),
                    payload: serde_json::json!({"kind": "unused"}),
                },
                &mut ctx,
            )
            .unwrap();
        assert_eq!(
            resp.0,
            serde_json::json!({"diagnostics": ["unused variable"]})
        );
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = BasicLinter::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let err = plugin
            .run(
                &CapabilityRequest {
                    capability: "fmt".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }

    // --- canonical linter unit tests ---

    #[test]
    fn lint_error_display() {
        let err = LintError::Parse("bad token".to_string());
        assert_eq!(format!("{}", err), "parse error: bad token");
    }

    #[test]
    fn lint_source_empty_input() {
        let diags = lint_source("").unwrap();
        // Empty input is lint-clean (no module doc required for empty modules).
        assert!(diags.is_empty());
    }

    #[test]
    fn lint_source_clean_input() {
        let src = "-- A simple module.\nmain = 42\n";
        let diags = lint_source(src).unwrap();
        let non_fmt: Vec<_> = diags.iter().filter(|d| d.rule != "fmt").collect();
        assert!(
            non_fmt.is_empty(),
            "expected no non-fmt diagnostics, got {:?}",
            non_fmt
        );
    }

    #[test]
    fn is_snake_case_helpers() {
        assert!(is_snake_case("foo"));
        assert!(is_snake_case("foo_bar"));
        assert!(is_snake_case("x"));
        assert!(!is_snake_case("Foo"));
        assert!(!is_snake_case("fooBar"));
        assert_eq!(to_snake_case("fooBar"), "foo_bar");
        assert_eq!(to_snake_case("MyFunc"), "my_func");
    }
}
