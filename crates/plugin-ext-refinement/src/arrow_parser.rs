//! A minimal arrow-graph parser for `.hnl` arrow files (ADR-0069).
//!
//! This parser reads the arrow-graph syntax used in `handl-packages/`
//! and produces `Term` IR. It uses the **data-driven** lexer + Pratt parser
//! (driven by the YAML language definition in `data/`) to build an AST, then
//! materializes the AST into a `Term` via [`ast_to_scheme`].
//!
//! # Two syntaxes
//!
//! 1. **Simple arrow-graph syntax** — the combinator expressions described in
//!    ADR-0069. Supports both function-call syntax and infix operator syntax:
//!    ```text
//!    loop(comp(insert, first(id)))       // function-call syntax
//!    loop(insert >>> first(id))          // equivalent operator syntax
//!    arr("sort")
//!    comp(a, b)                          // same as: a >>> b
//!    choice(a, b)                        // same as: a ||| b
//!    ```
//!    The infix operators `>>>` (sequence) and `|||` (choice) are sugar:
//!    `a >>> b` desugars to `comp(a, b)` (`Term::Seq`), and `a ||| b`
//!    desugars to `choice(a, b)` (`Term::Choice`). `|||` binds tighter
//!    than `>>>`, so `a >>> b ||| c` parses as `comp(a, choice(b, c))`.
//!
//! 2. **Rich `loop { ... } ( params ~> body )` syntax** — the surface syntax
//!    actually used in the stdlib `.hnl` files. The YAML grammar tokenizes
//!    and parses this syntax into a `loop_rich` AST node. The
//!    `ast_to_scheme` materializer then pattern-matches on the arrow body's
//!    recursive match arm to determine the corresponding `Term` graph —
//!    the same heuristic as the legacy ad-hoc parser, but operating on the
//!    AST instead of raw text.
//!
//! # Fallback strategy
//!
//! The data-driven parser is tried first. If it fails (e.g. for unsupported
//! constructs), the legacy ad-hoc recursive-descent parser is used as a
//! fallback. This ensures existing `.hnl` files continue to parse correctly.
//! The simple `loop(body)` form now falls through to the legacy parser
//! because the `loop` prefix grammar row was changed to handle the rich
//! `loop { ... } ( ... )` form (arity 2 instead of 1).

use std::collections::{HashMap, HashSet};

use handl_language::{Ast, AstNode, Language, Term, Value};
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use plugin_engine::{ArrSource, RewriteWitness, TermData};

use crate::arrow_lang::arrow_language;

// ============================================================================
// Public API
// ============================================================================

/// The result of parsing an arrow file.
///
/// A single-expression file is [`ParsedFile::Single`] — the file IS the arrow.
/// A multi-name file is [`ParsedFile::Named`] — it produces a record of named
/// arrows (per ADR-0069 §1).
#[derive(Debug, Clone)]
pub enum ParsedFile {
    /// A single-expression file — the file IS the arrow.
    Single(Term),
    /// A multi-name file — produces a record of named arrows.
    Named(HashMap<String, Term>),
}

/// Parse a `.hnl` arrow file's content.
///
/// Tries the data-driven parser first (lex + Pratt parse + materialize),
/// falling back to the legacy ad-hoc parser on failure. Rich-loop syntax
/// (`loop { ... } ( params ~> body )`) is handled by the data-driven parser:
/// the YAML grammar tokenizes and parses it into a `loop_rich` AST node,
/// which `ast_to_scheme` pattern-matches to determine the corresponding
/// `Term` graph.
pub fn parse_arrow_file(src: &str) -> Result<ParsedFile, String> {
    let stripped = strip_comments(src);
    let trimmed = stripped.trim();

    if trimmed.is_empty() {
        return Err("empty file (after stripping comments)".to_string());
    }

    // Try the data-driven parser first.
    if let Ok(result) = parse_arrow_file_data_driven(&stripped) {
        return Ok(result);
    }

    // Fall back to the legacy ad-hoc parser.
    parse_arrow_file_legacy(&stripped)
}

/// Parse a simple arrow expression (the combinator syntax).
///
/// Tries the data-driven parser first, falling back to the legacy ad-hoc
/// parser on failure.
///
/// Handles: `arr("name")`, `comp(a, b)`, `first(a)`, `choice(a, b)`,
/// `loop(a)`, `id`, `app("name", args)`, and bare identifiers (references
/// to named arrows). Also supports infix operator syntax: `a >>> b`
/// (sequence, same as `comp(a, b)`) and `a ||| b` (choice, same as
/// `choice(a, b)`).
pub fn parse_arrow_expr(src: &str) -> Result<Term, String> {
    // Check for simple rec form: `rec { body }` (without `( ... )` after `}`).
    // This is the v0.1 replacement for `loop(body)`.
    if let Some(scheme) = try_parse_simple_rec(src) {
        return Ok(scheme);
    }
    let lang = arrow_language();
    if let Ok(scheme) = parse_arrow_expr_data_driven(src, lang) {
        return Ok(scheme);
    }
    parse_arrow_expr_legacy(src)
}

// ============================================================================
// Module parsing: import/export declarations (ADR-0069)
// ============================================================================

/// A parsed import declaration.
///
/// Represents a single `import` line in a `.hnl` module file. Each import
/// binds one or more names from a remote module path into the local scope.
/// The `names` vector contains `(remote_name, local_name)` pairs; `local`
/// equals `remote` when no `as` alias is given. For whole-module imports
/// (`import "path" as alias`), the remote name is `"*"`.
#[derive(Debug, Clone)]
pub struct ImportDecl {
    /// `(remote_name, local_name)` pairs. `local == remote` when no `as`.
    /// For whole-module imports, `remote_name` is `"*"`.
    pub names: Vec<(String, String)>,
    /// The module path being imported from.
    pub path: String,
}

/// A parsed export declaration.
///
/// Controls which names a module makes public. An empty export list means
/// all named arrows are public (ADR-0069 §1).
#[derive(Debug, Clone)]
pub enum ExportDecl {
    /// Export specific names: `export name1, name2`.
    Names(Vec<String>),
    /// Export all public names: `export *`.
    All,
    /// Export specific names from another module: `export name1 from "path"`.
    NamesFrom(Vec<String>, String),
    /// Export all from another module: `export * from "path"`.
    AllFrom(String),
}

/// A parsed `op` declaration (ADR-0077 Phase 2).
///
/// Represents a single `op` line in a `.hnl` module file. An `op` declares a
/// native operation (Rust-backed function) with its type signature and
/// optional axiom predicates (`guarantee`, `requires`, `termination`).
///
/// # Syntax
///
/// ```handl
/// op "base64.encode" : String -> String
///   guarantee: IsBase64(output)
/// ```
///
/// The `op` line may be followed by indented `guarantee:`/`requires:`/
/// `termination:` lines, which are parsed as part of the op declaration.
#[derive(Debug, Clone)]
pub struct OpDecl {
    /// The op name (e.g. `"base64.encode"`).
    pub name: String,
    /// The input type signature text (e.g. `"String"`).
    pub input_type: String,
    /// The output type signature text (e.g. `"String"`).
    pub output_type: String,
    /// Optional `guarantee` axiom predicate text.
    pub guarantee: Option<String>,
    /// Optional `requires` precondition predicate text.
    pub requires: Option<String>,
    /// Optional `termination` predicate text.
    pub termination: Option<String>,
}

/// A parsed `handl` declaration (ADR-0077 Phase 2).
///
/// Binds a Rust function (the handler) to an `op` name. The Rust function
/// identifier is symbolic — the host crate registers a `Function` impl under
/// the op name via an [`OpRegistry`](handl_plugin_lib_handl::OpRegistry).
///
/// # Syntax
///
/// ```handl
/// handl "base64.encode" = rust_base64_encode
/// ```
#[derive(Debug, Clone)]
pub struct HandlDecl {
    /// The op name (e.g. `"base64.encode"`).
    pub op_name: String,
    /// The Rust function symbolic name (e.g. `"rust_base64_encode"`).
    pub rust_fn: String,
}

/// A parsed `cap` declaration (ADR-0077 Phase 5).
///
/// Declares a capability that the module requires. The capability name
/// matches an `op` name — the host must grant the capability for the op's
/// handl to be callable.
///
/// # Syntax
///
/// ```handl
/// cap "fs.read"
/// ```
#[derive(Debug, Clone)]
pub struct CapDecl {
    /// The capability name (e.g. `"fs.read"`).
    pub name: String,
}

/// The result of parsing an arrow module file with import/export declarations.
///
/// A `ParsedModule` carries the import/export metadata alongside the parsed
/// arrow definitions. The `arrows` field is a [`ParsedFile`] — either a
/// single expression or a named map of arrows.
#[derive(Debug, Clone)]
pub struct ParsedModule {
    /// Import declarations: `(local_name, module_path)` pairs.
    pub imports: Vec<ImportDecl>,
    /// Export declarations. Empty = all names public.
    pub exports: Vec<ExportDecl>,
    /// `op` declarations (ADR-0077 Phase 2).
    pub ops: Vec<OpDecl>,
    /// `handl` declarations (ADR-0077 Phase 2).
    pub handls: Vec<HandlDecl>,
    /// `cap` declarations (ADR-0077 Phase 5).
    pub caps: Vec<CapDecl>,
    /// `tag` declarations (ADR-0077). Each tag generates a constructor
    /// arrow that wraps its input in a tagged value.
    pub tags: Vec<TagDecl>,
    /// The parsed arrows (single expression or named map).
    pub arrows: ParsedFile,
}

/// A `tag name = Type` declaration (ADR-0077).
///
/// At module load time, each tag declaration generates a constructor
/// arrow: `name = _0 ~> tag.tag("name", {value: _0})`. The payload type
/// is recorded for future typechecking but does not affect runtime
/// behavior.
#[derive(Debug, Clone)]
pub struct TagDecl {
    /// The tag name, e.g. `"ok"` in `tag ok = Value`.
    pub name: String,
    /// The payload type expression, e.g. `"Value"` in `tag ok = Value`.
    pub payload_type: String,
}

/// Parse a `.hnl` arrow module file's content, extracting import/export
/// and `op`/`handl`/`cap` declarations (ADR-0069, ADR-0077 Phase 2).
///
/// This is the module-aware counterpart to [`parse_arrow_file`]. It:
/// 1. Strips comments (reuses [`strip_comments`]).
/// 2. Scans for `import`, `export`, `op`, `handl`, and `cap` lines at the
///    top level.
/// 3. Parses import lines: `import name from "path"`,
///    `import name1, name2 from "path"`, `import name as local from "path"`,
///    `import "path" as alias`.
/// 4. Parses export lines: `export name`, `export *`,
///    `export name from "path"`, `export * from "path"`.
/// 5. Parses `op` lines: `op "name" : Type -> Type`, optionally followed by
///    indented `guarantee:`/`requires:`/`termination:` lines.
/// 6. Parses `handl` lines: `handl "op.name" = rust_fn_name`.
/// 7. Parses `cap` lines: `cap "cap.name"`.
/// 8. Parses the remaining content as a [`ParsedFile`] (reuses
///    [`parse_arrow_file`] on the non-declaration lines).
///
/// If the remaining content (after removing declaration lines) is empty,
/// the `arrows` field is `ParsedFile::Named` with an empty map (a pure
/// re-export or op-only module).
pub fn parse_arrow_module(src: &str) -> Result<ParsedModule, String> {
    let mut parsed = parse_arrow_module_inner(src)?;

    // Rewrite references to imported names into module_ref/project nodes so
    // the normalisation strategies (Strategy A / Strategy B) can eliminate
    // them. Local bindings shadow imports of the same name.
    rewrite_imports(&mut parsed.arrows, &parsed.imports);

    Ok(parsed)
}

/// Parse a `.hnl` arrow module file's content, extracting import/export
/// and `op`/`handl`/`cap` declarations, but **without** rewriting import
/// references.
///
/// This is the same as [`parse_arrow_module`] except that bare name
/// references to imported names are preserved (not rewritten to
/// `module_ref`/`project` nodes). This is used by `load_all_arrows` and
/// `load_arrow_file` to handle files with import/export declarations while
/// preserving the bare-reference form that `resolve_apps` expects.
pub fn parse_arrow_module_no_rewrite(src: &str) -> Result<ParsedModule, String> {
    parse_arrow_module_inner(src)
}

