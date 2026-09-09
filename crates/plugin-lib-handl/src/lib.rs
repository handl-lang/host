//! # handl-lib-handl (generic `.hnl` library loader)
//!
//! A generic crate that lets `.hnl` files replace hand-rolled Rust
//! `plugin-lib-*` bridge crates for pure-logic libraries.
//!
//! Each former pure-logic library crate (e.g. `plugin-lib-stats`,
//! `plugin-lib-hello` — both now `plugin-lib-stdlib` package-table rows)
//! had ~100+ lines of Rust bridge code that
//! `include_str!`s a `.hnl` file, builds a cached per-thread `Runtime`, and
//! evaluates a small applied expression per call. This crate eliminates that
//! boilerplate with two reusable types:
//!
//! - **[`HandlModule`]** — implements [`Module`](handl_platform::Module),
//!   exposing every field of a `.hnl` record as a callable
//!   [`Function`](handl_platform::Function). Each call evaluates
//!   `(source).field(args)` via a cached per-thread `Runtime`.
//! - **[`HandlLibraryPlugin`]** — implements
//!   [`Plugin`](handl_platform::Plugin), contributing a [`HandlModule`] into
//!   the platform's [`ModuleRuntime`] during `contribute()`.
//!
//! ## Key constraint (thread-local AST)
//!
//! The tree-walk engine indexes into a thread-local `CURRENT_AST` arena, so a
//! handl closure `Value` produced by one bridge runtime cannot be cached and
//! called later from an outer runtime. Therefore the `Runtime` is cached
//! per-thread (via [`thread_local!`]), and the applied expression
//! `(source).func(__arg_N)` — whose text is argument-independent (arguments
//! cross the boundary as `__arg_N` globals) — is lexed/parsed **once** per
//! (module, function, arity) into a per-thread `AST_CACHE`
//! `Rc<Ast>` and re-evaluated per call through
//! [`Runtime::eval_shared_with_globals`], which installs the cached arena
//! without a deep clone. Caching the *evaluated* record is not possible
//! across the `serde_json::Value` boundary: arrows capture earlier
//! `do`-block bindings, and materializing each arrow's `env` expands
//! recursively (exponential on ~100-binding modules like `str-handl`).
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_lib_handl::{HandlModule, HandlLibraryPlugin};
//! use handl_plugin_lib_stdlib as stdlib;
//!
//! const STATS_HNL: &str = include_str!("../../../handl-packages/stats/stats.hnl");
//! const STATS_FUNCS: &[&str] = &["sum", "avg", "product", "min", "max", "median", "stddev"];
//!
//! let module = HandlModule::new("stats", STATS_HNL, STATS_FUNCS)
//!     .with_dependency(MathModule)
//!     .with_dependency(TypeModule);
//!
//! // Or as a plugin:
//! let plugin = HandlLibraryPlugin::new("stats", "lib.stats", "lib-stats", STATS_HNL, STATS_FUNCS);
//! // or via the package table: stdlib::library("stats")
//! ```
//!
//! ## Text transforms (ADR-0094)
//!
//! The text-level transform logic (`strip_declaration_lines`, `parse_*`,
//! `expand_tag_declarations`, export→object rewriting, etc.) is canonically
//! implemented in `handl-packages/loader/loader.hnl` — fully, in pure handl
//! over the `char`/`str` floors.  This crate keeps a two-level seam:
//!
//! - `mod bootstrap` — the irreducible Rust implementations.  They prepare
//!   the `str` floor itself (`str-handl` needs `from "char" import`
//!   stripping before any loader exists — the bootstrap paradox) and are
//!   the correctness fallback: [`HandlLoader`] runs its module on the
//!   **bytecode** eval engine (`HandlModule::with_eval_engine`), which
//!   removed the interpreted `str.*`/`char.*` per-character cost that made
//!   the loader prohibitive under tree-walk (~59s → ~0.16s for
//!   `strip_decls` on the 22k-char `str-handl` source; see
//!   `examples/loader_bench.rs`).
//! - [`HandlLoader`] / [`install_loader`] — the opt-in self-hosting seam.
//!   Once a host installs a loader built over `char` + `str` modules, every
//!   public transform here (and [`HandlModule::new`]'s `analyze_source`
//!   pipeline) delegates to `loader.hnl`, falling back to `bootstrap` on
//!   any loader error.
//!
//! `HandlModule::new_raw` loads the `loader` itself raw, without applying
//! those transforms to its own source.

pub mod lint;
pub mod op_registry;
pub mod transform;
pub use lint::HandlNativeLint;
pub use op_registry::{OpFunction, OpRegistry, RuntimePredicateChecker};
pub use transform::{HandlNativeRewrite, HANDL_PREFIX, KEEP_KEY};

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use handl_language::Source;
use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    EvalContext, EvalError, Function, Module, ModuleLookup, PlatformError, Plugin, PluginContext,
};
use handl_plugin_module::{ModuleRule, ModuleRuntime};
use handl_runtime::{EngineRegistry, Runtime, RuntimeConfig};

// ---------------------------------------------------------------------------
// Module resolver (ADR-0077: data-driven dependency wiring)
// ---------------------------------------------------------------------------

/// Resolves module names (e.g. `"str"`, `"fn"`) to [`Module`] implementations.
///
/// Used by [`HandlModule::with_resolver`] to auto-wire dependencies declared
/// in `.hnl` source via `from 'str' import * as 'str'` lines, replacing
/// hardcoded `.with_dependency(...)` calls in Rust.
pub trait ModuleResolver: Send + Sync {
    /// Resolve a module name to a shared [`Module`] impl, or `None` if unknown.
    fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>>;
}

// ---------------------------------------------------------------------------
// Helpers (generalized from the stats/hello bridges)
// ---------------------------------------------------------------------------

/// A parsed import declaration from a `.hnl` source file.
#[derive(Debug, Clone)]
pub struct ImportDecl {
    /// The module path being imported from (e.g. `"str"`, `"fn"`).
    pub path: String,
    /// The local alias for the imported module (e.g. `"str"`).
    pub alias: String,
}

