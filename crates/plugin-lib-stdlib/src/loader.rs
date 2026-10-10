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
    src.lines()
        .filter(|line| is_top_import_line(line))
        .filter_map(|line| extract_quoted_path(line).map(|(path, _)| path))
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
    for line in src.lines() {
        let trimmed = line.trim_start();
        if is_top_import_line(line) {
            if let Some((_, reexports)) = from_export_to_import(line) {
                for (name, _) in &reexports {
                    names.push(name.clone());
                }
                for (name, _) in reexports {
                    push_all(&mut all, name);
                }
            } else if import_line_spec(line).is_some() {
                for local in import_bound_locals(line) {
                    push_all(&mut all, local);
                }
            }
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            if trimmed.starts_with("export ") {
                let rest = &trimmed["export ".len()..];
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
    }
    if names.is_empty() {
        // A no-exports module whose last statement is a `do { … }` or
        // record literal produces that value as the module record —
        // merge its field keys so the registered functions cover them
        // (mirrors `handl_runtime::module_prep::module_export_names`).
        for key in trailing_record_keys(&strip_comment_lines(&src)) {
            push_all(&mut all, key);
        }
        all
    } else {
        names
    }
}

fn transform_export_to_object(src: &str, op_names: &[String]) -> String {
    // Each top-level binding is collected as `(name, expr_lines)`.
    // Non-private entries become object fields (`{ name: expr }`); private
    // entries become `do`-block assignments (`name := expr;`) emitted *before*
    // the object so exported closures can reference private helpers such as
    // `clock_gettime` in `time-c/clock_gettime.hnl`.
    let mut entries: Vec<(String, Vec<String>)> = Vec::new();
    let mut privates: Vec<(String, Vec<String>)> = Vec::new();
    let mut current: Option<(String, Vec<String>)> = None;
    let mut current_is_private: bool = false;
    // Whether the module's last top-level statement is a binding. When it
    // is not (a trailing `do { … }` or record literal) the body's own
    // value is the module record and no name projection is appended —
    // mirrors `handl_runtime::module_prep`.
    let mut tail_is_binding = true;

    // Finalize `current` into the right collection.
    macro_rules! finalize {
        () => {
            if let Some(entry) = current.take() {
                if current_is_private {
                    privates.push(entry);
                } else {
                    entries.push(entry);
                }
                current_is_private = false;
            }
        };
    }

    for line in src.lines() {
        let trimmed = line.trim_start();
        if is_top_import_line(line) {
            finalize!();
            if let Some((_, names)) = from_export_to_import(line) {
                for (name, alias) in names {
                    entries.push((name, vec![alias]));
                }
            }
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') && trimmed.starts_with("export ") {
            finalize!();
            let rest = &trimmed["export ".len()..];
            if let Some((sep_pos, sep_len)) = find_binding_sep(rest) {
                let name = strip_export_name(&rest[..sep_pos]);
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
                current = Some((name, expr_lines));
                current_is_private = false;
                tail_is_binding = true;
            }
            continue;
        } else if !line.starts_with(' ') && !line.starts_with('\t') {
            // Column-0 line that is not an import or `export` declaration.
            // Detect a *private* (non-exported) top-level binding so that
            // helper functions are preserved instead of dropped.
            if let Some((sep_pos, sep_len)) = find_binding_sep(trimmed) {
                let name = strip_export_name(&trimmed[..sep_pos]).trim().to_string();
                if is_valid_ident(&name) {
                    finalize!();
                    let is_type_only =
                        sep_len == 1 && trimmed.as_bytes().get(sep_pos) == Some(&b':');
                    let expr_start = if is_type_only {
                        "null"
                    } else {
                        trimmed[sep_pos + sep_len..].trim()
                    };
                    let mut expr_lines = Vec::new();
                    if !expr_start.is_empty() {
                        expr_lines.push(expr_start.to_string());
                    }
                    current = Some((name, expr_lines));
                    current_is_private = true;
                    tail_is_binding = true;
                    continue;
                }
            }
            if current.is_none() && !trimmed.is_empty() {
                tail_is_binding = false;
            }
        }
        if let Some((_, lines)) = &mut current {
            if !line.trim().is_empty() {
                lines.push(line.to_string());
            }
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
        if local.is_empty() || !is_valid_ident(local) || entries.iter().any(|(n, _)| n == local) {
            continue;
        }
        entries.push((local.to_string(), vec![format!("\"{qualified}\"")]));
    }

    let imports = collect_executable_import_lines(src);

    // No `export` declarations: the module record is *every* top-level
    // bound name — private bindings plus the locals each hoisted import
    // statement binds. The body is wrapped in `do { … }` (imports first,
    // verbatim statements next, the record last) so the whole module
    // evaluates as one expression under the single-statement parser —
    // mirrors `loader.hnl`'s `transform_export_to_object` and
    // `handl_runtime::module_prep`.
    if entries.is_empty() {
        if imports.is_empty() && privates.is_empty() {
            return src.to_string();
        }
        let stripped: String = src
            .lines()
            .filter(|line| !is_top_import_line(line))
            .collect::<Vec<_>>()
            .join("\n");
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
            for (name, _) in &privates {
                push_field(name);
            }
            for line in src.lines() {
                if is_top_import_line(line) && import_line_spec(line).is_some() {
                    for local in import_bound_locals(line) {
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
    // export object literal. A name that isn't a valid identifier (e.g.
    // the keyword `first`) needs a quoted record key.
    let mut body = String::new();
    for (name, expr_lines) in &privates {
        if !is_valid_ident(name) {
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
    for (i, (name, expr_lines)) in entries.iter().enumerate() {
        if i > 0 {
            body.push_str(", ");
        }
        // Always quote record keys: an export name may be a keyword
        // (`first`, `export`) that lexes as `token.keyword.*`, not an
        // identifier — `is_valid_ident` is only a shape check.
        body.push('"');
        body.push_str(name);
        body.push_str("\": ");
        body.push_str(expr_lines.join("\n").trim());
    }
    body.push_str(" }");

    if imports.is_empty() {
        body
    } else {
        format!("do {{\n{}\n{}\n}}", imports.join("\n"), body)
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

fn is_top_import_line(line: &str) -> bool {
    !line.starts_with(' ') && !line.starts_with('\t') && is_import_line(line.trim_start())
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

/// Top-level import lines that name a quoted specifier — every quoted
/// spec is executable here because `load_hnl_file` pre-registers each as
/// a module dependency (file paths relative to the module dir, bare
/// names via the stdlib package table). Bare `import <name>` declarations
/// are decorative and excluded.
fn collect_executable_import_lines(src: &str) -> Vec<String> {
    src.lines()
        .filter(|line| is_top_import_line(line))
        .filter(|line| import_line_spec(line).is_some())
        .map(|line| {
            let line = from_export_to_import(line)
                .map(|(import_line, _)| import_line)
                .unwrap_or_else(|| line.to_string());
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