/// Inner implementation: scan declaration lines, parse the remaining content
/// into a [`ParsedFile`], and return a [`ParsedModule`] without import
/// rewriting. Both [`parse_arrow_module`] (with rewriting) and
/// [`parse_arrow_module_no_rewrite`] (without) delegate to this function.
fn parse_arrow_module_inner(src: &str) -> Result<ParsedModule, String> {
    let stripped = strip_comments(src);
    let mut imports: Vec<ImportDecl> = Vec::new();
    let mut exports: Vec<ExportDecl> = Vec::new();
    let mut ops: Vec<OpDecl> = Vec::new();
    let mut handls: Vec<HandlDecl> = Vec::new();
    let mut caps: Vec<CapDecl> = Vec::new();
    let mut tags: Vec<TagDecl> = Vec::new();
    let mut arrow_lines: Vec<&str> = Vec::new();

    // Track the current `op` declaration being built (for indented
    // guarantee/requires/termination lines).
    let mut current_op: Option<OpDecl> = None;

    for line in stripped.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // Flush any pending op before the blank line.
            if let Some(op) = current_op.take() {
                ops.push(op);
            }
            arrow_lines.push(line);
            continue;
        }

        // Check for indented continuation lines (guarantee/requires/
        // termination) belonging to the current `op` declaration.
        let is_indented = line.starts_with(' ') || line.starts_with('\t');
        if is_indented {
            if let Some(ref mut op) = current_op {
                if let Some((key, value)) = parse_op_clause(trimmed) {
                    match key {
                        "guarantee" => op.guarantee = Some(value),
                        "requires" => op.requires = Some(value),
                        "termination" => op.termination = Some(value),
                        _ => {}
                    }
                    continue;
                }
            }
        }

        // Flush any pending op before processing a new top-level line.
        if let Some(op) = current_op.take() {
            ops.push(op);
        }

        if let Some(rest) = trimmed.strip_prefix("import") {
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                if let Some(decl) = parse_import_line(trimmed) {
                    imports.push(decl);
                    continue;
                }
            }
        }

        if let Some(rest) = trimmed.strip_prefix("export") {
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                if let Some(decl) = parse_export_line(trimmed) {
                    exports.push(decl);
                    continue;
                }
            }
        }

        if let Some(rest) = trimmed.strip_prefix("op") {
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                if let Some(decl) = parse_op_line(trimmed) {
                    // Start a new op declaration — indented lines may follow.
                    current_op = Some(decl);
                    continue;
                }
            }
        }

        if let Some(rest) = trimmed.strip_prefix("handl") {
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                if let Some(decl) = parse_handl_line(trimmed) {
                    handls.push(decl);
                    continue;
                }
            }
        }

        if let Some(rest) = trimmed.strip_prefix("cap") {
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                if let Some(decl) = parse_cap_line(trimmed) {
                    caps.push(decl);
                    continue;
                }
            }
        }

        if let Some(rest) = trimmed.strip_prefix("tag") {
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                if let Some(decl) = parse_tag_line(trimmed) {
                    tags.push(decl);
                    continue;
                }
            }
        }

        arrow_lines.push(line);
    }

    // Flush any pending op at end of file.
    if let Some(op) = current_op.take() {
        ops.push(op);
    }

    // Generate constructor arrows for each tag declaration.
    // Each `tag name = Type` produces: `name :: _0 ~> tag.tag("name", {value: _0})`
    let tag_constructor_src: String = tags
        .iter()
        .map(|t| format!("{} :: _0 ~> tag.tag(\"{}\", {{value: _0}})", t.name, t.name))
        .collect::<Vec<_>>()
        .join("\n");

    let arrow_src = arrow_lines.join("\n");
    let arrows = if arrow_src.trim().is_empty() && tag_constructor_src.is_empty() {
        ParsedFile::Named(HashMap::new())
    } else if arrow_src.trim().is_empty() {
        // Only tag constructors, no other arrows.
        parse_arrow_file(&tag_constructor_src)?
    } else if !tag_constructor_src.is_empty() {
        // Both tag constructors and regular arrows — parse separately
        // and merge into a single Named map. This avoids issues with
        // concatenating `name = expr` lines with a `{ ... }` record
        // literal, which parse_arrow_file cannot handle as a single
        // source.
        let tag_file = parse_arrow_file(&tag_constructor_src)?;
        let arrow_file = parse_arrow_file(&arrow_src)?;
        match (tag_file, arrow_file) {
            (ParsedFile::Named(mut tag_map), ParsedFile::Named(arrow_map)) => {
                tag_map.extend(arrow_map);
                ParsedFile::Named(tag_map)
            }
            (ParsedFile::Named(_tag_map), ParsedFile::Single(_expr)) => {
                // The arrow source is a single expression (e.g. a record
                // literal). Merge into the tag map under a special key
                // is not appropriate — instead, wrap both in a record.
                // For now, if the single expression is a record, its
                // fields should be merged. But since we can't easily
                // destructure a Term here, we fall back to concatenation
                // only when the arrow source is a single expression.
                // This case is uncommon (tag decls + single expr).
                // Re-parse with concatenation as a fallback.
                let combined = format!("{}\n{}", tag_constructor_src, arrow_src);
                parse_arrow_file(&combined)?
            }
            (ParsedFile::Single(_), _) => {
                // Tag constructors didn't parse as Named — shouldn't happen.
                let combined = format!("{}\n{}", tag_constructor_src, arrow_src);
                parse_arrow_file(&combined)?
            }
        }
    } else {
        parse_arrow_file(&arrow_src)?
    };

    Ok(ParsedModule {
        imports,
        exports,
        ops,
        handls,
        caps,
        tags,
        arrows,
    })
}

/// Rewrite references to imported names in a [`ParsedFile`] so they become
/// proper `module_ref`/`project` nodes.
///
/// After [`parse_arrow_module`] extracts the import declarations and parses
/// the remaining content into a [`ParsedFile`], imported names still appear
/// as bare `Arr { op: <local_name>, args: [], source: Native, lit: None }`
/// references — indistinguishable from local arrow references. This function
/// walks every `Term` in the `ParsedFile` and replaces such references with
/// `project(module_ref(path), remote_name)` for named imports, or
/// `module_ref(path)` for whole-module imports (`import "path" as alias`).
///
/// References to names that are **locally defined** in the `ParsedFile` are
/// not rewritten — a local binding shadows an import of the same name.
pub fn rewrite_imports(file: &mut ParsedFile, imports: &[ImportDecl]) {
    // Build the set of locally-defined names. A single-expression file has
    // no local names; a named file's keys are the local bindings.
    let local_names: HashSet<String> = match file {
        ParsedFile::Named(map) => map.keys().cloned().collect(),
        ParsedFile::Single(_) => HashSet::new(),
    };

    // Build the import map: local_name -> (path, remote_name).
    // For whole-module imports, remote_name is "*".
    let mut import_map: HashMap<String, (String, String)> = HashMap::new();
    for import in imports {
        for (remote_name, local_name) in &import.names {
            import_map.insert(
                local_name.clone(),
                (import.path.clone(), remote_name.clone()),
            );
        }
    }

    if import_map.is_empty() {
        return;
    }

    match file {
        ParsedFile::Single(scheme) => {
            *scheme = rewrite_scheme(scheme, &import_map, &local_names);
        }
        ParsedFile::Named(map) => {
            for scheme in map.values_mut() {
                *scheme = rewrite_scheme(scheme, &import_map, &local_names);
            }
        }
    }
}

/// Recursively rewrite a [`Term`], replacing bare references to imported
/// names with `module_ref`/`project` nodes.
///
/// A "reference" is either:
/// - A standalone `Arr { op: <name>, args: [], source: Native, lit: None }`
///   (e.g. passing an imported arrow as a higher-order argument).
/// - The callee of an `App { callee: Arr { op: <name>, ... }, args }` — the
///   callee is rewritten recursively, so the bare-`Arr` case covers this.
///
/// Only names in `import_map` that are **not** in `local_names` are rewritten
/// (local bindings shadow imports).
fn rewrite_scheme(
    scheme: &Term,
    import_map: &HashMap<String, (String, String)>,
    local_names: &HashSet<String>,
) -> Term {
    let td = TermData::from_term(scheme);
    rewrite_scheme_td(&td, import_map, local_names)
        .to_term()
        .expect("rewrite_scheme: TermData -> Term conversion failed")
}

/// Data-driven counterpart of [`rewrite_scheme`], operating on [`TermData`]
/// instead of the `Term` enum. Dispatches on `td.tag()` and accesses fields
/// via `get_term` / `get_str` / `arr_source`, constructing results with
/// `TermData` constructors.
fn rewrite_scheme_td(
    td: &TermData,
    import_map: &HashMap<String, (String, String)>,
    local_names: &HashSet<String>,
) -> TermData {
    match td.tag() {
        "Op" => {
            // Recurse into args first (bottom-up).
            let new_args: Vec<TermData> = td
                .get_term_list("args")
                .iter()
                .map(|a| rewrite_scheme_td(a, import_map, local_names))
                .collect();

            // Check if this is a bare reference to an imported name.
            let is_bare_ref = new_args.is_empty()
                && td.get("lit").map(|v| v.is_null()).unwrap_or(true)
                && td.arr_source() == Some(ArrSource::Native);
            if is_bare_ref {
                if let Some(op) = td.get_str("name") {
                    if let Some((path, remote_name)) = import_map.get(op) {
                        if !local_names.contains(op) {
                            if remote_name == "*" {
                                // Whole-module import: the whole module record.
                                return module_ref_node_td(path);
                            } else {
                                // Named import: project the field from the module.
                                return project_node_td(module_ref_node_td(path), remote_name);
                            }
                        }
                    }
                }
            }

            // Not a bare reference — rebuild the Op with rewritten args.
            let source = td.arr_source().unwrap_or(ArrSource::Native);
            let lit = td.get("lit").filter(|v| !v.is_null()).cloned();
            let name = td.get_str("name").unwrap_or("").to_string();
            TermData::op(name, new_args, &source, lit)
        }
        "Seq" => {
            let fst = td.get_term("fst").expect("Seq: missing fst");
            let snd = td.get_term("snd").expect("Seq: missing snd");
            TermData::seq(
                rewrite_scheme_td(&fst, import_map, local_names),
                rewrite_scheme_td(&snd, import_map, local_names),
            )
        }
        "First" => {
            let inner = td.get_term("inner").expect("First: missing inner");
            TermData::first(rewrite_scheme_td(&inner, import_map, local_names))
        }
        "Choice" => {
            let l = td.get_term("l").expect("Choice: missing l");
            let r = td.get_term("r").expect("Choice: missing r");
            TermData::choice(
                rewrite_scheme_td(&l, import_map, local_names),
                rewrite_scheme_td(&r, import_map, local_names),
            )
        }
        "Loop" => {
            let body = td.get_term("body").expect("Loop: missing body");
            TermData::loop_feedback(rewrite_scheme_td(&body, import_map, local_names))
        }
        "App" => {
            let callee = td.get_term("callee").expect("App: missing callee");
            let args: Vec<TermData> = td
                .get_term_list("args")
                .iter()
                .map(|a| rewrite_scheme_td(a, import_map, local_names))
                .collect();
            TermData::app(rewrite_scheme_td(&callee, import_map, local_names), args)
        }
        // `handle body with target = impl_` (ADR-0076): eliminate the handler
        // at compile time, then rewrite the resulting handler-free graph.
        "Handle" => {
            let eliminated = td
                .to_term()
                .expect("Handle: TermData -> Term conversion failed")
                .eliminate_handlers();
            let eliminated_td = TermData::from_term(&eliminated);
            rewrite_scheme_td(&eliminated_td, import_map, local_names)
        }
        "Arr" => {
            let value = td.get("value").cloned().unwrap_or(Value::Null);
            TermData::arr(value)
        }
        "Child" => {
            let role = td.get_str("role").unwrap_or("").to_string();
            TermData::child(role)
        }
        "Iter" => {
            let fuel = td.get_term("fuel").expect("Iter: missing fuel");
            let predicate = td.get_term("predicate").expect("Iter: missing predicate");
            let step = td.get_term("step").expect("Iter: missing step");
            let initial = td.get_term("initial").expect("Iter: missing initial");
            TermData::iter(
                rewrite_scheme_td(&fuel, import_map, local_names),
                rewrite_scheme_td(&predicate, import_map, local_names),
                rewrite_scheme_td(&step, import_map, local_names),
                rewrite_scheme_td(&initial, import_map, local_names),
            )
        }
        "Rewrite" => {
            let source = td.get_term("source").expect("Rewrite: missing source");
            let target = td.get_term("target").expect("Rewrite: missing target");
            let witness = parse_rewrite_witness(td);
            TermData::rewrite(
                rewrite_scheme_td(&source, import_map, local_names),
                rewrite_scheme_td(&target, import_map, local_names),
                &witness,
            )
        }
        // Unknown/unsupported tags: preserve as-is.
        _ => td.clone(),
    }
}