// ---------------------------------------------------------------------------
// Bootstrap transforms (irreducible Rust seam)
// ---------------------------------------------------------------------------
//
// These line-based text transforms are the *bootstrap* implementations of
// `handl-packages/loader/loader.hnl`.  They are required because the loader
// itself needs `str`/`char` to run, and `str` (`str-handl`) is a `.hnl`
// module that can only be prepared by these transforms — the bootstrap
// paradox.  Once a [`HandlLoader`] is installed via [`install_loader`], the
// public functions below delegate to the handl implementations and these
// remain only as the bootstrap path and correctness fallback.
mod bootstrap {
    use super::ImportDecl;
    /// Strip `--` comment lines from a `.hnl` source so the embedded source
    /// parses as a bare expression.
    ///
    /// Data files often have a `--` header comment that documents the
    /// definitions but is not valid inside the applied `(source).func(input)`
    /// form the Pratt parser evaluates, so it is removed before evaluation.
    pub(super) fn strip_comment_lines(src: &str) -> String {
    src.lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip `cap`/`op`/`handl` declaration lines (and their indented
/// continuation lines) from a `.hnl` source, leaving only the handl logic.
///
/// This is the companion to [`strip_comment_lines`]: `strip_comment_lines`
/// removes `--`/`---` comment lines, while this function removes the
/// ADR-0077 declaration forms (`cap "..."`, `op "..." : ...`,
/// `handl "..." = ...`) and their indented continuation lines
/// (`guarantee:`, `requires:`, `termination:`). Together they prepare a
/// `.hnl` source for evaluation as a bare handl expression.
///
/// This is necessary because `.hnl` files may now contain `cap`/`op`/`handl`
/// declarations (ADR-0077) alongside the handl logic. The [`HandlModule`]
/// evaluates the source as a single handl expression, so declaration lines
/// (which are not valid handl expressions) must be removed first.
///
/// # Example
///
/// ```
/// use handl_plugin_lib_handl::strip_declaration_lines;
///
/// let src = r#"cap "hello.greet"
/// op "hello.greet" : String -> String
///   guarantee: True
/// name ~> "Hello, " + name + "!"
/// "#;
/// let cleaned = strip_declaration_lines(src);
/// assert!(!cleaned.contains("cap"));
/// assert!(!cleaned.contains("op "));
/// assert!(cleaned.contains("name ~>"));
/// ```
pub(super) fn strip_declaration_lines(src: &str) -> String {
    src.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            // Strip indented continuation lines from `op` declarations
            // (e.g. `guarantee:`, `requires:`, `termination:`).
            if line.starts_with(' ') || line.starts_with('\t') {
                if trimmed.starts_with("guarantee")
                    || trimmed.starts_with("requires")
                    || trimmed.starts_with("termination")
                {
                    return false;
                }
            }
            // Strip `cap "..."`, `op "..."`, and `handl "..." = ...` lines.
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
            // Strip `import ...` and `from ... import ...` lines.
            if is_import_line(trimmed) {
                return false;
            }
            true
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Transform top-level `export name = expr` declarations into an object
/// literal `{ name: expr, ... }` so the source evaluates to a record of
/// exported functions.
///
/// Each `export name = expr` line (at column 0) starts a binding. The
/// expression continues until the next top-level `export` line or end of
/// source. The collected expressions are joined into an object literal:
///
/// ```text
/// { name1: expr1, name2: expr2, ... }
/// ```
///
/// If no `export` lines are found, the source is returned unchanged (the
/// existing object-literal form is preserved).
pub(super) fn transform_export_to_object(src: &str) -> String {
    // Collect (name, expr_lines) pairs from top-level export lines.
    let mut entries: Vec<(String, Vec<String>)> = Vec::new();
    let mut current: Option<(String, Vec<String>)> = None;

    for line in src.lines() {
        let trimmed = line.trim_start();
        // A top-level export line starts at column 0 with `export `.
        if !line.starts_with(' ') && !line.starts_with('\t') && trimmed.starts_with("export ") {
            // Flush the previous entry.
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            let rest = &trimmed["export ".len()..];
            // Find the binding separator (`::`, or legacy `=`).
            if let Some((sep_pos, sep_len)) = find_binding_sep(rest) {
                let name = rest[..sep_pos].trim().to_string();
                let expr_start = rest[sep_pos + sep_len..].trim();
                let mut expr_lines = Vec::new();
                if !expr_start.is_empty() {
                    expr_lines.push(expr_start.to_string());
                }
                current = Some((name, expr_lines));
            }
            // If no `=` found, skip (not a valid export binding).
        } else if let Some((_, ref mut lines)) = current {
            // Accumulate indented continuation lines (part of the export
            // expression body). Stop at blank lines only if the next
            // non-blank line is a top-level export.
            if !line.trim().is_empty() {
                lines.push(line.to_string());
            }
        }
    }
    if let Some(entry) = current.take() {
        entries.push(entry);
    }

    if entries.is_empty() {
        return src.to_string();
    }

    // Build the object literal.
    let mut out = String::from("{ ");
    for (i, (name, expr_lines)) in entries.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(name);
        out.push_str(": ");
        let expr = expr_lines.join("\n");
        out.push_str(expr.trim());
    }
    out.push_str(" }");
    out
}

/// Find the position and length of the binding separator in `s`: the `::`
/// bind token (canonical) or a legacy `=` (skipping `==`, `>=`, `<=`, `!=`,
/// `=>`). `::` takes precedence — a lone `:` (type annotations, object
/// fields, `match` arms) is not a separator. Returns `None` if no binding
/// separator is found.
pub(super) fn find_binding_sep(s: &str) -> Option<(usize, usize)> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut depth: i32 = 0;
    let mut first_eq: Option<usize> = None;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b':' if depth == 0 => {
                if i + 1 < bytes.len() && bytes[i + 1] == b':' {
                    return Some((i, 2));
                }
            }
            b'=' if depth == 0 => {
                let prev = if i > 0 { bytes[i - 1] } else { 0 };
                let next = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
                if next == b'=' || prev == b'>' || prev == b'<' || prev == b'!' || next == b'>' {
                    i += 1;
                    continue;
                }
                // Legacy `=` bind: remember the first one, but keep
                // scanning for a `::` that precedes it in the same line
                // (an `=` earlier would already have returned).
                if first_eq.is_none() {
                    first_eq = Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    first_eq.map(|pos| (pos, 1))
}

/// Returns true if the line is an import declaration (`import ...` or
/// `from ... import ...`).
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

/// Parse import declarations from a `.hnl` source string.
///
/// Supports: `from 'path' import * as 'alias'` and `import "path" as alias`.
pub(super) fn parse_import_declarations(src: &str) -> Vec<ImportDecl> {
    src.lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("--") {
                return None;
            }
            parse_from_import_line(trimmed).or_else(|| parse_legacy_import_line(trimmed))
        })
        .collect()
}

fn parse_from_import_line(line: &str) -> Option<ImportDecl> {
    let rest = line.strip_prefix("from")?.trim();
    if rest.is_empty() {
        return None;
    }
    let (path, after_path) = extract_quoted(rest)?;
    let import_kw = after_path.trim().strip_prefix("import")?.trim();
    let star = import_kw.strip_prefix("*")?.trim();
    let as_kw = star.strip_prefix("as")?.trim();
    if as_kw.is_empty() {
        return None;
    }
    let alias = if as_kw.starts_with('"') || as_kw.starts_with('\'') {
        extract_quoted(as_kw)?.0
    } else {
        as_kw.split_whitespace().next()?.to_string()
    };
    if alias.is_empty() {
        return None;
    }
    Some(ImportDecl { path, alias })
}

fn parse_legacy_import_line(line: &str) -> Option<ImportDecl> {
    let rest = line.strip_prefix("import")?.trim();
    if rest.is_empty() {
        return None;
    }
    let (path, after) = extract_quoted(rest)?;
    let after = after.trim();
    let alias = if let Some(as_part) = after.strip_prefix("as") {
        let a = as_part.trim();
        if a.is_empty() {
            return None;
        }
        if a.starts_with('"') || a.starts_with('\'') {
            extract_quoted(a)?.0
        } else {
            a.split_whitespace().next()?.to_string()
        }
    } else {
        path.rsplit('/').next().unwrap_or(&path).to_string()
    };
    Some(ImportDecl { path, alias })
}

fn extract_quoted(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    let quote = s.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &s[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some((rest[..end].to_string(), &rest[end + quote.len_utf8()..]))
}

/// Parse `cap` declarations from a `.hnl` source string (ADR-0077 Phase 5).
///
/// Scans each line for the top-level `cap "name"` form, extracting the
/// quoted capability name. Comment lines (`-- ...`) are skipped. Lines that
/// do not match the `cap` form are silently ignored (resilient parsing).
///
/// This is a lightweight line-based scan that avoids a dependency on
/// `plugin-ext-refinement` (which is inference-side). It mirrors the
/// `parse_cap_line` logic in `arrow_parser.rs`.
///
/// # Example
///
/// ```
/// use handl_plugin_lib_handl::parse_cap_declarations;
///
/// let src = r#"
/// cap "fs.read"
/// cap "fs.write"
/// op "fs.read" : Path -> Bytes
/// "#;
/// let caps = parse_cap_declarations(src);
/// assert_eq!(caps, vec!["fs.read".to_string(), "fs.write".to_string()]);
/// ```
pub(super) fn parse_cap_declarations(src: &str) -> Vec<String> {
    let mut caps = Vec::new();
    for line in src.lines() {
        let trimmed = line.trim();
        // Skip comment lines and empty lines.
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }
        // Check for `cap` prefix followed by whitespace (whole-word match).
        let rest = match trimmed.strip_prefix("cap") {
            Some(rest) if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') => {
                rest.trim_start()
            }
            _ => continue,
        };
        // Extract the quoted capability name.
        if let Some(name) = extract_quoted_cap_name(rest) {
            caps.push(name);
        }
    }
    caps
}

/// Extract a quoted string from the start of `s`, returning the content.
///
/// Supports both `"..."` and `'...'` quoting. Returns `None` if the string
/// does not start with a quote or has no closing quote.
fn extract_quoted_cap_name(s: &str) -> Option<String> {
    let s = s.trim_start();
    let quote = s.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &s[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

/// Parse `tag` declarations from a `.hnl` source string (ADR-0077).
///
/// Scans each line for the top-level `tag name = Type` form, extracting the
/// tag name and payload type. Comment lines (`-- ...`) are skipped. Lines
/// that do not match the `tag` form are silently ignored (resilient
/// parsing).
///
/// This is a lightweight line-based scan that mirrors the `parse_tag_line`
/// logic in `arrow_parser.rs`. It returns `(name, payload_type)` pairs.
///
/// # Example
///
/// ```
/// use handl_plugin_lib_handl::parse_tag_declarations;
///
/// let src = r#"
/// tag ok = Value
/// tag err = Error
/// "#;
/// let tags = parse_tag_declarations(src);
/// assert_eq!(tags, vec![("ok".to_string(), "Value".to_string()),
///                       ("err".to_string(), "Error".to_string())]);
/// ```
pub(super) fn parse_tag_declarations(src: &str) -> Vec<(String, String)> {
    let mut tags = Vec::new();
    for line in src.lines() {
        let trimmed = line.trim();
        // Skip comment lines and empty lines.
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }
        // Check for `tag` prefix followed by whitespace (whole-word match).
        let rest = match trimmed.strip_prefix("tag") {
            Some(rest) if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') => {
                rest.trim_start()
            }
            _ => continue,
        };
        // Find the `::` (canonical) or legacy `=` separator.
        let sep = rest
            .find("::")
            .map(|i| (i, 2))
            .or_else(|| rest.find('=').map(|i| (i, 1)));
        if let Some((eq_idx, sep_len)) = sep {
            let name = rest[..eq_idx].trim().to_string();
            let payload_type = rest[eq_idx + sep_len..].trim().to_string();
            if !name.is_empty()
                && !payload_type.is_empty()
                && name.chars().all(|c| c.is_alphanumeric() || c == '_')
            {
                tags.push((name, payload_type));
            }
        }
    }
    tags
}

/// Expand `tag` declarations in `.hnl` source into constructor bindings
/// (ADR-0077).
///
/// Scans for `tag name = Type` lines, strips them from the source, and
/// generates constructor bindings: `name = _0 ~> tag.tag("name", {value: _0})`.
/// If there are tag declarations, the remaining source is wrapped in a
/// `do { ... }` block with the constructor bindings prepended.
///
/// If there are no tag declarations, the source is returned unchanged.
///
/// # Example
///
/// Input:
/// ```text
/// tag ok = Value
/// tag err = Error
///
/// {
///   result_ok: value ~> value >>> ok,
/// }
/// ```
///
/// Output:
/// ```text
/// do {
///   ok = _0 ~> tag.tag("ok", {value: _0})
///   err = _0 ~> tag.tag("err", {value: _0})
///   {
///     result_ok: value ~> value >>> ok,
///   }
/// }
/// ```
pub(super) fn expand_tag_declarations(src: &str) -> String {
    let tags = parse_tag_declarations(src);
    if tags.is_empty() {
        return src.to_string();
    }
    // Strip `tag` declaration lines from the source.
    let remaining: String = src
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                return true; // Keep comments and blank lines.
            }
            // Skip lines that are `tag` declarations.
            if let Some(rest) = trimmed.strip_prefix("tag") {
                if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                    let sep_idx = rest.find("::").or_else(|| rest.find('='));
                    if let Some(eq_idx) = sep_idx {
                        let name = rest[..eq_idx].trim();
                        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                        {
                            return false;
                        }
                    }
                }
            }
            true
        })
        .collect::<Vec<_>>()
        .join("\n");
    // Build the `do { ... }` block with constructor bindings.
    let mut out = String::from("do {\n");
    for (name, _payload_type) in &tags {
        out.push_str(&format!(
            "  {name} :: _0 ~> tag.tag(\"{name}\", {{value: _0}})\n"
        ));
    }
    // Append the remaining source, indented by 2 spaces.
    for line in remaining.lines() {
        if line.trim().is_empty() {
            out.push('\n');
        } else {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str("}");
    out
}

}

