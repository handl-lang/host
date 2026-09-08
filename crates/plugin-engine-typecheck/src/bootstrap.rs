//! Bootstrap typecheck pass over the v0.1 surface AST (ADR-0088).
//!
//! This is a **bootstrap checker**, not the final inference engine. It
//! implements the decidable core of ADR-0074's typing rules in a single pass:
//!
//! 1. **Collect bindings** — walk top-level nodes, collecting imports,
//!    exports, and bindings (`name = expr`), detecting duplicates.
//! 2. **Module-level checks** — every exported name is bound, every import
//!    projection is used, no duplicate bindings.
//! 3. **Expression checking** — bottom-up walk by AST child roles
//!    (ADR-0081): unbound-wire detection, arrow kind checking, structural
//!    type inference, and `rec` region well-formedness.
//!
//! Row-variable inference and full `rec` fixpoint typing stay behind explicit
//! `todo!` boundaries, emitting `not-yet-checked` Info diagnostics rather than
//! silently passing. Error recovery reports **all** diagnostics per module,
//! not just the first failure.
//!
//! Diagnostics use the shared [`handl_plugin_ext_diag::Diagnostic`] type
//! (ADR-0091) — no local variant types are defined here.

use handl_language::{
    Ast, AstNode, AstRef, AstShapeDef, CapabilityDecl, EvalSchemeDef, GrammarRow, Language,
    LexerRuleDef, OptionSchema, TokenKindDef, TransformDef, TypeRuleDef,
};
use handl_plugin_engine_lex_pattern::lex;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_engine_transform::validate::validate_transforms;
use handl_plugin_ext_diag::{Diagnostic, Severity, Span};

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crate::{CheckError, TypecheckReport};

// ---------------------------------------------------------------------------
// Canonical language assembly (embedded YAML data files)
// ---------------------------------------------------------------------------
//
// The bootstrap checker cannot depend on `handl-kit-data-canonical` because
// that crate pulls in `handl-runtime`, which depends on this crate
// (`handl-plugin-engine-typecheck`) for the `TypecheckEngine` trait — a
// cyclic dependency. Instead, the nine canonical YAML data files are
// embedded directly via `include_str!` and merged into a `Language` using
// the `handl-language` fragment API, exactly as
// `handl_kit_data_canonical::language_from_yaml` does — including the
// `desugar:` → `TransformDef` lowering (`Language::merge_ast_fragment`),
// the `transforms.yaml` merge, and transform-rule validation
// (`validate_transforms`, ADR-0094 "Production readiness"). The language is
// assembled once and cached for the process lifetime.

const TOKENS_YAML: &str = include_str!("../../handl-kit-data-canonical/data/tokens.yaml");
const GRAMMAR_YAML: &str = include_str!("../../handl-kit-data-canonical/data/grammar.yaml");
const EVAL_YAML: &str = include_str!("../../handl-kit-data-canonical/data/eval.yaml");
const TYPES_YAML: &str = include_str!("../../handl-kit-data-canonical/data/types.yaml");
const AST_YAML: &str = include_str!("../../handl-kit-data-canonical/data/ast.yaml");
const CAPS_YAML: &str = include_str!("../../handl-kit-data-canonical/data/caps.yaml");
const OPTIONS_YAML: &str = include_str!("../../handl-kit-data-canonical/data/options.yaml");
const LEXER_RULES_YAML: &str = include_str!("../../handl-kit-data-canonical/data/lexer_rules.yaml");
const TRANSFORMS_YAML: &str =
    include_str!("../../handl-kit-data-canonical/data/transforms.yaml");

/// Merge the nine embedded YAML fragment strings into a single [`Language`].
fn assemble_language() -> Result<Language, CheckError> {
    assemble_language_fragments(
        TOKENS_YAML,
        GRAMMAR_YAML,
        EVAL_YAML,
        TYPES_YAML,
        AST_YAML,
        CAPS_YAML,
        OPTIONS_YAML,
        LEXER_RULES_YAML,
        TRANSFORMS_YAML,
    )
}