/// Parse the `witness` field of a `Rewrite` [`TermData`] into a
/// [`RewriteWitness`]. Falls back to `Compute` for missing or unrecognised
/// witnesses so that data-driven reconstruction never panics.
fn parse_rewrite_witness(td: &TermData) -> RewriteWitness {
    match td.get("witness") {
        Some(w) => {
            let tag = w.get("__tag__").and_then(|v| v.as_str()).unwrap_or("");
            match tag {
                "Named" => w
                    .get("value")
                    .and_then(|v| v.as_str())
                    .map(|s| RewriteWitness::Named(s.to_string()))
                    .unwrap_or(RewriteWitness::Compute),
                "Compute" => RewriteWitness::Compute,
                "HandlerHint" => RewriteWitness::HandlerHint,
                "Cast" => RewriteWitness::Cast,
                _ => RewriteWitness::Compute,
            }
        }
        None => RewriteWitness::Compute,
    }
}

/// Build a `module_ref` node: `Op { name: "module_ref", args: [], source:
/// Native, lit: Some(path) }`.
fn module_ref_node_td(path: &str) -> TermData {
    TermData::op(
        "module_ref".to_string(),
        vec![],
        &ArrSource::Native,
        Some(Value::String(path.to_string())),
    )
}

/// Build a `project` node: `Op { name: "project", args: [record], source:
/// Native, lit: Some(field) }`.
fn project_node_td(record: TermData, field: &str) -> TermData {
    TermData::op(
        "project".to_string(),
        vec![record],
        &ArrSource::Native,
        Some(Value::String(field.to_string())),
    )
}

/// Parse a single `import` line into an [`ImportDecl`].
///
/// Supported forms:
/// - `import name from "path"`
/// - `import name1, name2 from "path"`
/// - `import name as local from "path"`
/// - `import "path" as alias`
fn parse_import_line(line: &str) -> Option<ImportDecl> {
    let rest = line.strip_prefix("import")?.trim();

    // Case: import "path" as alias  (whole-module import)
    if rest.starts_with('"') || rest.starts_with('\'') {
        let (path, after) = extract_quoted_string(rest)?;
        let after = after.trim();
        if let Some(alias_part) = after.strip_prefix("as") {
            let alias = alias_part.trim().to_string();
            if alias.is_empty() {
                return None;
            }
            return Some(ImportDecl {
                names: vec![("*".to_string(), alias)],
                path,
            });
        }
        // Bare `import "path"` with no alias — import the whole module
        // under the file stem name.
        let stem = path.rsplit('/').next().unwrap_or(&path).to_string();
        return Some(ImportDecl {
            names: vec![("*".to_string(), stem)],
            path,
        });
    }

    // Case: import name1, name2 from "path"
    // Case: import name as local from "path"
    let from_idx = find_keyword(rest, "from")?;
    let names_part = rest[..from_idx].trim();
    let path_part = rest[from_idx + 4..].trim();
    let path = extract_quoted_string(path_part)?.0;

    let names: Vec<(String, String)> = names_part
        .split(',')
        .filter_map(|n| {
            let n = n.trim();
            if n.is_empty() {
                return None;
            }
            if let Some(as_idx) = find_keyword(n, "as") {
                let remote = n[..as_idx].trim().to_string();
                let local = n[as_idx + 3..].trim().to_string();
                if remote.is_empty() || local.is_empty() {
                    return None;
                }
                Some((remote, local))
            } else {
                Some((n.to_string(), n.to_string()))
            }
        })
        .collect();

    if names.is_empty() {
        return None;
    }

    Some(ImportDecl { names, path })
}

/// Parse a single `export` line into an [`ExportDecl`].
///
/// Supported forms:
/// - `export name1, name2`
/// - `export *`
/// - `export name1 from "path"`
/// - `export * from "path"`
fn parse_export_line(line: &str) -> Option<ExportDecl> {
    let rest = line.strip_prefix("export")?.trim();

    // export * from "path"
    if rest.starts_with('*') {
        let after_star = rest[1..].trim();
        if let Some(from_idx) = find_keyword(after_star, "from") {
            let path_part = after_star[from_idx + 4..].trim();
            let path = extract_quoted_string(path_part)?.0;
            return Some(ExportDecl::AllFrom(path));
        }
        return Some(ExportDecl::All);
    }

    // export name1, name2 from "path"
    // export name1, name2
    if let Some(from_idx) = find_keyword(rest, "from") {
        let names_part = rest[..from_idx].trim();
        let path_part = rest[from_idx + 4..].trim();
        let path = extract_quoted_string(path_part)?.0;
        let names: Vec<String> = names_part
            .split(',')
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect();
        if names.is_empty() {
            return None;
        }
        return Some(ExportDecl::NamesFrom(names, path));
    }

    // export name1, name2
    let names: Vec<String> = rest
        .split(',')
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect();
    if names.is_empty() {
        return None;
    }
    Some(ExportDecl::Names(names))
}

/// Parse a single `op` line into an [`OpDecl`].
///
/// Supported form:
/// - `op "name" : InputType -> OutputType`
///
/// The op name must be a quoted string. The type signature uses `->` to
/// separate input and output types. Indented `guarantee:`/`requires:`/
/// `termination:` lines are handled separately by [`parse_op_clause`] in
/// the main parsing loop.
fn parse_op_line(line: &str) -> Option<OpDecl> {
    let rest = line.strip_prefix("op")?.trim();

    // Extract the quoted op name.
    let (name, after_name) = extract_quoted_string(rest)?;
    let after_name = after_name.trim();

    // Expect `:` after the name, then `InputType -> OutputType`.
    let after_colon = after_name.strip_prefix(':')?.trim();

    // Find the `->` separator. We search for `->` as a standalone token.
    let arrow_idx = find_arrow(after_colon)?;
    let input_type = after_colon[..arrow_idx].trim().to_string();
    let output_type = after_colon[arrow_idx + 2..].trim().to_string();

    if input_type.is_empty() || output_type.is_empty() {
        return None;
    }

    Some(OpDecl {
        name,
        input_type,
        output_type,
        guarantee: None,
        requires: None,
        termination: None,
    })
}

/// Parse an indented `op` clause line (`guarantee:`/`requires:`/
/// `termination:`).
///
/// Returns `Some((key, value))` if the line matches `key: value` where `key`
/// is one of `guarantee`, `requires`, `termination`. Returns `None` otherwise
/// (the line is not an op clause and should be treated as a regular line).
fn parse_op_clause(line: &str) -> Option<(&'static str, String)> {
    for key in &["guarantee", "requires", "termination"] {
        if let Some(rest) = line.strip_prefix(key) {
            // Ensure the keyword is word-complete.
            let after = rest.chars().next();
            if after == Some(':') || after.is_none() {
                let value = rest.strip_prefix(':').unwrap_or(rest).trim();
                if value.is_empty() {
                    return Some((*key, String::new()));
                }
                return Some((*key, value.to_string()));
            }
        }
    }
    None
}

/// Find the byte index of the `->` arrow separator in a type signature,
/// ensuring it is not part of a larger token. Returns `None` if not found.
fn find_arrow(s: &str) -> Option<usize> {
    s.find("->")
}

/// Parse a single `handl` line into a [`HandlDecl`].
///
/// Supported form:
/// - `handl "op.name" = rust_fn_name`
///
/// The op name must be a quoted string. The Rust function name is a bare
/// identifier after the `=` sign.
fn parse_handl_line(line: &str) -> Option<HandlDecl> {
    let rest = line.strip_prefix("handl")?.trim();

    // Extract the quoted op name.
    let (op_name, after_name) = extract_quoted_string(rest)?;
    let after_name = after_name.trim();

    // Expect `= rust_fn_name`.
    let after_eq = after_name.strip_prefix('=')?.trim();
    if after_eq.is_empty() {
        return None;
    }

    // The Rust function name is a bare identifier (may contain underscores
    // and alphanumeric chars).
    let rust_fn = after_eq.to_string();

    Some(HandlDecl { op_name, rust_fn })
}

/// Parse a single `cap` line into a [`CapDecl`].
///
/// Supported form:
/// - `cap "cap.name"`
///
/// The capability name must be a quoted string.
fn parse_cap_line(line: &str) -> Option<CapDecl> {
    let rest = line.strip_prefix("cap")?.trim();

    // Extract the quoted capability name.
    let (name, after_name) = extract_quoted_string(rest)?;
    let after_name = after_name.trim();

    // Nothing should follow the quoted name (except possibly a comment,
    // but comments are already stripped).
    if !after_name.is_empty() {
        return None;
    }

    Some(CapDecl { name })
}

/// Parse a `tag name = Type` declaration line (ADR-0077).
///
/// Returns `Some(TagDecl)` if the line matches the `tag` form with a
/// valid name and payload type. Returns `None` otherwise.
///
/// # Example
///
/// ```text
/// tag ok = Value
/// tag err = Error
/// ```
fn parse_tag_line(line: &str) -> Option<TagDecl> {
    let rest = line.strip_prefix("tag")?.trim();
    // Find the `=` separator.
    let eq_idx = rest.find('=')?;
    let name = rest[..eq_idx].trim();
    let payload_type = rest[eq_idx + 1..].trim();
    if name.is_empty() || payload_type.is_empty() {
        return None;
    }
    // Validate name: alphanumeric + underscores only.
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some(TagDecl {
        name: name.to_string(),
        payload_type: payload_type.to_string(),
    })
}

/// Extract a quoted string from the start of `s`, returning the content and
/// the remaining text after the closing quote.
fn extract_quoted_string(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    let quote = s.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &s[quote.len_utf8()..];
    let end = rest.find(quote)?;
    let content = rest[..end].to_string();
    let remaining = &rest[end + quote.len_utf8()..];
    Some((content, remaining))
}

/// Find the byte index of a keyword in `s`, ensuring it is a whole word
/// (not part of a larger identifier). Returns `None` if not found.
fn find_keyword(s: &str, keyword: &str) -> Option<usize> {
    let mut search_start = 0;
    while let Some(idx) = s[search_start..].find(keyword) {
        let abs_idx = search_start + idx;
        let before_ok =
            abs_idx == 0 || !s[..abs_idx].chars().next_back().unwrap().is_alphanumeric();
        let after_idx = abs_idx + keyword.len();
        let after_ok =
            after_idx >= s.len() || !s[after_idx..].chars().next().unwrap().is_alphanumeric();
        if before_ok && after_ok {
            return Some(abs_idx);
        }
        search_start = abs_idx + 1;
    }
    None
}

// ============================================================================
// Data-driven parsing (lex + Pratt + materialize)
// ============================================================================

/// Parse an arrow file using the data-driven parser.
///
/// Dispatches to named-arrow parsing (multi-line `name = expr`) or
/// single-expression parsing. Rich-loop syntax (`loop { ... }`) is always
/// treated as a single expression (not named arrows) because the arrow body
/// may contain ` = ` in let-bindings (`cmp = expr in body`) that would
/// falsely trigger the named-arrow heuristic.
fn parse_arrow_file_data_driven(src: &str) -> Result<ParsedFile, String> {
    let lang = arrow_language();
    let trimmed = src.trim();

    // Simple rec form: `rec { body }` (without `( ... )` after `}`).
    // This is the v0.1 replacement for `loop(body)`.
    if let Some(scheme) = try_parse_simple_rec(trimmed) {
        return Ok(ParsedFile::Single(scheme));
    }

    // Rich-loop syntax starts with `loop {` or `rec {` followed by `( ... )`.
    // Parse it as a single expression — do NOT check for named arrows, because
    // the body may contain ` = ` in let-bindings (`cmp = expr in body`).
    if is_rich_loop(trimmed) {
        let scheme = parse_arrow_expr_data_driven(trimmed, lang)?;
        return Ok(ParsedFile::Single(scheme));
    }

    // Named arrows: lines with ` = `
    if has_named_arrows(trimmed) {
        let map = parse_named_arrows_data_driven(trimmed, lang)?;
        return Ok(ParsedFile::Named(map));
    }

    // Single expression
    let scheme = parse_arrow_expr_data_driven(trimmed, lang)?;
    Ok(ParsedFile::Single(scheme))
}