// ---------------------------------------------------------------------------
// Public transform API — delegates to the installed [`HandlLoader`]
// (`handl-packages/loader/loader.hnl`) when present, else to `bootstrap`.
// ---------------------------------------------------------------------------

fn loader_string(func: &str, src: &str) -> Option<String> {
    match loader_call(func, src) {
        Some(serde_json::Value::String(s)) => Some(s),
        _ => None,
    }
}

/// Strip `cap`/`op`/`handl`/`import` declaration lines (and their indented
/// `guarantee:`/`requires:`/`termination:` continuation lines) from a `.hnl`
/// source.  Delegates to `loader.strip_decls` when a [`HandlLoader`] is
/// installed; otherwise uses the bootstrap implementation.
///
/// # Example
///
/// ```
/// use handl_plugin_lib_handl::strip_declaration_lines;
///
/// let src = r#"cap "hello.greet"
/// op "hello.greet" : String -> String
///   guarantee: True
/// name ~> "Hello, " + name + "!"
/// "#;
/// let cleaned = strip_declaration_lines(src);
/// assert!(!cleaned.contains("cap"));
/// assert!(!cleaned.contains("op "));
/// assert!(cleaned.contains("name ~>"));
/// ```
pub fn strip_declaration_lines(src: &str) -> String {
    loader_string("strip_decls", src)
        .unwrap_or_else(|| bootstrap::strip_declaration_lines(src))
}

/// Parse `from 'path' import * as 'alias'` / `import "path" as alias`
/// declarations.  Delegates to `loader.parse_imports` when a [`HandlLoader`]
/// is installed; otherwise uses the bootstrap implementation.
pub fn parse_import_declarations(src: &str) -> Vec<ImportDecl> {
    if let Some(v) = loader_call("parse_imports", src) {
        if let Some(imports) = imports_from_json(&v) {
            return imports;
        }
    }
    bootstrap::parse_import_declarations(src)
}

/// Parse `cap "name"` declarations (ADR-0077 Phase 5).  Delegates to
/// `loader.parse_caps` when a [`HandlLoader`] is installed; otherwise uses
/// the bootstrap implementation.
pub fn parse_cap_declarations(src: &str) -> Vec<String> {
    if let Some(v) = loader_call("parse_caps", src) {
        if let Some(caps) = caps_from_json(&v) {
            return caps;
        }
    }
    bootstrap::parse_cap_declarations(src)
}

/// Parse `tag name <::|=> Type` declarations into `(name, payload_type)`
/// pairs.  Delegates to `loader.parse_tags` when a [`HandlLoader`] is
/// installed; otherwise uses the bootstrap implementation.
pub fn parse_tag_declarations(src: &str) -> Vec<(String, String)> {
    if let Some(v) = loader_call("parse_tags", src) {
        if let Some(tags) = tags_from_json(&v) {
            return tags;
        }
    }
    bootstrap::parse_tag_declarations(src)
}

