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
use handl_plugin_ext_diag::{Diagnostic, Severity, Span, split_statements};

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
/// The v0.1 surface kind for an inductive type declaration (`data Name :: ...`).
const KIND_DATA: &str = "data";

// ---------------------------------------------------------------------------
// Source-chunk splitting
// ---------------------------------------------------------------------------
//
// The splitter is shared across the typechecker, linter, and test runner —
// it lives in `plugin-ext-diag` (`split_statements`, AGENTS.md "Source
// splitting").

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

/// Whether a module specifier is a bare host-module name (e.g. `"str"`,
/// `"str-handl"`) rather than a path. A bare name is a single segment of
/// identifier/`kebab` characters — no `/`, `\`, `.`, or leading `./`.
/// See ADR-0096: bare names resolve via the host module chain, not the
/// file loader.
fn is_bare_module_name(spec: &str) -> bool {
    !spec.is_empty()
        && spec
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && spec.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
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

/// Recursively collect the constructor names declared by a `data` declaration.
///
/// Constructor shapes are `var`/`call`/`alt` (and possibly paren-wrapped);
/// field/type names inside the constructor's `type-annot` arguments are
/// intentionally not collected. Positions are resolved via `AstShapeDef` roles
/// (ADR-0081).
fn collect_data_ctors(
    lang: &Language,
    ast: &Ast,
    node_ref: AstRef,
    out: &mut HashSet<String>,
) {
    let node = match ast.at(node_ref) {
        Ok(n) => n,
        Err(_) => return,
    };

    // Bare constructor name.
    if let Some(name) = var_name(node) {
        out.insert(name);
        return;
    }

    // `call(...)` — the `callee` child is the constructor name; the `args`
    // children are field/type annotations and are ignored here.
    if let Ok(Some(callee_idx)) = lang.child_role_index(&node.kind, "callee") {
        let children = children_of(node);
        if let Some(&callee_raw) = children.get(callee_idx) {
            collect_data_ctors(lang, ast, AstRef::new(callee_raw), out);
        }
        return;
    }

    // `alt` constructor alternatives (`|`).
    if let Ok(Some(left_idx)) = lang.child_role_index(&node.kind, "left") {
        let children = children_of(node);
        if let Some(&left_raw) = children.get(left_idx) {
            collect_data_ctors(lang, ast, AstRef::new(left_raw), out);
        }
        if let Ok(Some(right_idx)) = lang.child_role_index(&node.kind, "right") {
            if let Some(&right_raw) = children.get(right_idx) {
                collect_data_ctors(lang, ast, AstRef::new(right_raw), out);
            }
        }
        return;
    }

    // Paren-wrapped constructor (e.g. `(Cons)`); unwrap via the `body` role.
    if let Ok(Some(body_idx)) = lang.child_role_index(&node.kind, "body") {
        let children = children_of(node);
        if let Some(&body_raw) = children.get(body_idx) {
            collect_data_ctors(lang, ast, AstRef::new(body_raw), out);
        }
    }
}

/// The name bound by a binding's `target` child.
///
/// Normally the target is a `var`, but nested binding-shaped targets occur
/// when a statement chains bind operators — `a :: f = g` parses as
/// `assign(assign(a, f), g)`, and the bound name is the innermost `target`.
/// `subject`/`body` roles are also followed so paren- or
/// type-annot-wrapped targets still resolve their binder (ADR-0081 —
/// role-driven, never positional).
fn binding_target_name(lang: &Language, ast: &Ast, node_ref: AstRef) -> Option<String> {
    let node = ast.at(node_ref).ok()?;
    if let Some(name) = var_name(node) {
        return Some(name);
    }
    let children = children_of(node);
    for role in ["target", "subject", "body"] {
        if let Ok(Some(idx)) = lang.child_role_index(&node.kind, role) {
            if let Some(&raw) = children.get(idx) {
                if let Some(name) = binding_target_name(lang, ast, AstRef::new(raw)) {
                    return Some(name);
                }
            }
        }
    }
    None
}

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
) -> (Vec<Binding>, Vec<Import>, Vec<Export>, HashSet<String>) {
    let mut bindings = Vec::new();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut data_ctors = HashSet::new();
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
                if let Some(name) = binding_target_name(lang, &stmt.ast, AstRef::new(target_raw)) {
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
            continue;
        }

        // Import: `stmt.import` (no roles → positional child 0).
        if root.kind == KIND_IMPORT {
            for name in stmt_import_bound_names(&stmt.ast, root) {
                imports.push(Import { name, stmt_idx: i });
            }
            continue;
        }

        // Export: `stmt.export` (no roles → positional child 0). The child
        // is either a plain `var` (`export x` / `export x from "path"`) or
        // a binding-shaped subtree (`export x :: expr` parses as
        // `export(assign(x, expr))`) — in the latter case the export both
        // declares `x` and binds it in module scope.
        if root.kind == KIND_EXPORT {
            let children = children_of(root);
            if let Some(&child_raw) = children.first() {
                let child_ref = AstRef::new(child_raw);
                if let Ok(child_node) = stmt.ast.at(child_ref) {
                    if let Some(name) = var_name(child_node) {
                        // `export x` / `export x from "path"` — a re-export
                        // of an existing binding, not a new binding.
                        exports.push(Export { name, stmt_idx: i });
                    } else if let Some(name) =
                        binding_target_name(lang, &stmt.ast, child_ref)
                    {
                        // `export x :: expr` — the export declares `x` and
                        // binds it in module scope.
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
                        bindings.push(Binding {
                            name: name.clone(),
                            stmt_idx: i,
                        });
                        exports.push(Export { name, stmt_idx: i });
                    }
                }
            }
            continue;
        }

        // `data` declaration — the constructors are bound into module scope
        // so later references to them (e.g. `True`, `Sorted`) do not fire.
        if root.kind == KIND_DATA {
            if let Ok(Some(ctors_idx)) = lang.child_role_index(&root.kind, "constructors") {
                let children = children_of(root);
                if let Some(&ctors_raw) = children.get(ctors_idx) {
                    collect_data_ctors(lang, &stmt.ast, AstRef::new(ctors_raw), &mut data_ctors);
                }
            }
            continue;
        }
    }

    (bindings, imports, exports, data_ctors)
}