/// Check if the source is a rich-loop expression (`loop { ... }` or `rec { ... }`).
///
/// Returns true if the trimmed source starts with the `loop` or `rec` keyword
/// followed by `{` (with possible whitespace between). This distinguishes
/// the rich-loop form from the simple `loop(...)` / `rec { ... }` form.
fn is_rich_loop(src: &str) -> bool {
    let trimmed = src.trim_start();
    for kw in ["loop", "rec"] {
        if let Some(rest) = trimmed.strip_prefix(kw) {
            // Ensure the keyword is word-complete (not a prefix of a longer
            // identifier like `looped` or `record`).
            let after = rest.chars().next();
            if after.is_none()
                || !after.unwrap().is_alphanumeric()
                    && after.unwrap() != '_'
                    && after.unwrap() != '.'
            {
                if rest.trim_start().starts_with('{') {
                    return true;
                }
            }
        }
    }
    false
}

/// Try to parse a simple `rec { body }` expression (v0.1 replacement for
/// `loop(body)`).
///
/// Returns `Some(Term::Loop { body })` if the source is `rec { body }`
/// (with no `( ... )` after the closing `}`). Returns `None` if the source
/// is not a simple rec form (e.g. it's a rich-loop `rec { state } ( body )`,
/// or it doesn't start with `rec {`).
fn try_parse_simple_rec(src: &str) -> Option<Term> {
    let trimmed = src.trim();

    // Must start with `rec` followed by whitespace and `{`.
    let rest = trimmed.strip_prefix("rec")?;
    // Word-boundary check: next char must not be alphanumeric/_/.
    let next = rest.chars().next()?;
    if next.is_alphanumeric() || next == '_' || next == '.' {
        return None;
    }
    let rest = rest.trim_start();
    if !rest.starts_with('{') {
        return None;
    }

    // Find the matching `}` (accounting for nested braces).
    let mut depth: i32 = 0;
    let mut close_idx: Option<usize> = None;
    for (i, c) in rest.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    close_idx = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let close_idx = close_idx?;

    // Check what's after the closing `}`.
    let after_brace = rest[close_idx + 1..].trim();

    // If there's a `(` after `}`, it's a rich-loop — not a simple rec.
    if after_brace.starts_with('(') {
        return None;
    }

    // If there's anything else after `}`, it's not a simple rec.
    if !after_brace.is_empty() {
        return None;
    }

    // Extract the body between `{` and `}` and parse it recursively.
    let body_src = rest[1..close_idx].trim();
    let body_scheme = parse_arrow_expr(body_src).ok()?;
    Some(
        TermData::loop_feedback(TermData::from_term(&body_scheme))
            .to_term()
            .expect("try_parse_simple_rec: TermData -> Term failed"),
    )
}

/// Parse a single arrow expression using the data-driven lexer + Pratt parser.
fn parse_arrow_expr_data_driven(src: &str, lang: &Language) -> Result<Term, String> {
    let tokens =
        handl_plugin_engine_lex_pattern::lex(lang, src).map_err(|e| format!("lex error: {}", e))?;

    if tokens.0.is_empty() {
        return Err("no tokens produced by lexer".to_string());
    }

    let engine = PrattEngine::new();
    let req = ParserRequest::new(tokens);
    let ast = engine
        .parse(lang, &req)
        .map_err(|e| format!("parse error: {}", e))?;

    ast_to_scheme(&ast, lang)
}

/// Parse named arrows (`name = expr` lines) using the data-driven parser.
///
/// Splits the source into lines, and for each line containing ` = `, parses
/// the expression part with the data-driven parser.
fn parse_named_arrows_data_driven(
    src: &str,
    _lang: &Language,
) -> Result<HashMap<String, Term>, String> {
    let mut map = HashMap::new();

    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            continue;
        }

        // Find ` :: ` (top-level binding); ` = ` is accepted for
        // pre-migration sources so old `name = expr` lines still bind.
        let sep = trimmed
            .find(" :: ")
            .map(|i| (i, 4))
            .or_else(|| trimmed.find(" = ").map(|i| (i, 3)));
        if let Some((eq_idx, sep_len)) = sep {
            let name = trimmed[..eq_idx].trim().to_string();
            let expr_str = trimmed[eq_idx + sep_len..].trim();
            // `parse_arrow_expr` covers the simple `rec { body }` form and
            // falls back to the legacy parser when the data-driven grammar
            // rejects the expression (e.g. arity-1 `rec`).
            let scheme = parse_arrow_expr(expr_str)?;
            map.insert(name, scheme);
        }
    }

    if map.is_empty() {
        return Err("no named arrows found".to_string());
    }

    Ok(map)
}

// ============================================================================
// AST-to-Term materializer
// ============================================================================

/// Materialize an [`Ast`] (produced by the Pratt parser) into a [`Term`].
///
/// The root node is the last node in the arena (Pratt convention: children
/// are pushed before parents). For each node, the eval scheme is looked up
/// via `lang.eval.get_by_identity(&node.kind)`, then walked to produce a
/// concrete `Term` with `proj_child(N)` placeholders replaced by the
/// materialized Nth child.
///
/// # Special placeholders
///
/// - `proj_child` (with `lit: N`): recursively materialize child N.
/// - `$str_child0` (as `op`): the op name is child 0's payload string
///   (used by `arr_call` / `app_call` for `arr("name")`).
/// - `$payload` (as `op`): the op name is the current node's payload string
///   (used by `arrow_var` for bare identifiers).
/// - `proj_rest` (in `args`): expand to the materialized children 1..N
///   (the argument expressions after the string name in `arr(...)` / `app(...)`).
/// - `$payload` (as `lit`): the literal value is the node's payload.
///
/// # Errors
///
/// Returns a `String` error if a node kind has no eval scheme, a placeholder
/// cannot be resolved, or a child index is out of bounds.
pub fn ast_to_scheme(ast: &Ast, lang: &Language) -> Result<Term, String> {
    if ast.is_empty() {
        return Err("empty AST — no root node".to_string());
    }
    // Root is the last node pushed (Pratt convention).
    let root_idx = ast.len() - 1;
    materialize_node(ast, root_idx, lang)
}

/// Materialize the node at arena index `idx` into a [`Term`].
fn materialize_node(ast: &Ast, idx: usize, lang: &Language) -> Result<Term, String> {
    let node = ast.nodes().get(idx).ok_or_else(|| {
        format!(
            "materialize_node: arena index {} out of bounds (len {})",
            idx,
            ast.len()
        )
    })?;
    let kind = &node.kind;

    // Data-drive v0.1 `assign` and legacy `named_arrow` bindings: the eval
    // scheme uses `extend_env`/`assign` for runtime evaluation, but for
    // static materialization we just materialize the `value` child. The
    // `target` role identifies a binding shape; the expression lives under
    // the `value` role. `named_arrow` is pre-v0.1 bootstrap with no
    // AstShapeDef, so it falls through to the positional fallback.
    if child_role_start(lang, kind, "target").is_some() || kind == "named_arrow" {
        let children = get_children(&node.payload)?;
        let value_idx = child_role_start(lang, kind, "value").unwrap_or(1);
        let expr_idx = children
            .get(value_idx)
            .and_then(|v| v.as_u64())
            .ok_or("binding: missing expression child")? as usize;
        return materialize_node(ast, expr_idx, lang);
    }

    // Special-case `function` (v0.1 replacement for `arrow_proc`): a lambda
    // `params ~> body`. For refinement, we materialize the `body`; the
    // params are the input binding pattern, not a runtime concern. The
    // `body` child is resolved via the canonical `function` AstShapeDef.
    if kind == "function" {
        let children = get_children(&node.payload)?;
        let body_idx = child_role_start(lang, kind, "body")
            .and_then(|idx| children.get(idx).and_then(|v| v.as_u64()))
            .ok_or("function: missing body child")? as usize;
        return materialize_node(ast, body_idx, lang);
    }

    // Special-case `loop_rich` (pre-v0.1) and `rec` (v0.1): the rich-loop
    // syntax is SUGAR. The YAML grammar parses it into a `loop_rich`/`rec`
    // AST node, but the actual `Term` graph is determined by pattern-
    // matching on the arrow body structure (which primitives are called,
    // what the state transitions are) — the same heuristic as the legacy
    // `analyze_branch`, but operating on the AST instead of raw text. The
    // `function` node (from `params ~> body`) has children [params, body].
    if kind == "loop_rich" || kind == "rec" {
        return rich_loop_to_scheme(ast, node, lang);
    }

    // --- Canonical node-kind handling (ADR-0079) ---
    //
    // The arrow dialect now uses the canonical handl language. Arrow
    // keywords (arr, comp, first, choice, loop, app, id) are no longer
    // special tokens — they lex as regular identifiers and parse as `var`
    // or `call` nodes. We intercept these canonical node kinds and produce
    // the corresponding arrow `Term` directly, bypassing the canonical
    // eval schemes (which produce runtime dispatch IR, not arrow IR).
    //
    // `flow` and `binary.lt`/`binary.lte` are no longer special-cased:
    // the arrow `Language` overrides the canonical eval schemes with
    // arrow-calculus `Term` templates that `walk_scheme` materializes.

    // `var` — bare identifier. `id` → arr_native("id"); other names →
    // App reference (matching the legacy parser's bare-identifier behavior).
    if kind == "var" {
        let name = node
            .payload
            .as_str()
            .ok_or("var: payload is not a string")?;
        if name == "id" {
            return Ok(
                TermData::op("id".to_string(), vec![], &ArrSource::Native, None)
                    .to_term()
                    .expect("materialize_node: id TermData -> Term failed"),
            );
        }
        return Ok(TermData::app(
            TermData::op(name.to_string(), vec![], &ArrSource::Native, None),
            vec![],
        )
        .to_term()
        .expect("materialize_node: var App TermData -> Term failed"));
    }

    // `value` — literal (number, string, etc.). Produces a constant `Arr`.
    if kind == "value" {
        return Ok(TermData::const_value(node.payload.clone())
            .to_term()
            .expect("materialize_node: value TermData -> Term failed"));
    }

    // `paren` — parenthesised expression. The eval scheme is just
    // `Op { name: "paren" }` without a proj_child, so handle it directly
    // by materializing the single inner child.
    if kind == "paren" {
        let children = get_children(&node.payload)?;
        let inner_idx = children
            .first()
            .and_then(|v| v.as_u64())
            .ok_or("paren: missing inner child")? as usize;
        return materialize_node(ast, inner_idx, lang);
    }

    // `binary.lte` / `binary.lt` are handled by the arrow `Language`'s
    // data-driven eval-scheme overrides for `binary.lte` / `binary.lt`.

    // Arrow combinator kinds — the canonical grammar produces dedicated
    // node kinds (`first`, `choice`, `flow`, etc.) for arrow combinators.
    // Lower them to the corresponding `Term` variants directly, since the
    // eval schemes produce `Op { name: "first" }` templates that would
    // otherwise not be lowered.
    if kind == "first" || kind == "second" {
        let children = get_children(&node.payload)?;
        let inner_idx = children
            .first()
            .and_then(|v| v.as_u64())
            .ok_or("first/second: missing inner child")? as usize;
        let inner = materialize_node(ast, inner_idx, lang)?;
        if kind == "first" {
            return Ok(TermData::first(TermData::from_term(&inner))
                .to_term()
                .expect("materialize_node: first TermData -> Term failed"));
        }
        // `second` maps to `First` on the second component — represent as
        // `First` wrapping a `Second` marker is not needed; the eval scheme
        // handles it. For now, treat it as a regular op.
        return Ok(TermData::op(
            kind.to_string(),
            vec![TermData::from_term(&inner)],
            &ArrSource::Native,
            None,
        )
        .to_term()
        .expect("materialize_node: second TermData -> Term failed"));
    }
    if kind == "choice" {
        let children = get_children(&node.payload)?;
        let l_idx = children
            .first()
            .and_then(|v| v.as_u64())
            .ok_or("choice: missing left child")? as usize;
        let r_idx = children
            .get(1)
            .and_then(|v| v.as_u64())
            .ok_or("choice: missing right child")? as usize;
        let l = materialize_node(ast, l_idx, lang)?;
        let r = materialize_node(ast, r_idx, lang)?;
        return Ok(
            TermData::choice(TermData::from_term(&l), TermData::from_term(&r))
                .to_term()
                .expect("materialize_node: choice TermData -> Term failed"),
        );
    }

    // `call` — function call. Dispatch on callee name for arrow combinators.
    if kind == "call" {
        let children = get_children(&node.payload)?;
        let callee_start = child_role_start(lang, kind, "callee").unwrap_or(0);
        let args_start = child_role_start(lang, kind, "args").unwrap_or(1);
        let callee_idx = children
            .get(callee_start)
            .and_then(|v| v.as_u64())
            .ok_or("call: missing callee child")? as usize;
        let callee_node = ast
            .nodes()
            .get(callee_idx)
            .ok_or("call: callee node out of bounds")?;

        // Extract callee name from a `var` callee.
        let callee_name = if callee_node.kind == "var" {
            callee_node.payload.as_str().map(|s| s.to_string())
        } else {
            None
        };

        if let Some(callee_name) = callee_name {
            match callee_name.as_str() {
                "arr" => {
                    // arr("op_name", ...extra_args) → Arr { op, args, Native }
                    let arg_idx = children
                        .get(args_start)
                        .and_then(|v| v.as_u64())
                        .ok_or("arr: missing string arg")?
                        as usize;
                    let arg_node = ast
                        .nodes()
                        .get(arg_idx)
                        .ok_or("arr: arg node out of bounds")?;
                    let op_name = arg_node
                        .payload
                        .as_str()
                        .ok_or("arr: arg payload is not a string")?
                        .to_string();
                    let mut args = Vec::new();
                    for i in (args_start + 1)..children.len() {
                        let idx = children[i]
                            .as_u64()
                            .ok_or("arr: child index not a number")?
                            as usize;
                        let arg_term = materialize_node(ast, idx, lang)?;
                        args.push(TermData::from_term(&arg_term));
                    }
                    return Ok(TermData::op(op_name, args, &ArrSource::Native, None)
                        .to_term()
                        .expect("materialize_node: arr TermData -> Term failed"));
                }
                "comp" => {
                    // comp(a, b) → Seq { fst: a, snd: b }
                    let a_idx = children
                        .get(args_start)
                        .and_then(|v| v.as_u64())
                        .ok_or("comp: missing first arg")? as usize;
                    let b_idx = children
                        .get(args_start + 1)
                        .and_then(|v| v.as_u64())
                        .ok_or("comp: missing second arg")?
                        as usize;
                    let a = materialize_node(ast, a_idx, lang)?;
                    let b = materialize_node(ast, b_idx, lang)?;
                    return Ok(
                        TermData::seq(TermData::from_term(&a), TermData::from_term(&b))
                            .to_term()
                            .expect("materialize_node: comp TermData -> Term failed"),
                    );
                }
                "first" => {
                    // first(a) → First { inner: a }
                    let a_idx = children
                        .get(args_start)
                        .and_then(|v| v.as_u64())
                        .ok_or("first: missing arg")? as usize;
                    let a = materialize_node(ast, a_idx, lang)?;
                    return Ok(TermData::first(TermData::from_term(&a))
                        .to_term()
                        .expect("materialize_node: first-call TermData -> Term failed"));
                }
                "choice" => {
                    // choice(a, b) → Choice { l: a, r: b }
                    let a_idx = children
                        .get(args_start)
                        .and_then(|v| v.as_u64())
                        .ok_or("choice: missing first arg")?
                        as usize;
                    let b_idx = children
                        .get(args_start + 1)
                        .and_then(|v| v.as_u64())
                        .ok_or("choice: missing second arg")?
                        as usize;
                    let a = materialize_node(ast, a_idx, lang)?;
                    let b = materialize_node(ast, b_idx, lang)?;
                    return Ok(
                        TermData::choice(TermData::from_term(&a), TermData::from_term(&b))
                            .to_term()
                            .expect("materialize_node: choice-call TermData -> Term failed"),
                    );
                }
                "loop" => {
                    // loop(body) → Loop { body }
                    let body_idx = children
                        .get(args_start)
                        .and_then(|v| v.as_u64())
                        .ok_or("loop: missing body arg")?
                        as usize;
                    let body = materialize_node(ast, body_idx, lang)?;
                    return Ok(TermData::loop_feedback(TermData::from_term(&body))
                        .to_term()
                        .expect("materialize_node: loop TermData -> Term failed"));
                }
                "app" => {
                    // app("name", ...args) → App { callee: arr_native("name"), args }
                    let arg_idx = children
                        .get(args_start)
                        .and_then(|v| v.as_u64())
                        .ok_or("app: missing string arg")?
                        as usize;
                    let arg_node = ast
                        .nodes()
                        .get(arg_idx)
                        .ok_or("app: arg node out of bounds")?;
                    let callee_name = arg_node
                        .payload
                        .as_str()
                        .ok_or("app: arg payload is not a string")?
                        .to_string();
                    let mut args = Vec::new();
                    for i in (args_start + 1)..children.len() {
                        let idx = children[i]
                            .as_u64()
                            .ok_or("app: child index not a number")?
                            as usize;
                        let arg_term = materialize_node(ast, idx, lang)?;
                        args.push(TermData::from_term(&arg_term));
                    }
                    return Ok(TermData::app(
                        TermData::op(callee_name, vec![], &ArrSource::Native, None),
                        args,
                    )
                    .to_term()
                    .expect("materialize_node: app TermData -> Term failed"));
                }
                _ => {
                    // Non-combinator call with var callee — treat as App
                    // reference with materialized arguments (arrow semantics).
                    let mut args = Vec::new();
                    for i in args_start..children.len() {
                        let idx = children[i]
                            .as_u64()
                            .ok_or("call: child index not a number")?
                            as usize;
                        let arg_term = materialize_node(ast, idx, lang)?;
                        args.push(TermData::from_term(&arg_term));
                    }
                    return Ok(TermData::app(
                        TermData::op(callee_name, vec![], &ArrSource::Native, None),
                        args,
                    )
                    .to_term()
                    .expect("materialize_node: call-default TermData -> Term failed"));
                }
            }
        }
        // Non-var callee (e.g. field-access) — fall through to eval scheme.
    }

    // Look up the eval scheme for this node kind.
    let eval_def = lang
        .eval
        .get_by_identity(kind)
        .ok_or_else(|| format!("no eval scheme for node kind: {}", kind))?;

    walk_scheme(&eval_def.scheme, ast, node, lang)
}