/// Expand `tag` declarations into `tag.tag` constructor bindings wrapped in
/// a `do { ... }` block.  Delegates to `loader.expand_tags` when a
/// [`HandlLoader`] is installed; otherwise uses the bootstrap implementation.
pub fn expand_tag_declarations(src: &str) -> String {
    loader_string("expand_tags", src)
        .unwrap_or_else(|| bootstrap::expand_tag_declarations(src))
}

/// Strip `--` comment lines.  Delegates to `loader.strip_comments` when a
/// [`HandlLoader`] is installed; otherwise uses the bootstrap implementation.
pub fn strip_comment_lines(src: &str) -> String {
    loader_string("strip_comments", src)
        .unwrap_or_else(|| bootstrap::strip_comment_lines(src))
}

/// Rewrite top-level `export name <::|=> expr` declarations into an object
/// literal.  Delegates to `loader.transform_export_to_object` when a
/// [`HandlLoader`] is installed; otherwise uses the bootstrap implementation.
pub fn transform_export_to_object(src: &str) -> String {
    loader_string("transform_export_to_object", src)
        .unwrap_or_else(|| bootstrap::transform_export_to_object(src))
}

/// The result of running the `.hnl` text transforms over a module source:
/// the evaluatable source plus the declaration scans the host needs.
struct SourceAnalysis {
    prepared: String,
    imports: Vec<ImportDecl>,
}

fn imports_from_json(v: &serde_json::Value) -> Option<Vec<ImportDecl>> {
    let arr = v.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let path = item.get("path")?.as_str()?.to_string();
        let alias = item.get("alias")?.as_str()?.to_string();
        out.push(ImportDecl { path, alias });
    }
    Some(out)
}

fn caps_from_json(v: &serde_json::Value) -> Option<Vec<String>> {
    let arr = v.as_array()?;
    arr.iter()
        .map(|x| x.as_str().map(|s| s.to_string()))
        .collect()
}

fn tags_from_json(v: &serde_json::Value) -> Option<Vec<(String, String)>> {
    let arr = v.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let name = item.get("name")?.as_str()?.to_string();
        let payload = item.get("payload")?.as_str()?.to_string();
        out.push((name, payload));
    }
    Some(out)
}

/// Run the full source-preparation pipeline once.  When a [`HandlLoader`]
/// is installed this is a single `loader.analyze` evaluation; otherwise it
/// runs the bootstrap transforms.
fn analyze_source(src: &str) -> SourceAnalysis {
    if let Some(v) = loader_call("analyze", src) {
        if let (Some(prepared), Some(imports)) = (
            v.get("source").and_then(|s| s.as_str()).map(|s| s.to_string()),
            v.get("imports").and_then(imports_from_json),
        ) {
            return SourceAnalysis { prepared, imports };
        }
    }
    let stripped = bootstrap::strip_declaration_lines(&bootstrap::strip_comment_lines(src));
    let exported = bootstrap::transform_export_to_object(&stripped);
    SourceAnalysis {
        prepared: bootstrap::expand_tag_declarations(&exported),
        imports: bootstrap::parse_import_declarations(src),
    }
}

/// Escape a Rust string for embedding inside a handl `"..."` string literal.
///
/// Only `"` and `\` need escaping: the delimited lexer consumes any other
/// character (including newlines) raw, and the Pratt parser's string-literal
/// unescaping only rewrites the four canonical escapes (`\\`, `\"`, `\n`,
/// `\t`), so all other characters pass through faithfully.
///
/// This is provided as a public helper for callers that need to construct
/// handl string literals manually (e.g. for single-string-argument bridges
/// like the `hello` greet function).
pub fn escape_handl_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}

/// Whether `value` contains an arrow value that carries no `__home__`
/// frame id — i.e. an arrow whose `body` index refers to the *ambient*
/// eval's AST and must be stamped before it crosses a runtime boundary.
fn contains_unhomed_arrow(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(a) => a.iter().any(contains_unhomed_arrow),
        serde_json::Value::Object(o) => {
            if o.get("__arrow__").and_then(|v| v.as_bool()) == Some(true) {
                return o.get("__home__").and_then(|v| v.as_u64()).is_none()
                    || o.values().any(contains_unhomed_arrow);
            }
            o.values().any(contains_unhomed_arrow)
        }
        _ => false,
    }
}

