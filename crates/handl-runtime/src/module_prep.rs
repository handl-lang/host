//! Text preparation for canonical `.hnl` modules loaded by the runtime.
//!
//! This is a lightweight mirror of the export-to-object rewrite used by
//! `handl_plugin_lib_handl` so that `ModuleLoader` can evaluate
//! multi-export package modules without depending on the higher-level
//! `handl_plugin_lib_handl` crate (which itself depends on `handl_runtime`).
//!
//! The rewrite is **structure-aware**: the source is segmented into
//! top-level statements with a bracket-depth/string/comment scanner
//! ([`top_level_segments`]), so an `export` declaration is recognized at
//! any statement position — after a newline, after `;`, or mid-line
//! following another statement — and its `:=` RHS is captured whole no
//! matter how the author laid out newlines inside `do {…}`/`if`/`else`
//! blocks. A construct that cannot be transformed (an `export`
//! declaration without a `:=`/`:` separator) is a hard
//! [`LangError`] — module prep never silently drops a declaration.

use handl_language::LangError;
use hnl_exec::coverage::OriginLineMap;

/// A prepared module plus the provenance map that resolves its line
/// numbers back to the input source's lines.
///
/// `line_map[i]` is the 1-based line in the *input* source that prepared
/// line `i + 1` derives from; `None` marks generated lines (the `do {`
/// wrapper, the export projection record, erased `null` bindings, …).
/// `origin_lines` is the input's total line count.
#[derive(Debug)]
pub struct PreparedModule {
    /// The prepared source — the exact text the parser sees.
    pub source: String,
    /// Prepared-line → input-line provenance (see above).
    pub line_map: Vec<Option<u32>>,
    /// Total lines in the input source.
    pub origin_lines: u32,
    /// Whether the source declares any `export` (including `export data`,
    /// `from "…" export …` re-exports, host-module `export` imports, and
    /// `op "…"` declarations). Star imports (`import *`, `import * as`,
    /// bare `import "…"`) bind only the declared-export record — a module
    /// with `has_exports == false` yields an empty namespace even though
    /// its evaluated value may be an all-bindings/trailing record (I-007).
    pub has_exports: bool,
}

/// Prepare a module source for evaluation: strip comments and declarations,
/// then rewrite top-level `export` declarations into a single object literal.
///
/// `is_file_spec` reports whether an import specifier resolves to a file —
/// those are the executable import op (ADR-0096) and must survive prep.
/// Specifiers naming host modules (`from "str" import …`, `import <name>`)
/// are decorative declarations and are dropped.
///
/// `is_bindable` reports whether a name can be a `:=` binding target —
/// i.e. lexes as `token.identifier`, not a keyword like `first`. Keyword
/// names can't be bound; their export field carries the expression inline
/// under a quoted record key (`{ "first": expr }`).
///
/// Returns an error when a top-level `export` declaration cannot be
/// transformed (e.g. `export` with no binding separator) — a declaration
/// that would previously vanish into an empty module record.
pub fn prepare_module(
    src: &str,
    is_file_spec: &dyn Fn(&str) -> bool,
    is_bindable: &dyn Fn(&str) -> bool,
) -> Result<String, LangError> {
    prepare_module_mapped(src, is_file_spec, is_bindable).map(|p| p.source)
}

/// [`prepare_module`] plus a prepared-line → input-line provenance map
/// (see [`PreparedModule`]). Coverage uses the map to report file lines
/// instead of prepared-source coordinates.
pub fn prepare_module_mapped(
    src: &str,
    is_file_spec: &dyn Fn(&str) -> bool,
    is_bindable: &dyn Fn(&str) -> bool,
) -> Result<PreparedModule, LangError> {
    let origin_lines = src.lines().count() as u32;
    let (src, map_comments) = strip_comment_lines_mapped(src);
    let op_names = collect_op_decl_names(&src);
    let (src, map_decls) = strip_declaration_lines_mapped(&src);
    let (prepared, mut map, has_exports) =
        transform_export_to_object(&src, is_file_spec, &op_names, is_bindable)?;
    // Compose the stage maps: `map` entries are lines of the
    // decl-stripped source — resolve each through `map_decls` (→
    // comment-stripped line) then `map_comments` (→ original line).
    for entry in map.iter_mut() {
        *entry = entry.and_then(|l| {
            let decl_line = *map_decls.get(l as usize - 1)?;
            map_comments.get(decl_line as usize - 1).copied()
        });
    }
    Ok(PreparedModule {
        source: prepared,
        line_map: map,
        origin_lines,
        has_exports,
    })
}

/// The 1-based line number containing byte `offset` in `src` (a byte
/// offset need not be a char boundary — only `\n` bytes are counted).
pub fn line_number_at(src: &str, offset: usize) -> u32 {
    (src.bytes()
        .take(offset.min(src.len()))
        .filter(|&b| b == b'\n')
        .count()
        + 1) as u32
}

/// Collect the names a module source exports — the keys of the
/// declared-export record star imports bind — without preparing or
/// evaluating it. Hosts use this to learn which names a spliced
/// `import "<spec>"` / `from "<spec>" import *` binds (e.g. seeding a
/// pre-run typecheck scope).
///
/// Only *declared* exports are reported (I-007): a module with no
/// `export` declarations yields an empty list even though its evaluated
/// value may be an all-bindings or trailing-record value — star imports
/// bind the empty namespace, not that record.
pub fn module_export_names(src: &str) -> Vec<String> {
    let src = strip_comment_lines(src);
    let op_names = collect_op_decl_names(&src);
    let src = strip_declaration_lines(&src);
    let mut exported: Vec<String> = Vec::new();
    for seg in top_level_segments(&src) {
        let trimmed = seg.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_import_line(trimmed) {
            // `from "<spec>" export a, b` re-exports `a`/`b`; a
            // `from "<host-mod>" export a` line exports `a` bound to the
            // qualified name `"mod.a"`. Either way the declared names are
            // module exports.
            if let Some((_, reexports)) = from_export_to_import(trimmed) {
                exported.extend(reexports.iter().map(|(name, _)| name.clone()));
            } else {
                for (local, _, exp) in host_import_bindings(trimmed) {
                    if exp {
                        exported.push(local.clone());
                    }
                }
            }
            continue;
        }
        if let Some(rest) = export_decl_rest(trimmed) {
            if is_data_decl_rest(rest) {
                // `export data F := …` exports the family and each
                // constructor the declaration binds.
                if let Some(names) = data_decl_bound_names(rest) {
                    exported.extend(names);
                }
            } else if let Some((sep_pos, _)) = find_binding_sep(rest) {
                let name = strip_export_name(&rest[..sep_pos]);
                exported.push(name.clone());
            }
            // A separator-less `export` is skipped here (name seeding is
            // best-effort); `prepare_module` reports it as an error.
        }
    }
    for qualified in op_names {
        let local = qualified.rsplit('.').next().unwrap_or(&qualified);
        if !local.is_empty() && is_valid_ident(local) && !exported.iter().any(|n| n == local) {
            exported.push(local.to_string());
        }
    }
    exported
}

/// Collect the qualified names from `op "<name>" : ...` declaration lines
/// (e.g. `op "bytes.from_list"` → `bytes.from_list`).
fn collect_op_decl_names(src: &str) -> Vec<String> {
    src.lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let rest = trimmed.strip_prefix("op")?;
            if !rest.starts_with(' ') && !rest.starts_with('\t') && !rest.starts_with('"') {
                return None;
            }
            let rest = rest.trim_start();
            let quote = rest.chars().next()?;
            if quote != '"' && quote != '\'' {
                return None;
            }
            let after = &rest[quote.len_utf8()..];
            let end = after.find(quote)?;
            Some(after[..end].to_string())
        })
        .collect()
}

/// Drop the lines `keep` rejects, returning the filtered text plus a
/// map from each output line (1-based) to its input line (1-based).
/// Line filtering is the only transformation these stages perform, so
/// provenance is exact — this is what makes file-line coverage possible
/// even though the *text* is re-serialized.
fn strip_lines(src: &str, keep: impl Fn(&str) -> bool) -> (String, Vec<u32>) {
    let mut out = String::new();
    let mut map = Vec::new();
    // Separator tracking must be a line count, not `out.is_empty()` —
    // a kept *blank* line contributes no bytes, and keying on emptiness
    // would collapse it (and desync the map from the text).
    let mut first = true;
    for (i, line) in src.lines().enumerate() {
        if keep(line) {
            if !first {
                out.push('\n');
            }
            first = false;
            out.push_str(line);
            map.push(i as u32 + 1);
        }
    }
    (out, map)
}

/// Strip `--` comment lines from a `.hnl` source.
fn strip_comment_lines(src: &str) -> String {
    strip_comment_lines_mapped(src).0
}