/// Walk a [`Term`] template (from the eval definition), substituting
/// `proj_child(N)` placeholders with materialized children and resolving
/// `$str_child0` / `$payload` / `proj_rest` conventions.
///
/// Thin wrapper that converts to [`TermData`] and delegates to
/// [`walk_scheme_td`].
fn walk_scheme(scheme: &Term, ast: &Ast, node: &AstNode, lang: &Language) -> Result<Term, String> {
    let td = TermData::from_term(scheme);
    let result = walk_scheme_td(&td, ast, node, lang)?;
    result.to_term().map_err(|e| format!("walk_scheme: {}", e))
}

/// Data-driven counterpart of [`walk_scheme`], operating on [`TermData`].
fn walk_scheme_td(
    td: &TermData,
    ast: &Ast,
    node: &AstNode,
    lang: &Language,
) -> Result<TermData, String> {
    match td.tag() {
        "Op" => {
            let op = td.get_str("name").unwrap_or("");
            match op {
                "proj_child" => {
                    // Recurse into child N of the current node.
                    let idx = td
                        .get("lit")
                        .and_then(|v| v.as_u64())
                        .ok_or("proj_child: missing index literal")?
                        as usize;
                    let children = get_children(&node.payload)?;
                    let child_arena_idx = children
                        .get(idx)
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| format!("proj_child: child {} not found", idx))?
                        as usize;
                    let child_term = materialize_node(ast, child_arena_idx, lang)?;
                    Ok(TermData::from_term(&child_term))
                }
                "$str_child0" => {
                    // The op name is child 0's payload string (arr("name") / app("name")).
                    let children = get_children(&node.payload)?;
                    let child0_idx = children
                        .first()
                        .and_then(|v| v.as_u64())
                        .ok_or("$str_child0: missing child 0")?
                        as usize;
                    let child0 = ast
                        .nodes()
                        .get(child0_idx)
                        .ok_or("$str_child0: child 0 out of bounds")?;
                    let name = child0
                        .payload
                        .as_str()
                        .ok_or("$str_child0: child 0 payload is not a string")?
                        .to_string();
                    let materialized_args =
                        walk_args_td(&td.get_term_list("args"), ast, node, lang)?;
                    let source = td.arr_source().unwrap_or(ArrSource::Native);
                    Ok(TermData::op(name, materialized_args, &source, None))
                }
                "$payload" => {
                    // The op name is the current node's payload string (bare identifier).
                    let name = node
                        .payload
                        .as_str()
                        .ok_or("$payload: node payload is not a string")?
                        .to_string();
                    let materialized_args =
                        walk_args_td(&td.get_term_list("args"), ast, node, lang)?;
                    let source = td.arr_source().unwrap_or(ArrSource::Native);
                    Ok(TermData::op(name, materialized_args, &source, None))
                }
                "proj_rest" => {
                    // proj_rest should only appear inside an args list (handled by
                    // walk_args). If reached here, it's a malformed eval scheme.
                    Err("proj_rest: should only appear in an args list".to_string())
                }
                _ => {
                    // Normal arr: walk args recursively, resolve $payload in lit.
                    let materialized_args =
                        walk_args_td(&td.get_term_list("args"), ast, node, lang)?;
                    let lit_val = td.get("lit").filter(|v| !v.is_null());
                    let materialized_lit = resolve_lit_val(lit_val, node);
                    let source = td.arr_source().unwrap_or(ArrSource::Native);
                    Ok(TermData::op(
                        op.to_string(),
                        materialized_args,
                        &source,
                        materialized_lit,
                    ))
                }
            }
        }

        "Seq" => {
            let fst = td.get_term("fst").ok_or("Seq: missing fst")?;
            let snd = td.get_term("snd").ok_or("Seq: missing snd")?;
            Ok(TermData::seq(
                walk_scheme_td(&fst, ast, node, lang)?,
                walk_scheme_td(&snd, ast, node, lang)?,
            ))
        }

        "First" => {
            let inner = td.get_term("inner").ok_or("First: missing inner")?;
            Ok(TermData::first(walk_scheme_td(&inner, ast, node, lang)?))
        }

        "Choice" => {
            let l = td.get_term("l").ok_or("Choice: missing l")?;
            let r = td.get_term("r").ok_or("Choice: missing r")?;
            Ok(TermData::choice(
                walk_scheme_td(&l, ast, node, lang)?,
                walk_scheme_td(&r, ast, node, lang)?,
            ))
        }

        "Loop" => {
            let body = td.get_term("body").ok_or("Loop: missing body")?;
            Ok(TermData::loop_feedback(walk_scheme_td(
                &body, ast, node, lang,
            )?))
        }

        "App" => {
            let callee = td.get_term("callee").ok_or("App: missing callee")?;
            let materialized_callee = walk_scheme_td(&callee, ast, node, lang)?;
            let materialized_args = walk_args_td(&td.get_term_list("args"), ast, node, lang)?;
            Ok(TermData::app(materialized_callee, materialized_args))
        }
        // `handle body with target = impl_` (ADR-0076): eliminate the handler
        // at compile time, then walk the resulting handler-free graph.
        "Handle" => {
            let eliminated = td
                .to_term()
                .map_err(|e| format!("Handle: {}", e))?
                .eliminate_handlers();
            let eliminated_td = TermData::from_term(&eliminated);
            walk_scheme_td(&eliminated_td, ast, node, lang)
        }

        "Arr" => {
            let value = td.get("value").cloned().unwrap_or(Value::Null);
            Ok(TermData::arr(value))
        }
        "Child" => Err("child projection not supported".to_string()),
        "Iter" => {
            let fuel = td.get_term("fuel").ok_or("Iter: missing fuel")?;
            let predicate = td.get_term("predicate").ok_or("Iter: missing predicate")?;
            let step = td.get_term("step").ok_or("Iter: missing step")?;
            let initial = td.get_term("initial").ok_or("Iter: missing initial")?;
            Ok(TermData::iter(
                walk_scheme_td(&fuel, ast, node, lang)?,
                walk_scheme_td(&predicate, ast, node, lang)?,
                walk_scheme_td(&step, ast, node, lang)?,
                walk_scheme_td(&initial, ast, node, lang)?,
            ))
        }
        "Rewrite" => Err("rewrite scheme not supported".to_string()),
        // Unknown/unsupported tags: return an error.
        _ => Err(format!("walk_scheme_td: unsupported tag '{}'", td.tag())),
    }
}