/// Names bound into scope by a `stmt.import` node (ADR-0096).
///
/// Covers all import surface forms:
/// - `import <name>` (and the `import <name> from "path"` module-declaration
///   order, which parses as `import (<name>)`) — the child `var` is bound;
/// - `from "<spec>" import <name>` — `name` is bound (named projection);
/// - `from "<spec>" import <name> as <alias>` — `alias` is bound;
/// - `from "<spec>" import * as <alias>` — `alias` is bound;
/// - `from "<bare-name>" import *` — the bare module name itself is bound
///   (host-module names like `str` resolve qualified calls through the
///   module chain);
/// - `import "<spec>"` and `from "<spec>" import *` with a path specifier
///   splice exports statically unknown here — they bind no name.
fn stmt_import_bound_names(ast: &Ast, root: &AstNode) -> Vec<String> {
    let mut bound = Vec::new();
    // `import <name>` — the single child is a `var` carrying the name (the
    // `from`-form child is the path string, which `var_name` rejects).
    let children = children_of(root);
    if let Some(&child_raw) = children.first() {
        if let Ok(child_node) = ast.at(AstRef::new(child_raw)) {
            if let Some(name) = var_name(child_node) {
                bound.push(name);
            }
        }
    }
    let alias = root.payload.get("alias").and_then(|v| v.as_str());
    let names = root.payload.get("names").and_then(|v| v.as_str());
    let path = root.payload.get("path").and_then(|v| v.as_str());
    if let Some(a) = alias {
        if !a.is_empty() {
            bound.push(a.to_string());
        }
    } else {
        match names {
            Some(n) if n != "*" => bound.push(n.to_string()),
            // `from "str" import *` — bare module-name spec: the module
            // name is usable qualified (`str.trim`) via the host module
            // chain.
            Some("*") => {
                if let Some(p) = path {
                    if is_bare_module_name(p) {
                        bound.push(p.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    bound
}

// ---------------------------------------------------------------------------
// Expression checking
// ---------------------------------------------------------------------------

/// Collect the names bound by a pattern using `AstShapeDef::pattern_role`
/// metadata and declared child roles (ADR-0081). The conventional wildcard
/// `_` is skipped: it binds nothing and must not be reported as unbound.
fn pattern_bound_names(
    lang: &Language,
    ast: &Ast,
    node_ref: AstRef,
    out: &mut HashSet<String>,
) {
    let node = match ast.at(node_ref) {
        Ok(n) => n,
        Err(_) => return,
    };
    let role = lang
        .ast
        .get_by_identity(&node.kind)
        .and_then(|s| s.pattern_role.as_deref());

    match role {
        Some("leaf") => {
            if let Some(name) = node.payload.as_str() {
                if name != "_" {
                    out.insert(name.to_string());
                }
            }
        }
        Some("rest") => {
            if let Ok(Some(subject_idx)) = lang.child_role_index(&node.kind, "subject") {
                let children = children_of(node);
                if let Some(&subject_raw) = children.get(subject_idx) {
                    pattern_bound_names(lang, ast, AstRef::new(subject_raw), out);
                }
            }
        }
        Some("sequence") => {
            if let Ok(Some(elements_idx)) = lang.child_role_index(&node.kind, "elements") {
                for &child_raw in children_of(node).iter().skip(elements_idx) {
                    pattern_bound_names(lang, ast, AstRef::new(child_raw), out);
                }
            }
        }
        Some("entries") => {
            if let Ok(Some(entries_idx)) = lang.child_role_index(&node.kind, "entries") {
                for &child_raw in children_of(node).iter().skip(entries_idx) {
                    pattern_bound_names(lang, ast, AstRef::new(child_raw), out);
                }
            }
        }
        Some("entry") => {
            if let Ok(Some(value_idx)) = lang.child_role_index(&node.kind, "value") {
                let children = children_of(node);
                if let Some(&value_raw) = children.get(value_idx) {
                    pattern_bound_names(lang, ast, AstRef::new(value_raw), out);
                }
            }
        }
        _ => {
            // Wrapper fallbacks for role-less shapes that still appear in
            // patterns: a `paren`-wrapped pattern uses the `body` role, and
            // a `type-annot`-wrapped pattern uses the `subject` role.
            if let Ok(Some(body_idx)) = lang.child_role_index(&node.kind, "body") {
                for &child_raw in children_of(node).iter().skip(body_idx) {
                    pattern_bound_names(lang, ast, AstRef::new(child_raw), out);
                }
            } else if let Ok(Some(subject_idx)) =
                lang.child_role_index(&node.kind, "subject")
            {
                let children = children_of(node);
                if let Some(&subject_raw) = children.get(subject_idx) {
                    pattern_bound_names(lang, ast, AstRef::new(subject_raw), out);
                }
            }
        }
    }
}

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

    // Var reference — check scope. The `_` wildcard is a conventional
    // pattern binder that never refers to an in-scope name.
    if let Some(name) = var_name(node) {
        if name == "_" {
            return;
        }
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

    // Function (arrow lambda `x ~> body` or `[p, ...rest] ~> body`): add
    // the pattern-bound params to scope, then check the body.
    // Identified data-drivenly by the `params` + `body` roles (ADR-0081).
    let params_idx = lang.child_role_index(&node.kind, "params").ok().flatten();
    let body_idx = lang.child_role_index(&node.kind, "body").ok().flatten();
    if let (Some(pi), Some(bi)) = (params_idx, body_idx) {
        let children = children_of(node);
        let mut inner_scope = scope.clone();
        if let Some(&params_raw) = children.get(pi) {
            pattern_bound_names(lang, ast, AstRef::new(params_raw), &mut inner_scope);
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

    // Pattern arm (`pattern: body`): bind the pattern names, then check
    // the body. Identified data-drivenly by the `pattern` + `body` child
    // roles (ADR-0081) — covers `match` arms and any other arm-like form.
    let pattern_idx = lang.child_role_index(&node.kind, "pattern").ok().flatten();
    let body_idx = lang.child_role_index(&node.kind, "body").ok().flatten();
    if let (Some(pi), Some(bi)) = (pattern_idx, body_idx) {
        let children = children_of(node);
        let mut arm_scope = scope.clone();
        if let Some(&pattern_raw) = children.get(pi) {
            pattern_bound_names(lang, ast, AstRef::new(pattern_raw), &mut arm_scope);
        }
        if let Some(&body_raw) = children.get(bi) {
            check_expr(
                lang,
                src,
                ast,
                AstRef::new(body_raw),
                &arm_scope,
                stmt,
                diags,
                references,
            );
        }
        return;
    }

    // `data` declaration — the family name and constructor payloads live in
    // the type namespace; constructor names have already been collected into
    // the module scope above, so there are no wire references to check here.
    if node.kind == KIND_DATA {
        return;
    }

    // Type annotation (`subject : annot`/`subject :: annot`): the `subject` is
    // a pattern/field name and is never a wire reference; the `annot` is a
    // type (or, in record-value position, an expression). A bare `var` in the
    // `annot` position is treated as a type name unless it is a known
    // in-scope reference or a lowercase name (more likely an unbound wire).
    // Non-`var` annotations (e.g. `option.unwrap_or(...)`) are still checked.
    if let (Ok(Some(_subject_idx)), Ok(Some(annot_idx))) = (
        lang.child_role_index(&node.kind, "subject"),
        lang.child_role_index(&node.kind, "annot"),
    ) {
        let children = children_of(node);
        if let Some(&annot_raw) = children.get(annot_idx) {
            if let Ok(annot_node) = ast.at(AstRef::new(annot_raw)) {
                let is_bare_type = var_name(&annot_node).map_or(false, |n| {
                    !scope.contains(&n)
                        && n.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                });
                if !is_bare_type {
                    check_expr(
                        lang,
                        src,
                        ast,
                        AstRef::new(annot_raw),
                        scope,
                        stmt,
                        diags,
                        references,
                    );
                }
            }
        }
        // `subject` is intentionally not walked — it is a binder/label, not a
        // reference (ADR-0081).
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

    // Object entry (`key: value`): the `key` child is a field name (a `var`
    // that is NOT a reference). The `value` child is a type in record-type
    // literals and an expression in record values. A bare `var` value is
    // treated as a type name unless it is a known in-scope reference or a
    // lowercase name (more likely an unbound wire). Non-`var` values
    // (e.g. `option.unwrap_or(...)`) are still checked.
    if node.kind == KIND_ENTRY {
        if let Ok(Some(value_idx)) = lang.child_role_index(&node.kind, "value") {
            let children = children_of(node);
            if let Some(&value_raw) = children.get(value_idx) {
                if let Ok(value_node) = ast.at(AstRef::new(value_raw)) {
                    let is_bare_type = var_name(&value_node).map_or(false, |n| {
                        !scope.contains(&n)
                            && n.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                    });
                    if !is_bare_type {
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
                }
            }
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

    // Sequential-statement containers: a node declaring a variadic
    // `statements` role (`block` from `do { ... }`) binds names
    // left-to-right — an `assign` target or an import declaration's bound
    // names are in scope for all following siblings, mirroring
    // `eval_block`/`prim_import` runtime semantics (ADR-0096).
    if let Ok(Some(stmts_idx)) = lang.child_role_index(&node.kind, "statements") {
        let children = children_of(node);
        let mut block_scope = scope.clone();
        for &child_raw in children.iter().skip(stmts_idx) {
            let child_ref = AstRef::new(child_raw);
            let child = match ast.at(child_ref) {
                Ok(n) => n,
                Err(_) => continue,
            };
            // `stmt.import`: its bound names enter the sibling scope; the
            // statement itself carries no checkable expression (the child
            // is a specifier string or bound name, not a reference).
            if child.kind == KIND_IMPORT {
                for name in stmt_import_bound_names(ast, child) {
                    block_scope.insert(name);
                }
                continue;
            }
            // Binding (`assign`-like: `target` + `value` roles): bind the
            // target into the sibling scope *before* checking the value —
            // a block binding's name is visible to closures in its own
            // initializer (the runtime's update-or-define `ScopeStack`
            // resolves the wire at call time), so `rev :: xs ~> f(rev(x))`
            // is a legal self-recursive binding, not an unbound wire.
            let target_idx = lang.child_role_index(&child.kind, "target").ok().flatten();
            let value_idx = lang.child_role_index(&child.kind, "value").ok().flatten();
            if let (Some(ti), Some(vi)) = (target_idx, value_idx) {
                let grandchildren = children_of(child);
                if let Some(&target_raw) = grandchildren.get(ti) {
                    if let Some(name) = binding_target_name(lang, ast, AstRef::new(target_raw)) {
                        block_scope.insert(name);
                    }
                }
                if let Some(&value_raw) = grandchildren.get(vi) {
                    check_expr(
                        lang,
                        src,
                        ast,
                        AstRef::new(value_raw),
                        &block_scope,
                        stmt,
                        diags,
                        references,
                    );
                }
                continue;
            }
            check_expr(
                lang, src, ast, child_ref, &block_scope, stmt, diags, references,
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
///
/// `prelude` carries host-supplied wire names — the names the eval-time
/// module chain / resolver would auto-wire for `.hnl` sources (module
/// names, flat host-function names, ambient globals). The checker itself
/// does not know any host names; the host (e.g. `hnl typecheck` in
/// `host-cli`) supplies the same set its runtime registers, per ADR-0096's
/// unified import model.
fn check_pass(
    lang: &Language,
    src: &str,
    stmts: &[Stmt],
    prelude: &HashSet<String>,
    diags: &mut Vec<Diagnostic>,
) {
    // Phase 1: collect bindings, imports, exports, and data constructors
    // (+ duplicate detection).
    let (bindings, imports, exports, data_ctors) = collect_decls(lang, stmts, diags, src);

    // Build the module-level scope: all binding names + all import names +
    // all `data` constructor names + the host-supplied prelude wires.
    let binding_names: HashSet<String> = bindings.iter().map(|b| b.name.clone()).collect();
    let import_names: HashSet<String> = imports.iter().map(|i| i.name.clone()).collect();
    let module_scope: HashSet<String> = binding_names
        .iter()
        .cloned()
        .chain(import_names.iter().cloned())
        .chain(data_ctors.iter().cloned())
        .chain(prelude.iter().cloned())
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
        // `export x :: expr` wraps the binding in `stmt.export(assign(x,
        // expr))` — descend to the wrapped binding child (role-less, so
        // positional child 0 per ADR-0081's fallback).
        let root = if root.kind == KIND_EXPORT {
            match children_of(root).first() {
                Some(&raw) => match stmt.ast.at(AstRef::new(raw)) {
                    Ok(n) => n,
                    Err(_) => continue,
                },
                None => continue,
            }
        } else {
            root
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

        // Chained binds (`a :: f = g` → `assign(assign(a, f), g)`): the
        // inner `value` children carry checkable expressions too — walk
        // each binding-shaped target's `value` subtree.
        let target_idx = lang
            .child_role_index(&root.kind, "target")
            .ok()
            .flatten()
            .unwrap_or(0);
        let mut cursor = children.get(target_idx).copied();
        while let Some(raw) = cursor {
            let target_node = match stmt.ast.at(AstRef::new(raw)) {
                Ok(n) => n,
                Err(_) => break,
            };
            let ti = lang
                .child_role_index(&target_node.kind, "target")
                .ok()
                .flatten();
            let vi = lang
                .child_role_index(&target_node.kind, "value")
                .ok()
                .flatten();
            match (ti, vi) {
                (Some(t2), Some(v2)) => {
                    let inner = children_of(target_node);
                    if let Some(&v_raw) = inner.get(v2) {
                        check_expr(
                            lang,
                            src,
                            &stmt.ast,
                            AstRef::new(v_raw),
                            &module_scope,
                            stmt,
                            diags,
                            &mut references,
                        );
                    }
                    cursor = inner.get(t2).copied();
                }
                _ => break,
            }
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
    check_module_with_prelude(src, &HashSet::new())
}

/// Check a handl module source text with a host-supplied wire prelude
/// (ADR-0088, ADR-0096).
///
/// Identical to [`check_module`], but `prelude` names are seeded into the
/// module scope before unbound-wire checking. The prelude is how a host
/// tells the checker which names its runtime would auto-wire — the eval
/// engines resolve module-chain names (`str`, `json`, `assert`, …), flat
/// host-function names (`map`, `pipe`, `greet`, …), and ambient globals
/// (`self`, `hello`) that never appear as source-level bindings. The set
/// is data from the host's resolver/module registry — this crate carries
/// no hardcoded host names (dep cycle with `handl-kit-canonical`,
/// AGENTS.md "Cyclic dependency workaround").
///
/// Prelude names are inert for the other module-level checks: they are
/// scope-only, so they cannot satisfy `unbound-export` (an export must
/// still be bound by a source-level binding) and are never reported as
/// `unused-import`.
///
/// # Errors
///
/// Returns [`CheckError::Parse`] if any statement cannot be parsed.
pub fn check_module_with_prelude(
    src: &str,
    prelude: &HashSet<String>,
) -> Result<TypecheckReport, CheckError> {
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
    check_pass(lang, src, &stmts, prelude, &mut diags);

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