/// [`strip_comment_lines`] with a output-line → input-line map.
fn strip_comment_lines_mapped(src: &str) -> (String, Vec<u32>) {
    strip_lines(src, |line| !line.trim_start().starts_with("--"))
}

/// Strip `cap`/`op`/`handl` declaration lines and their indented
/// continuation lines from a `.hnl` source.
fn strip_declaration_lines(src: &str) -> String {
    strip_declaration_lines_mapped(src).0
}

/// [`strip_declaration_lines`] with a output-line → input-line map.
fn strip_declaration_lines_mapped(src: &str) -> (String, Vec<u32>) {
    strip_lines(src, |line| {
        let trimmed = line.trim_start();
        if line.starts_with(' ') || line.starts_with('\t') {
            if trimmed.starts_with("guarantee")
                || trimmed.starts_with("requires")
                || trimmed.starts_with("termination")
            {
                return false;
            }
        }
        for kw in ["cap", "op", "handl"] {
            if let Some(rest) = trimmed.strip_prefix(kw) {
                if rest.is_empty()
                    || rest.starts_with(' ')
                    || rest.starts_with('\t')
                    || rest.starts_with('"')
                {
                    return false;
                }
            }
        }
        true
    })
}

/// A chunk of emitted text plus the intermediate-source line (1-based,
/// in the post-strip source [`transform_export_to_object`] sees) its
/// first line derives from. `None` = generated text with no source
/// origin (the `null` erasure of type-only exports, `op`-decl string
/// bindings whose declaration lines were stripped).
#[derive(Clone)]
struct Piece {
    text: String,
    line: Option<u32>,
}

impl Piece {
    fn new(text: impl Into<String>, line: Option<u32>) -> Self {
        Self {
            text: text.into(),
            line,
        }
    }
}

/// One top-level item of the prepared `do { … }` body, in source order.
///
/// Most items are `name := expr` assignments; `export data …`
/// declarations are *verbatim statements* instead — the desugar pass
/// expands them into former/constructor/recursor assigns, so they have
/// no `name := expr` surface form to rewrite into.
enum PrepItem {
    /// `name := expr` — emitted as a do-block assignment; `exported`
    /// names join the trailing record projection. `stmt_line` is the
    /// intermediate-source line the binding *name* sits on (the
    /// `name :=` prefix shares the first expr line's prepared line, so
    /// it needs its own provenance when the RHS wrapped).
    Bind {
        name: String,
        stmt_line: Option<u32>,
        pieces: Vec<Piece>,
        exported: bool,
    },
    /// A verbatim declaration statement (`export data F := …` — the
    /// text keeps its `data` keyword). `exports` lists the names the
    /// declaration binds that join the module record (the family name
    /// and each constructor name; `F.rec` is dotted and cannot be a
    /// record field).
    Decl {
        pieces: Vec<Piece>,
        exports: Vec<String>,
    },
}

impl PrepItem {
    /// The statement text lines accumulated so far — the continuation
    /// sink for wrapped expressions/decls.
    fn pieces_mut(&mut self) -> &mut Vec<Piece> {
        match self {
            PrepItem::Bind { pieces, .. } | PrepItem::Decl { pieces, .. } => pieces,
        }
    }

    /// Read-only view of the accumulated pieces — used by the
    /// continuation check to see whether the item's expression is
    /// still incomplete.
    fn pieces(&self) -> &[Piece] {
        match self {
            PrepItem::Bind { pieces, .. } | PrepItem::Decl { pieces, .. } => pieces,
        }
    }
}

/// Transform top-level `export name <:=|:> expr` declarations into an
/// object literal.  Type-only `export Name: Type` declarations are given the
/// value `null` so that type expressions are not evaluated at runtime. The
/// same erasure applies to signature exports `export Name := A ~[N]~> B`
/// whose RHS is a bare graded-arrow signature ([`is_signature_rhs`]): its
/// boundary children are type expressions that exec must not evaluate.
/// `export data F := …` declarations are preserved verbatim and export
/// the family/constructor names they bind ([`data_decl_bound_names`]).
///
/// The source is consumed as top-level *statements*
/// ([`top_level_segments`]) — not lines — so an `export` is recognized
/// wherever a statement can begin: at column 0, indented, after `;`, or
/// mid-line after another statement; and its RHS expression is captured
/// whole regardless of how newlines fall inside `~>`-chained `do {…}` /
/// `if`/`else` bodies. A separator-less `export` declaration is a
/// [`LangError`] — declarations are never dropped silently.
/// Prepared output text plus its provenance map — every `push` records
/// the origin line of the text's first line (see [`OriginLineMap`]).
struct PreparedOut {
    text: String,
    map: OriginLineMap,
}

impl PreparedOut {
    fn new() -> Self {
        Self {
            text: String::new(),
            map: OriginLineMap::new(),
        }
    }

    /// Append `text`; `first` is the origin line of `text`'s first line
    /// (`None` for generated text).
    fn push(&mut self, text: &str, first: Option<u32>) {
        self.text.push_str(text);
        self.map.push(text, first);
    }

    /// Append `pieces` joined by `\n` — verbatim declaration bodies.
    fn push_pieces(&mut self, pieces: &[Piece]) {
        for (i, p) in pieces.iter().enumerate() {
            if i > 0 {
                self.push("\n", None);
            }
            self.push(&p.text, p.line);
        }
    }

    /// Append `pieces` joined by `\n` and `.trim()`ed — binding RHS
    /// text. Trimming the joined string only touches the first piece's
    /// leading edge and the last piece's trailing edge.
    fn push_expr_pieces(&mut self, pieces: &[Piece]) {
        let n = pieces.len();
        for (i, p) in pieces.iter().enumerate() {
            if i > 0 {
                self.push("\n", None);
            }
            let text = if n == 1 {
                p.text.trim()
            } else if i == 0 {
                p.text.trim_start()
            } else if i == n - 1 {
                p.text.trim_end()
            } else {
                p.text.as_str()
            };
            self.push(text, p.line);
        }
    }
}