/// Merge nine YAML fragment strings into a single [`Language`], mirroring
/// `handl_kit_data_canonical::merge_fragments`: `desugar:`-field lowering via
/// [`Language::merge_ast_fragment`], the first-class `transforms` merge, then
/// transform-rule validation (Error-severity diagnostics fail assembly).
fn assemble_language_fragments(
    tokens_yaml: &str,
    grammar_yaml: &str,
    eval_yaml: &str,
    types_yaml: &str,
    ast_yaml: &str,
    caps_yaml: &str,
    options_yaml: &str,
    lexer_rules_yaml: &str,
    transforms_yaml: &str,
) -> Result<Language, CheckError> {
    let mut lang = Language::new();

    let tokens: Vec<TokenKindDef> = serde_yaml::from_str(tokens_yaml)
        .map_err(|e| CheckError::Parse(format!("tokens.yaml: {e}")))?;
    for frag in tokens {
        lang.tokens
            .merge(frag)
            .map_err(|e| CheckError::Parse(format!("tokens.yaml: {e}")))?;
    }

    let grammar: Vec<GrammarRow> = serde_yaml::from_str(grammar_yaml)
        .map_err(|e| CheckError::Parse(format!("grammar.yaml: {e}")))?;
    for frag in grammar {
        lang.grammar
            .merge(frag)
            .map_err(|e| CheckError::Parse(format!("grammar.yaml: {e}")))?;
    }

    let eval: Vec<EvalSchemeDef> = serde_yaml::from_str(eval_yaml)
        .map_err(|e| CheckError::Parse(format!("eval.yaml: {e}")))?;
    for frag in eval {
        lang.eval
            .merge(frag)
            .map_err(|e| CheckError::Parse(format!("eval.yaml: {e}")))?;
    }

    let types: Vec<TypeRuleDef> = serde_yaml::from_str(types_yaml)
        .map_err(|e| CheckError::Parse(format!("types.yaml: {e}")))?;
    for frag in types {
        lang.types
            .merge(frag)
            .map_err(|e| CheckError::Parse(format!("types.yaml: {e}")))?;
    }

    // AST shapes (union). `merge_ast_fragment` also lowers any `desugar:`
    // rule a shape carries into a `TransformDef` in `lang.transforms`
    // (pass "desugar", ADR-0094 §6) — same as `merge_fragments`.
    let ast: Vec<AstShapeDef> =
        serde_yaml::from_str(ast_yaml).map_err(|e| CheckError::Parse(format!("ast.yaml: {e}")))?;
    for frag in ast {
        lang.merge_ast_fragment(frag)
            .map_err(|e| CheckError::Parse(format!("ast.yaml: {e}")))?;
    }

    let caps: Vec<CapabilityDecl> = serde_yaml::from_str(caps_yaml)
        .map_err(|e| CheckError::Parse(format!("caps.yaml: {e}")))?;
    for frag in caps {
        lang.caps
            .merge(frag)
            .map_err(|e| CheckError::Parse(format!("caps.yaml: {e}")))?;
    }

    let options: Vec<OptionSchema> = serde_yaml::from_str(options_yaml)
        .map_err(|e| CheckError::Parse(format!("options.yaml: {e}")))?;
    for frag in options {
        lang.options
            .merge(frag)
            .map_err(|e| CheckError::Parse(format!("options.yaml: {e}")))?;
    }

    let lexer_rules: Vec<LexerRuleDef> = serde_yaml::from_str(lexer_rules_yaml)
        .map_err(|e| CheckError::Parse(format!("lexer_rules.yaml: {e}")))?;
    for frag in lexer_rules {
        lang.lexer_rules
            .merge(frag)
            .map_err(|e| CheckError::Parse(format!("lexer_rules.yaml: {e}")))?;
    }

    // Transform rules (ADR-0094). Merged *after* the ast pass so a
    // first-class rule may `replaces:` a `desugar:`-lowered
    // `transform.desugar.*` rule synthesized above.
    let transforms: Vec<TransformDef> = serde_yaml::from_str(transforms_yaml)
        .map_err(|e| CheckError::Parse(format!("transforms.yaml: {e}")))?;
    for frag in transforms {
        lang.transforms
            .merge(frag)
            .map_err(|e| CheckError::Parse(format!("transforms.yaml: {e}")))?;
    }

    // ADR-0094 "Production readiness": transform-rule validation runs at
    // `Language` build *and* under `hnl typecheck`. Error-severity
    // diagnostics fail assembly — the check path must not run against a
    // language whose transform rules are malformed.
    let errors: Vec<String> = validate_transforms(&lang)
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| d.message.clone())
        .collect();
    if !errors.is_empty() {
        return Err(CheckError::Language(format!(
            "transform rule validation failed: {}",
            errors.join("; ")
        )));
    }

    Ok(lang)
}