/// Walk a list of argument schemes, expanding `proj_rest` placeholders into
/// the materialized children 1..N of the current node.
///
/// Operates on [`TermData`] (data-driven).
fn walk_args_td(
    args: &[TermData],
    ast: &Ast,
    node: &AstNode,
    lang: &Language,
) -> Result<Vec<TermData>, String> {
    let mut result = Vec::new();
    for arg in args {
        if arg.tag() == "Op" && arg.get_str("name") == Some("proj_rest") {
            // Expand to materialized children 1..N (all children after
            // the string-name child 0).
            let children = get_children(&node.payload)?;
            for i in 1..children.len() {
                let child_idx = children[i]
                    .as_u64()
                    .ok_or("proj_rest: child index not a number")?
                    as usize;
                let child_term = materialize_node(ast, child_idx, lang)?;
                result.push(TermData::from_term(&child_term));
            }
            continue;
        }
        result.push(walk_scheme_td(arg, ast, node, lang)?);
    }
    Ok(result)
}

/// Resolve a `lit` payload that may contain the `$payload` placeholder.
///
/// If `lit` is the string `"$payload"`, it is replaced with the node's actual
/// payload value. Otherwise, the literal is passed through unchanged.
///
/// Takes an `Option<&Value>` (as produced by [`TermData::get`]).
fn resolve_lit_val(lit: Option<&Value>, node: &AstNode) -> Option<Value> {
    lit.and_then(|v| {
        if v == &Value::String("$payload".to_string()) {
            Some(node.payload.clone())
        } else {
            Some(v.clone())
        }
    })
}

/// Extract the `children` array from a node's payload.
fn get_children(payload: &Value) -> Result<&Vec<Value>, String> {
    payload
        .get("children")
        .and_then(|c| c.as_array())
        .ok_or("expected 'children' array in node payload".to_string())
}

/// Resolve a named child role to its positional index, if declared.
fn child_role_start(lang: &Language, kind: &str, role: &str) -> Option<usize> {
    lang.child_role_index(kind, role).ok().flatten()
}

/// Resolve a pattern-role tag for a node kind, if any.
fn pattern_role<'a>(lang: &'a Language, kind: &str) -> Option<&'a str> {
    lang.ast
        .get_by_identity(kind)
        .and_then(|s| s.pattern_role.as_deref())
}

// ============================================================================
// Rich-loop AST-to-Term conversion (pattern matching on arrow body)
// ============================================================================
//
// The rich-loop syntax (`loop { ... } ( state ~> match ... )`) is
// SUGAR. The YAML grammar tokenizes and parses it into a `loop_rich` AST
// node, but the actual `Term` graph is determined by recognizing the
// structural patterns in the arrow body's recursive match arm — the same
// heuristic as the legacy `analyze_branch`, but operating on the AST
// instead of raw text.
//
// AST structure of a rich-loop:
//   loop_rich (children: [decl_block, arrow_expr])
//     └─ arrow_expr: arrow_proc (children: [params, body])
//        └─ body: arrow_match (children: [scrutinee, arms_container])
//           └─ arms_container: block (children: [arm1, arm2, ...])
//              ├─ arm1: arrow_arm (children: [pattern, body])  — base case
//              │   └─ pattern: arrow_array (children: [])      — `[]`
//              └─ arm2: arrow_arm (children: [pattern, body])  — recursive case
//                  └─ pattern: arrow_array (children: [...])   — `[h, ...t]`
//                  └─ body: ...                                 — recursive body

/// Convert a `loop_rich` AST node into a [`Term`] by pattern-matching on
/// the arrow body's recursive match arm.
///
/// This navigates the AST to find the `[h, ...t]:` arm (the recursive case),
/// then calls [`analyze_branch_ast`] to determine which `Term` graph the
/// arm body corresponds to.
fn rich_loop_to_scheme(ast: &Ast, node: &AstNode, lang: &Language) -> Result<Term, String> {
    let children = get_children(&node.payload)?;

    // The arrow expression is the `body` child (child 1 for the canonical
    // `rec` shape and for the legacy `loop_rich` shape).
    let body_idx = child_role_start(lang, &node.kind, "body").unwrap_or(1);
    let arrow_idx = children
        .get(body_idx)
        .and_then(|v| v.as_u64())
        .ok_or("loop_rich: missing arrow child")? as usize;
    let arrow_node = ast
        .nodes()
        .get(arrow_idx)
        .ok_or("loop_rich: arrow node out of bounds")?;

    if arrow_node.kind != "arrow_proc" && arrow_node.kind != "function" {
        return Err(format!(
            "loop_rich: expected arrow_proc or function, got {}",
            arrow_node.kind
        ));
    }

    let arrow_children = get_children(&arrow_node.payload)?;
    // The arrow's `body` child (child 1 in the canonical `function` shape).
    let fn_body_idx = child_role_start(lang, &arrow_node.kind, "body").unwrap_or(1);
    let body_idx = arrow_children
        .get(fn_body_idx)
        .and_then(|v| v.as_u64())
        .ok_or("loop_rich: missing arrow body")? as usize;
    let body_node = ast
        .nodes()
        .get(body_idx)
        .ok_or("loop_rich: body node out of bounds")?;

    if body_node.kind != "arrow_match" && body_node.kind != "match" {
        return Err(format!(
            "loop_rich: expected arrow_match or match body, got {}",
            body_node.kind
        ));
    }

    let match_children = get_children(&body_node.payload)?;
    // Child 1 of match is the arms container (child 0 is scrutinee).
    let arms_idx = match_children
        .get(1)
        .and_then(|v| v.as_u64())
        .ok_or("loop_rich: missing arms container")? as usize;
    let arms_node = ast
        .nodes()
        .get(arms_idx)
        .ok_or("loop_rich: arms node out of bounds")?;

    let arms_children = get_children(&arms_node.payload)?;

    // Find the recursive case arm (non-empty array pattern `[h, ...t]`)
    // and the base case arm (empty array pattern `[]`).
    let mut recursive_body_idx: Option<usize> = None;
    let mut base_body_idx: Option<usize> = None;
    for arm_idx_val in arms_children {
        let arm_idx = arm_idx_val
            .as_u64()
            .ok_or("loop_rich: arm index not a number")? as usize;
        let arm_node = ast
            .nodes()
            .get(arm_idx)
            .ok_or("loop_rich: arm node out of bounds")?;
        if arm_node.kind != "arrow_arm" && arm_node.kind != "arm" {
            continue;
        }
        let arm_children = get_children(&arm_node.payload)?;
        let pattern_start = child_role_start(lang, &arm_node.kind, "pattern").unwrap_or(0);
        let body_start = child_role_start(lang, &arm_node.kind, "body").unwrap_or(1);
        let pattern_idx = arm_children
            .get(pattern_start)
            .and_then(|v| v.as_u64())
            .ok_or("loop_rich: missing pattern")? as usize;
        let pattern_node = ast
            .nodes()
            .get(pattern_idx)
            .ok_or("loop_rich: pattern node out of bounds")?;

        // Check if the pattern is a sequence (array). The legacy
        // `arrow_array` kind has no AstShapeDef, so it is kept as a fallback.
        if pattern_role(lang, &pattern_node.kind) == Some("sequence")
            || pattern_node.kind == "arrow_array"
        {
            let arm_body_idx = arm_children
                .get(body_start)
                .and_then(|v| v.as_u64())
                .ok_or("loop_rich: missing arm body")? as usize;
            if let Ok(pattern_children) = get_children(&pattern_node.payload) {
                if !pattern_children.is_empty() {
                    // Recursive case arm (`[h, ...t]`).
                    recursive_body_idx = Some(arm_body_idx);
                } else {
                    // Base case arm (`[]`).
                    base_body_idx = Some(arm_body_idx);
                }
            }
        }
    }

    let body_idx = recursive_body_idx.ok_or("loop_rich: no recursive case arm found")?;

    // Analyze the recursive body AST to determine the Term graph.
    // The base case body is also collected to detect projection patterns
    // (e.g. `id(state.acc)` in the base case signals reduce vs fold).
    let body_scheme = analyze_branch_ast(ast, body_idx, base_body_idx, lang)?;

    Ok(TermData::loop_feedback(TermData::from_term(&body_scheme))
        .to_term()
        .expect("rich_loop_to_scheme: TermData -> Term failed"))
}

/// Analyze the recursive case arm body AST and map it to a [`Term`] graph.
///
/// This mirrors the legacy `analyze_branch` heuristic but operates on the AST
/// instead of raw text. The patterns recognized (in order of specificity):
///
/// 0. Nested `loop_rich` → recursively materialize the inner loop
/// 1. `prim.compare` identifier → `comp(prim.compare, choice(cons, cons))` (insert)
/// 2. `*.p` in arrow_call (`callee(arg)`) → `comp(filter_step, id)` (filter)
/// 3. `*.f` in call with `*.acc` argument, no `id` on `*.acc` → `fold_step` (fold)
/// 3a. `*.f` in call with `*.acc` argument, `id` on `*.acc` → `comp(fold_step, first(id))` (reduce)
/// 4. `insert` in arrow_call (`callee(arg)`) → `comp(app(insert), first(id))` (sort)
/// 5. `*.f` in arrow_call with `*.acc` in right args → `comp(fold_step, first(list.cons))` (scan)
/// 6. `list.head` AND `list.tail` both in arrow_call → `comp(first(list.head), comp(first(list.tail), list.cons))` (zip)
/// 7. `list.head` in arrow_call (`callee(arg)`) → `comp(list.head, choice(cons, id))` (take)
/// 8. `list.tail` in arrow_call → `comp(list.tail, first(id))` (drop)
/// 9. `*.f` in arrow_call (`callee(arg)`) → `first(fn.apply)` (map)
///
/// Patterns 2, 3, 5, and 9 use suffix-based matching (e.g. `*.f` matches
/// `state.f`, `inner_state.f`, etc.) so that nested loops with different
/// state variable names are recognized correctly.
///
/// Early-exit patterns (a `match` arm that returns without recursing) are
/// inherently handled: the base-case arm (`[]:`) body is collected for
/// pattern detection (e.g. `id(state.acc)` signals reduce vs fold) but only
/// the recursive arm (`[h, ...t]:`) is analyzed for the main graph
/// structure. Conditionals within the recursive arm (e.g. `if state.n <= 0
/// return ...`) are tracked via `has_conditional` for future pattern
/// discrimination.
fn analyze_branch_ast(
    ast: &Ast,
    root_idx: usize,
    base_case_idx: Option<usize>,
    lang: &Language,
) -> Result<Term, String> {
    analyze_branch_ast_td(ast, root_idx, base_case_idx, lang)?
        .to_term()
        .map_err(|e| format!("analyze_branch_ast: {}", e))
}