fn transform_export_to_object(
    src: &str,
    is_file_spec: &dyn Fn(&str) -> bool,
    op_names: &[String],
    is_bindable: &dyn Fn(&str) -> bool,
) -> Result<(String, Vec<Option<u32>>, bool), LangError> {
    // Line index for provenance: piece origins are computed as
    // `line_of(byte_offset)` where offsets come from subslice pointer
    // arithmetic against `src` (every `rest`/`trimmed`/`rhs` below is a
    // subslice of `src` via `seg`).
    let mut line_starts = vec![0usize];
    for (i, &b) in src.as_bytes().iter().enumerate() {
        if b == b'\n' {
            line_starts.push(i + 1);
        }
    }
    let line_of = |off: usize| -> u32 { line_starts.partition_point(|&s| s <= off).max(1) as u32 };
    let src_ptr = src.as_ptr() as usize;
    let offset_of = |sub: &str| sub.as_ptr() as usize - src_ptr;

    // Each top-level item is collected in source order. All bindings
    // become `do`-block assignments (`name := expr;`) emitted *before*
    // the object so closures can reference siblings — exported or not.
    // The object literal then projects the exported names
    // (`{ name: name }`).
    let mut items: Vec<PrepItem> = Vec::new();
    // Names bound by file-resolving `from "f" import a, b as c` /
    // `import n from "f"` statements — bound at eval time by the import
    // itself, so they join the no-exports record projection without a
    // prelude line.
    let mut import_bound: Vec<String> = Vec::new();
    // Whether the module's last top-level statement is a binding. When it
    // is not (e.g. a trailing `do { … }` or record literal) the body's own
    // value is the module record and no name projection is appended.
    let mut tail_is_binding = true;
    let mut current: Option<PrepItem> = None;

    // Finalize `current` into the ordered item list.
    macro_rules! finalize {
        () => {
            if let Some(entry) = current.take() {
                items.push(entry);
            }
        };
    }

    for &(seg_start, seg_end) in &top_level_segment_spans(src) {
        let seg = &src[seg_start..seg_end];
        let seg_line = line_of(seg_start);
        let trimmed = seg.trim();
        if trimmed.is_empty() {
            continue;
        }
        // A segment may open with a whitespace-only first line — the
        // trimmed statement's first line is `trimmed`'s own offset, not
        // necessarily `seg_line`.
        let trimmed_line = line_of(offset_of(trimmed));
        if is_import_line(trimmed) {
            finalize!();
            // Only file-resolving specs can re-export; module-name specs
            // name host modules (ADR-0096).
            let file_spec = import_line_spec(trimmed).map_or(false, |s| is_file_spec(&s));
            if file_spec {
                if let Some((_, names)) = from_export_to_import(trimmed) {
                    for (name, alias) in names {
                        items.push(PrepItem::Bind {
                            name,
                            stmt_line: Some(trimmed_line),
                            pieces: vec![Piece::new(alias, Some(trimmed_line))],
                            exported: true,
                        });
                    }
                } else {
                    // `from "f" import a, b as c` binds locals in the
                    // module's scope; a module with no `export` decls
                    // exports every bound name, so collect them for the
                    // record projection (they need no prelude line — the
                    // import statement itself binds them).
                    for local in import_bound_locals(trimmed) {
                        if is_valid_ident(&local)
                            && !items.iter().any(|item| match item {
                                PrepItem::Bind { name, .. } => *name == local,
                                PrepItem::Decl { .. } => false,
                            })
                        {
                            import_bound.push(local);
                        }
                    }
                }
            } else {
                // Host-module import: named imports become private
                // bindings of the qualified-name string (`slice :=
                // "bytes.slice"`), so `slice(x)` dispatches through
                // `call_host` → the module chain exactly like the
                // ambient dotted form `bytes.slice(x)`. `from "m"
                // export n` re-exports `"m.n"`. `* as alias` binds the
                // bare module name string (`alias.f` → `"m.f"`).
                for (local, expr, exported) in host_import_bindings(trimmed) {
                    items.push(PrepItem::Bind {
                        name: local,
                        stmt_line: Some(trimmed_line),
                        pieces: vec![Piece::new(expr, Some(trimmed_line))],
                        exported,
                    });
                }
            }
            continue;
        }
        if let Some(rest) = export_decl_rest(trimmed) {
            finalize!();
            // `export data F := …` — a family declaration, not a
            // `name := expr` binding: keep the decl verbatim (its
            // `data` keyword included — the desugar pass consumes the
            // stray `var "data"` marker) and export the names it binds.
            if is_data_decl_rest(rest) {
                let Some(exports) = data_decl_bound_names(rest) else {
                    return Err(LangError::engine(format!(
                        "module prep: cannot read the family/constructor \
                         names of `export data` declaration: {}",
                        trimmed
                    )));
                };
                current = Some(PrepItem::Decl {
                    pieces: vec![Piece::new(rest.trim(), Some(line_of(offset_of(rest))))],
                    exports,
                });
                tail_is_binding = true;
                continue;
            }
            let Some((sep_pos, sep_len)) = find_binding_sep(rest) else {
                // Never drop an export declaration silently — a module
                // that reaches eval with a vanished export produces an
                // empty record (`{}`) or an "unbound name" error far
                // from the cause.
                return Err(LangError::engine(format!(
                    "module prep: cannot transform `export` declaration \
                     (expected `export <name> <:=|: > …`): {}",
                    trimmed
                )));
            };
            let name = strip_export_name(&rest[..sep_pos]);
            if name.is_empty() {
                return Err(LangError::engine(format!(
                    "module prep: `export` declaration has no binding name: {}",
                    trimmed
                )));
            }
            // The binding name's line: first non-whitespace byte of the
            // pre-separator text (after the `export` keyword).
            let name_src = &rest[..sep_pos];
            let name_off = offset_of(rest) + (name_src.len() - name_src.trim_start().len());
            let stmt_line = Some(line_of(name_off));
            let rhs = rest[sep_pos + sep_len..].trim();
            let is_type_only = (sep_len == 1 && rest.as_bytes().get(sep_pos) == Some(&b':'))
                || is_signature_rhs(rhs);
            let mut pieces: Vec<Piece> = Vec::new();
            if is_type_only {
                // `null` is generated — attribute it to the decl line.
                pieces.push(Piece::new("null", stmt_line));
            } else if !rhs.is_empty() {
                pieces.push(Piece::new(rhs, Some(line_of(offset_of(rhs)))));
            }
            current = Some(PrepItem::Bind {
                name,
                stmt_line,
                pieces,
                exported: true,
            });
            tail_is_binding = true;
            continue;
        }
        // A statement that is not an import or `export` declaration.
        // Detect a *private* (non-exported) top-level binding so that
        // helper functions are preserved instead of dropped.
        if let Some((sep_pos, sep_len)) = find_binding_sep(trimmed) {
            let name = strip_export_name(&trimmed[..sep_pos]).trim().to_string();
            if is_valid_ident(&name) {
                finalize!();
                // The name starts the (already trimmed) statement.
                let stmt_line = Some(trimmed_line);
                let is_type_only = sep_len == 1 && trimmed.as_bytes().get(sep_pos) == Some(&b':');
                let rhs = trimmed[sep_pos + sep_len..].trim();
                let mut pieces: Vec<Piece> = Vec::new();
                if is_type_only {
                    pieces.push(Piece::new("null", stmt_line));
                } else if !rhs.is_empty() {
                    pieces.push(Piece::new(rhs, Some(line_of(offset_of(rhs)))));
                }
                current = Some(PrepItem::Bind {
                    name,
                    stmt_line,
                    pieces,
                    exported: false,
                });
                tail_is_binding = true;
                continue;
            }
        }
        // A non-binding segment that cannot continue the in-flight
        // item is a new top-level statement. When it is the module's
        // last statement and is itself a bare expression (`do { … }`,
        // a record literal, a call) its value is the module record —
        // no `{ name: name }` projection is appended. Segments that
        // *do* continue (a `}`/`)` closer, `.field`, an operator-led
        // operand, `else { … }`, or the `{ … }` body of a pending
        // `if`/`match`/`do`) join the item's expression verbatim.
        // Unconditionally absorbing the segment kept `tail_is_binding`
        // true, so a binding followed by a trailing `do { … }` record
        // exported `{ name: name }` and silently dropped the record —
        // e.g. `from "char" import * as char` lowers to `char :=
        // "char"`, after which lib-pred's trailing record evaluated to
        // `{ "char": "char" }` instead of the combinator table.
        let continues = match &current {
            Some(item) => segment_continues_item(item.pieces(), trimmed),
            None => false,
        };
        if continues {
            if let Some(item) = &mut current {
                item.pieces_mut().push(Piece::new(seg, Some(seg_line)));
            }
        } else {
            finalize!();
            tail_is_binding = false;
        }
    }
    finalize!();

    // `op "mod.name" : T` declarations name host-registered operations.
    // Emit each as an exported binding of the qualified-name *string*: a
    // string callee dispatches through `call_host` → the module chain →
    // the op's native impl, so an interface-only module's exports stay
    // callable when it is imported as a file. The `op` lines were
    // stripped before this stage, so the bindings are fully generated —
    // no provenance.
    for qualified in op_names {
        let local = qualified.rsplit('.').next().unwrap_or(qualified);
        if local.is_empty()
            || !is_valid_ident(local)
            || items.iter().any(|item| match item {
                PrepItem::Bind { name, .. } => name == local,
                PrepItem::Decl { .. } => false,
            })
        {
            continue;
        }
        items.push(PrepItem::Bind {
            name: local.to_string(),
            stmt_line: None,
            pieces: vec![Piece::new(format!("\"{qualified}\""), None)],
            exported: true,
        });
    }

    let has_exports = items.iter().any(|item| match item {
        PrepItem::Bind { exported, .. } => *exported,
        PrepItem::Decl { exports, .. } => !exports.is_empty(),
    });

    let imports = collect_executable_import_pieces(src, is_file_spec, &line_starts);

    // No `export` declarations: the module record is *every* top-level
    // bound name. The Pratt parser reads one statement per `parse` call,
    // so the body must be a single `do { … }` — executable path imports
    // (`import "f.hnl"`, `from "./f" import …`) hoisted first so their
    // names scope over the body, the verbatim statements next (private
    // bindings stay `x := …`; host-module declarations lower to
    // `name := "spec.name"` string bindings), then the record projecting
    // every bound name.
    if !has_exports {
        let body = drop_all_import_pieces(src, is_file_spec, &line_starts);
        let mut out = PreparedOut::new();
        out.push("do {\n", None);
        for imp in &imports {
            out.push(&imp.text, imp.line);
            out.push("\n", None);
        }
        out.push_pieces(&body);
        // The projection is appended only when the module's last top-level
        // statement is a binding; a trailing `do { … }`/record literal is
        // already the module's interface value.
        if tail_is_binding {
            let mut seen = std::collections::HashSet::new();
            // Emit `{ ` lazily on the first field so an empty projection
            // contributes nothing (mirrors `record.is_empty()`).
            let mut record_open = false;
            let mut push_field =
                |out: &mut PreparedOut, name: &str, pieces: &[Piece], line: Option<u32>| {
                    if !seen.insert(name.to_string()) {
                        return;
                    }
                    if !record_open {
                        out.push("\n{ ", None);
                        record_open = true;
                    } else {
                        out.push(", ", None);
                    }
                    if is_bindable(name) {
                        out.push(&format!("{name}: {name}"), line);
                    } else {
                        out.push(&format!("\"{name}\": "), line);
                        out.push_expr_pieces(pieces);
                    }
                };
            for item in &items {
                match item {
                    PrepItem::Bind {
                        name,
                        stmt_line,
                        pieces,
                        ..
                    } => push_field(&mut out, name, pieces, *stmt_line),
                    PrepItem::Decl { pieces, exports } => {
                        let decl_line = pieces.first().and_then(|p| p.line);
                        for name in exports {
                            push_field(&mut out, name, &[], decl_line);
                        }
                    }
                }
            }
            for name in &import_bound {
                if is_bindable(name) {
                    push_field(&mut out, name, &[], None);
                }
            }
            if record_open {
                out.push(" }", None);
            }
        }
        out.push("\n}", None);
        return Ok((out.text, out.map.into_vec(), false));
    }

    // Build the body: every binding as a do-block assignment in source
    // order, then the export object literal projecting exported names.
    // A name that isn't a bindable identifier (e.g. the keyword `first` —
    // `export first := ...` / `from "./first" export first`) can't be a
    // `:=` target, so it gets no binding; its record field carries the
    // expression inline under a quoted key (`{ "first": __first }`).
    let mut out = PreparedOut::new();
    out.push("do {\n", None);
    if !imports.is_empty() {
        for (i, imp) in imports.iter().enumerate() {
            if i > 0 {
                out.push("\n", None);
            }
            out.push(&imp.text, imp.line);
        }
        out.push("\n", None);
    }
    for item in &items {
        match item {
            PrepItem::Bind {
                name,
                stmt_line,
                pieces,
                ..
            } => {
                if !is_bindable(name) {
                    continue;
                }
                out.push(name, *stmt_line);
                out.push(" := ", None);
                out.push_expr_pieces(pieces);
                out.push(";\n", None);
            }
            // `export data …` — the verbatim declaration statement; the
            // desugar pass expands it into the family/ctor/recursor
            // assigns inside this same `do` block.
            PrepItem::Decl { pieces, .. } => {
                out.push_pieces(pieces);
                out.push(";\n", None);
            }
        }
    }
    out.push("{ ", None);
    let mut first = true;
    for item in &items {
        let (names, pieces, stmt_line, exported): (Vec<&str>, &[Piece], Option<u32>, bool) =
            match item {
                PrepItem::Bind {
                    name,
                    stmt_line,
                    pieces,
                    exported,
                } => (
                    vec![name.as_str()],
                    pieces.as_slice(),
                    *stmt_line,
                    *exported,
                ),
                PrepItem::Decl { pieces, exports } => (
                    exports.iter().map(String::as_str).collect(),
                    &[],
                    pieces.first().and_then(|p| p.line),
                    true,
                ),
            };
        if !exported {
            continue;
        }
        for name in names {
            if !first {
                out.push(", ", None);
            }
            first = false;
            if is_bindable(name) {
                out.push(&format!("{name}: {name}"), stmt_line);
            } else {
                out.push(&format!("\"{name}\": "), stmt_line);
                out.push_expr_pieces(pieces);
            }
        }
    }
    out.push(" }", None);
    out.push("\n}", None);
    Ok((out.text, out.map.into_vec(), has_exports))
}