/// Stamp every arrow value inside `value` that lacks a `__home__` frame id
/// with `frame_id` — recursively, including arrows nested inside a
/// closure's captured `env` (their `body` indices refer to the same AST).
///
/// `__home__` records the eval frame (AST + Language + module chain) that
/// minted the arrow so the receiving runtime can route application back to
/// it — see `plugin-engine-exec-tree-walk`'s home-frame registry. Arrows
/// that already carry a `__home__` keep it: their provenance is elsewhere.
fn stamp_home(value: &mut serde_json::Value, frame_id: u64) {
    match value {
        serde_json::Value::Array(a) => {
            for v in a.iter_mut() {
                stamp_home(v, frame_id);
            }
        }
        serde_json::Value::Object(o) => {
            if o.get("__arrow__").and_then(|v| v.as_bool()) == Some(true)
                && o.get("__home__").and_then(|v| v.as_u64()).is_none()
            {
                o.insert("__home__".to_string(), serde_json::Value::from(frame_id));
            }
            for v in o.values_mut() {
                stamp_home(v, frame_id);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Per-thread runtime cache
// ---------------------------------------------------------------------------

/// Global counter for assigning unique module IDs.
///
/// Each [`HandlModule`] instance gets a unique ID at construction time. This
/// ID is used as a key in the per-thread runtime cache, so that each thread
/// can independently build and cache a `Runtime` for the module.
static NEXT_MODULE_ID: AtomicUsize = AtomicUsize::new(0);

/// A shared, thread-safe runtime builder closure.
///
/// The builder captures the module's dependency configuration (which is
/// `Send + Sync`) but produces a non-`Send` `Runtime`. The builder itself
/// is `Send + Sync`, so it can be shared across threads via [`Arc`]; each
/// thread calls it independently to construct its own per-thread `Runtime`.
type RuntimeBuilder = Arc<dyn Fn() -> Result<Runtime, handl_language::LangError> + Send + Sync>;

thread_local! {
    /// Per-thread cache of runtimes, keyed by module ID. Each thread
    /// independently builds and caches a `Runtime` for each [`HandlModule`]
    /// on first use. This is necessary because the tree-walk engine stores
    /// the active AST / `Language` in thread-locals, so `Runtime` is not
    /// `Send`.
    static RUNTIME_CACHE: RefCell<HashMap<usize, std::rc::Rc<Runtime>>> = RefCell::new(HashMap::new());

    /// Per-thread cache of parsed applied-form ASTs.
    ///
    /// [`HandlFunction::call`] evaluates the applied expression
    /// `(source).func(__arg_0, ..., __arg_{n-1})`; that source text is a
    /// pure function of `(module id, function name, arity)` — call
    /// arguments cross the boundary as `__arg_N` globals, never as source
    /// text — so the lexed/parsed/desugared [`handl_language::Ast`] can be
    /// parsed once and reused across calls. This removes the per-call
    /// re-parse of the full module source that made the interpreted
    /// `loader.hnl` transforms pathologically slow (every nested `str.*`
    /// call re-parsed the ~740-line `str-handl` bridge source).
    ///
    /// The cache is thread-local (like `RUNTIME_CACHE`) because `Ast`
    /// is not `Send`. It is keyed by module ID rather than source identity:
    /// a module's prepared source is fixed at construction, and parse does
    /// not depend on the module's dependency set (`with_dependency` only
    /// changes the eval-time module chain), so entries stay valid across
    /// `rebuild_functions` (which allocates a fresh module ID anyway).
    ///
    /// Note: caching the *evaluated* module record instead of the AST is
    /// not possible across the `serde_json::Value` boundary — arrows in
    /// `do { ... }`-block modules capture earlier bindings, and
    /// materializing the record would expand each arrow's captured `env`
    /// recursively (exponential blowup on ~100-binding modules such as
    /// `str-handl`). Record *construction* inside the per-call eval is
    /// cheap (`Rc` env captures), so the AST cache is the right seam.
    static AST_CACHE: RefCell<HashMap<(usize, String, usize), std::rc::Rc<handl_language::Ast>>> =
        RefCell::new(HashMap::new());
}

/// Allocate a unique module ID (called at [`HandlModule`] construction time).
fn allocate_module_id() -> usize {
    NEXT_MODULE_ID.fetch_add(1, Ordering::Relaxed)
}

/// Build a minimal runtime for evaluating `.hnl` sources, optionally with
/// dependency modules registered in the module registry.
///
/// Reuses the canonical `Language` (assembled via
/// [`handl_kit_data_canonical`]) bound to the data lexer, Pratt parser, and
/// tree-walk evaluator. If `dependencies` is non-empty, a
/// [`ModuleRegistry`](handl_plugin_module::ModuleRegistry) is wired with the
/// given modules and layered under the canonical `FpHostFunctions` (for
/// field-access support). If no dependencies are needed, no module registry
/// is wired (matching the `hello` bridge pattern).
fn build_runtime(
    dependencies: Vec<Arc<dyn Module + Send + Sync>>,
    eval_name: &str,
) -> Result<Runtime, handl_language::LangError> {
    let kit = handl_kit_data_canonical::assemble()?;
    let lang = kit.lang().clone();
    let config = RuntimeConfig::new()
        .with_lex("data")
        .with_parse("pratt")
        .with_eval(eval_name);
    let mut registry = EngineRegistry::new();
    registry.register_lex(
        "data",
        Box::new(handl_plugin_engine_lex_pattern::DataLexerEngine::new()),
    );
    registry.register_parse("pratt", handl_plugin_engine_parse_pratt::PrattEngine::new());
    registry.register_eval(
        "tree-walk",
        Box::new(handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new()),
    );
    registry.register_eval(
        "bytecode",
        Box::new(handl_plugin_engine_exec_bytecode::BytecodeEngine::new()),
    );
    let registry = Rc::new(registry);

    // Always wire the canonical FpHostFunctions (for field_access support
    // used by the `(source).func(args)` applied expression form). If
    // dependency modules are present, layer them underneath.
    let module_chain: Option<Rc<dyn ModuleLookup>> = if dependencies.is_empty() {
        None
    } else {
        let mut modules = handl_plugin_module::ModuleRegistry::new();
        for dep in dependencies {
            modules.register_arc(dep);
        }
        Some(Rc::new(modules) as Rc<dyn ModuleLookup>)
    };
    let globals = handl_runtime::fp::canonical_fp_host_functions(module_chain);
    Runtime::from_registry_with_globals(lang, &registry, &config, Some(globals), None)
}

// ---------------------------------------------------------------------------
// HandlFunction
// ---------------------------------------------------------------------------

/// A callable [`Function`] backed by a `.hnl` source field.
///
/// Each call evaluates `(source).func_name(__arg_N)` via a cached per-thread
/// `Runtime`. The `Runtime` is built lazily on first use and reused for
/// subsequent calls on the same thread; the applied form's parsed `Ast` is
/// likewise cached per (module, function, arity) in `AST_CACHE`, so only the
/// eval stage runs per call.
///
/// This type is created by [`HandlModule`]; you do not typically construct
/// it directly.
pub struct HandlFunction {
    /// The module name (for error messages).
    module_name: String,
    /// The function/field name to apply.
    func_name: String,
    /// The `.hnl` source (with comment lines stripped).
    source: String,
    /// Unique module ID, used as a key in the per-thread runtime cache.
    module_id: usize,
    /// Shared runtime builder (called lazily per-thread on first use).
    builder: RuntimeBuilder,
}

impl HandlFunction {
    /// Create a new `HandlFunction`.
    fn new(
        module_name: String,
        func_name: String,
        source: String,
        module_id: usize,
        builder: RuntimeBuilder,
    ) -> Self {
        Self {
            module_name,
            func_name,
            source,
            module_id,
            builder,
        }
    }
}

impl Function for HandlFunction {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        // Arguments are bound as per-call globals (`__arg_N`) — never
        // serialized into the applied source text. Source literals cannot
        // express closures (`__arrow__` — the object constructor rejects
        // the reserved key, and a closure's `body` index only has meaning
        // in the AST that minted it) or other reserved-key values
        // (`__tag__`, `__pair__`, `__either__`, `__bytes__`), so values
        // cross the boundary as *values*.
        //
        // Provenance: an arrow argument's `body` index refers to the
        // *caller's* AST, so it is stamped with `__home__` = the ambient
        // eval frame (captured before the bridge eval replaces the
        // thread-locals); likewise, arrows in the result are stamped with
        // this call's eval frame so the caller can apply them.
        let caller_frame = if args.iter().any(contains_unhomed_arrow) {
            Some(
                handl_plugin_engine_exec_tree_walk::current_home_frame().ok_or_else(|| {
                    EvalError::Other(format!(
                        "{}.{}: arrow argument passed with no active eval frame",
                        self.module_name, self.func_name
                    ))
                })?,
            )
        } else {
            None
        };
        let mut globals: HashMap<String, serde_json::Value> = HashMap::with_capacity(args.len());
        let mut arg_names: Vec<String> = Vec::with_capacity(args.len());
        for (i, arg) in args.iter().enumerate() {
            let name = format!("__arg_{i}");
            let mut v = arg.clone();
            if let Some(frame_id) = caller_frame {
                stamp_home(&mut v, frame_id);
            }
            globals.insert(name.clone(), v);
            arg_names.push(name);
        }
        let module_id = self.module_id;
        let builder = Arc::clone(&self.builder);
        let module_name = &self.module_name;
        let func_name = &self.func_name;
        // Clone the `Rc<Runtime>` out of the cache and release the
        // `RefCell` borrow *before* `run`: a `.hnl` function may itself call
        // another `HandlFunction` (e.g. `str-handl` → `char.*`, or `dedent`
        // → `str.*`), which re-enters `call` and would otherwise hit
        // `RefCell already borrowed`.
        let runtime = RUNTIME_CACHE.with(|cache| {
            std::rc::Rc::clone(cache.borrow_mut().entry(module_id).or_insert_with(|| {
                std::rc::Rc::new(builder().expect("canonical kit assembly should not fail"))
            }))
        });
        let errf = |e: handl_language::LangError| {
            EvalError::Other(format!("{module_name}.{func_name}: {e}"))
        };
        // Parse the applied form `(source).func(__arg_0, ...)` once per
        // (module, function, arity) and reuse the `Ast` across calls — the
        // source text cannot vary with the arguments (they are `__arg_N`
        // globals). The `Rc<Ast>` is cloned out of the thread-local cache
        // and the `RefCell` borrow released *before* eval so re-entrant
        // nested calls never hold the borrow; eval only reads the AST
        // (`EvalRequest` takes a clone), so sharing is sound.
        let ast_key = (module_id, func_name.clone(), args.len());
        let ast = AST_CACHE.with(|cache| cache.borrow().get(&ast_key).cloned());
        let ast = match ast {
            Some(ast) => ast,
            None => {
                let source = format!(
                    "({}).{}({})",
                    self.source,
                    self.func_name,
                    arg_names.join(", ")
                );
                let toks = runtime.lex(&Source::new(&source)).map_err(errf)?;
                let parsed = Rc::new(runtime.parse(&toks).map_err(errf)?);
                AST_CACHE.with(|cache| {
                    cache.borrow_mut().insert(ast_key, Rc::clone(&parsed))
                });
                parsed
            }
        };
        // Keep the parsed AST: if the result contains arrows, their `body`
        // indices refer to it — register it as a home frame so the caller
        // can apply them.
        eprintln!("[hfcall] {}.{}/{} ast={}", module_name, func_name, args.len(), ast.len());
        let result = runtime
            .eval_shared_with_globals(&ast, &globals)
            .map_err(errf)?;
        if contains_unhomed_arrow(&result) {
            let frame_id = handl_plugin_engine_exec_tree_walk::register_home_frame(
                Rc::clone(&ast),
                Rc::new(runtime.lang().clone()),
                runtime.modules().cloned(),
                None,
            );
            let mut result = result;
            stamp_home(&mut result, frame_id);
            return Ok(result);
        }
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// HandlModule
// ---------------------------------------------------------------------------

/// A [`Module`] backed by a `.hnl` source file.
///
/// The `.hnl` source must evaluate to a record of arrows (functions). Each
/// field name listed in `func_names` becomes a callable [`Function`] via
/// [`HandlFunction`].
///
/// Dependency modules (e.g. the [`module`](handl_plugin_lib_stdlib::module)
/// table (e.g. `math`, `typeof`)) can be registered via
/// [`HandlModule::with_dependency`] so that module-qualified calls inside the
/// `.hnl` source (e.g. `math.sqrt`) resolve during evaluation.
///
/// # Example
///
/// ```ignore
/// use handl_plugin_lib_handl::HandlModule;
/// use handl_plugin_lib_stdlib as stdlib;
///
/// const STATS_HNL: &str = include_str!("../../../handl-packages/stats/stats.hnl");
///
/// let module = HandlModule::new("stats", STATS_HNL, &["sum", "avg", "stddev"])
///     .with_resolver(&stdlib::DEFAULT_RESOLVER);
/// ```
pub struct HandlModule {
    /// The module name (namespace prefix for qualified calls).
    name: String,
    /// Pre-constructed pure-logic function objects, keyed by field name.
    functions: HashMap<String, HandlFunction>,
    /// Op-backed function objects (from `op`/`handl` declarations), keyed by
    /// the local name (the segment after the last `.` in the op name).
    op_functions: HashMap<String, OpFunction>,
    /// Dependency modules registered in the bridge runtime.
    dependencies: Vec<Arc<dyn Module + Send + Sync>>,
    /// Import declarations parsed from the source during [`analyze_source`]
    /// (used by [`with_resolver`](Self::with_resolver) for dep wiring).
    imports: Vec<ImportDecl>,
    /// The eval engine name this module's bridge runtime binds
    /// (`"tree-walk"` default; `"bytecode"` selects the bytecode VM —
    /// much faster on eval-heavy module bodies such as the loader's
    /// per-character `str.*`/`char.*` work). Per-module selection keeps
    /// the change data-driven: the name resolves through the same
    /// [`EngineRegistry`] the tree-walk uses.
    eval_name: &'static str,
}

impl HandlModule {
    /// Create a new `HandlModule` with the given name, `.hnl` source, and
    /// function (record field) names.
    ///
    /// The source should evaluate to a record of arrows (functions). Comment
    /// lines (`-- ...`) and `cap`/`op`/`handl` declaration lines
    /// (ADR-0077) are stripped automatically. Each name in `func_names`
    /// becomes a callable function that evaluates
    /// `(source).name(args)` on call.
    pub fn new(name: impl Into<String>, hnl_source: &str, func_names: &[&str]) -> Self {
        let name = name.into();
        // ADR-0094: the full text-transform pipeline runs in handl
        // (`loader.analyze`) once a `HandlLoader` is installed; otherwise
        // the bootstrap transforms run.
        let analysis = analyze_source(hnl_source);
        let source = analysis.prepared;
        let imports = analysis.imports;
        let module_id = allocate_module_id();
        let eval_name = "tree-walk";

        // Initial builder with no dependencies. `with_dependency` will
        // replace this with a builder that includes the dependencies.
        let builder: RuntimeBuilder = Arc::new(|| build_runtime(Vec::new(), eval_name));

        let functions = func_names
            .iter()
            .map(|&fname| {
                let func = HandlFunction::new(
                    name.clone(),
                    fname.to_string(),
                    source.clone(),
                    module_id,
                    Arc::clone(&builder),
                );
                (fname.to_string(), func)
            })
            .collect();

        Self {
            name,
            functions,
            op_functions: HashMap::new(),
            dependencies: Vec::new(),
            imports,
            eval_name,
        }
    }

    /// Create a new `HandlModule` without applying text transforms.
    ///
    /// This is intended for the bootstrap `loader` module, whose source
    /// (`handl-packages/loader/loader.hnl`) must be evaluated raw so it can
    /// then transform the sources of other `.hnl` modules.
    pub fn new_raw(name: impl Into<String>, hnl_source: &str, func_names: &[&str]) -> Self {
        let name = name.into();
        let source = hnl_source.to_string();
        let module_id = allocate_module_id();
        let eval_name = "tree-walk";

        // Initial builder with no dependencies. `with_dependency` will
        // replace this with a builder that includes the dependencies.
        let builder: RuntimeBuilder = Arc::new(|| build_runtime(Vec::new(), eval_name));

        let functions = func_names
            .iter()
            .map(|&fname| {
                let func = HandlFunction::new(
                    name.clone(),
                    fname.to_string(),
                    source.clone(),
                    module_id,
                    Arc::clone(&builder),
                );
                (fname.to_string(), func)
            })
            .collect();

        Self {
            name,
            functions,
            op_functions: HashMap::new(),
            dependencies: Vec::new(),
            imports: Vec::new(),
            eval_name,
        }
    }

    /// Add a dependency module to be registered in the bridge runtime.
    ///
    /// This allows the `.hnl` source to call module-qualified functions
    /// (e.g. `math.sqrt`, `typeof.type_of`) during evaluation. The runtime
    /// builder is updated to include the new dependency; any previously
    /// cached per-thread runtime is rebuilt on next access.
    pub fn with_dependency(mut self, module: impl Module + Send + Sync + 'static) -> Self {
        self.dependencies.push(Arc::new(module));
        self.rebuild_functions();
        self
    }

    /// Add a dependency module (already `Arc`ed) to the bridge runtime.
    pub fn with_dependency_arc(mut self, module: Arc<dyn Module + Send + Sync>) -> Self {
        self.dependencies.push(module);
        self.rebuild_functions();
        self
    }

    /// Auto-wire dependencies from `from 'path' import * as 'alias'`
    /// declarations in the `.hnl` source, using a [`ModuleResolver`] to
    /// map module names to [`Module`] impls.
    ///
    /// This replaces hardcoded `.with_dependency(...)` calls: the `.hnl`
    /// source declares its own dependencies, and the resolver maps names
    /// to concrete module implementations.
    pub fn with_resolver(mut self, resolver: &dyn ModuleResolver) -> Self {
        for decl in &self.imports {
            if let Some(module) = resolver.resolve(&decl.path) {
                self.dependencies.push(module);
            }
        }
        self.rebuild_functions();
        self
    }

    /// Select the eval engine this module's bridge runtime binds
    /// (`"tree-walk"`, `"bytecode"`).
    ///
    /// `"bytecode"` runs module bodies on the bytecode VM
    /// (`handl_plugin_engine_exec_bytecode::BytecodeEngine`): the
    /// per-call record construction and function bodies compile to
    /// instructions once per eval instead of paying tree-walk scheme
    /// dispatch per node — measured ~100-400× faster on eval-heavy
    /// transforms (`loader.hnl` `strip_decls` on a 22k-char source:
    /// ~59s tree-walk → ~0.16s bytecode).
    ///
    /// Note: the bytecode engine now supports the same home-frame
    /// provenance for arrows crossing the `Function::call` boundary as
    /// tree-walk. Modules whose functions take or return closures can opt
    /// into `"bytecode"` as well.
    pub fn with_eval_engine(mut self, eval_name: &'static str) -> Self {
        self.eval_name = eval_name;
        self.rebuild_functions();
        self
    }

    /// Attach op functions for the given op names, dispatching to the shared
    /// [`OpRegistry`].
    ///
    /// Each op name (e.g. `"base64.encode"`) is exposed as a callable
    /// [`OpFunction`] under its local name — the segment after the last `.`
    /// in the op name (e.g. `"encode"`). This allows the module to be looked
    /// up via `ModuleLookup::lookup("base64.encode")` (the module name
    /// `"base64"` + the local name `"encode"`).
    ///
    /// Op functions are merged with the regular record-field functions; if a
    /// name collision occurs, the op function takes precedence.
    pub fn with_ops(mut self, op_names: &[&str], registry: Arc<OpRegistry>) -> Self {
        for &op_name in op_names {
            let local_name = op_name.rsplit('.').next().unwrap_or(op_name);
            self.op_functions.insert(
                local_name.to_string(),
                OpFunction::new(op_name, Arc::clone(&registry)),
            );
        }
        self
    }

    /// Rebuild all [`HandlFunction`] instances with an updated runtime
    /// builder that includes the current dependencies.
    ///
    /// This is called after `with_dependency` / `with_dependency_arc` to
    /// ensure the builder reflects the latest dependency set. All
    /// per-thread caches (`RUNTIME_CACHE`, `AST_CACHE`)
    /// key on `module_id`, so a *fresh* module ID is
    /// allocated here — previously cached runtimes/ASTs/evaluated records
    /// for the old ID are abandoned and rebuilt on next access against the
    /// new dependency set.
    fn rebuild_functions(&mut self) {
        let deps = self.dependencies.clone();
        let eval_name = self.eval_name;
        let builder: RuntimeBuilder =
            Arc::new(move || build_runtime(deps.clone(), eval_name));
        let module_id = allocate_module_id();
        for func in self.functions.values_mut() {
            func.builder = Arc::clone(&builder);
            func.module_id = module_id;
        }
    }
}

impl Module for HandlModule {
    fn name(&self) -> &str {
        &self.name
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        // Op functions take precedence (ops are the declared interface).
        if let Some(f) = self.op_functions.get(name) {
            return Some(f as &dyn Function);
        }
        self.functions.get(name).map(|f| f as &dyn Function)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        let mut result: Vec<(&str, &dyn Function)> = Vec::new();
        // Op functions first (declared interface).
        for (name, func) in &self.op_functions {
            result.push((name.as_str(), func as &dyn Function));
        }
        // Then regular functions (skip names already covered by ops).
        for (name, func) in &self.functions {
            if !self.op_functions.contains_key(name) {
                result.push((name.as_str(), func as &dyn Function));
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// str-handl shared builder
// ---------------------------------------------------------------------------

/// The handl source implementing the `str` contract as pure handl over `char`.
const STR_HANDL_HNL: &str = include_str!("../../../handl-packages/str-handl/str-handl.hnl");

/// The canonical text-transform loader source (ADR-0094).
///
/// Loaded raw via [`HandlModule::new_raw`] by [`HandlLoader::new`]; once
/// installed through [`install_loader`], every `.hnl` text transform
/// (declaration/comment stripping, export transformation, import/
/// capability/tag parsing, tag expansion) is evaluated in handl.
const LOADER_HNL: &str = include_str!("../../../handl-packages/loader/loader.hnl");

/// The record field names exposed by `loader.hnl`.
pub const LOADER_FUNCS: &[&str] = &[
    "strip_comments",
    "strip_decls",
    "transform_export_to_object",
    "find_binding_sep",
    "parse_imports",
    "parse_caps",
    "parse_tags",
    "expand_tags",
    "escape_string",
    "prepare",
    "analyze",
];

// ---------------------------------------------------------------------------
// HandlLoader — the self-hosted `.hnl` text-transform pipeline (ADR-0094)
// ---------------------------------------------------------------------------

/// A handle to the pure-handl loader (`handl-packages/loader/loader.hnl`).
///
/// The loader is a [`HandlModule`] built *raw* — its own source is the code
/// that strips declarations, so it must not be transformed.  It needs the
/// `char` and `str` floors injected as dependency modules: `char` is a
/// native floor (no loader needed) and `str` (`str-handl`) is prepared by
/// the bootstrap transforms — this is the two-phase resolution of the
/// bootstrap paradox.
///
/// Once a `HandlLoader` is installed via [`install_loader`], all public
/// transform functions in this crate (and [`HandlModule::new`]'s
/// preparation pipeline) delegate to the handl implementations.  Before
/// installation, or if a loader call fails, the `bootstrap` module's Rust
/// implementations are used.
pub struct HandlLoader {
    /// The raw loader module.
    module: HandlModule,
}

impl HandlLoader {
    /// Build the loader with the given dependency modules (typically a
    /// `char` backend and the `str` implementation built over it).
    ///
    /// The loader module runs on the **bytecode** eval engine: its
    /// transforms are eval-heavy per-character `str.*`/`char.*` work
    /// (~59s under tree-walk for a 22k-char source → ~0.16s under
    /// bytecode; see `examples/loader_bench.rs`). The loader's functions
    /// are pure text transforms — strings in, strings/arrays out — so no
    /// arrows cross the `Function::call` boundary. For the same speedup,
    /// build the `str` dep with `HandlModule::with_eval_engine("bytecode")`
    /// too.
    pub fn new(deps: Vec<Arc<dyn Module + Send + Sync>>) -> Self {
        let mut module =
            HandlModule::new_raw("loader", LOADER_HNL, LOADER_FUNCS)
                .with_eval_engine("bytecode");
        for dep in deps {
            module = module.with_dependency_arc(dep);
        }
        Self { module }
    }

    /// Invoke a loader transform function on `src`.
    fn call(&self, func: &str, src: &str) -> Result<serde_json::Value, EvalError> {
        let f = self.module.function(func).ok_or_else(|| {
            EvalError::Other(format!("loader: no transform function '{func}'"))
        })?;
        f.call(
            &[serde_json::Value::String(src.to_string())],
            &mut EvalContext::new(),
        )
    }
}

/// The process-wide installed loader (ADR-0094).
static LOADER: std::sync::OnceLock<HandlLoader> = std::sync::OnceLock::new();

/// Install the process-wide [`HandlLoader`].  Subsequent `.hnl` module
/// construction routes text transforms through `loader.hnl`.  The first
/// installation wins; later calls return the rejected loader.
pub fn install_loader(loader: HandlLoader) -> Result<(), HandlLoader> {
    LOADER.set(loader)
}

/// The installed [`HandlLoader`], if any.
pub fn loader() -> Option<&'static HandlLoader> {
    LOADER.get()
}

/// Call a transform on the installed loader; `None` when no loader is
/// installed or the call fails (callers fall back to `bootstrap`).
fn loader_call(func: &str, src: &str) -> Option<serde_json::Value> {
    loader()?.call(func, src).ok()
}

/// The record field names exposed by `str-handl.hnl`.
pub const STR_HANDL_FUNCS: &[&str] = &[
    "upper",
    "lower",
    "trim",
    "trim_start",
    "trim_end",
    "trim_matches",
    "reverse",
    "length",
    "concat",
    "is_empty",
    "equals",
    "is_alpha",
    "is_numeric",
    "is_alphanumeric",
    "capitalize",
    "title_case",
    "snake_case",
    "camel_case",
    "pascal_case",
    "kebab_case",
    "contains",
    "starts_with",
    "ends_with",
    "count",
    "find",
    "rfind",
    "levenshtein",
    "similarity",
    "split",
    "join",
    "replace",
    "replace_first",
    "repeat",
    "char_at",
    "slice",
    "pad_left",
    "pad_right",
    "strip_prefix",
    "strip_suffix",
    "lines",
    "split_at",
    "split_whitespace",
    "truncate",
    "slugify",
    "word_wrap",
];

/// Build the [`HandlModule`] for `str-handl`.
///
/// The source imports `char`, so the supplied resolver must map `"char"` to
/// a `char` backend (typically [`CharModule`](handl_plugin_lib_char_native::CharModule)).
pub fn build_str_handl_module(resolver: &dyn ModuleResolver) -> HandlModule {
    HandlModule::new("str", STR_HANDL_HNL, STR_HANDL_FUNCS).with_resolver(resolver)
}

// ---------------------------------------------------------------------------
// HandlLibraryPlugin
// ---------------------------------------------------------------------------

/// A [`Plugin`] that contributes a [`HandlModule`] into the platform's
/// [`ModuleRuntime`].
///
/// This is the generic replacement for hand-rolled library plugin crates.
/// Construct it with a module name, plugin id, plugin display name, the
/// `.hnl` source, and the function (record field) names, then add any
/// dependency modules via [`HandlLibraryPlugin::with_dependency`].
///
/// # Example
///
/// ```ignore
/// use handl_plugin_lib_handl::HandlLibraryPlugin;
/// use handl_plugin_lib_stdlib as stdlib;
///
/// const STATS_HNL: &str = include_str!("../../../handl-packages/stats/stats.hnl");
///
/// let plugin = HandlLibraryPlugin::new(
///     "stats", "lib.stats", "lib-stats", STATS_HNL,
///     &["sum", "avg", "product", "min", "max", "median", "stddev"],
/// )
/// .with_dependency(MathModule)
/// .with_dependency(TypeModule);
/// ```
pub struct HandlLibraryPlugin {
    /// The module name.
    module_name: String,
    /// The plugin id.
    plugin_id: String,
    /// The plugin display name.
    plugin_name: String,
    /// The `.hnl` source (with comments stripped).
    source: String,
    /// Function (record field) names.
    func_names: Vec<String>,
    /// Dependency modules.
    dependencies: Vec<Arc<dyn Module + Send + Sync>>,
    /// Op names (from `op`/`handl` declarations) to expose as callable
    /// functions dispatching to the [`OpRegistry`].
    op_names: Vec<String>,
    /// The shared op registry (if any ops are declared).
    op_registry: Option<Arc<OpRegistry>>,
    /// Capability names declared in the `.hnl` source (via `cap`
    /// declarations, ADR-0077 Phase 5).
    capabilities: Vec<CapabilityId>,
}

impl HandlLibraryPlugin {
    /// Create a new `HandlLibraryPlugin`.
    ///
    /// - `module_name` — the module namespace (e.g. `"stats"`)
    /// - `plugin_id` — the stable plugin id (e.g. `"lib.stats"`)
    /// - `plugin_name` — the display name (e.g. `"lib-stats"`)
    /// - `hnl_source` — the `.hnl` source (typically via `include_str!`)
    /// - `func_names` — the record field names exposed as callable functions
    pub fn new(
        module_name: impl Into<String>,
        plugin_id: impl Into<String>,
        plugin_name: impl Into<String>,
        hnl_source: &str,
        func_names: &[&str],
    ) -> Self {
        Self {
            module_name: module_name.into(),
            plugin_id: plugin_id.into(),
            plugin_name: plugin_name.into(),
            // The raw source is kept here; `build_module` runs the full
            // prepare pipeline (handl loader when installed, bootstrap
            // otherwise) via `HandlModule::new`.
            source: hnl_source.to_string(),
            func_names: func_names.iter().map(|s| s.to_string()).collect(),
            dependencies: Vec::new(),
            op_names: Vec::new(),
            op_registry: None,
            capabilities: Vec::new(),
        }
    }

    /// Add a dependency module to be registered in the bridge runtime.
    pub fn with_dependency(mut self, module: impl Module + Send + Sync + 'static) -> Self {
        self.dependencies.push(Arc::new(module));
        self
    }

    /// Add a dependency module (already `Arc`ed) to the bridge runtime.
    pub fn with_dependency_arc(mut self, module: Arc<dyn Module + Send + Sync>) -> Self {
        self.dependencies.push(module);
        self
    }

    /// Attach op functions for the given op names, dispatching to the shared
    /// [`OpRegistry`].
    ///
    /// Each op name (e.g. `"base64.encode"`) is exposed as a callable
    /// [`OpFunction`] under its local name — the segment after the last `.`
    /// in the op name (e.g. `"encode"`). This allows the module to be looked
    /// up via `ModuleLookup::lookup("base64.encode")` (the module name
    /// `"base64"` + the local name `"encode"`).
    ///
    /// Op functions are merged with the regular record-field functions; if a
    /// name collision occurs, the op function takes precedence.
    pub fn with_ops(mut self, op_names: &[&str], registry: Arc<OpRegistry>) -> Self {
        self.op_names = op_names.iter().map(|s| s.to_string()).collect();
        self.op_registry = Some(registry);
        self
    }

    /// Set the capabilities declared by this library's `.hnl` source
    /// (ADR-0077 Phase 5).
    ///
    /// The capability names (e.g. `"base64.encode"`) are exposed via
    /// [`Plugin::capabilities`] and contributed via
    /// [`ContributionContext`] in [`contribute`](Plugin::contribute).
    pub fn with_capabilities(mut self, caps: Vec<CapabilityId>) -> Self {
        self.capabilities = caps;
        self
    }

    /// Create a [`HandlLibraryPlugin`], automatically extracting `cap`
    /// declarations from the `.hnl` source (ADR-0077 Phase 5).
    ///
    /// This is a convenience constructor that parses `cap "name"` lines from
    /// `hnl_source` via [`parse_cap_declarations`] and passes them to
    /// [`with_capabilities`](Self::with_capabilities).
    ///
    /// - `module_name` — the module namespace (e.g. `"base64"`)
    /// - `plugin_id` — the stable plugin id (e.g. `"lib.base64"`)
    /// - `plugin_name` — the display name (e.g. `"lib-base64"`)
    /// - `hnl_source` — the `.hnl` source (typically via `include_str!`)
    /// - `func_names` — the record field names exposed as callable functions
    pub fn new_with_caps(
        module_name: impl Into<String>,
        plugin_id: impl Into<String>,
        plugin_name: impl Into<String>,
        hnl_source: &str,
        func_names: &[&str],
    ) -> Self {
        let caps = parse_cap_declarations(hnl_source);
        Self::new(module_name, plugin_id, plugin_name, hnl_source, func_names)
            .with_capabilities(caps.into_iter().map(CapabilityId::from).collect())
    }

    /// Build the [`HandlModule`] from this plugin's configuration.
    fn build_module(&self) -> HandlModule {
        let func_refs: Vec<&str> = self.func_names.iter().map(|s| s.as_str()).collect();
        let mut module = HandlModule::new(&self.module_name, &self.source, &func_refs);
        for dep in &self.dependencies {
            module = module.with_dependency_arc(Arc::clone(dep));
        }
        // Attach op functions if an op registry is configured.
        if let Some(ref registry) = self.op_registry {
            let op_refs: Vec<&str> = self.op_names.iter().map(|s| s.as_str()).collect();
            module = module.with_ops(&op_refs, Arc::clone(registry));
        }
        module
    }
}

impl Plugin for HandlLibraryPlugin {
    fn id(&self) -> &str {
        &self.plugin_id
    }

    fn name(&self) -> &str {
        &self.plugin_name
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        let module = self.build_module();
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(module))?;
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

// ---------------------------------------------------------------------------