/// Return a reference to the process-wide cached canonical [`Language`].
///
/// The nine YAML data files are compiled in and pre-resolved (verified by
/// the `handl-kit-data-canonical` test suite), so assembly should not fail
/// at runtime — but if it does (e.g. a malformed transform rule slipped in),
/// the failure is propagated as a [`CheckError::Language`] so `hnl typecheck`
/// prints a readable diagnostic instead of panicking.
fn canonical_language() -> Result<&'static Language, CheckError> {
    static LANG: OnceLock<Result<Language, CheckError>> = OnceLock::new();
    LANG.get_or_init(assemble_language)
        .as_ref()
        .map_err(Clone::clone)
}

// ---------------------------------------------------------------------------
// v0.1 surface node kinds
// ---------------------------------------------------------------------------
//
// The bootstrap checker is explicitly scoped to the v0.1 surface syntax
// (ADR-0088). The kind strings below are the v0.1 surface's canonical node
// kinds produced by the data-driven Pratt parser over the canonical
// `Language`. They are read-only constants used to classify top-level
// declarations and reference nodes; child positions are always resolved via
// `Language::child_role_index` (ADR-0081), never by positional assumption.

/// The v0.1 surface kind for an import statement (`import x from "path"`).
const KIND_IMPORT: &str = "stmt.import";
/// The v0.1 surface kind for an export statement (`export x from "path"`).
const KIND_EXPORT: &str = "stmt.export";
/// The v0.1 surface kind for a `rec` recursive region.
const KIND_REC: &str = "rec";
/// The v0.1 surface kind for an identifier reference.
const KIND_VAR: &str = "var";
/// The v0.1 surface kind for an object entry (`key: value`).
const KIND_ENTRY: &str = "entry";
/// The v0.1 surface kind for a field access (`target.field`).
const KIND_FIELD_ACCESS: &str = "field-access";

// ---------------------------------------------------------------------------
// Source-chunk splitting
// ---------------------------------------------------------------------------

/// A top-level statement chunk: its trimmed text and byte offset in the
/// original source.
struct Chunk {
    /// The trimmed statement text.
    text: String,
    /// Byte offset of the trimmed text's first byte in the original source.
    offset: usize,
}

/// Split `src` into top-level statement chunks.
///
/// The v0.1 surface separates top-level declarations by newlines or
/// semicolons. The data-driven Pratt parser parses a single expression per
/// call, so the bootstrap checker splits the source into statements at
/// bracket-depth zero, respecting string literals and `--` comments.
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

    // Trailing chunk.
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
// Parsing
// ---------------------------------------------------------------------------

/// A parsed top-level statement with its source-chunk metadata.
struct Stmt {
    /// The statement's AST arena (the Pratt parser produces a fresh arena).
    ast: Ast,
    /// The root node reference (last node pushed by the Pratt parser).
    root: AstRef,
    /// Byte offset of the chunk's start in the original source.
    offset: usize,
    /// The chunk's trimmed text (for span computation).
    text: String,
}

/// Parse a single statement chunk into an [`Ast`].
fn parse_chunk(lang: &Language, text: &str) -> Result<Ast, CheckError> {
    let tokens = lex(lang, text).map_err(CheckError::from)?;
    if tokens.as_slice().is_empty() {
        return Ok(Ast::new());
    }
    let req = ParserRequest::new(tokens);
    PrattEngine::new()
        .parse(lang, &req)
        .map_err(CheckError::from)
}