/// Whether a non-binding top-level `seg` continues the in-flight item
/// (`name := …` / `export data …`) instead of opening a new statement.
///
/// The emitted text is verbatim either way — this decides only whether
/// the item's expression is *complete* (so a trailing bare expression
/// keeps its own module-record value) and, for `export` modules,
/// whether the segment is absorbed into the binding's RHS.
///
/// A segment continues when:
/// - it opens with a fragment token that cannot begin a statement — a
///   closer/delimiter (`.`, `)`, `]`, `}`, `,`, `;`, `:`), an operator
///   or arrow (`+`, `==`, `~>`, `->`, `=>`, `|>`, …), or a continuation
///   keyword (`else`, `catch`, `finally`, `and`, `or`, …);
/// - the accumulated expression is *incomplete* — it ends with a binary
///   operator or dangling keyword (`x := a +` ⏎ `b`), or its last
///   block-introducing keyword has not yet seen its `{ … }` body
///   (`x := if cond` ⏎ `{ … } else { … }`).
///
/// `(`, `[`, and `{`-led segments are *not* fragment continuations —
/// the parser does not join `f` ⏎ `(args)` across a newline, and a
/// `{ … }` after a complete expression is a record literal (a valid
/// module-record tail), not a block. They continue only via the
/// incompleteness checks above.
fn segment_continues_item(pieces: &[Piece], seg: &str) -> bool {
    let seg = seg.trim_start();
    let Some(&b) = seg.as_bytes().first() else {
        return false;
    };
    // Fragment-led: punctuation that cannot open a statement.
    if matches!(
        b,
        b'.' | b')'
            | b']'
            | b'}'
            | b','
            | b';'
            | b':'
            | b'?'
            | b'|'
            | b'&'
            | b'='
            | b'<'
            | b'>'
            | b'+'
            | b'*'
            | b'/'
            | b'%'
            | b'^'
            | b'~'
            | b'@'
            | b'#'
            | b'$'
            | b'`'
            | b'-'
            | b'!'
    ) {
        return true;
    }
    // Continuation keywords: `else { … }`, `and`/`or`/`in`/`of`/`as`/…
    // operand tails, `catch`/`finally` handlers.
    let first_word = seg
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()
        .unwrap_or("");
    if matches!(
        first_word,
        "else"
            | "catch"
            | "finally"
            | "then"
            | "elif"
            | "otherwise"
            | "and"
            | "or"
            | "xor"
            | "not"
            | "in"
            | "of"
            | "is"
            | "as"
            | "mod"
            | "div"
            | "rem"
            | "where"
            | "with"
            | "when"
            | "unless"
            | "until"
            | "by"
            | "via"
    ) {
        return true;
    }
    // A `{`-led segment continues a pending block construct
    // (`if cond` ⏎ `{ … }`); otherwise it is a record literal — a new
    // statement whose value can be the module record.
    if b == b'{' && item_expects_block(pieces) {
        return true;
    }
    item_expression_incomplete(pieces)
}

/// Whether the accumulated expression text ends mid-expression — a
/// binary operator, comma, colon, dot, or arrow tail whose operand
/// arrives on the next segment (`x := a +` ⏎ `b`, `f ~>` ⏎ `g`), or a
/// dangling keyword (`… if`, `… else`, `… and`, `… in`, …) still owed
/// its operand/block.
fn item_expression_incomplete(pieces: &[Piece]) -> bool {
    let Some(last) = pieces.last() else {
        return false;
    };
    let tail = last.text.trim_end();
    let Some(&tb) = tail.as_bytes().last() else {
        return false;
    };
    if matches!(
        tb,
        b'+' | b'-'
            | b'*'
            | b'/'
            | b'%'
            | b'='
            | b'<'
            | b'>'
            | b'!'
            | b'&'
            | b'|'
            | b'^'
            | b'~'
            | b'?'
            | b':'
            | b','
            | b'.'
    ) {
        return true;
    }
    if tail.ends_with("->")
        || tail.ends_with("=>")
        || tail.ends_with("~>")
        || tail.ends_with("|>")
        || tail.ends_with("<|")
    {
        return true;
    }
    // The last word of the accumulated expression: a keyword that
    // requires a following operand or block means the expression is
    // still open.
    let word = tail
        .rsplit(|c: char| !(c.is_alphanumeric() || c == '_'))
        .find(|w| !w.is_empty())
        .unwrap_or("");
    matches!(
        word,
        "if" | "else"
            | "match"
            | "while"
            | "for"
            | "handle"
            | "do"
            | "then"
            | "in"
            | "of"
            | "and"
            | "or"
            | "xor"
            | "not"
            | "is"
            | "as"
            | "mod"
            | "div"
            | "rem"
            | "return"
            | "yield"
            | "where"
            | "with"
    )
}

