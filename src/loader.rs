//! Generic on-disk `.hnl` loader.
//!
//! [`load_hnl_file`] builds a [`HandlModule`] directly from a `.hnl` file
//! path, resolving `from` imports relative to the file's directory and
//! falling back to the stdlib package table for bare module names such as
//! `hash_map` or `typeof`.  It is not specific to `array` — any `.hnl`
//! package that uses top-level `export` and `from "..." export` forms can
//! be loaded from disk.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use handl_platform::Module;
use handl_plugin_lib_handl::HandlModule;

use crate::packages::module;

// ---------------------------------------------------------------------------
// Caches
// ---------------------------------------------------------------------------

fn top_module_cache() -> &'static Mutex<HashMap<PathBuf, Arc<HandlModule>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<HandlModule>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

thread_local! {
    static LOADING: RefCell<HashSet<PathBuf>> = RefCell::new(HashSet::new());
}

struct LoadingGuard(PathBuf);

impl Drop for LoadingGuard {
    fn drop(&mut self) {
        LOADING.with(|loading| {
            loading.borrow_mut().remove(&self.0);
        });
    }
}

fn table_module_cache() -> &'static Mutex<HashMap<String, Arc<HandlModule>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<HandlModule>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build a [`HandlModule`] from an on-disk `.hnl` file.
///
/// The module name is the file stem, or — for `index.hnl` — the parent
/// directory name (so `.../array/index.hnl` becomes the `array` module).
/// Relative `from` imports are resolved against the file's directory, bare
/// names fall back to the stdlib package table, and the resulting module
/// plus all of its transitive file/table imports are pre-registered as
/// dependencies so that the handl runtime does not need dynamic table
/// lookups at call time.
pub fn load_hnl_file<P: AsRef<Path>>(path: P) -> Option<Arc<HandlModule>> {
    let path = path.as_ref().to_path_buf();

    if let Ok(cache) = top_module_cache().lock() {
        if let Some(m) = cache.get(&path) {
            return Some(Arc::clone(m));
        }
    }

    if LOADING.with(|loading| !loading.borrow_mut().insert(path.clone())) {
        return None;
    }
    let _loading_guard = LoadingGuard(path.clone());

    let src = fs::read_to_string(&path).ok()?;
    let base_dir = path.parent()?.to_path_buf();
    let name = module_name_for_path(&path)?;
    let funcs = collect_export_names(&src);
    let prepared = prepare_module_source(&src);
    let func_refs: Vec<&str> = funcs.iter().map(|s| s.as_str()).collect();

    let mut module = HandlModule::new_raw(&name, &prepared, &func_refs);

    for import in collect_direct_imports(&src) {
        let dep: Arc<dyn Module + Send + Sync> = if import.starts_with("./") || import.contains('/')
        {
            let sub_path = base_dir.join(resolve_relative_path(&import));
            load_hnl_file(&sub_path)?
        } else {
            get_table_module(&import)?
        };
        module = module.with_dependency_arc(dep);
    }

    let module = Arc::new(module);
    if let Ok(mut cache) = top_module_cache().lock() {
        cache.insert(path, Arc::clone(&module));
    }
    Some(module)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_table_module(name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
    if let Ok(cache) = table_module_cache().lock() {
        if let Some(m) = cache.get(name) {
            return Some(Arc::clone(m) as Arc<dyn Module + Send + Sync>);
        }
    }
    let m = Arc::new(module(name)?);
    if let Ok(mut cache) = table_module_cache().lock() {
        cache.insert(name.to_string(), Arc::clone(&m));
    }
    Some(m)
}

fn resolve_relative_path(path: &str) -> String {
    let mut p = path.strip_prefix("./").unwrap_or(path).to_string();
    if !p.ends_with(".hnl") {
        p.push_str(".hnl");
    }
    p
}

fn collect_direct_imports(src: &str) -> Vec<String> {
    top_level_segments(src)
        .iter()
        .map(|seg| seg.trim().to_string())
        .filter(|seg| is_import_line(seg))
        .filter_map(|seg| extract_quoted_path(&seg).map(|(path, _)| path))
        .collect()
}

fn module_name_for_path(path: &Path) -> Option<String> {
    if path.file_name()?.to_str()? == "index.hnl" {
        path.parent()?.file_name()?.to_str().map(|s| s.to_string())
    } else {
        path.file_stem()?.to_str().map(|s| s.to_string())
    }
}

// ---------------------------------------------------------------------------
// Source preparation (mirror of the runtime module-prep pipeline)
// ---------------------------------------------------------------------------

fn prepare_module_source(src: &str) -> String {
    let src = strip_comment_lines(src);
    let op_names = collect_op_decl_names(&src);
    let src = strip_declaration_lines(&src);
    transform_export_to_object(&src, &op_names)
}

/// Collect the qualified names from `op "<name>" : ...` declaration
/// statements (e.g. `op "bytes.from_list"` → `bytes.from_list`). An `op`
/// declaration is a top-level statement, so segmentation — not line shape —
/// decides where one begins.
fn collect_op_decl_names(src: &str) -> Vec<String> {
    top_level_segments(src)
        .iter()
        .filter_map(|seg| {
            let trimmed = seg.trim_start();
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

fn strip_comment_lines(src: &str) -> String {
    src.lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
}

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

fn collect_export_names(src: &str) -> Vec<String> {
    // `names` collects explicit `export` declarations; `all` collects
    // every top-level bound name. A module with no `export` declarations
    // exports every top-level binding (see `transform_export_to_object`).
    let mut names: Vec<String> = Vec::new();
    let mut all: Vec<String> = Vec::new();
    let push_all = |all: &mut Vec<String>, name: String| {
        if !all.iter().any(|n| *n == name) {
            all.push(name);
        }
    };
    for seg in top_level_segments(src) {
        let trimmed = seg.trim();
        if is_import_line(trimmed) {
            if let Some((_, reexports)) = from_export_to_import(trimmed) {
                for (name, _) in &reexports {
                    names.push(name.clone());
                }
                for (name, _) in reexports {
                    push_all(&mut all, name);
                }
            } else if import_line_spec(trimmed).is_some() {
                for local in import_bound_locals(trimmed) {
                    push_all(&mut all, local);
                }
            }
            continue;
        }
        if let Some(rest) = export_decl_rest(trimmed) {
            if let Some((sep_pos, _)) = find_binding_sep(rest) {
                let name = strip_export_name(&rest[..sep_pos]);
                names.push(name.clone());
                push_all(&mut all, name);
            }
        } else if let Some((sep_pos, _)) = find_binding_sep(trimmed) {
            let name = strip_export_name(&trimmed[..sep_pos]).trim().to_string();
            if is_valid_ident(&name) {
                push_all(&mut all, name);
            }
        }
    }
    if names.is_empty() {
        // A no-exports module whose last statement is a `do { … }` or
        // record literal produces that value as the module record —
        // merge its field keys so the registered functions cover them
        // (mirrors `handl_runtime::module_prep::module_export_names`).
        for key in trailing_record_keys(&strip_comment_lines(src)) {
            push_all(&mut all, key);
        }
        all
    } else {
        names
    }
}

/// Split a module source into its top-level statements, returning each
/// statement's verbatim text (internal newlines and indentation
/// preserved) in source order.
///
/// Mirrors `handl_runtime::module_prep::top_level_segments` (and the
/// `top_level_segments` port in `plugin-lib-handl`). A statement boundary
/// is recognized purely by structure — never by a hardcoded line shape:
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
fn top_level_segments(src: &str) -> Vec<String> {
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

/// The text after the `export` keyword when `trimmed` begins an export
/// declaration — `None` when `export` does not sit at a word boundary at
/// the start of the statement (`exporter := …` is a binding, not an
/// export). Mirrors `handl_runtime::module_prep::export_decl_rest`.
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

/// `src` with top-level import statements removed (all other statements
/// kept verbatim). Statement segmentation mirrors
/// `handl_runtime::module_prep::drop_all_import_lines`.
fn drop_import_lines(src: &str) -> String {
    top_level_segments(src)
        .iter()
        .filter(|seg| !is_import_line(seg.trim()))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Transform top-level `export name <::|:=|=|:> expr` declarations into an
/// object literal `{ name: expr, ... }` so the source evaluates to a record
/// of exported functions.
///
/// The source is consumed as top-level *statements*
/// ([`top_level_segments`]) — not lines — so an `export` is recognized
/// wherever a statement can begin: at column 0, indented, after `;`, or
/// mid-line after another statement; and its RHS expression is captured
/// whole regardless of how newlines fall inside `~>`-chained `do {…}` /
/// `if`/`else` bodies. A declaration that cannot be transformed (an
/// `export` with no `::`/`:=`/`:`/`=` separator, or an empty binding
/// name) panics — a module prep must never silently drop an export into
/// an empty record (mirrors `handl_runtime::module_prep`'s `LangError`).
///
/// Top-level `import`/`from` statements are hoisted — in original order —
/// ahead of the record literal inside a `do { … }` block so the runtime
/// can evaluate them (`prim_import`) and bind their names for the record.
/// `op "mod.name" : T` declarations (`op_names`, collected by
/// [`collect_op_decl_names`]) name host-registered operations and are
/// emitted as exported fields whose value is the qualified-name *string*.
///
/// If no `export` statements are found and there are no imports or
/// bindings, the source is returned unchanged (the existing
/// object-literal form is preserved).
fn transform_export_to_object(src: &str, op_names: &[String]) -> String {
    // Collect (name, expr_lines, exported) triples in source order.
    // Private bindings become `do`-block assignments (`name := expr;`)
    // emitted *before* the object so exported closures can reference
    // private helpers such as `clock_gettime` in
    // `time-c/clock_gettime.hnl`; exported bindings become record fields.
    let mut bindings: Vec<(String, Vec<String>, bool)> = Vec::new();
    let mut current: Option<(String, Vec<String>, bool)> = None;
    // Whether the module's last top-level statement is a binding. When it
    // is not (a trailing `do { … }` or record literal) the body's own
    // value is the module record and no name projection is appended —
    // mirrors `handl_runtime::module_prep`.
    let mut tail_is_binding = true;

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
        // Import statements are hoisted separately; never treat them as
        // part of an export body. `from "..." export x` re-export
        // statements contribute one record field per name.
        if is_import_line(trimmed) {
            finalize!();
            if let Some((_, names)) = from_export_to_import(trimmed) {
                for (name, alias) in names {
                    bindings.push((name, vec![alias], true));
                }
            }
            continue;
        }
        if let Some(rest) = export_decl_rest(trimmed) {
            // Flush the previous entry.
            finalize!();
            // Find the binding separator (`::`, `:=`, or legacy `=`).
            let Some((sep_pos, sep_len)) = find_binding_sep(rest) else {
                // Never drop an export declaration silently — a module
                // that reaches eval with a vanished export produces an
                // empty record (`{}`) or an "unbound name" error far
                // from the cause (mirrors `module_prep`'s `LangError`).
                panic!(
                    "module prep: cannot transform `export` declaration \
                     (expected `export <name> <::|:=|=|: > …`): {trimmed}"
                );
            };
            let name = strip_export_name(&rest[..sep_pos]);
            if name.is_empty() {
                panic!("module prep: `export` declaration has no binding name: {trimmed}");
            }
            let is_type_only = sep_len == 1 && rest.as_bytes().get(sep_pos) == Some(&b':');
            let expr_start = if is_type_only {
                "null"
            } else {
                rest[sep_pos + sep_len..].trim()
            };
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
        // helper functions are preserved instead of dropped — mirrors
        // `handl_runtime::module_prep`.
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
        // continues the current binding's expression verbatim.
        if current.is_none() {
            tail_is_binding = false;
        } else if let Some((_, lines, _)) = &mut current {
            lines.push(seg.clone());
        }
    }
    finalize!();

    // `op "mod.name" : T` declarations name host-registered operations.
    // Emit each as an exported field of the qualified-name *string*: a
    // string callee dispatches through `call_host` → the module chain →
    // the op's native impl, so an interface-only module's exports stay
    // callable when it is imported as a file.
    for qualified in op_names {
        let local = qualified.rsplit('.').next().unwrap_or(qualified);
        if local.is_empty()
            || !is_valid_ident(local)
            || bindings
                .iter()
                .any(|(n, _, exported)| *exported && n == local)
        {
            continue;
        }
        bindings.push((local.to_string(), vec![format!("\"{qualified}\"")], true));
    }

    let has_exports = bindings.iter().any(|(_, _, exported)| *exported);

    let imports = collect_executable_import_lines(src);

    // No `export` declarations: the module record is *every* top-level
    // bound name — private bindings plus the locals each hoisted import
    // statement binds. The body is wrapped in `do { … }` (imports first,
    // verbatim statements next, the record last) so the whole module
    // evaluates as one expression under the single-statement parser —
    // mirrors `loader.hnl`'s `transform_export_to_object` and
    // `handl_runtime::module_prep`.
    if !has_exports {
        if imports.is_empty() && bindings.is_empty() {
            return src.to_string();
        }
        let stripped = drop_import_lines(src);
        // The projection is appended only when the module's last top-level
        // statement is a binding; a trailing `do { … }`/record literal is
        // already the module's interface value.
        let mut fields: Vec<String> = Vec::new();
        if tail_is_binding {
            let mut seen = std::collections::HashSet::new();
            let mut push_field = |name: &str| {
                if is_valid_ident(name) && seen.insert(name.to_string()) {
                    fields.push(format!("{name}: {name}"));
                }
            };
            for (name, _, _) in &bindings {
                push_field(name);
            }
            for seg in top_level_segments(src) {
                let trimmed = seg.trim();
                if is_import_line(trimmed) && import_line_spec(trimmed).is_some() {
                    for local in import_bound_locals(trimmed) {
                        push_field(&local);
                    }
                }
            }
        }
        let mut out = String::from("do {\n");
        for imp in &imports {
            out.push_str(imp);
            out.push('\n');
        }
        out.push_str(&stripped);
        if !fields.is_empty() {
            out.push_str("\n{ ");
            out.push_str(&fields.join(", "));
            out.push_str(" }");
        }
        out.push_str("\n}");
        return out;
    }

    // Build the body: private bindings as do-block assignments, then the
    // export object literal. Record keys are always quoted: an export name
    // may be a keyword (`first`, `export`) that lexes as
    // `token.keyword.*`, not an identifier.
    let mut prelude = String::new();
    for (name, expr_lines, exported) in &bindings {
        if *exported {
            continue;
        }
        prelude.push_str(name);
        prelude.push_str(" := ");
        prelude.push_str(expr_lines.join("\n").trim());
        prelude.push_str(";\n");
    }
    let mut record = String::from("{ ");
    let mut first = true;
    for (name, expr_lines, exported) in &bindings {
        if !exported {
            continue;
        }
        if !first {
            record.push_str(", ");
        }
        first = false;
        record.push('"');
        record.push_str(name);
        record.push_str("\": ");
        record.push_str(expr_lines.join("\n").trim());
    }
    record.push_str(" }");

    // A bare export record stands alone; a private prelude or import
    // lines turn the module into a multi-statement body — wrap in
    // `do { … }` so it evaluates as a single expression (mirrors
    // `loader.hnl` and `module_prep`).
    if imports.is_empty() && prelude.is_empty() {
        record
    } else if imports.is_empty() {
        format!("do {{\n{prelude}{record}\n}}")
    } else {
        format!("do {{\n{}\n{prelude}{record}\n}}", imports.join("\n"))
    }
}

/// Split `s` at commas at bracket depth zero (strings respected) —
/// mirrors `handl_runtime::module_prep::split_top_commas`.
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
/// followed by a depth-zero `:` that is not `::`. Mirrors
/// `handl_runtime::module_prep::record_field_key`.
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
/// trailing `do { … }` or bare `{ … }` produces. Mirrors
/// `handl_runtime::module_prep::trailing_record_keys`.
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
            .map(|(n, a)| format!("{n} as {a}"))
            .collect();
        let import_line = format!("from \"{path}\" import {}", imports.join(", "));
        return Some((import_line, names.into_iter().zip(aliases).collect()));
    }
    None
}

fn extract_quoted_path(line: &str) -> Option<(String, &str)> {
    let trimmed = line.trim_start();
    let after_from = trimmed
        .strip_prefix("from")
        .or_else(|| trimmed.strip_prefix("import"))?;
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

/// Top-level import statements that name a quoted specifier — every
/// quoted spec is executable here because `load_hnl_file` pre-registers
/// each as a module dependency (file paths relative to the module dir,
/// bare names via the stdlib package table). Bare `import <name>`
/// declarations are decorative and excluded. An import is recognized
/// wherever a statement can begin, not only at column 0 of a line.
fn collect_executable_import_lines(src: &str) -> Vec<String> {
    top_level_segments(src)
        .iter()
        .map(|seg| seg.trim().to_string())
        .filter(|seg| is_import_line(seg))
        .filter(|seg| import_line_spec(seg).is_some())
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

/// Local names bound by a quoted-spec import statement: `from "<spec>"
/// import a, b as c` → `[a, c]`, `* as m` → `[m]`. Bare splices
/// (`import "<spec>"`, `from "<spec>" import *`) bind names that are not
/// statically enumerable — skipped. Mirrors
/// `handl_runtime::module_prep::import_bound_locals`.
fn import_bound_locals(line: &str) -> Vec<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("from") {
        return Vec::new();
    }
    let Some((_, rest)) = extract_quoted_path(trimmed) else {
        return Vec::new();
    };
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
    inner
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
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `export` sharing a line with an import statement: the `from` tail
    /// suppresses the split at `import`, then `export` opens a new
    /// statement mid-line (mirrors the platform's
    /// `single_line_module_exports_evaluate` case).
    #[test]
    fn export_sharing_a_line_with_an_import() {
        let src =
            "from \"char\" import * as char export f: F := (x) ~> do { if x > 0 { 1 } else { 0 } }";
        assert_eq!(
            transform_export_to_object(src, &[]),
            "do {\nfrom \"char\" import * as char;\n{ \"f\": (x) ~> do { if x > 0 { 1 } else { 0 } } }\n}"
        );
    }

    /// `;`-separated exports on one line split into their statements.
    #[test]
    fn semicolon_separated_exports() {
        let src = "export inc := (x) ~> x + 1; export dec := (x) ~> x - 1\n";
        assert_eq!(
            transform_export_to_object(src, &[]),
            "{ \"inc\": (x) ~> x + 1, \"dec\": (x) ~> x - 1 }"
        );
    }

    /// An indented `export` is a top-level declaration, not a
    /// continuation of the previous statement.
    #[test]
    fn indented_export_is_top_level() {
        let src = "export a := 1\n  export double := (x) ~> x * 2\n";
        assert_eq!(
            transform_export_to_object(src, &[]),
            "{ \"a\": 1, \"double\": (x) ~> x * 2 }"
        );
    }

    /// An `export` keyword mid-statement (after a private binding on the
    /// same line) still opens a new statement; the private binding is
    /// hoisted into the `do`-block prelude.
    #[test]
    fn export_mid_line_after_private_binding() {
        let src = "helper := (x) ~> x + 1 export apply := (x) ~> helper(x)";
        assert_eq!(
            transform_export_to_object(src, &[]),
            "do {\nhelper := (x) ~> x + 1;\n{ \"apply\": (x) ~> helper(x) }\n}"
        );
    }

    /// `exporter := …` is a private binding, not a malformed export —
    /// the `export` keyword must sit at a word boundary.
    #[test]
    fn exporter_prefix_is_not_an_export() {
        let src = "exporter := 1";
        assert_eq!(
            transform_export_to_object(src, &[]),
            "do {\nexporter := 1\n{ exporter: exporter }\n}"
        );
    }

    /// A multi-line `do {…}` export body is captured whole — the indented
    /// continuation lines are part of the same statement.
    #[test]
    fn export_body_keeps_indented_continuation() {
        let src = "export f := (x) ~>\n  do {\n    x\n  }\nexport g := 2";
        assert_eq!(
            transform_export_to_object(src, &[]),
            "{ \"f\": (x) ~>\n  do {\n    x\n  }, \"g\": 2 }"
        );
    }

    /// `from "<spec>" export a, b` re-exports contribute one record field
    /// per name and hoist as a rewritten `import … as __…` statement.
    #[test]
    fn from_export_reexport_fields() {
        let src = "from \"./pair.hnl\" export a, b\nexport z := x ~> x";
        assert_eq!(
            transform_export_to_object(src, &[]),
            "do {\nfrom \"./pair.hnl\" import a as __a, b as __b;\n{ \"a\": __a, \"b\": __b, \"z\": x ~> x }\n}"
        );
    }

    /// `op "mod.name"` declarations become exported string-valued fields.
    #[test]
    fn op_decl_names_become_string_fields() {
        let src = "export a := 1\n";
        assert_eq!(
            transform_export_to_object(src, &["bytes.from_list".to_string()]),
            "{ \"a\": 1, \"from_list\": \"bytes.from_list\" }"
        );
    }

    /// `prepare_module_source` end-to-end: the `op` line is stripped from
    /// the evaluatable source but still contributes its string field.
    #[test]
    fn prepare_strips_op_decl_but_keeps_field() {
        let src = "op \"bytes.from_list\" : List(Nat) -> Bytes\nexport a := 1\n";
        assert_eq!(
            prepare_module_source(src),
            "{ \"a\": 1, \"from_list\": \"bytes.from_list\" }"
        );
    }

    /// A module with only private bindings wraps in `do { … }` with a
    /// trailing record projecting every bound name.
    #[test]
    fn no_exports_projects_all_bindings() {
        let src = "helper :: x ~> x + 1\nother := helper(1)";
        assert_eq!(
            transform_export_to_object(src, &[]),
            "do {\nhelper :: x ~> x + 1\nother := helper(1)\n{ helper: helper, other: other }\n}"
        );
    }

    /// A malformed `export` — no binding separator — must never silently
    /// drop into an empty record (mirrors `module_prep`'s `LangError`).
    #[test]
    #[should_panic(expected = "cannot transform `export` declaration")]
    fn separatorless_export_fails_loudly() {
        transform_export_to_object("export just_a_name\n", &[]);
    }

    /// An `export` with a separator but no binding name is loud too.
    #[test]
    #[should_panic(expected = "no binding name")]
    fn nameless_export_fails_loudly() {
        transform_export_to_object("export := 1\n", &[]);
    }
}