/// Data-driven counterpart of [`analyze_branch_ast`], returning [`TermData`].
///
/// Constructs `TermData` for each recognized pattern using the `TermData`
/// constructors instead of direct `Term` enum construction.
fn analyze_branch_ast_td(
    ast: &Ast,
    root_idx: usize,
    base_case_idx: Option<usize>,
    lang: &Language,
) -> Result<TermData, String> {
    let mut collector = PatternCollector::default();
    collector.collect(ast, root_idx);
    // Also collect from the base case arm to detect projection patterns
    // (e.g. `id(state.acc)` in the base case signals reduce vs fold).
    if let Some(base_idx) = base_case_idx {
        collector.collect(ast, base_idx);
    }

    // 0. Nested loop → recursively materialize the inner loop_rich node.
    //    If the recursive arm body contains a nested `loop { ... }`, we find
    //    it and recursively call `rich_loop_to_scheme` to materialize it.
    //    Note: `rich_loop_to_scheme` already wraps the result in `Term::Loop`,
    //    and the outer `rich_loop_to_scheme` will wrap this result again,
    //    producing the correct `Loop(Loop(...))` structure.
    if collector.has_nested_loop {
        // Look for either `loop_rich` (pre-v0.1) or `rec` (v0.1 canonical).
        let nested_idx = find_node_kind(ast, root_idx, "loop_rich")
            .or_else(|| find_node_kind(ast, root_idx, "rec"));
        if let Some(nested_idx) = nested_idx {
            let nested_node = ast
                .nodes()
                .get(nested_idx)
                .ok_or("nested loop_rich/rec node out of bounds")?;
            let nested_td = rich_loop_to_scheme(ast, nested_node, lang)?;
            return Ok(TermData::from_term(&nested_td));
        }
    }

    // Helper: check if any name in a set ends with the given suffix.
    // This allows matching `state.f`, `inner_state.f`, etc.
    let has_suffix =
        |set: &HashSet<String>, suffix: &str| set.iter().any(|name| name.ends_with(suffix));

    // Helper: construct a native op TermData.
    let arr_native = |name: &str| TermData::op(name.to_string(), vec![], &ArrSource::Native, None);

    // 1. prim.compare → insert pattern
    if collector.var_names.contains("prim.compare") {
        let step = arr_native("prim.compare");
        let choice = TermData::choice(arr_native("cons"), arr_native("cons"));
        return Ok(TermData::seq(step, choice));
    }

    // 2. *.p in arrow_call → filter pattern
    if has_suffix(&collector.app_left_names, ".p") {
        let step = arr_native("filter_step");
        let rest = arr_native("id");
        return Ok(TermData::seq(step, rest));
    }

    // 3. *.f in call with *.acc argument → fold or reduce pattern.
    //    - reduce: `id` is applied to the accumulator (projection) →
    //      `comp(fold_step, first(id))` — returns only the accumulator.
    //    - fold: no projection → `fold_step` — returns both channels.
    for (callee, args) in &collector.call_callee_args {
        if callee.ends_with(".f") && args.iter().any(|a| a.ends_with(".acc")) {
            let step = arr_native("fold_step");
            // Check for reduce: `id` called with a `*.acc` argument
            // (projection pattern — discards the element stream).
            let has_id_on_acc = collector
                .call_callee_args
                .get("id")
                .map(|id_args| id_args.iter().any(|a| a.ends_with(".acc")))
                .unwrap_or(false);
            if has_id_on_acc {
                // reduce: projects to accumulator only.
                let rest = TermData::first(arr_native("id"));
                return Ok(TermData::seq(step, rest));
            } else {
                // fold: returns both channels (no projection).
                return Ok(step);
            }
        }
    }

    // 4. insert in arrow_call → sort pattern
    if collector.app_left_names.contains("insert") {
        let step = TermData::app(arr_native("insert"), vec![]);
        let rest = TermData::first(arr_native("id"));
        return Ok(TermData::seq(step, rest));
    }

    // 5. *.f in arrow_call with *.acc in right args → scan pattern
    //    scan is like fold (uses f with acc) but also conses
    //    each intermediate result onto an output list.
    for (left, right_args) in &collector.app_left_args {
        if left.ends_with(".f") && right_args.iter().any(|a| a.ends_with(".acc")) {
            let step = arr_native("fold_step");
            let rest = TermData::first(arr_native("list.cons"));
            return Ok(TermData::seq(step, rest));
        }
    }

    // 6. list.head AND list.tail both in arrow_call → zip pattern.
    //    Zip pairs two lists: it takes the head of both lists (one via
    //    pattern destructuring, the other via list.head() call), advances
    //    both (one via pattern tail, the other via list.tail() call), and
    //    conses the pair onto the output via list.cons.
    //    Graph: first(list.head) >>> (first(list.tail) >>> list.cons)
    //    This must be checked BEFORE the take and drop patterns (7 and 8)
    //    because zip uses list.tail in arrow_call (which would match drop)
    //    and the combination of both list.head and list.tail in arrow_call
    //    is unique to zip.
    //    The `!let_bound_callees` check excludes take, where list.head is
    //    called in a let binding (`y = list.head(...) in ...`).
    if collector.call_callee_args.contains_key("list.head")
        && collector.call_callee_args.contains_key("list.tail")
        && !collector.let_bound_callees.contains("list.head")
    {
        let step1 = TermData::first(arr_native("list.head"));
        let step2 = TermData::first(arr_native("list.tail"));
        let cons = arr_native("list.cons");
        return Ok(TermData::seq(step1, TermData::seq(step2, cons)));
    }

    // 7. list.head in arrow_call → take pattern
    if collector.app_left_names.contains("list.head") {
        let step = arr_native("list.head");
        let choice = TermData::choice(arr_native("cons"), arr_native("id"));
        return Ok(TermData::seq(step, choice));
    }

    // 8. list.tail in arrow_call → drop pattern
    if collector.call_callee_args.contains_key("list.tail") {
        let step = arr_native("list.tail");
        let rest = TermData::first(arr_native("id"));
        return Ok(TermData::seq(step, rest));
    }

    // 9. *.f in arrow_call → map pattern
    if has_suffix(&collector.app_left_names, ".f") {
        return Ok(TermData::first(arr_native("fn.apply")));
    }

    Err("could not analyze branch body — no recognised pattern found".to_string())
}

/// Recursively collects structural patterns from the AST subtree for the
/// rich-loop heuristic.
#[derive(Default)]
struct PatternCollector {
    /// All `arrow_var` identifier names found in the subtree.
    var_names: HashSet<String>,
    /// Callee names of `arrow_call` nodes where the callee is an
    /// `arrow_var` (e.g. `state.f(h)` → `"state.f"`). Used for filter,
    /// sort, take, and map pattern detection.
    app_left_names: HashSet<String>,
    /// For each `arrow_call` node, maps the callee name to the set of
    /// `arrow_var` names found recursively in the argument subtrees (e.g.
    /// `state.f({ acc: state.acc, h: h })` →
    /// `"state.f" → {"state.acc", "h"}`). Used for scan pattern detection.
    app_left_args: HashMap<String, HashSet<String>>,
    /// Callee name → direct argument names for `arrow_call` nodes where the
    /// callee is an `arrow_var` (e.g. `state.f(state.acc, h)` →
    /// `"state.f" → ["state.acc", "h"]`). Used for fold/reduce/zip/drop
    /// pattern detection.
    call_callee_args: HashMap<String, Vec<String>>,
    /// Callee names that appear as the expression in a `named_arrow` (let
    /// binding) node, e.g. `y = list.head(state.remaining) in ...` →
    /// `"list.head"`. Used to distinguish take (let-bound `list.head`)
    /// from zip (direct `list.head` in state update).
    let_bound_callees: HashSet<String>,
    /// Whether the subtree contains an `arrow_if` node (conditional).
    has_conditional: bool,
    /// Whether the subtree contains an `arrow_array` with an `arrow_rest`
    /// child (i.e. a cons-like `[x, ...rest]` construction).
    has_array_spread: bool,
    /// Whether the subtree contains a nested `loop_rich` node.
    has_nested_loop: bool,
}