/// Whether the accumulated expression ends inside a construct still
/// owed its `{ … }` block — the last block-introducing keyword
/// (`if`/`match`/`while`/`for`/`handle`/`do`/`else`/`then`/`in`/`of`)
/// has no balanced `{…}` after it. Covers wrapped heads like
/// `x := if cond` ⏎ `{ … } else { … }` and `x := match scrut` ⏎
/// `{ … }`.
///
/// String literals are not stripped: a `"if"` inside a string can
/// produce a false "pending" verdict, which only preserves the old
/// merge behavior — never a wrong split.
fn item_expects_block(pieces: &[Piece]) -> bool {
    const KEYWORDS: &[&str] = &[
        "if", "else", "match", "while", "for", "handle", "do", "then", "in", "of",
    ];
    let text = pieces
        .iter()
        .map(|p| p.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let bytes = text.as_bytes();
    // Byte offset just after the last block keyword (word-boundary
    // scan — keywords inside string literals are not excluded; see the
    // doc comment).
    let mut last_end: Option<usize> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_alphanumeric() || c == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            if KEYWORDS.contains(&&text[start..i]) {
                last_end = Some(i);
            }
        } else {
            i += 1;
        }
    }
    let Some(pos) = last_end else {
        return false;
    };
    // Pending while no `{` has opened after the keyword, or the braces
    // it opened are still unclosed.
    let mut depth = 0i32;
    let mut seen_open = false;
    for &c in &bytes[pos..] {
        match c {
            b'{' => {
                depth += 1;
                seen_open = true;
            }
            b'}' => depth -= 1,
            _ => {}
        }
    }
    !seen_open || depth > 0
}

/// The text after the `export` keyword when `trimmed` begins an export
/// declaration — `None` when `export` does not sit at a word boundary at
/// the start of the statement (`exporter := …` is a binding, not an
/// export).
fn export_decl_rest(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix("export")?;
    if rest.is_empty() {
        return Some(rest);
    }
    let next = rest.as_bytes()[0];
    if !(next.is_ascii_alphanumeric() || next == b'_') {
        Some(rest.trim_start())
    } else {
        None
    }
}

/// Whether `rest` (the text after `export`) opens a `data` family
/// declaration — the `data` keyword followed by the family name.
/// `export data_transfer := …` is an ordinary binding (`data_transfer`
/// is one identifier) and `export data := …` binds the name `data`;
/// neither is a declaration.
fn is_data_decl_rest(rest: &str) -> bool {
    let rest = rest.trim_start();
    let Some(after) = rest.strip_prefix("data") else {
        return false;
    };
    // `data` must end at a word boundary …
    match after.chars().next() {
        Some(c) if c.is_alphanumeric() || c == '_' => return false,
        _ => {}
    }
    // … and be followed by the family name (an identifier).
    after
        .trim_start()
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_')
}

/// Skip a balanced `open…close` group at the start of `s`, returning the
/// remainder. `None` when the group never closes.
fn skip_balanced<'a>(s: &'a str, open: char, close: char) -> Option<&'a str> {
    if !s.starts_with(open) {
        return Some(s);
    }
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    for (i, c) in s.char_indices() {
        if let Some(q) = in_str {
            if c as u32 == q as u32 {
                in_str = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => in_str = Some(c as u8),
            _ if c == open => depth += 1,
            _ if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[i + c.len_utf8()..]);
                }
            }
            _ => {}
        }
    }
    None
}

