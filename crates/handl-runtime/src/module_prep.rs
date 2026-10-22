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
//! declaration without a `::`/`:=`/`:`/`=` separator) is a hard
//! [`LangError`] — module prep never silently drops a declaration.

use handl_language::LangError;

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
    let src = strip_comment_lines(src);
    let op_names = collect_op_decl_names(&src);
    let src = strip_declaration_lines(&src);
    transform_export_to_object(&src, is_file_spec, &op_names, is_bindable)
}

/// Collect the names a module source exports — the keys of the record
/// [`prepare_module`] would produce — without preparing or evaluating it.
/// Hosts use this to learn which names a spliced `import "<spec>"` /
/// `from "<spec>" import *` binds (e.g. seeding a pre-run typecheck scope).
pub fn module_export_names(src: &str) -> Vec<String> {
    let src = strip_comment_lines(src);
    let op_names = collect_op_decl_names(&src);
    let src = strip_declaration_lines(&src);
    // `exported` collects names from `export` declarations (including
    // re-exports and `op` decls); `all` collects every top-level bound
    // name. A module with no `export` declarations exports every
    // top-level binding (see `transform_export_to_object`).
    let mut exported: Vec<String> = Vec::new();
    let mut all: Vec<String> = Vec::new();
    let push_all = |all: &mut Vec<String>, name: String| {
        if !all.iter().any(|n| *n == name) {
            all.push(name);
        }
    };
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
                for (name, _) in reexports {
                    push_all(&mut all, name);
                }
            } else {
                for (local, _, exp) in host_import_bindings(trimmed) {
                    if exp {
                        exported.push(local.clone());
                    }
                    push_all(&mut all, local);
                }
                for local in import_bound_locals(trimmed) {
                    push_all(&mut all, local);
                }
            }
            continue;
        }
        if let Some(rest) = export_decl_rest(trimmed) {
            if let Some((sep_pos, _)) = find_binding_sep(rest) {
                let name = strip_export_name(&rest[..sep_pos]);
                exported.push(name.clone());
                push_all(&mut all, name);
            }
            // A separator-less `export` is skipped here (name seeding is
            // best-effort); `prepare_module` reports it as an error.
        } else if let Some((sep_pos, _)) = find_binding_sep(trimmed) {
            let name = strip_export_name(&trimmed[..sep_pos]).trim().to_string();
            if is_valid_ident(&name) {
                push_all(&mut all, name);
            }
        }
    }
    for qualified in op_names {
        let local = qualified.rsplit('.').next().unwrap_or(&qualified);
        if !local.is_empty() && is_valid_ident(local) && !exported.iter().any(|n| n == local) {
            exported.push(local.to_string());
            push_all(&mut all, local.to_string());
        }
    }
    if exported.is_empty() {
        // A no-exports module whose last statement is a `do { … }` or
        // record literal produces that value as the module record — merge
        // its field keys so the seed covers them (extra seeds are
        // harmless; missing ones break `hnl run`'s splice pre-check).
        for key in trailing_record_keys(&src) {
            push_all(&mut all, key);
        }
        all
    } else {
        exported
    }
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