impl PatternCollector {
    /// Recursively walk the AST from `idx`, collecting patterns.
    fn collect(&mut self, ast: &Ast, idx: usize) {
        let node = match ast.nodes().get(idx) {
            Some(n) => n,
            None => return,
        };

        match node.kind.as_str() {
            "arrow_var" | "var" => {
                // Identifier — record its name.
                if let Some(name) = node.payload.as_str() {
                    self.var_names.insert(name.to_string());
                }
                // No children to recurse into.
                return;
            }
            "field-access" => {
                // Dotted identifier (e.g. `state.acc`, `prim.compare`) —
                // build the full dotted name and record it.
                if let Some(dotted) = extract_dotted_name(ast, idx) {
                    self.var_names.insert(dotted);
                }
                // Still recurse into children for component-level vars.
            }
            "arrow_if" | "if" => {
                // Conditional (if/else) — record its presence.
                self.has_conditional = true;
            }
            "named_arrow" | "assign" => {
                // `name = expr` let binding. If expr is an arrow_call/call
                // with a var/field-access callee, track the callee name as
                // let-bound. This distinguishes take (y = list.head(...) in
                // ...) from zip (list.head(...) used directly in state
                // update).
                if let Ok(children) = get_children(&node.payload) {
                    if let Some(expr_idx) = children.get(1).and_then(|v| v.as_u64()) {
                        if let Some(expr_node) = ast.nodes().get(expr_idx as usize) {
                            if expr_node.kind == "arrow_call" || expr_node.kind == "call" {
                                if let Ok(expr_children) = get_children(&expr_node.payload) {
                                    if let Some(callee_idx) =
                                        expr_children.first().and_then(|v| v.as_u64())
                                    {
                                        if let Some(callee_node) =
                                            ast.nodes().get(callee_idx as usize)
                                        {
                                            // Extract callee name from var,
                                            // field-access, or arrow_id.
                                            let callee_name =
                                                extract_dotted_name(ast, callee_idx as usize)
                                                    .or_else(|| {
                                                        if callee_node.kind == "arrow_id" {
                                                            Some("id".to_string())
                                                        } else if callee_node.kind == "value" {
                                                            callee_node
                                                                .payload
                                                                .as_str()
                                                                .filter(|s| *s == "id")
                                                                .map(|s| s.to_string())
                                                        } else {
                                                            None
                                                        }
                                                    });
                                            if let Some(name) = callee_name {
                                                self.let_bound_callees.insert(name);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            "loop_rich" | "rec" => {
                // Nested rich loop — record its presence.
                self.has_nested_loop = true;
            }
            "arrow_array" | "array" => {
                // Check for cons-like construction: `[x, ...rest]`.
                // An `arrow_rest`/`rest` child indicates a spread/cons.
                if let Ok(children) = get_children(&node.payload) {
                    for child_val in children {
                        if let Some(child_idx) = child_val.as_u64() {
                            if let Some(child_node) = ast.nodes().get(child_idx as usize) {
                                if child_node.kind == "arrow_rest" || child_node.kind == "rest" {
                                    self.has_array_spread = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            "arrow_call" | "call" => {
                // `callee(args...)` function call: child 0 is callee,
                // children 1..N are arguments.
                if let Ok(children) = get_children(&node.payload) {
                    if let Some(callee_idx) = children.first().and_then(|v| v.as_u64()) {
                        if let Some(callee_node) = ast.nodes().get(callee_idx as usize) {
                            // The callee can be a `var` (bare identifier
                            // like `state.f`), a `field-access` (dotted
                            // name like `prim.compare`), or an `arrow_id`
                            // (the `id` keyword, used in projection patterns
                            // like `id(state.acc)`).
                            let callee_name = extract_dotted_name(ast, callee_idx as usize)
                                .or_else(|| {
                                    if callee_node.kind == "arrow_id" {
                                        Some("id".to_string())
                                    } else if callee_node.kind == "value" {
                                        // v0.1: `id` may parse as a `value` node with
                                        // payload "id" (keyword-as-identifier fallback).
                                        callee_node
                                            .payload
                                            .as_str()
                                            .filter(|s| *s == "id")
                                            .map(|s| s.to_string())
                                    } else {
                                        None
                                    }
                                });
                            if let Some(callee_name) = callee_name {
                                // Populate app_left_names for filter/sort/take/map
                                // pattern detection.
                                self.app_left_names.insert(callee_name.clone());
                                // Collect direct argument names for
                                // fold/reduce/zip/drop pattern detection.
                                let mut arg_names = Vec::new();
                                // Recursively collect var names from all
                                // argument subtrees for scan pattern detection.
                                let mut all_var_args = HashSet::new();
                                for i in 1..children.len() {
                                    if let Some(arg_idx) = children[i].as_u64() {
                                        if let Some(_arg_node) = ast.nodes().get(arg_idx as usize) {
                                            // Direct argument: extract dotted
                                            // name from var or field-access.
                                            if let Some(arg_name) =
                                                extract_dotted_name(ast, arg_idx as usize)
                                            {
                                                arg_names.push(arg_name);
                                            }
                                            // Recursively collect all var names
                                            // from this argument subtree.
                                            let vars = collect_var_names(ast, arg_idx as usize);
                                            all_var_args.extend(vars);
                                        }
                                    }
                                }
                                self.call_callee_args.insert(callee_name.clone(), arg_names);
                                self.app_left_args
                                    .entry(callee_name)
                                    .or_default()
                                    .extend(all_var_args);
                            }
                        }
                    }
                    // Recurse into all children.
                    for child_val in children {
                        if let Some(child_idx) = child_val.as_u64() {
                            self.collect(ast, child_idx as usize);
                        }
                    }
                }
                return;
            }
            _ => {}
        }

        // Default: recurse into children for all other node kinds.
        if let Ok(children) = get_children(&node.payload) {
            for child_val in children {
                if let Some(child_idx) = child_val.as_u64() {
                    self.collect(ast, child_idx as usize);
                }
            }
        }
    }
}

/// Collect all `arrow_var` identifier names in an AST subtree.
///
/// This is used by `PatternCollector` to gather the variable names in the
/// argument subtrees of an `arrow_call` node.
fn collect_var_names(ast: &Ast, idx: usize) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_var_names_recursive(ast, idx, &mut names);
    names
}

/// Extract a dotted name from a `var` or `field-access` node chain.
///
/// For a `var` node, returns its payload string (e.g. `var("state")` →
/// `"state"`). For a `field-access` node, recursively extracts the base
/// name and appends the field name (e.g.
/// `field-access(var("prim"), var("compare"))` → `"prim.compare"`).
/// Returns `None` if the node is neither a `var` nor a `field-access`.
fn extract_dotted_name(ast: &Ast, idx: usize) -> Option<String> {
    let node = ast.nodes().get(idx)?;
    if node.kind == "var" {
        return node.payload.as_str().map(|s| s.to_string());
    }
    if node.kind == "field-access" {
        let children = get_children(&node.payload).ok()?;
        let base_idx = children.first()?.as_u64()? as usize;
        let field_idx = children.get(1)?.as_u64()? as usize;
        let base = extract_dotted_name(ast, base_idx)?;
        let field_node = ast.nodes().get(field_idx)?;
        // The field name may be in a `var` child (payload is the name
        // string), or in `payload.name` / `payload.value` / payload-as-string.
        let field = field_node
            .payload
            .get("name")
            .and_then(|v| v.as_str())
            .or_else(|| field_node.payload.get("value").and_then(|v| v.as_str()))
            .or_else(|| field_node.payload.as_str())
            .or_else(|| {
                // If the field child is itself a `var`, use its payload.
                if field_node.kind == "var" {
                    field_node.payload.as_str()
                } else {
                    None
                }
            })?;
        return Some(format!("{}.{}", base, field));
    }
    None
}

fn collect_var_names_recursive(ast: &Ast, idx: usize, names: &mut HashSet<String>) {
    let node = match ast.nodes().get(idx) {
        Some(n) => n,
        None => return,
    };

    // `field-access` — build a dotted name (e.g. `state.acc` → "state.acc")
    // and also recurse into children for individual component names.
    if node.kind == "field-access" {
        if let Some(dotted) = extract_dotted_name(ast, idx) {
            names.insert(dotted);
        }
    }

    if node.kind == "arrow_var" || node.kind == "var" {
        if let Some(name) = node.payload.as_str() {
            names.insert(name.to_string());
        }
        return;
    }

    if let Ok(children) = get_children(&node.payload) {
        for child_val in children {
            if let Some(child_idx) = child_val.as_u64() {
                collect_var_names_recursive(ast, child_idx as usize, names);
            }
        }
    }
}

/// Find the first AST node of the given `kind` in the subtree rooted at `idx`.
///
/// Used by `analyze_branch_ast` to locate nested `loop_rich` nodes for
/// recursive materialization.
fn find_node_kind(ast: &Ast, idx: usize, kind: &str) -> Option<usize> {
    let node = ast.nodes().get(idx)?;
    if node.kind == kind {
        return Some(idx);
    }
    if let Ok(children) = get_children(&node.payload) {
        for child_val in children {
            if let Some(child_idx) = child_val.as_u64() {
                if let Some(found) = find_node_kind(ast, child_idx as usize, kind) {
                    return Some(found);
                }
            }
        }
    }
    None
}

// ============================================================================
// Legacy ad-hoc parser (fallback)
// ============================================================================

/// Legacy: parse a `.hnl` arrow file using the ad-hoc recursive-descent parser.
fn parse_arrow_file_legacy(src: &str) -> Result<ParsedFile, String> {
    let trimmed = src.trim();

    if trimmed.is_empty() {
        return Err("empty file (after stripping comments)".to_string());
    }

    // Named arrows: lines with `name = expr`
    if has_named_arrows(trimmed) {
        let map = parse_named_arrows(trimmed)?;
        return Ok(ParsedFile::Named(map));
    }

    // Simple single-expression file
    let scheme = parse_arrow_expr_legacy(trimmed)?;
    Ok(ParsedFile::Single(scheme))
}

/// Legacy: parse a simple arrow expression using the ad-hoc tokenizer +
/// recursive-descent parser.
///
/// The parser builds [`TermData`] internally and converts to [`Term`] at the
/// end (thin-wrapper pattern).
fn parse_arrow_expr_legacy(src: &str) -> Result<Term, String> {
    let tokens = tokenize(src)?;
    let mut parser = Parser::new(&tokens);
    let scheme = parser.parse_expr()?;
    if parser.pos != tokens.len() {
        return Err(format!(
            "unexpected tokens after expression: {} remaining",
            tokens.len() - parser.pos
        ));
    }
    scheme.to_term().map_err(|e| format!("legacy parse: {}", e))
}

// ============================================================================
// Comment stripping
// ============================================================================

/// Remove `--` comment lines from the source.
fn strip_comments(src: &str) -> String {
    src.lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ============================================================================
// Named arrow detection and parsing (legacy)
// ============================================================================

/// Check if the source contains named arrow definitions (`name :: expr`).
///
/// `name = expr` at line start is also detected so pre-`::`-migration
/// sources route to named-arrow parsing (legacy fallback) rather than
/// silently parsing as a single `equal` expression.
fn has_named_arrows(src: &str) -> bool {
    src.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            return false;
        }
        if line.starts_with(char::is_whitespace) {
            return false;
        }
        trimmed.contains(" :: ") || trimmed.contains(" = ")
    })
}

/// Legacy: parse named arrows: `name = expr` on each line.
fn parse_named_arrows(src: &str) -> Result<HashMap<String, Term>, String> {
    let mut map = HashMap::new();

    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Find ` :: ` or ` = ` (top-level binding); both are detected by
        // `has_named_arrows`, so both must be split here. ` :: ` is the
        // canonical spelling; ` = ` is the pre-migration form.
        let sep = trimmed
            .find(" :: ")
            .map(|i| (i, 4))
            .or_else(|| trimmed.find(" = ").map(|i| (i, 3)));
        if let Some((eq_idx, sep_len)) = sep {
            let name = trimmed[..eq_idx].trim().to_string();
            let expr_str = trimmed[eq_idx + sep_len..].trim();
            // `parse_arrow_expr` also handles `rec { body }` bodies — the
            // legacy tokenizer has no brace tokens.
            let scheme = parse_arrow_expr(expr_str)?;
            map.insert(name, scheme);
        }
    }

    if map.is_empty() {
        return Err("no named arrows found".to_string());
    }

    Ok(map)
}

// ============================================================================
// Tokenizer (for simple arrow-graph syntax — legacy fallback)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
enum Token {
    LParen,
    RParen,
    Comma,
    StringLit(String),
    Ident(String),
    /// `>>>` — sequence (flow) operator, desugars to `Seq`.
    Flow,
    /// `|||` — choice operator, desugars to `Choice`.
    ChoiceOp,
}

fn tokenize(src: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = src.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            ',' => {
                tokens.push(Token::Comma);
                chars.next();
            }
            '>' => {
                // Expect ">>>" (the flow / sequence operator)
                chars.next(); // consume first '>'
                if chars.next() != Some('>') {
                    return Err("expected '>>>' but got incomplete '>'".to_string());
                }
                if chars.next() != Some('>') {
                    return Err("expected '>>>' but got '>>'".to_string());
                }
                tokens.push(Token::Flow);
            }
            '~' => {
                // v0.1: `~>` is the arrow operator. In the legacy tokenizer
                // (fallback only), treat it as a no-op separator — the
                // data-driven parser handles it properly. This prevents
                // "unexpected character: '~'" errors on v0.1 syntax.
                chars.next(); // consume '~'
                if chars.peek() == Some(&'>') {
                    chars.next(); // consume '>'
                }
                // Don't push any token — `~>` separates params from body,
                // and the legacy parser doesn't model arrow lambdas.
            }
            '|' => {
                // Expect "|||" (the choice operator)
                chars.next(); // consume first '|'
                if chars.next() != Some('|') {
                    return Err("expected '|||' but got incomplete '|'".to_string());
                }
                if chars.next() != Some('|') {
                    return Err("expected '|||' but got '||'".to_string());
                }
                tokens.push(Token::ChoiceOp);
            }
            '"' => {
                chars.next(); // skip opening quote
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '"' {
                        chars.next();
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                tokens.push(Token::StringLit(s));
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '.' {
                        s.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(s));
            }
            _ => {
                return Err(format!("unexpected character: '{}'", c));
            }
        }
    }

    Ok(tokens)
}

// ============================================================================
// Recursive descent parser (for simple arrow-graph syntax — legacy fallback)
// ============================================================================

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&Token> {
        let t = self.tokens.get(self.pos);
        self.pos += 1;
        t
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        match self.next() {
            Some(t) if t == expected => Ok(()),
            Some(t) => Err(format!("expected {:?}, got {:?}", expected, t)),
            None => Err("unexpected end of input".to_string()),
        }
    }

    /// Parse a full arrow expression (top-level entry point).
    fn parse_expr(&mut self) -> Result<TermData, String> {
        self.parse_seq()
    }

    /// Parse a sequence expression: `choice_expr (">>>" choice_expr)*`.
    fn parse_seq(&mut self) -> Result<TermData, String> {
        let mut left = self.parse_choice_expr()?;
        while self.peek() == Some(&Token::Flow) {
            self.next(); // consume >>>
            let right = self.parse_choice_expr()?;
            left = TermData::seq(left, right);
        }
        Ok(left)
    }

    /// Parse a choice expression: `atom ("|||" atom)*`.
    fn parse_choice_expr(&mut self) -> Result<TermData, String> {
        let mut left = self.parse_atom()?;
        while self.peek() == Some(&Token::ChoiceOp) {
            self.next(); // consume |||
            let right = self.parse_atom()?;
            left = TermData::choice(left, right);
        }
        Ok(left)
    }

    /// Parse an atom: the simplest unit of an arrow expression.
    fn parse_atom(&mut self) -> Result<TermData, String> {
        match self.next() {
            Some(Token::LParen) => {
                let inner = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(inner)
            }
            Some(Token::Ident(name)) => match name.as_str() {
                "id" => Ok(arr_native_td("id", vec![])),
                "arr" => self.parse_arr(),
                "app" => self.parse_app(),
                "comp" => self.parse_comp(),
                "first" => self.parse_first(),
                "choice" => self.parse_choice(),
                "loop" => self.parse_loop(),
                "rec" => self.parse_loop(), // v0.1: rec(body) alias for loop(body)
                _ => Ok(TermData::app(arr_native_td(name, vec![]), vec![])),
            },
            Some(t) => Err(format!("expected identifier, got {:?}", t)),
            None => Err("unexpected end of input".to_string()),
        }
    }

    fn parse_arr(&mut self) -> Result<TermData, String> {
        self.expect(&Token::LParen)?;
        let name = match self.next() {
            Some(Token::StringLit(s)) => s.clone(),
            other => return Err(format!("expected string literal, got {:?}", other)),
        };
        let args = self.parse_optional_args()?;
        self.expect(&Token::RParen)?;
        Ok(arr_native_td(name, args))
    }

    fn parse_app(&mut self) -> Result<TermData, String> {
        self.expect(&Token::LParen)?;
        let name = match self.next() {
            Some(Token::StringLit(s)) => s.clone(),
            other => return Err(format!("expected string literal, got {:?}", other)),
        };
        let args = self.parse_optional_args()?;
        self.expect(&Token::RParen)?;
        Ok(TermData::app(arr_native_td(name, vec![]), args))
    }

    fn parse_comp(&mut self) -> Result<TermData, String> {
        self.expect(&Token::LParen)?;
        let fst = self.parse_expr()?;
        self.expect(&Token::Comma)?;
        let snd = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(TermData::seq(fst, snd))
    }

    fn parse_first(&mut self) -> Result<TermData, String> {
        self.expect(&Token::LParen)?;
        let inner = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(TermData::first(inner))
    }

    fn parse_choice(&mut self) -> Result<TermData, String> {
        self.expect(&Token::LParen)?;
        let l = self.parse_expr()?;
        self.expect(&Token::Comma)?;
        let r = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(TermData::choice(l, r))
    }

    fn parse_loop(&mut self) -> Result<TermData, String> {
        self.expect(&Token::LParen)?;
        let body = self.parse_expr()?;
        self.expect(&Token::RParen)?;
        Ok(TermData::loop_feedback(body))
    }

    fn parse_optional_args(&mut self) -> Result<Vec<TermData>, String> {
        if self.peek() == Some(&Token::Comma) {
            self.next(); // consume comma
            let mut args = vec![self.parse_expr()?];
            while self.peek() == Some(&Token::Comma) {
                self.next();
                args.push(self.parse_expr()?);
            }
            Ok(args)
        } else {
            Ok(vec![])
        }
    }
}

/// Build a native `Op` [`TermData`] (the data-driven counterpart of
/// `Term::arr_native`): `Op { name, args, source: Native, lit: null }`.
fn arr_native_td(name: impl Into<String>, args: Vec<TermData>) -> TermData {
    TermData::op(name, args, &ArrSource::Native, None)
}