/// The interior of the balanced `open…close` group at the start of `s`
/// (which must begin with `open`). `None` when the group never closes.
fn balanced_inner<'a>(s: &'a str, open: char, close: char) -> Option<&'a str> {
    if !s.starts_with(open) {
        return None;
    }
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    for (i, c) in s.char_indices() {
        if let Some(q) = in_str {
            if c as u32 == q as u32 {
                in_str = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => in_str = Some(c as u8),
            _ if c == open => depth += 1,
            _ if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[open.len_utf8()..i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split `s` at `sep` characters at bracket depth zero (strings
/// respected) — the `|` constructor separator of a `data F := …` arm
/// list.
fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let bytes = s.as_bytes();
    let sep = sep as u8;
    let mut items = Vec::new();
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = in_str {
            if c == b'\\' {
                i += 1;
            } else if c == q {
                in_str = None;
            }
        } else {
            match c {
                b'"' | b'\'' => in_str = Some(c),
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                _ if c == sep && depth == 0 => {
                    items.push(&s[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
        i += 1;
    }
    items.push(&s[start..]);
    items
}

/// The leading identifier of a constructor arm (`C1(x : T)` → `C1`).
fn ctor_arm_name(arm: &str) -> Option<String> {
    let arm = arm.trim_start();
    let end = arm
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(arm.len());
    let name = &arm[..end];
    is_valid_ident(name).then(|| name.to_string())
}

/// The names a `data` declaration binds, for the module record: the
/// family name plus each constructor name. `F.rec` is a dotted name —
/// it can't be a record field — so only `F` and the bare constructor
/// bindings are projected. `decl` is the full `data …` text (keyword
/// included). `None` when no family name is recognizable — the caller
/// reports rather than silently dropping the declaration.
fn data_decl_bound_names(decl: &str) -> Option<Vec<String>> {
    let mut s = decl.trim_start().strip_prefix("data")?.trim_start();
    // Family name: leading identifier; optional `<…>`/`(…)` parameter
    // lists may follow before the declaration separator.
    let fam_end = s
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(s.len());
    let family = &s[..fam_end];
    if !is_valid_ident(family) {
        return None;
    }
    let mut names = vec![family.to_string()];
    s = s[fam_end..].trim_start();
    if s.starts_with('<') {
        s = skip_balanced(s, '<', '>')?.trim_start();
    } else if s.starts_with('(') {
        s = skip_balanced(s, '(', ')')?.trim_start();
    }
    if let Some(ctors) = s.strip_prefix(":=") {
        let rhs = ctors.trim_start();
        if rhs.starts_with('{') {
            // `data F := { C : R, … }` — the record form's keys are the
            // constructor names.
            if let Some(inner) = balanced_inner(rhs, '{', '}') {
                for entry in split_top_commas(inner) {
                    if let Some(key) = record_field_key(entry) {
                        if is_valid_ident(&key) && !names.iter().any(|n| *n == key) {
                            names.push(key);
                        }
                    }
                }
            }
        } else {
            // `data F := C0 | C1(x : T) | …` — constructor arms (`:=` is
            // the only binding operator; the old `::` spelling is gone).
            for arm in split_top_level(rhs, '|') {
                if let Some(name) = ctor_arm_name(arm) {
                    if !names.iter().any(|n| *n == name) {
                        names.push(name);
                    }
                }
            }
        }
    }
    Some(names)
}

/// Split a module source into its top-level statements, returning each
/// statement's verbatim text (internal newlines and indentation
/// preserved) in source order.
///
/// The module surface has no layout-sensitive constructs, so a
/// statement boundary is recognized purely by structure — never by a
/// hardcoded line shape:
///
/// - `;` at bracket depth 0,
/// - a newline at depth 0 **not** followed by an indented line (an
///   indented line continues the statement — `export f := (x) ~>\n
///   <body>` keeps its wrapped RHS),
/// - an `export`/`import`/`from` keyword at depth 0 mid-statement —
///   several statements sharing a line (`from "x" import a export f :=
///   e`, `export a := e; export b := e2`) split into their statements.
///
/// Inside a `from`/`import` statement the first `import`/`export`/`from`
/// keyword belongs to that statement's tail (`from "x" import a`,
/// `from "x" export a, b`, `import a from "x"`) and does not split.
/// String literals, `--` line comments, and `/-- … --/` block comments
/// are tracked so keywords and brackets inside them are ignored.
pub(crate) fn top_level_segments(src: &str) -> Vec<String> {
    top_level_segment_spans(src)
        .iter()
        .map(|&(s, e)| src[s..e].to_string())
        .collect()
}

/// [`top_level_segments`] as `(start, end)` byte spans into `src` —
/// provenance-aware callers slice `src` directly so piece origins stay
/// exact.
pub(crate) fn top_level_segment_spans(src: &str) -> Vec<(usize, usize)> {
    /// What the statement under construction opened with — decides which
    /// keywords are part of its own tail rather than a new statement.
    #[derive(Clone, Copy, PartialEq)]
    enum Lead {
        /// Nothing but whitespace/comments seen so far.
        Pending,
        From,
        Import,
        Export,
        Other,
    }

    let bytes = src.as_bytes();
    let mut segs: Vec<(usize, usize)> = Vec::new();
    let mut start = 0usize;
    let mut depth: i32 = 0;
    let mut in_str: Option<u8> = None;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut lead = Lead::Pending;
    // Whether the in-flight `from` statement has consumed its
    // `import`/`export` keyword already, and whether an `import`
    // statement has consumed `from`.
    let mut tail_seen = false;

    // Record the accumulated statement `[start, $end)` when non-blank
    // and open the next segment at `$next` with lead `$lead`.
    macro_rules! cut {
        ($end:expr, $next:expr, $lead:expr) => {{
            let text = &src[start..$end];
            if !text.trim().is_empty() {
                segs.push((start, $end));
            }
            start = $next;
            lead = $lead;
            tail_seen = false;
        }};
    }

    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if in_line_comment {
            if c == b'\n' {
                in_line_comment = false;
                // fall through: the newline may end the statement
            } else {
                i += 1;
                continue;
            }
        }
        if in_block_comment {
            if c == b'-' && bytes.get(i + 1) == Some(&b'-') && bytes.get(i + 2) == Some(&b'/') {
                in_block_comment = false;
                i += 3;
            } else {
                i += 1;
            }
            continue;
        }
        if let Some(q) = in_str {
            if c == b'\\' {
                i += 1;
            } else if c == q {
                in_str = None;
            }
            if lead == Lead::Pending {
                lead = Lead::Other;
            }
            i += 1;
            continue;
        }
        if c == b'-' && bytes.get(i + 1) == Some(&b'-') {
            in_line_comment = true;
            i += 2;
            continue;
        }
        if c == b'/' && bytes.get(i + 1) == Some(&b'-') && bytes.get(i + 2) == Some(&b'-') {
            in_block_comment = true;
            if lead == Lead::Pending {
                lead = Lead::Other;
            }
            i += 3;
            continue;
        }
        match c {
            b'"' | b'\'' => {
                in_str = Some(c);
                if lead == Lead::Pending {
                    lead = Lead::Other;
                }
                i += 1;
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                if lead == Lead::Pending {
                    lead = Lead::Other;
                }
                i += 1;
            }
            b')' | b']' | b'}' => {
                depth -= 1;
                if lead == Lead::Pending {
                    lead = Lead::Other;
                }
                i += 1;
            }
            b';' if depth == 0 => {
                cut!(i, i + 1, Lead::Pending);
                i += 1;
            }
            b'\n' if depth == 0 => {
                // Boundary only when the next line is not indented —
                // indented lines are statement continuations.
                let next = bytes.get(i + 1).copied();
                if next != Some(b' ') && next != Some(b'\t') {
                    cut!(i, i + 1, Lead::Pending);
                }
                i += 1;
            }
            _ if depth == 0 && (c.is_ascii_alphabetic() || c == b'_') => {
                // Consume a whole identifier word so keywords inside
                // larger names (`exporter`, `fromage`) never match.
                let mut j = i + 1;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                let word = &src[i..j];
                // A word after `.` is a field name (`char.from_chars`,
                // `x.from`), not a statement keyword.
                let dotted = i > 0 && bytes[i - 1] == b'.';
                let kw = if !dotted && (word == "export" || word == "import" || word == "from") {
                    Some(word)
                } else {
                    None
                };
                match (lead, kw) {
                    (Lead::Pending, Some("export")) => lead = Lead::Export,
                    (Lead::Pending, Some("from")) => lead = Lead::From,
                    (Lead::Pending, Some("import")) => lead = Lead::Import,
                    (Lead::Pending, _) => lead = Lead::Other,
                    // `from "x" import …` / `from "x" export …` — the
                    // first tail keyword belongs to the statement.
                    (Lead::From, Some("import" | "export")) if !tail_seen => {
                        tail_seen = true;
                    }
                    // `import <name> from "spec"` consumes one `from`.
                    (Lead::Import, Some("from")) if !tail_seen => {
                        tail_seen = true;
                    }
                    (_, Some(k)) => {
                        // A statement-keyword at depth 0 mid-statement:
                        // the next statement starts here.
                        let next_lead = match k {
                            "export" => Lead::Export,
                            "from" => Lead::From,
                            _ => Lead::Import,
                        };
                        cut!(i, i, next_lead);
                    }
                    (_, None) => {}
                }
                i = j;
            }
            _ => {
                if lead == Lead::Pending && !c.is_ascii_whitespace() {
                    lead = Lead::Other;
                }
                i += 1;
            }
        }
    }
    let text = &src[start..];
    if !text.trim().is_empty() {
        segs.push((start, src.len()));
    }
    segs
}

/// True when `s` is a plausible Handl identifier: starts with an
/// alphabetic character or underscore, followed by zero or more
/// alphanumeric characters or underscores.
fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' => chars.all(|c| c.is_alphanumeric() || c == '_'),
        _ => false,
    }
}

/// If a `from` line uses the re-export form `from "..." export x, y`, rewrite
/// it to a plain import `from "..." import x as __x, y as __y` and return the
/// (name, local-alias) pairs that should appear in the module record.
fn from_export_to_import(line: &str) -> Option<(String, Vec<(String, String)>)> {
    let (path, rest) = extract_quoted_path(line)?;
    let rest = rest.trim_start();
    if let Some(tail) = rest.strip_prefix("export") {
        if !tail.starts_with(' ') && !tail.starts_with('\t') && !tail.is_empty() {
            return None;
        }
        let names: Vec<String> = tail
            .trim_start()
            .split(',')
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect();
        if names.is_empty() {
            return None;
        }
        let aliases: Vec<String> = names.iter().map(|n| format!("__{n}")).collect();
        let imports: Vec<String> = names
            .iter()
            .zip(&aliases)
            .map(|(n, a)| format!("{} as {}", n, a))
            .collect();
        let import_line = format!("from \"{}\" import {}", path, imports.join(", "));
        return Some((import_line, names.into_iter().zip(aliases).collect()));
    }
    None
}

/// Lower a host-module import declaration to `(local_name, expr,
/// exported)` binding triples whose `expr` is the qualified-name string
/// literal `"spec.name"` — a string callee dispatches through
/// `call_host` → the module chain, mirroring the ambient `spec.name(x)`
/// dotted form.
///
/// - `from "spec" import a, b as x` / `import { a, b as x }` →
///   `a := "spec.a"`, `x := "spec.b"`
/// - `from "spec" import * as m` → `m := "spec"`
/// - `from "spec" export a, b` → exported `a := "spec.a"`
/// - `import <name>` / `import <name> from "spec"` → `name := "spec"`
/// - `import "<spec>"` (side-effect only) → nothing
///
/// Only bare module-name specs (`"bytes"`, `"str"`) produce `spec.name`
/// bindings — a host module is registered under its plain name. A
/// path-like spec (`"../index"`, `"math/add"`, `"x.hnl"`) that failed
/// file resolution is a decorative declaration, not a module name:
/// `"../index.len"` can never dispatch. Those names are left unbound so
/// they keep ambient semantics — an unbound `list` evaluates to `"list"`,
/// which is exactly what `list := "list"` would bind anyway.
pub fn host_import_bindings(line: &str) -> Vec<(String, String, bool)> {
    let trimmed = line.trim_start();
    if let Some((spec, rest)) = extract_quoted_path(trimmed).filter(|_| trimmed.starts_with("from"))
    {
        if !is_module_name_spec(&spec) {
            return Vec::new();
        }
        let rest = rest.trim_start();
        let (tail, exported) = match rest
            .strip_prefix("import")
            .map(|t| (t, false))
            .or_else(|| rest.strip_prefix("export").map(|t| (t, true)))
        {
            Some((t, e)) if t.is_empty() || t.starts_with(' ') || t.starts_with('\t') => {
                (t.trim_start(), e)
            }
            _ => return Vec::new(),
        };
        let tail = tail
            .strip_prefix('{')
            .and_then(|t| t.strip_suffix('}'))
            .map(str::to_string)
            .unwrap_or_else(|| tail.to_string());
        tail.split(',')
            .filter_map(|item| {
                let item = item.trim();
                if item.is_empty() {
                    return None;
                }
                let (imported, local) = match item.split_once(" as ") {
                    Some((n, a)) => (n.trim(), a.trim()),
                    None => (item, item),
                };
                if imported == "*" {
                    // `* as m` binds the module-name string itself.
                    return (local != "*")
                        .then(|| (local.to_string(), format!("\"{spec}\""), exported));
                }
                Some((
                    local.to_string(),
                    format!("\"{spec}.{imported}\""),
                    exported,
                ))
            })
            .collect()
    } else if let Some(rest) = trimmed.strip_prefix("import") {
        let rest = rest.trim_start();
        if rest.starts_with('"') || rest.starts_with('\'') {
            // `import "<spec>"` — side-effect-only declaration; nothing
            // to bind for a host module.
            return Vec::new();
        }
        // `import <name>` or `import <name> from "<spec>"`. A path-like
        // spec isn't a dispatchable module name — self-bind the name so
        // `name.f` resolves to `"name.f"` ambiently.
        let (name, spec) = match rest.split_once("from") {
            Some((n, t)) => {
                let spec = t
                    .trim_start()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();
                let spec = if is_module_name_spec(&spec) {
                    spec
                } else {
                    n.trim().to_string()
                };
                (n.trim().to_string(), spec)
            }
            None => (rest.trim().to_string(), rest.trim().to_string()),
        };
        if name.is_empty() {
            Vec::new()
        } else {
            vec![(name, format!("\"{spec}\""), false)]
        }
    } else {
        Vec::new()
    }
}

/// Whether a quoted import spec names a host module (`"bytes"`, `"str"`)
/// rather than a file path. Path-like specs (`"./x"`, `"../index"`,
/// `"math/add"`, `"x.hnl"`) that fail file resolution are decorative —
/// not dispatchable module names.
fn is_module_name_spec(spec: &str) -> bool {
    !spec.is_empty() && !spec.starts_with('.') && !spec.contains('/') && !spec.ends_with(".hnl")
}

/// Extract the first quoted path in a `from` line and the text that follows it.
fn extract_quoted_path(line: &str) -> Option<(String, &str)> {
    let trimmed = line.trim_start();
    let after_from = trimmed.strip_prefix("from")?;
    let after_from = after_from.trim_start();
    let quote = after_from.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let after_quote = &after_from[quote.len_utf8()..];
    let end = after_quote.find(quote)?;
    let path = &after_quote[..end];
    let rest = &after_quote[end + quote.len_utf8()..];
    Some((path.to_string(), rest))
}

/// Top-level import statements whose specifier resolves to a file — the
/// executable import op (ADR-0096). Decorative declarations
/// (`import <name>`, module-name specs like `from "str" import …`) are
/// excluded: the host module chain supplies those bindings. Returned as
/// provenance-tracked [`Piece`]s (each import's segment line).
fn collect_executable_import_pieces(
    src: &str,
    is_file_spec: &dyn Fn(&str) -> bool,
    line_starts: &[usize],
) -> Vec<Piece> {
    let line_of = |off: usize| -> u32 { line_starts.partition_point(|&s| s <= off).max(1) as u32 };
    top_level_segment_spans(src)
        .iter()
        .filter_map(|&(s, e)| {
            let raw = &src[s..e];
            let seg = raw.trim();
            if !is_import_line(seg) {
                return None;
            }
            if !import_line_spec(seg).map_or(false, |s| is_file_spec(&s)) {
                return None;
            }
            // The trimmed statement's first line — a segment can open
            // with a whitespace-only line.
            let line = Some(line_of(s + (raw.len() - raw.trim_start().len())));
            let text = from_export_to_import(seg)
                .map(|(import_line, _)| import_line)
                .unwrap_or_else(|| seg.to_string());
            // Inside a `do {}` block, `import <names>` trailing-tokens
            // parsing does not stop at the newline — terminate each
            // import with `;` so the following statement can't merge in.
            Some(Piece::new(format!("{text};"), line))
        })
        .collect()
}

/// Split `s` at commas at bracket depth zero (strings respected).
fn split_top_commas(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut items = Vec::new();
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = in_str {
            if c == b'\\' {
                i += 1;
            } else if c == q {
                in_str = None;
            }
        } else {
            match c {
                b'"' | b'\'' => in_str = Some(c),
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b',' if depth == 0 => {
                    items.push(&s[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
        i += 1;
    }
    items.push(&s[start..]);
    items
}

/// The field key of a `name : value` record entry — an identifier
/// followed by a depth-zero `:` that is not `:=` (a binding, not a
/// field). `None` when the item is not a keyed field (a binding, an
/// expression, a type annotation).
fn record_field_key(item: &str) -> Option<String> {
    let item = item.trim();
    let bytes = item.as_bytes();
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = in_str {
            if c == b'\\' {
                i += 1;
            } else if c == q {
                in_str = None;
            }
        } else {
            match c {
                b'"' | b'\'' => in_str = Some(c),
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b':' if depth == 0 => {
                    if bytes.get(i + 1).is_some_and(|&n| n == b':' || n == b'=') {
                        return None;
                    }
                    let key = item[..i].trim();
                    return is_valid_ident(key).then(|| key.to_string());
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// Drop every top-level import statement, as provenance-tracked
/// [`Piece`]s: file-resolving imports are hoisted into `imports`
/// separately (see `collect_executable_import_pieces`); host-module
/// named imports lower to `name := "spec.name"` string bindings so bare
/// names dispatch through the module chain. Non-import segments are
/// kept *verbatim* (unlike the trimmed import text) so their interior
/// lines keep exact provenance.
fn drop_all_import_pieces(
    src: &str,
    is_file_spec: &dyn Fn(&str) -> bool,
    line_starts: &[usize],
) -> Vec<Piece> {
    let line_of = |off: usize| -> u32 { line_starts.partition_point(|&s| s <= off).max(1) as u32 };
    let mut pieces = Vec::new();
    for &(s, e) in &top_level_segment_spans(src) {
        let seg = &src[s..e];
        let trimmed = seg.trim();
        if !is_import_line(trimmed) {
            pieces.push(Piece::new(seg, Some(line_of(s))));
            continue;
        }
        if import_line_spec(trimmed).map_or(false, |s| is_file_spec(&s)) {
            continue;
        }
        // Generated `local := "spec.name"` bindings — attribute them to
        // the import declaration's first (trimmed) line.
        let decl_line = line_of(s + (seg.len() - seg.trim_start().len()));
        for (local, expr, _) in host_import_bindings(trimmed) {
            if is_valid_ident(&local) {
                pieces.push(Piece::new(format!("{local} := {expr}"), Some(decl_line)));
            }
        }
    }
    pieces
}

/// Local names bound by a file-resolving import statement:
/// `from "<spec>" import a, b as c` → `[a, c]`, `* as m` → `[m]`,
/// `import <name> [from "<spec>"]` → `[name]`. Bare splices
/// (`import "<spec>"`, `from "<spec>" import *`) bind names that are not
/// statically enumerable — they are skipped.
fn import_bound_locals(line: &str) -> Vec<String> {
    let trimmed = line.trim_start();
    if let Some((_, rest)) = extract_quoted_path(trimmed).filter(|_| trimmed.starts_with("from")) {
        let rest = rest.trim_start();
        let Some(tail) = rest.strip_prefix("import") else {
            return Vec::new();
        };
        if !tail.is_empty() && !tail.starts_with(' ') && !tail.starts_with('\t') {
            return Vec::new();
        }
        let tail = tail.trim_start();
        let inner = tail
            .strip_prefix('{')
            .and_then(|t| t.strip_suffix('}'))
            .unwrap_or(tail);
        return inner
            .split(',')
            .filter_map(|item| {
                let item = item.trim();
                if item.is_empty() {
                    return None;
                }
                match item.split_once(" as ") {
                    Some((_, alias)) => {
                        let alias = alias.trim();
                        (!alias.is_empty()).then(|| alias.to_string())
                    }
                    None => (item != "*").then(|| item.to_string()),
                }
            })
            .collect();
    }
    if let Some(rest) = trimmed.strip_prefix("import") {
        let rest = rest.trim_start();
        if rest.starts_with('"') || rest.starts_with('\'') {
            return Vec::new();
        }
        // `import <name>` or `import <name> from "<spec>"`.
        let name = rest
            .split_once("from")
            .map(|(n, _)| n.trim())
            .unwrap_or_else(|| rest.trim());
        return if is_valid_ident(name) {
            vec![name.to_string()]
        } else {
            Vec::new()
        };
    }
    Vec::new()
}

/// Extract the quoted specifier from `import "<spec>"` or
/// `from "<spec>" …`. Non-quoted forms (`import <name>`, bare `import`)
/// return `None`.
fn import_line_spec(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("import")
        .or_else(|| trimmed.strip_prefix("from"))?;
    let rest = rest.trim_start();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let after = &rest[quote.len_utf8()..];
    let end = after.find(quote)?;
    Some(after[..end].to_string())
}

fn is_import_line(trimmed: &str) -> bool {
    if let Some(rest) = trimmed.strip_prefix("import") {
        if rest.is_empty()
            || rest.starts_with(' ')
            || rest.starts_with('\t')
            || rest.starts_with('"')
            || rest.starts_with('\'')
        {
            return true;
        }
    }
    if let Some(rest) = trimmed.strip_prefix("from") {
        if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
            return true;
        }
    }
    false
}

/// Whether `rhs` is a top-level arrow *signature* — contract data whose
/// domain/codomain are *type* expressions that exec must not evaluate
/// (Agda-style erasure; annotations are static metadata owned by the
/// typecheck engine). Two shapes, both un-callable at runtime:
///
/// - a grade-1 signature arrow `Type ~[1]~> Type` (`~[` at bracket
///   depth 0, distinguished from n-cell *value* exports like
///   `export alpha := f ~[2]~> g` by [`graded_arrow_is_signature`]) —
///   always mints an `Arrow` descriptor record, and codomains like
///   `Result<T, E>` are generic applications (`dimension` nodes) that
///   crash eval;
/// - an ungraded arrow `(…, [T], …) ~> B` whose parenthesized params
///   contain a *bare* `[` element (a list type `[String]` — never
///   preceded by `:` like a `x: [T]` annotation, nor part of an index
///   expression) — `closure` only binds `var`/`type-annot` params, so the
///   `~>` can only ever be a signature.
///
/// Erasing binds `null` — the same value type-only `export Name: Type`
/// gets — losing no runtime behavior. Real arrow literals
/// (`export f := (x) ~> body`, `export f := (x: [Int]) ~> body`) and
/// n-cell value exports (`export alpha := f ~[2]~> g`) are untouched.
fn is_signature_rhs(rhs: &str) -> bool {
    let bytes = rhs.as_bytes();
    let mut depth: i32 = 0;
    let mut paren_depth: i32 = 0;
    let mut in_str: Option<u8> = None;
    let mut prev_sig = 0u8;
    let mut list_type_param = false;
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if let Some(q) = in_str {
            if c == b'\\' {
                i += 1;
            } else if c == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' | b'\'' => in_str = Some(c),
            b'(' => {
                depth += 1;
                paren_depth += 1;
            }
            b'[' | b'{' => {
                depth += 1;
                // A `[`/`{` element inside a params list — a list/record
                // *type* — is bare only when it directly follows `(` or
                // `,` (`x: [T]` annotations and `xs[i]` indexes don't).
                if paren_depth > 0 && (prev_sig == b'(' || prev_sig == b',') {
                    list_type_param = true;
                }
            }
            b')' => {
                depth -= 1;
                paren_depth -= 1;
            }
            b']' | b'}' => depth -= 1,
            b'~' if depth == 0 => match bytes.get(i + 1) {
                Some(&b'[') => return graded_arrow_is_signature(rhs, i),
                Some(&b'>') => return list_type_param,
                _ => {}
            },
            _ => {}
        }
        if !c.is_ascii_whitespace() {
            prev_sig = c;
        }
        i += 1;
    }
    false
}

/// Whether a depth-0 `~[` at byte `tilde` opens a graded-arrow
/// *signature* `Type ~[1]~> Type` (erase) rather than a genuine n-cell
/// *value* `f ~[N]~> g` (keep) — the `export alpha := f ~[2]~> g`
/// footgun this guard used to erase to `null`. Corpus sweep (every `~[`
/// across lib-*/kit-*/handl-packages, ~7,200 sites): **all** signatures
/// grade `~[1]~>`, and every `~[2]~>`+/`~[N]~>` is a cell term (the
/// `effects`/`optimize` law exports, `lib-type`'s dependent
/// unit/counit). So:
///
/// - any grade other than the literal `1` → a cell value, keep;
/// - grade `1` → the *domain operand* decides: a signature's domain is a
///   type expression — a parenthesized params `(...)`, a record
///   `{...}`, a bare list type `[T]`, or a `Capitalized` type head
///   (`Int`, `Result<A, B>`, `T`). A lowercase/literal/index-led LHS
///   (`f`, `map inc`, `"a >>> b"`, `xs[i]`) is a value expression → a
///   cell, keep.
///
/// Documented limits (no corpus instance today): a grade-1 cell over a
/// parenthesized domain `(f >>> g) ~[1]~> h` still erases; a grade-1
/// signature over *lowercase* type vars `a ~[1]~> b` now evals (a loud
/// unbound-name error) rather than binding `null`; a symbolic grade
/// (`A ~[n]~> B` contract) is kept and evals — none exist outside
/// comments.
fn graded_arrow_is_signature(rhs: &str, tilde: usize) -> bool {
    let bytes = rhs.as_bytes();
    // Grade text between `~[` and its matching `]` (balance nested
    // brackets defensively — grades are flat today).
    let mut gdepth = 1i32;
    let mut gend = bytes.len();
    let mut j = tilde + 2;
    while j < bytes.len() {
        match bytes[j] {
            b'[' => gdepth += 1,
            b']' => {
                gdepth -= 1;
                if gdepth == 0 {
                    gend = j;
                    break;
                }
            }
            _ => {}
        }
        j += 1;
    }
    if rhs[tilde + 2..gend].trim() != "1" {
        return false;
    }
    let lhs = rhs[..tilde].trim();
    match lhs.as_bytes().last() {
        // `~[1]~>` with no domain text — degenerate; keep erasing.
        None => true,
        // Parenthesized params / record-type domain.
        Some(b')') | Some(b'}') => true,
        // `[T]`-led list-type domain; `xs[i]` is an index — a value.
        Some(b']') => lhs.starts_with('['),
        // `Int`/`Result<A, B>`/`T` type head vs a lowercase-led value
        // term (`f`, `map inc`, `"a >>> b"`, `xs[i]` handled above).
        Some(_) => lhs.as_bytes()[0].is_ascii_uppercase(),
    }
}

fn find_binding_sep(s: &str) -> Option<(usize, usize)> {
    // `:=` is the ONLY binding separator (`=`/`==` are equality, `::` is
    // gone). A lone `:` is the type-only `export x : T` separator.
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut depth: i32 = 0;
    let mut first_colon: Option<usize> = None;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b':' if depth == 0 => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    return Some((i, 2));
                }
                if first_colon.is_none() {
                    first_colon = Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    first_colon.map(|pos| (pos, 1))
}

fn strip_export_name(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut depth: i32 = 0;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'<' | b':' if depth == 0 => {
                return s[..i].trim().to_string();
            }
            _ => {}
        }
        i += 1;
    }
    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// File-line provenance must survive comment stripping, declaration
    /// stripping, and the export-to-object rewrite: a fixture with `--`
    /// comments and blank lines (which naive offset math cannot map)
    /// resolves each prepared line to its true file line.
    #[test]
    fn mapped_lines_resolve_through_comments_and_blanks() {
        // File lines: 1 `-- doc`, 2 blank, 3 `export add`, 4 blank,
        // 5 `-- note`, 6 `helper`, 7 `export answer`.
        let src = "-- doc\n\
                   \n\
                   export add := (a, b) ~> a + b\n\
                   \n\
                   -- note\n\
                   helper := (x) ~> x * 2\n\
                   export answer := add(1, 2)\n";
        let p = prepare_module_mapped(src, &|_| false, &|_| true).expect("prep");
        assert_eq!(p.origin_lines, 7);
        let line_of = |needle: &str| {
            p.source
                .lines()
                .position(|l| l.contains(needle))
                .map(|i| i + 1)
                .unwrap_or_else(|| panic!("{needle:?} not in prepared source:\n{}", p.source))
        };
        // The `name :=` prefix shares its line with the RHS — the map
        // records the binding's file line.
        assert_eq!(p.line_map[line_of("add :=") - 1], Some(3));
        assert_eq!(p.line_map[line_of("helper :=") - 1], Some(6));
        assert_eq!(p.line_map[line_of("answer :=") - 1], Some(7));
        // Generated wrapper/projection lines carry no file origin.
        assert_eq!(p.line_map[0], None, "prepared source:\n{}", p.source);
        assert!(p.line_map.iter().any(|e| e.is_none()));
        // No entry may point at a comment or blank file line — those
        // lines were stripped and own no prepared text.
        for (i, e) in p.line_map.iter().enumerate() {
            if let Some(l) = e {
                assert!(
                    ![1u32, 2, 4, 5].contains(l),
                    "prepared line {} maps to stripped file line {}",
                    i + 1,
                    l
                );
            }
        }
        // The unmapped variant returns identical text.
        assert_eq!(
            prepare_module(src, &|_| false, &|_| true).expect("prep"),
            p.source
        );
    }

    /// A multi-line export RHS keeps per-line provenance: each prepared
    /// line of the wrapped expression maps to the file line it came from.
    #[test]
    fn mapped_lines_track_multiline_rhs() {
        // File lines: 1 `export f :=`, 2 `  (x) ~>`, 3 `    do {`,
        // 4 `      x + 1`, 5 `    }`, 6 `export g := 2`.
        let src = "export f :=\n  (x) ~>\n    do {\n      x + 1\n    }\nexport g := 2\n";
        let p = prepare_module_mapped(src, &|_| false, &|_| true).expect("prep");
        assert_eq!(p.origin_lines, 6);
        let lines: Vec<&str> = p.source.lines().collect();
        // Find the prepared line holding `x + 1` — it must map to file 4.
        let idx = lines
            .iter()
            .position(|l| l.contains("x + 1"))
            .expect("rhs body in prepared source");
        assert_eq!(p.line_map[idx], Some(4), "prepared:\n{}", p.source);
        let g_idx = lines
            .iter()
            .position(|l| l.contains("g :="))
            .expect("g binding");
        assert_eq!(p.line_map[g_idx], Some(6));
    }
}