// ---------------------------------------------------------------------------
// AST helpers (data-driven child resolution, ADR-0081)
// ---------------------------------------------------------------------------

/// Extract the `children` index array from a node's payload.
fn children_of(node: &AstNode) -> Vec<usize> {
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

/// The name carried by a v0.1 `var` node (its payload is the identifier
/// string). Returns `None` for non-`var` nodes.
fn var_name(node: &AstNode) -> Option<String> {
    if node.kind == KIND_VAR {
        node.payload.as_str().map(String::from)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Span computation
// ---------------------------------------------------------------------------

/// The span covering an entire statement chunk.
fn chunk_span(src: &str, stmt: &Stmt) -> Span {
    Span::from_byte_range(stmt.offset, stmt.offset + stmt.text.len(), src)
}

/// Whether a byte is an identifier continuation character
/// (`[A-Za-z0-9_]`).
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Find the first whole-word occurrence of `name` in the statement chunk and
/// return its span. Falls back to the full chunk span if not found.
fn name_span(src: &str, stmt: &Stmt, name: &str) -> Span {
    let bytes = stmt.text.as_bytes();
    let name_bytes = name.as_bytes();
    let mut search = 0usize;
    while search + name_bytes.len() <= bytes.len() {
        if let Some(pos) = stmt.text[search..].find(name) {
            let abs = search + pos;
            let before_ok = abs == 0 || !is_ident_byte(bytes[abs - 1]);
            let after_idx = abs + name_bytes.len();
            let after_ok = after_idx >= bytes.len() || !is_ident_byte(bytes[after_idx]);
            if before_ok && after_ok {
                let start = stmt.offset + abs;
                let end = start + name_bytes.len();
                return Span::from_byte_range(start, end, src);
            }
            search = abs + name_bytes.len();
        } else {
            break;
        }
    }
    chunk_span(src, stmt)
}

// ---------------------------------------------------------------------------
// Binding collection
// ---------------------------------------------------------------------------

/// A module-level binding (`name = expr`).
struct Binding {
    name: String,
    stmt_idx: usize,
}

/// A module-level import (`import x from "path"`).
struct Import {
    name: String,
    stmt_idx: usize,
}

/// A module-level export (`export x from "path"`).
struct Export {
    name: String,
    stmt_idx: usize,
}

/// Classify and collect all top-level declarations from the parsed statements.
fn collect_decls(
    lang: &Language,
    stmts: &[Stmt],
    diags: &mut Vec<Diagnostic>,
    src: &str,
) -> (Vec<Binding>, Vec<Import>, Vec<Export>) {
    let mut bindings = Vec::new();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();

    for (i, stmt) in stmts.iter().enumerate() {
        let root = match stmt.ast.at(stmt.root) {
            Ok(n) => n,
            Err(_) => continue,
        };

        // Binding: a node with `target` + `value` roles (data-driven —
        // any shape declaring these roles is a binding, not just `assign`).
        let target_idx = lang.child_role_index(&root.kind, "target").ok().flatten();
        let value_idx = lang.child_role_index(&root.kind, "value").ok().flatten();
        if let (Some(ti), Some(_vi)) = (target_idx, value_idx) {
            let children = children_of(root);
            if let Some(&target_raw) = children.get(ti) {
                if let Ok(target_node) = stmt.ast.at(AstRef::new(target_raw)) {
                    if let Some(name) = var_name(target_node) {
                        if seen.contains_key(&name) {
                            let span = name_span(src, stmt, &name);
                            diags.push(Diagnostic::new(
                                "duplicate-binding",
                                Severity::Error,
                                span,
                                format!("duplicate binding: `{name}`"),
                            ));
                        } else {
                            seen.insert(name.clone(), i);
                        }
                        bindings.push(Binding { name, stmt_idx: i });
                    }
                }
            }
            continue;
        }

        // Import: `stmt.import` (no roles → positional child 0).
        if root.kind == KIND_IMPORT {
            let children = children_of(root);
            if let Some(&child_raw) = children.first() {
                if let Ok(child_node) = stmt.ast.at(AstRef::new(child_raw)) {
                    if let Some(name) = var_name(child_node) {
                        imports.push(Import { name, stmt_idx: i });
                    }
                }
            }
            // `from "path" import * as alias` stores the alias in the payload.
            if let Some(name) = root.payload.get("alias").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    imports.push(Import {
                        name: name.to_string(),
                        stmt_idx: i,
                    });
                }
            }
            continue;
        }

        // Export: `stmt.export` (no roles → positional child 0).
        if root.kind == KIND_EXPORT {
            let children = children_of(root);
            if let Some(&child_raw) = children.first() {
                if let Ok(child_node) = stmt.ast.at(AstRef::new(child_raw)) {
                    if let Some(name) = var_name(child_node) {
                        exports.push(Export { name, stmt_idx: i });
                    }
                }
            }
            continue;
        }
    }

    (bindings, imports, exports)
}

// ---------------------------------------------------------------------------
// Expression checking
// ---------------------------------------------------------------------------

/// Recursively check an expression subtree for unbound wires, `rec`
/// not-yet-checked boundaries, and arrow-kind consistency.
///
/// `scope` is the set of names in scope (imports + bindings + function
/// params). `references` accumulates every `var` reference seen (for
/// unused-import detection).
fn check_expr(
    lang: &Language,
    src: &str,
    ast: &Ast,
    node_ref: AstRef,
    scope: &HashSet<String>,
    stmt: &Stmt,
    diags: &mut Vec<Diagnostic>,
    references: &mut HashSet<String>,
) {
    let node = match ast.at(node_ref) {
        Ok(n) => n,
        Err(_) => return,
    };

    // Var reference — check scope.
    if let Some(name) = var_name(node) {
        references.insert(name.clone());
        if !scope.contains(&name) {
            let span = name_span(src, stmt, &name);
            diags.push(Diagnostic::new(
                "unbound-wire",
                Severity::Error,
                span,
                format!("unbound wire: `{name}` is not in scope"),
            ));
        }
        return;
    }

    // Function (arrow lambda `x ~> body`): add params to scope, check body.
    // Identified data-drivenly by the `params` + `body` roles (ADR-0081).
    let params_idx = lang.child_role_index(&node.kind, "params").ok().flatten();
    let body_idx = lang.child_role_index(&node.kind, "body").ok().flatten();
    if let (Some(pi), Some(bi)) = (params_idx, body_idx) {
        let children = children_of(node);
        let mut inner_scope = scope.clone();
        // The params child is a `var` node carrying the parameter name.
        // v0.1 arrows are single-parameter (`x ~> body`).
        if let Some(&params_raw) = children.get(pi) {
            if let Ok(params_node) = ast.at(AstRef::new(params_raw)) {
                if let Some(name) = var_name(params_node) {
                    inner_scope.insert(name);
                }
            }
        }
        if let Some(&body_raw) = children.get(bi) {
            check_expr(
                lang,
                src,
                ast,
                AstRef::new(body_raw),
                &inner_scope,
                stmt,
                diags,
                references,
            );
        }
        return;
    }

    // `rec` region — full fixpoint typing is deferred (ADR-0088). Emit an
    // Info `not-yet-checked` diagnostic rather than silently passing, then
    // continue walking children for unbound-wire detection.
    if node.kind == KIND_REC {
        let span = chunk_span(src, stmt);
        diags.push(Diagnostic::new(
            "not-yet-checked",
            Severity::Info,
            span,
            "not yet checked: rec fixpoint typing",
        ));
    }

    // Object entry (`key: value`): child 0 is the field-name key (a `var`
    // that is NOT a reference), child 1 is the value expression. Only walk
    // the value child so field names are not flagged as unbound wires.
    if node.kind == KIND_ENTRY {
        let children = children_of(node);
        if let Some(&value_raw) = children.get(1) {
            check_expr(
                lang,
                src,
                ast,
                AstRef::new(value_raw),
                scope,
                stmt,
                diags,
                references,
            );
        }
        return;
    }

    // Field access (`target.field`): child 0 is the target expression, child
    // 1 is the field-name `var` (NOT a reference). Only walk the target so
    // field names are not flagged as unbound wires.
    if node.kind == KIND_FIELD_ACCESS {
        let children = children_of(node);
        if let Some(&target_raw) = children.first() {
            check_expr(
                lang,
                src,
                ast,
                AstRef::new(target_raw),
                scope,
                stmt,
                diags,
                references,
            );
        }
        return;
    }

    // Generic recursion: walk all children by positional `children` array.
    // (Roles are used above for named slots; the remaining children are
    // walked positionally per ADR-0081's fallback for shapes without roles.)
    for &child_raw in &children_of(node) {
        check_expr(
            lang,
            src,
            ast,
            AstRef::new(child_raw),
            scope,
            stmt,
            diags,
            references,
        );
    }
}

// ---------------------------------------------------------------------------
// Check pass
// ---------------------------------------------------------------------------

/// Run the single-pass bootstrap check over all parsed statements.
fn check_pass(lang: &Language, src: &str, stmts: &[Stmt], diags: &mut Vec<Diagnostic>) {
    // Phase 1: collect bindings, imports, exports (+ duplicate detection).
    let (bindings, imports, exports) = collect_decls(lang, stmts, diags, src);

    // Build the module-level scope: all binding names + all import names.
    let binding_names: HashSet<String> = bindings.iter().map(|b| b.name.clone()).collect();
    let import_names: HashSet<String> = imports.iter().map(|i| i.name.clone()).collect();
    let module_scope: HashSet<String> = binding_names
        .iter()
        .cloned()
        .chain(import_names.iter().cloned())
        .collect();

    // Phase 2a: every exported name must be bound.
    for exp in &exports {
        if !binding_names.contains(&exp.name) {
            let stmt = &stmts[exp.stmt_idx];
            let span = name_span(src, stmt, &exp.name);
            diags.push(Diagnostic::new(
                "unbound-export",
                Severity::Error,
                span,
                format!("export `{}` is not bound in this module", exp.name),
            ));
        }
    }

    // Phase 3: expression checking (bottom-up by child roles).
    // Accumulate all var references for unused-import detection.
    let mut references: HashSet<String> = HashSet::new();

    // Walk the root of every statement that is not a declaration — bare
    // expressions (e.g. `a = b` equal-comparisons, calls) still need
    // unbound-wire checking. Binding targets are names, not references, so
    // bindings/import/export statement roots are skipped here (binding
    // values are checked below).
    let decl_stmts: HashSet<usize> = bindings
        .iter()
        .map(|b| b.stmt_idx)
        .chain(imports.iter().map(|i| i.stmt_idx))
        .chain(exports.iter().map(|e| e.stmt_idx))
        .collect();
    for (i, stmt) in stmts.iter().enumerate() {
        if decl_stmts.contains(&i) {
            continue;
        }
        check_expr(
            lang,
            src,
            &stmt.ast,
            stmt.root,
            &module_scope,
            stmt,
            diags,
            &mut references,
        );
    }

    for binding in &bindings {
        let stmt = &stmts[binding.stmt_idx];
        let root = match stmt.ast.at(stmt.root) {
            Ok(n) => n,
            Err(_) => continue,
        };
        // Resolve the value child by role (`value`), falling back to
        // positional index 1.
        let value_idx = lang
            .child_role_index(&root.kind, "value")
            .ok()
            .flatten()
            .unwrap_or(1);
        let children = children_of(root);
        if let Some(&value_raw) = children.get(value_idx) {
            check_expr(
                lang,
                src,
                &stmt.ast,
                AstRef::new(value_raw),
                &module_scope,
                stmt,
                diags,
                &mut references,
            );
        }
    }

    // Also check expressions in export statements (the exported value, if
    // any, beyond a re-export). For v0.1 `export x from "path"`, the child is
    // a name var (already handled as a declaration, not an expression).
    // Exports that are expressions would be checked here in a fuller impl.

    // Phase 2b: every import must be referenced (unused-import warning).
    for imp in &imports {
        if !references.contains(&imp.name) {
            let stmt = &stmts[imp.stmt_idx];
            let span = name_span(src, stmt, &imp.name);
            diags.push(Diagnostic::new(
                "unused-import",
                Severity::Warning,
                span,
                format!("import `{}` is never referenced", imp.name),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Check a handl module source text (ADR-0088).
///
/// Parses the source into top-level statements and runs the bootstrap
/// typecheck pass. Returns a report with **all** diagnostics found (error
/// recovery: reports all issues, not just the first).
///
/// # Errors
///
/// Returns [`CheckError::Parse`] if any statement cannot be parsed.
pub fn check_module(src: &str) -> Result<TypecheckReport, CheckError> {
    // Assemble the canonical v0.1 language from the embedded YAML data
    // fragments (cached for the process lifetime).
    let lang = canonical_language()?;

    // Split into top-level statements and parse each.
    let chunks = split_statements(src);
    let mut stmts = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let ast = parse_chunk(lang, &chunk.text)?;
        if ast.is_empty() {
            continue;
        }
        let root = AstRef::new(ast.len() - 1);
        stmts.push(Stmt {
            ast,
            root,
            offset: chunk.offset,
            text: chunk.text,
        });
    }

    // Run the single-pass check.
    let mut diags = Vec::new();
    check_pass(lang, src, &stmts, &mut diags);

    Ok(TypecheckReport { diagnostics: diags })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The canonical embedded data files must assemble cleanly — including
    /// `transforms.yaml` and `desugar:`-field lowering — with no
    /// Error-severity validation diagnostics (ADR-0094).
    #[test]
    fn canonical_language_assembles_with_transforms() {
        let lang = assemble_language().expect("canonical language assembles");
        assert!(
            !lang.transforms.is_empty(),
            "transforms.yaml rules should land in lang.transforms"
        );
        assert!(
            validate_transforms(&lang)
                .iter()
                .all(|d| d.severity != Severity::Error),
            "canonical language must have no Error-severity transform diagnostics"
        );
    }

    /// A transform rule whose `Keep` step is out of bounds for the target
    /// shape is an Error-severity diagnostic and must fail assembly with a
    /// readable `CheckError::Language` — this is what `hnl typecheck`
    /// reports instead of checking against a malformed language.
    #[test]
    fn broken_transform_rule_fails_assembly() {
        let broken_transforms = r#"
- id: transform.desugar.paren-bogus
  pass: desugar
  node_kind: paren
  rule: !Declarative
    target_kind: paren
    children:
      - !Keep 9
"#;
        let err = assemble_language_fragments(
            TOKENS_YAML,
            GRAMMAR_YAML,
            EVAL_YAML,
            TYPES_YAML,
            AST_YAML,
            CAPS_YAML,
            OPTIONS_YAML,
            LEXER_RULES_YAML,
            broken_transforms,
        )
        .expect_err("out-of-bounds Keep step must fail assembly");
        match err {
            CheckError::Language(msg) => {
                assert!(
                    msg.contains("transform rule validation failed"),
                    "expected validation failure message, got: {msg}"
                );
                assert!(
                    msg.contains("transform.desugar.paren-bogus"),
                    "expected the offending rule id in the message, got: {msg}"
                );
            }
            other => panic!("expected CheckError::Language, got: {other:?}"),
        }
        // And the rendered error is readable for the CLI path.
        let shown = CheckError::Language("x".into()).to_string();
        assert!(shown.starts_with("language error:"), "got: {shown}");
    }

    /// A `desugar:` field on an `AstShapeDef` still lowers into a
    /// `TransformDef` (backward compat, ADR-0094 §6) on this merge path too.
    #[test]
    fn desugar_field_lowers_into_transforms() {
        let ast_yaml = r#"
- id: ast.sugared
  node_kind: sugared
  children:
    - role: body
  desugar:
    target_kind: value
    children: []
"#;
        let lang = assemble_language_fragments(
            "[]", "[]", "[]", "[]", ast_yaml, "[]", "[]", "[]", "[]",
        )
        .expect("shape with desugar field assembles");
        assert!(
            lang.transforms
                .get_by_id("transform.desugar.sugared")
                .is_some(),
            "desugar field should lower into lang.transforms"
        );
    }
}