/// Strip `--` comment lines from a `.hnl` source.
fn strip_comment_lines(src: &str) -> String {
    src.lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip `cap`/`op`/`handl` declaration lines and their indented
/// continuation lines from a `.hnl` source.
fn strip_declaration_lines(src: &str) -> String {
    src.lines()
        .filter(|line| {
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
        .collect::<Vec<_>>()
        .join("\n")
}

/// Transform top-level `export name <::|:=|:|=> expr` declarations into an
/// object literal.  Type-only `export Name: Type` declarations are given the
/// value `null` so that type expressions are not evaluated at runtime. The
/// same erasure applies to signature exports `export Name := A ~[N]~> B`
/// whose RHS is a bare graded-arrow signature ([`is_signature_rhs`]): its
/// boundary children are type expressions that exec must not evaluate.
///
/// The source is consumed as top-level *statements*
/// ([`top_level_segments`]) — not lines — so an `export` is recognized
/// wherever a statement can begin: at column 0, indented, after `;`, or
/// mid-line after another statement; and its RHS expression is captured
/// whole regardless of how newlines fall inside `~>`-chained `do {…}` /
/// `if`/`else` bodies. A separator-less `export` declaration is a
/// [`LangError`] — declarations are never dropped silently.
fn transform_export_to_object(
    src: &str,
    is_file_spec: &dyn Fn(&str) -> bool,
    op_names: &[String],
    is_bindable: &dyn Fn(&str) -> bool,
) -> Result<String, LangError> {
    // Each top-level binding is collected as `(name, expr_parts, exported)`
    // in source order. All bindings become `do`-block assignments
    // (`name := expr;`) emitted *before* the object so closures can
    // reference siblings — exported or not. The object literal then
    // projects the exported names (`{ name: name }`).
    let mut bindings: Vec<(String, Vec<String>, bool)> = Vec::new();
    // Names bound by file-resolving `from "f" import a, b as c` /
    // `import n from "f"` statements — bound at eval time by the import
    // itself, so they join the no-exports record projection without a
    // prelude line.
    let mut import_bound: Vec<String> = Vec::new();
    // Whether the module's last top-level statement is a binding. When it
    // is not (e.g. a trailing `do { … }` or record literal) the body's own
    // value is the module record and no name projection is appended.
    let mut tail_is_binding = true;
    let mut current: Option<(String, Vec<String>, bool)> = None;

    // Finalize `current` into the ordered binding list.
    macro_rules! finalize {
        () => {
            if let Some(entry) = current.take() {
                bindings.push(entry);
            }
        };
    }

    for seg in top_level_segments(src) {
        let trimmed = seg.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_import_line(trimmed) {
            finalize!();
            // Only file-resolving specs can re-export; module-name specs
            // name host modules (ADR-0096).
            let file_spec = import_line_spec(trimmed).map_or(false, |s| is_file_spec(&s));
            if file_spec {
                if let Some((_, names)) = from_export_to_import(trimmed) {
                    for (name, alias) in names {
                        bindings.push((name, vec![alias], true));
                    }
                } else {
                    // `from "f" import a, b as c` binds locals in the
                    // module's scope; a module with no `export` decls
                    // exports every bound name, so collect them for the
                    // record projection (they need no prelude line — the
                    // import statement itself binds them).
                    for local in import_bound_locals(trimmed) {
                        if is_valid_ident(&local) && !bindings.iter().any(|(n, _, _)| *n == local) {
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
                    bindings.push((local, vec![expr], exported));
                }
            }
            continue;
        }
        if let Some(rest) = export_decl_rest(trimmed) {
            finalize!();
            let Some((sep_pos, sep_len)) = find_binding_sep(rest) else {
                // Never drop an export declaration silently — a module
                // that reaches eval with a vanished export produces an
                // empty record (`{}`) or an "unbound name" error far
                // from the cause.
                return Err(LangError::engine(format!(
                    "module prep: cannot transform `export` declaration \
                     (expected `export <name> <::|:=|=|: > …`): {}",
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
            let rhs = rest[sep_pos + sep_len..].trim();
            let is_type_only = (sep_len == 1 && rest.as_bytes().get(sep_pos) == Some(&b':'))
                || is_signature_rhs(rhs);
            let expr_start = if is_type_only { "null" } else { rhs };
            let mut expr_lines = Vec::new();
            if !expr_start.is_empty() {
                expr_lines.push(expr_start.to_string());
            }
            current = Some((name, expr_lines, true));
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
                let is_type_only = sep_len == 1 && trimmed.as_bytes().get(sep_pos) == Some(&b':');
                let expr_start = if is_type_only {
                    "null"
                } else {
                    trimmed[sep_pos + sep_len..].trim()
                };
                let mut expr_lines = Vec::new();
                if !expr_start.is_empty() {
                    expr_lines.push(expr_start.to_string());
                }
                current = Some((name, expr_lines, false));
                tail_is_binding = true;
                continue;
            }
        }
        // A non-binding statement that does not continue an in-flight
        // binding means the module's last statement is a bare expression
        // (`do { … }`, record literal, call) — its value is the module
        // record, so no projection is appended. Otherwise the segment
        // continues the current binding's expression verbatim (a `}`
        // closing a `do` block, a wrapped expression tail, …).
        if current.is_none() {
            tail_is_binding = false;
        } else if let Some((_, lines, _)) = &mut current {
            lines.push(seg.clone());
        }
    }
    finalize!();

    // `op "mod.name" : T` declarations name host-registered operations.
    // Emit each as an exported binding of the qualified-name *string*: a
    // string callee dispatches through `call_host` → the module chain →
    // the op's native impl, so an interface-only module's exports stay
    // callable when it is imported as a file.
    for qualified in op_names {
        let local = qualified.rsplit('.').next().unwrap_or(qualified);
        if local.is_empty() || !is_valid_ident(local) || bindings.iter().any(|(n, _, _)| n == local)
        {
            continue;
        }
        bindings.push((local.to_string(), vec![format!("\"{qualified}\"")], true));
    }

    let has_exports = bindings.iter().any(|(_, _, exported)| *exported);

    let imports = collect_executable_import_lines(src, is_file_spec);

    // No `export` declarations: the module record is *every* top-level
    // bound name. The Pratt parser reads one statement per `parse` call,
    // so the body must be a single `do { … }` — executable path imports
    // (`import "f.hnl"`, `from "./f" import …`) hoisted first so their
    // names scope over the body, the verbatim statements next (private
    // bindings stay `x :: …`/`x := …`; host-module declarations lower to
    // `name := "spec.name"` string bindings), then the record projecting
    // every bound name.
    if !has_exports {
        let body = drop_all_import_lines(src, is_file_spec);
        // The projection is appended only when the module's last top-level
        // statement is a binding; a trailing `do { … }`/record literal is
        // already the module's interface value.
        let record = if tail_is_binding {
            let mut seen = std::collections::HashSet::new();
            let mut fields: Vec<String> = Vec::new();
            let mut push_field = |name: &str, expr_lines: &[String]| {
                if !seen.insert(name.to_string()) {
                    return;
                }
                if is_bindable(name) {
                    fields.push(format!("{name}: {name}"));
                } else {
                    fields.push(format!("\"{name}\": {}", expr_lines.join("\n").trim()));
                }
            };
            for (name, expr_lines, _) in &bindings {
                push_field(name, expr_lines);
            }
            for name in &import_bound {
                if is_bindable(name) {
                    push_field(name, &[]);
                }
            }
            fields.join(", ")
        } else {
            String::new()
        };
        let mut out = String::from("do {\n");
        for imp in &imports {
            out.push_str(imp);
            out.push('\n');
        }
        out.push_str(&body);
        if !record.is_empty() {
            out.push_str("\n{ ");
            out.push_str(&record);
            out.push_str(" }");
        }
        out.push_str("\n}");
        return Ok(out);
    }

    // Build the body: every binding as a do-block assignment in source
    // order, then the export object literal projecting exported names.
    // A name that isn't a bindable identifier (e.g. the keyword `first` —
    // `export first := ...` / `from "./first" export first`) can't be a
    // `:=` target, so it gets no binding; its record field carries the
    // expression inline under a quoted key (`{ "first": __first }`).
    let mut body = String::new();
    for (name, expr_lines, _) in &bindings {
        if !is_bindable(name) {
            continue;
        }
        let expr = expr_lines.join("\n");
        let expr = expr.trim();
        body.push_str(name);
        body.push_str(" := ");
        body.push_str(expr);
        body.push_str(";\n");
    }
    body.push_str("{ ");
    let mut first = true;
    for (name, expr_lines, exported) in &bindings {
        if !exported {
            continue;
        }
        if !first {
            body.push_str(", ");
        }
        first = false;
        if is_bindable(name) {
            body.push_str(name);
            body.push_str(": ");
            body.push_str(name);
        } else {
            body.push('"');
            body.push_str(name);
            body.push_str("\": ");
            body.push_str(expr_lines.join("\n").trim());
        }
    }
    body.push_str(" }");

    if imports.is_empty() {
        // `bindings; { exports }` is a multi-statement body: wrap it
        // in `do { … }` (the same trick `hnl run` uses, and the same
        // shape `plugin-lib-handl`'s prep emits) so every binding
        // evaluates in one scope before the export record.
        Ok(format!("do {{\n{}\n}}", body))
    } else {
        Ok(format!("do {{\n{}\n{}\n}}", imports.join("\n"), body))
    }
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
    let mut segs: Vec<String> = Vec::new();
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

    // Emit the accumulated statement `[start, $end)` when non-blank and
    // open the next segment at `$next` with lead `$lead`.
    macro_rules! cut {
        ($end:expr, $next:expr, $lead:expr) => {{
            let text = &src[start..$end];
            if !text.trim().is_empty() {
                segs.push(text.to_string());
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
        segs.push(text.to_string());
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
/// excluded: the host module chain supplies those bindings.
fn collect_executable_import_lines(src: &str, is_file_spec: &dyn Fn(&str) -> bool) -> Vec<String> {
    top_level_segments(src)
        .iter()
        .map(|seg| seg.trim().to_string())
        .filter(|seg| is_import_line(seg))
        .filter(|seg| import_line_spec(seg).map_or(false, |s| is_file_spec(&s)))
        .map(|seg| {
            let line = from_export_to_import(&seg)
                .map(|(import_line, _)| import_line)
                .unwrap_or(seg);
            // Inside a `do {}` block, `import <names>` trailing-tokens
            // parsing does not stop at the newline — terminate each
            // import with `;` so the following statement can't merge in.
            format!("{};", line)
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
/// followed by a depth-zero `:` that is not `::`. `None` when the item
/// is not a keyed field (a binding, an expression, a type annotation).
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
                    if bytes.get(i + 1) == Some(&b':') {
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

/// Field names of the last record literal in `text` — the value a
/// trailing `do { … }` or bare `{ … }` produces. Walks the final
/// balanced `{…}` group; when its depth-one entries are all `name : …`
/// fields the keys are returned, otherwise the search recurses into the
/// group's interior (the record may be the last expression inside a
/// `do` block). Best-effort static extraction for name seeding.
fn trailing_record_keys(text: &str) -> Vec<String> {
    let mut text = text.trim_end();
    loop {
        if !text.ends_with('}') {
            return Vec::new();
        }
        let bytes = text.as_bytes();
        let mut depth = 0i32;
        let mut open = None;
        for i in (0..bytes.len()).rev() {
            match bytes[i] {
                b'}' => depth += 1,
                b'{' => {
                    depth -= 1;
                    if depth == 0 {
                        open = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(open) = open else { return Vec::new() };
        let inner = text[open + 1..bytes.len() - 1].trim_end();
        let items: Vec<&str> = split_top_commas(inner)
            .into_iter()
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .collect();
        if !items.is_empty() && items.iter().all(|item| record_field_key(item).is_some()) {
            return items
                .iter()
                .filter_map(|item| record_field_key(item))
                .collect();
        }
        text = inner;
    }
}

/// Drop every top-level import statement: file-resolving imports are
/// hoisted into `imports` separately (see
/// `collect_executable_import_lines`); host-module named imports lower to
/// `name := "spec.name"` string bindings so bare names dispatch through
/// the module chain.
fn drop_all_import_lines(src: &str, is_file_spec: &dyn Fn(&str) -> bool) -> String {
    top_level_segments(src)
        .iter()
        .flat_map(|seg| {
            let trimmed = seg.trim();
            if !is_import_line(trimmed) {
                return vec![seg.clone()];
            }
            if import_line_spec(trimmed).map_or(false, |s| is_file_spec(&s)) {
                return Vec::new();
            }
            host_import_bindings(trimmed)
                .into_iter()
                .filter(|(local, _, _)| is_valid_ident(local))
                .map(|(local, expr, _)| format!("{local} := {expr}"))
                .collect()
        })
        .collect::<Vec<_>>()
        .join("\n")
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
/// - a graded arrow `A ~[N]~> B` (`~[` at bracket depth 0) — always mints
///   an `Arrow` descriptor record, and codomains like `Result<T, E>` are
///   generic applications (`dimension` nodes) that crash eval;
/// - an ungraded arrow `(…, [T], …) ~> B` whose parenthesized params
///   contain a *bare* `[` element (a list type `[String]` — never
///   preceded by `:` like a `x: [T]` annotation, nor part of an index
///   expression) — `closure` only binds `var`/`type-annot` params, so the
///   `~>` can only ever be a signature.
///
/// Erasing binds `null` — the same value type-only `export Name: Type`
/// gets — losing no runtime behavior. Real arrow literals
/// (`export f := (x) ~> body`, `export f := (x: [Int]) ~> body`) are
/// untouched.
///
/// TODO(footgun): a *top-level* `export alpha := f ~[2]~> g` (a genuine
/// n-cell export, not a signature) also trips the depth-0 `~[` rule and
/// silently binds `null` — nested `f ~[2]~> g` inside bodies is fine,
/// only the export-RHS shape erases. Zero such exports exist in the
/// corpus today (verified by sweep); distinguishing them needs a
/// type-vs-value check on the `~[` operands (paren/record-led = type
/// signature, bare-term = cell) — implement if a cell-export ever
/// appears.
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
                Some(&b'[') => return true,
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

fn find_binding_sep(s: &str) -> Option<(usize, usize)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut depth: i32 = 0;
    let mut first_eq: Option<usize> = None;
    let mut first_colon: Option<usize> = None;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b':' if depth == 0 => {
                if i + 1 < bytes.len() && bytes[i + 1] == b':' {
                    return Some((i, 2));
                }
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    return Some((i, 2));
                }
                if first_colon.is_none() {
                    first_colon = Some(i);
                }
            }
            b'=' if depth == 0 => {
                let prev = if i > 0 { bytes[i - 1] } else { 0 };
                let next = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
                if next == b'=' || prev == b'>' || prev == b'<' || prev == b'!' || next == b'>' {
                    i += 1;
                    continue;
                }
                if first_eq.is_none() {
                    first_eq = Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    if let Some(pos) = first_eq {
        return Some((pos, 1));
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
