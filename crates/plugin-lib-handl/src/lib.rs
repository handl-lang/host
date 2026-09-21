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
//! cross the boundary as values bound directly on the call frame) — is
//! lexed/parsed **once** per (module, function, arity) into a per-thread
//! `AST_CACHE` `Rc<Ast>` and re-evaluated per call through
//! [`Runtime::eval_shared_with_call_args`], which binds args directly into
//! the `ScopeStack` globals map without cloning the engine. Caching the
//! *evaluated* record is not possible
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
pub mod model;
pub mod op_registry;
pub mod transform;
pub use lint::HandlNativeLint;
pub use model::{HandlModel, HandlModelAdapter, HandlModelEngine, term_to_value};
pub use op_registry::{OpFunction, OpRegistry, RuntimePredicateChecker};
pub use transform::{HANDL_PREFIX, HandlNativeRewrite, KEEP_KEY};

/// Re-export of the engine's [`ImportResolver`] trait so `.hnl` library
/// builders (e.g. `plugin-lib-stdlib`) can supply an import resolver
/// without taking a direct `plugin-engine-exec` dependency.
pub use handl_plugin_engine_exec::ImportResolver;

use std::cell::{Cell, RefCell};
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
/// Used to map a module name to its [`Module`] impl when building and
/// resolving stdlib packages (see `plugin-lib-stdlib`'s table resolver).
pub trait ModuleResolver: Send + Sync {
    /// Resolve a module name to a shared [`Module`] impl, or `None` if unknown.
    fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>>;
}

/// A parsed import declaration from a `.hnl` source file.
#[derive(Debug, Clone)]
pub struct ImportDecl {
    /// The module path being imported from (e.g. `"str"`, `"fn"`).
    pub path: String,
    /// The local alias for the imported module (e.g. `"str"`).
    pub alias: String,
}

// ---------------------------------------------------------------------------
// Helpers (generalized from the stats/hello bridges)
// ---------------------------------------------------------------------------

/// Normalize a path-like import spec to its module basename
/// (`./reverse.hnl` -> `reverse`; `a/b/c.hnl` -> `c`; `len` -> `len`).
///
/// Mirrors `handl-plugin-lib-stdlib`'s `normalize_module_name` so a module
/// declared as a dependency under its basename is found regardless of the
/// import path spelling.
fn normalize_import_name(path: &str) -> &str {
    let p = path.strip_prefix("./").unwrap_or(path);
    let p = p.strip_suffix(".hnl").unwrap_or(p);
    p.rsplit('/').next().unwrap_or(p)
}

/// The default [`ImportResolver`] used by a [`HandlModule`] runtime.
///
/// A `.hnl` import statement is evaluated by the runtime (`prim_import`),
/// which asks the resolver for the target module's bindings as a
/// [`handl_language::Value::Object`]. A handl closure cannot cross that
/// `serde_json` boundary as a callable, so -- to stay *exactly* compatible
/// with how `mod.fn` already resolves today (a **string** callee
/// `"mod.fn"` is dispatched to the importer's module registry) -- every
/// binding is the string `"<module>.<field>"`.
///
/// Applying such a binding (`m.f(x)`, or a bare named import `f(x)`)
/// therefore routes through the importer's [`ModuleLookup`] registry, whose
/// `<module>` entry is resolved from the module's dependency chain or, failing
/// that, the injected global module lookup.
pub struct DependencyImportResolver {
    modules: Vec<Arc<dyn Module + Send + Sync>>,
}

impl DependencyImportResolver {
    /// Wrap the resolved dependency modules of a [`HandlModule`].
    pub fn new(modules: Vec<Arc<dyn Module + Send + Sync>>) -> Self {
        Self { modules }
    }
}

/// Build the bindings object a [`resolve_import`] returns for a module:
/// function exports as `mod.fn` string callees plus *value* exports
/// ([`Module::value_names`]) as their evaluated values — so
/// `from "bool" import true` binds a `Bool`, not the string `"bool.true"`.
///
/// [`resolve_import`]: ImportResolver::resolve_import
pub fn module_bindings_object(module: &dyn Module) -> handl_language::Value {
    let mut obj = serde_json::Map::new();
    for (fname, _f) in module.functions() {
        obj.insert(
            fname.to_string(),
            handl_language::Value::String(format!("{}.{}", module.name(), fname)),
        );
    }
    for vname in module.value_names() {
        if let Some(v) = module.value(&vname) {
            obj.insert(vname, v);
        }
    }
    handl_language::Value::Object(obj)
}

impl ImportResolver for DependencyImportResolver {
    fn resolve_import(
        &self,
        path: &str,
    ) -> Result<handl_language::Value, handl_language::LangError> {
        let name = normalize_import_name(path);
        let module = self
            .modules
            .iter()
            .find(|m| m.name() == name || m.name() == path)
            .ok_or_else(|| {
                handl_language::LangError::engine(format!(
                    "import: no dependency module `{name}` (path `{path}`) is registered"
                ))
            })?;
        Ok(module_bindings_object(module.as_ref()))
    }
}

/// Adapts a `Send + Sync` import resolver into the `Rc`-based
/// [`ImportResolver`] handle a `Runtime` expects. The `Runtime` is
/// single-threaded, but the `HandlModule` builder closure is shared across
/// threads (each thread builds its own `Runtime`).
struct SharedImportResolver(Arc<dyn ImportResolver + Send + Sync>);

impl ImportResolver for SharedImportResolver {
    fn resolve_import(
        &self,
        path: &str,
    ) -> Result<handl_language::Value, handl_language::LangError> {
        self.0.resolve_import(path)
    }
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
                // NOTE: `import`/`from` lines are deliberately KEPT here --
                // the runtime evaluates them (`grammar.stmt-from.prefix` +
                // `prim_import`); `transform_export_to_object` hoists them
                // into the emitted `do { ... }` block.
                true
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// A top-level (non-indented) import statement line.
    fn is_top_import_line(line: &str) -> bool {
        !line.starts_with(' ') && !line.starts_with('\t') && is_import_line(line.trim_start())
    }

    /// Top-level import statements of `src`, in original order. `from "..."
    /// export x, y` re-export lines are rewritten to plain `import ... as`
    /// statements so the hoisted `do { ... }` block binds the local aliases
    /// the record literal references.
    fn collect_import_lines(src: &str) -> Vec<String> {
        src.lines()
            .filter(|line| is_top_import_line(line))
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

    /// If a `from` line uses the re-export form `from "..." export x, y`,
    /// rewrite it to a plain import `from "..." import x as __x, y as __y`
    /// and return the (name, local-alias) pairs that should appear in the
    /// module record. Mirrors `handl_runtime::module_prep`'s handling.
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

    /// Extract the first quoted path in a `from` line and the text that
    /// follows it.
    fn extract_quoted_path<'a>(line: &'a str) -> Option<(String, &'a str)> {
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

    /// `src` with top-level import statements removed (all other lines kept).
    fn drop_import_lines(src: &str) -> String {
        src.lines()
            .filter(|line| !is_top_import_line(line))
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
    /// When the source contains top-level `import`/`from` statements they are
    /// hoisted -- in original order -- ahead of the record literal inside a
    /// `do { ... }` block, so the runtime can evaluate them (`prim_import`)
    /// and bind their names for the record. This is required because the
    /// `(source).func(args)` applied form needs a *single* expression; a bare
    /// import statement followed by a record is not one.
    ///
    /// If no `export` lines are found and there are no imports, the source is
    /// returned unchanged (the existing object-literal form is preserved).
    pub(super) fn transform_export_to_object(src: &str) -> String {
        // Collect (name, expr_lines, exported) triples in source order.
        // All bindings become `do`-block assignments (`name := expr;`)
        // emitted *before* the object so closures can reference siblings
        // — exported or not. The object literal projects the exported
        // names (`{ name: name }`) — mirrors `handl_runtime::module_prep`.
        let mut bindings: Vec<(String, Vec<String>, bool)> = Vec::new();
        let mut current: Option<(String, Vec<String>, bool)> = None;

        // Finalize `current` into the ordered binding list.
        macro_rules! finalize {
            () => {
                if let Some(entry) = current.take() {
                    bindings.push(entry);
                }
            };
        }

        for line in src.lines() {
            let trimmed = line.trim_start();
            // Import lines are hoisted separately; never treat them as part
            // of an export body. `from "..." export x` re-export lines close
            // the current entry and contribute one record field per name.
            if is_top_import_line(line) {
                if let Some((_, names)) = from_export_to_import(line) {
                    finalize!();
                    for (name, alias) in names {
                        bindings.push((name, vec![alias], true));
                    }
                }
                continue;
            }
            // A top-level export line starts at column 0 with `export `.
            if !line.starts_with(' ') && !line.starts_with('\t') && trimmed.starts_with("export ") {
                // Flush the previous entry.
                finalize!();
                let rest = &trimmed["export ".len()..];
                // Find the binding separator (`::`, `:=`, or legacy `=`).
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
                    current = Some((name, expr_lines, true));
                }
                continue;
                // If no separator found, skip (not a valid export binding).
            } else if !line.starts_with(' ') && !line.starts_with('\t') {
                // Column-0 line that is not an import or `export`
                // declaration. Detect a *private* (non-exported) top-level
                // binding so that helper functions are preserved instead of
                // dropped — mirrors `handl_runtime::module_prep`.
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
                        current = Some((name, expr_lines, false));
                        continue;
                    }
                }
            }
            if let Some((_, ref mut lines, _)) = current {
                // Accumulate indented continuation lines (part of the export
                // expression body). Stop at blank lines only if the next
                // non-blank line is a top-level export.
                if !line.trim().is_empty() {
                    lines.push(line.to_string());
                }
            }
        }
        finalize!();

        let has_exports = bindings.iter().any(|(_, _, exported)| *exported);

        let imports = collect_import_lines(src);

        if !has_exports {
            if imports.is_empty() && bindings.is_empty() {
                return src.to_string();
            }
            // Preserve bindings verbatim when there are no exports —
            // the module is a script, not a record.
            let stripped = drop_import_lines(src);
            if imports.is_empty() {
                return stripped;
            }
            // Imports still need to execute — wrap like the self-hosted
            // loader does so they run ahead of the bare script body.
            return format!("do {{\n{}\n{}\n}}", imports.join("\n"), stripped);
        }

        // Private bindings hoist as `name := expr;` prelude assignments;
        // exports stay inline in the trailing record literal (`name: expr`)
        // — mirrors `loader.hnl`'s `prelude + emit_object`.
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
            // Always quote record keys: an export name may be a keyword
            // (`first`, `export`) that lexes as `token.keyword.*`, not an
            // identifier — `is_valid_ident` is only a shape check.
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
            format!("do {{\n{}{}\n}}", prelude, record)
        } else {
            format!("do {{\n{}\n{}{}\n}}", imports.join("\n"), prelude, record)
        }
    }

    /// Find the position and length of the binding separator in `s`: the `::`
    /// bind token (canonical), the `:=` graded-bind token, a legacy `=`
    /// (skipping `==`, `>=`, `<=`, `!=`, `=>`), or a bare `:` for type-only
    /// exports. `::` and `:=` return immediately; the first bare `=` and the
    /// first bare `:` are remembered so a stronger `::`/`:=`/later `=` wins.
    /// Returns `None` if no binding separator is found.
    pub(super) fn find_binding_sep(s: &str) -> Option<(usize, usize)> {
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
                        return Some((i, 2)); // ::
                    }
                    if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                        return Some((i, 2)); // :=  graded bind
                    }
                    if first_colon.is_none() {
                        first_colon = Some(i);
                    }
                }
                b'=' if depth == 0 => {
                    let prev = if i > 0 { bytes[i - 1] } else { 0 };
                    let next = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
                    if next == b'=' || prev == b'>' || prev == b'<' || prev == b'!' || next == b'>'
                    {
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
        if let Some(pos) = first_eq {
            return Some((pos, 1));
        }
        first_colon.map(|pos| (pos, 1))
    }

    /// Strip generics `<...>` and type annotation `: Type` from an export
    /// name portion, returning the bare identifier.
    ///
    /// e.g. `len<T>: List(T) ~[Total]~> Nat ` -> `len`
    /// e.g. `push<T>: (List(T), T) ~[Total]~> List(T)` -> `push`
    /// e.g. `is_empty` -> `is_empty`
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

    /// True when `s` is a plausible Handl identifier: starts with an
    /// alphabetic character or underscore, followed by zero or more
    /// alphanumeric characters or underscores. Mirrors
    /// `handl_runtime::module_prep`.
    fn is_valid_ident(s: &str) -> bool {
        let mut chars = s.chars();
        match chars.next() {
            Some(c) if c.is_alphabetic() || c == '_' => {
                chars.all(|c| c.is_alphanumeric() || c == '_')
            }
            _ => false,
        }
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
                Some(rest)
                    if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') =>
                {
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
                Some(rest)
                    if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') =>
                {
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
                            if !name.is_empty()
                                && name.chars().all(|c| c.is_alphanumeric() || c == '_')
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
    loader_string("strip_decls", src).unwrap_or_else(|| bootstrap::strip_declaration_lines(src))
}

/// Parse `from 'path' import * as 'alias'` / `import "path" as alias`
/// declarations.  Uses the bootstrap implementation.
pub fn parse_import_declarations(src: &str) -> Vec<ImportDecl> {
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
    loader_string("expand_tags", src).unwrap_or_else(|| bootstrap::expand_tag_declarations(src))
}

/// Strip `--` comment lines.  Delegates to `loader.strip_comments` when a
/// [`HandlLoader`] is installed; otherwise uses the bootstrap implementation.
pub fn strip_comment_lines(src: &str) -> String {
    loader_string("strip_comments", src).unwrap_or_else(|| bootstrap::strip_comment_lines(src))
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

thread_local! {
    /// When `true`, [`analyze_source`] skips the installed self-hosted
    /// loader and uses the bootstrap transforms. Used by canonical kit
    /// construction so repeated `Kit::new()` calls do not route built-in
    /// `.hnl` module sources through the process-wide loader.
    static SKIP_LOADER: Cell<bool> = const { Cell::new(false) };
}

/// Run `f` with the self-hosted loader disabled for source preparation.
///
/// Built-in module construction uses the bootstrap transforms regardless
/// of whether a loader has been installed in the current process.
pub fn with_loader_disabled<R>(f: impl FnOnce() -> R) -> R {
    SKIP_LOADER.with(|s| s.set(true));
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            SKIP_LOADER.with(|s| s.set(false));
        }
    }
    let _guard = Guard;
    f()
}

/// Run the full source-preparation pipeline once.  When a [`HandlLoader`]
/// is installed this is a single `loader.analyze` evaluation; otherwise it
/// runs the bootstrap transforms.
fn analyze_source(src: &str) -> SourceAnalysis {
    if !SKIP_LOADER.with(|s| s.get()) {
        if let Some(v) = loader_call("analyze", src) {
            if let Some(prepared) = v
                .get("source")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string())
            {
                return SourceAnalysis { prepared };
            }
        }
    }
    let stripped = bootstrap::strip_declaration_lines(&bootstrap::strip_comment_lines(src));
    let exported = bootstrap::transform_export_to_object(&stripped);
    SourceAnalysis {
        prepared: bootstrap::expand_tag_declarations(&exported),
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
            // Arrow envs serialize by reference (`{"__envref__": id}`) —
            // stamp env-resident arrows in `TaggedValue` space.
            if let Some(id) = o.get("__envref__").and_then(|v| v.as_u64()) {
                handl_value::stamp_env_home(id, frame_id);
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
    /// arguments bind directly on the call frame as `__arg_N` values,
    /// never as source text — so the lexed/parsed/desugared [`handl_language::Ast`]
    /// can be parsed once and reused across calls. This removes the per-call
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

    /// Per-thread cache of parsed `(source).<value-name>` projection ASTs
    /// for [`Module::value`] — value exports (e.g. `bool`'s `true`/`false`)
    /// project one field inside the module's own runtime so the record's
    /// arrows never cross the serde boundary.
    static VALUE_AST_CACHE: RefCell<HashMap<(usize, String), std::rc::Rc<handl_language::Ast>>> =
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
    import_resolver: Option<Arc<dyn ImportResolver + Send + Sync>>,
    global_lookup: Option<Arc<dyn ModuleLookup + Send + Sync>>,
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
    // Resolve the import resolver: an explicit one wins; otherwise derive
    // one from the registered dependencies (before they are moved into the
    // registry) so `import`/`from` statements can be evaluated. The derived
    // resolver is sufficient whenever every import path names a registered
    // dependency -- the case wired by `HandlModule::with_dependency`.
    let derived_resolver = if import_resolver.is_none() && !dependencies.is_empty() {
        Some(DependencyImportResolver::new(dependencies.clone()))
    } else {
        None
    };
    let resolver: Option<Rc<dyn ImportResolver>> = match import_resolver {
        Some(r) => Some(Rc::new(SharedImportResolver(r)) as Rc<dyn ImportResolver>),
        None => derived_resolver.map(|r| Rc::new(r) as Rc<dyn ImportResolver>),
    };

    let dep_registry = if dependencies.is_empty() {
        None
    } else {
        let mut modules = handl_plugin_module::ModuleRegistry::new();
        for dep in dependencies {
            modules.register_arc(dep);
        }
        Some(modules)
    };
    let module_chain: Option<Rc<dyn ModuleLookup>> =
        if dep_registry.is_none() && global_lookup.is_none() {
            None
        } else {
            Some(Rc::new(LayeredLookup {
                registry: dep_registry,
                global: global_lookup,
            }) as Rc<dyn ModuleLookup>)
        };
    let globals = handl_runtime::fp::canonical_fp_host_functions(module_chain);
    let runtime = Runtime::from_registry_with_resolver(
        lang,
        &registry,
        &config,
        Some(globals),
        None,
        resolver,
        None,
    )?;
    Ok(runtime)
}

/// A [`ModuleLookup`] that tries the module's registered dependency registry
/// first, then falls back to an injected global lookup (e.g. the stdlib
/// package table). This is how a string callee `"mod.fn"` resolves when the
/// module did not pre-list `mod` in its `deps`.
struct LayeredLookup {
    registry: Option<handl_plugin_module::ModuleRegistry>,
    global: Option<Arc<dyn ModuleLookup + Send + Sync>>,
}

impl ModuleLookup for LayeredLookup {
    fn lookup(&self, name: &str) -> Option<&dyn Function> {
        if let Some(registry) = &self.registry {
            if let Some(f) = registry.lookup(name) {
                return Some(f);
            }
        }
        self.global.as_deref().and_then(|g| g.lookup(name))
    }
}

// ---------------------------------------------------------------------------
// HandlFunction
// ---------------------------------------------------------------------------

/// A callable [`Function`] backed by a `.hnl` source field.
///
/// Each call evaluates `(source).func_name(__arg_N)` via a cached per-thread
/// `Runtime`. Arguments are bound directly on the call frame via
/// [Runtime::eval_shared_with_call_args] (not injected as per-call engine
/// globals), so no engine cloning occurs per call. The `Runtime` is built
/// lazily on first use and reused for subsequent calls on the same thread;
/// the applied form's parsed `Ast` is likewise cached per (module,
/// function, arity) in `AST_CACHE`, so only the eval stage runs per call.
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
        // Arguments are bound directly on the call frame (`__arg_N`) — never
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
        // Build the call args as `(name, value)` pairs. The names (`__arg_N`)
        // match the cached applied-form AST `(source).func_name(__arg_0, …)`,
        // so the parse cache stays valid. Args are bound directly on the call
        // frame via `Runtime::eval_shared_with_call_args` — NOT injected as
        // per-call engine globals via `eval_shared_with_globals`, which would
        // clone the eval engine once per argument. The tree-walk engine merges
        // them into the `ScopeStack`'s globals map (excluded from `capture()`,
        // so no env capture, ADR-0106).
        let mut call_args: Vec<(String, serde_json::Value)> = Vec::with_capacity(args.len());
        let mut arg_names: Vec<String> = Vec::with_capacity(args.len());
        for (i, arg) in args.iter().enumerate() {
            let name = format!("__arg_{i}");
            let mut v = arg.clone();
            if let Some(frame_id) = caller_frame {
                stamp_home(&mut v, frame_id);
            }
            call_args.push((name.clone(), v));
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
        // source text is a pure function of `(module id, function name,
        // arity)`. Arguments bind directly on the call frame (not as source
        // text), so the same parsed `Ast` is valid for every call with the
        // same arity. The `Rc<Ast>` is cloned out of the thread-local cache
        // and the `RefCell` borrow released *before* eval so re-entrant
        // nested calls never hold the borrow; eval only reads the AST, so
        // sharing is sound.
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
                AST_CACHE.with(|cache| cache.borrow_mut().insert(ast_key, Rc::clone(&parsed)));
                parsed
            }
        };
        // Keep the parsed AST: if the result contains arrows, their `body`
        // indices refer to it — register it as a home frame so the caller
        // can apply them.
        let result = runtime
            .eval_shared_with_call_args(&ast, &call_args)
            .map_err(errf)?;
        if contains_unhomed_arrow(&result) {
            let frame_id = handl_plugin_engine_exec_tree_walk::register_home_frame(
                Rc::clone(&ast),
                Rc::new(runtime.lang().clone()),
                runtime.modules().cloned(),
                None,
                runtime.import_resolver().cloned(),
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
/// let module = HandlModule::new("stats", STATS_HNL, &["sum", "avg", "stddev"]);
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
    /// Optional global [`ModuleLookup`] layered *under* the dependency
    /// registry, so a string callee `"mod.fn"` that is not a registered
    /// dependency resolves through it (e.g. `plugin-lib-stdlib`'s table
    /// lookup reaches every stdlib module without pre-listing `deps`).
    global_lookup: Option<Arc<dyn ModuleLookup + Send + Sync>>,
    /// The eval engine name this module's bridge runtime binds
    /// (`"tree-walk"` default; `"bytecode"` selects the bytecode VM —
    /// much faster on eval-heavy module bodies such as the loader's
    /// per-character `str.*`/`char.*` work). Per-module selection keeps
    /// the change data-driven: the name resolves through the same
    /// [`EngineRegistry`] the tree-walk uses.
    eval_name: &'static str,
    /// Optional explicit [`ImportResolver`] for `import`/`from` statements
    /// in the source. When `None`, `build_runtime` derives a
    /// [`DependencyImportResolver`] from the resolved dependencies.
    import_resolver: Option<Arc<dyn ImportResolver + Send + Sync>>,
    /// The prepared `.hnl` source (for [`Module::value`] projections).
    source: String,
    /// The current runtime builder (rebuilt by `rebuild_functions`).
    builder: RuntimeBuilder,
    /// The current module ID — key into `RUNTIME_CACHE`/`AST_CACHE`/
    /// `VALUE_AST_CACHE`; re-allocated by `rebuild_functions` so stale
    /// caches are abandoned when the dependency set changes.
    module_id: usize,
    /// Names of the record's value (non-arrow) exports, declared by the
    /// host via [`HandlModule::with_value_names`] — `from "<mod>" import
    /// <name>` binds the evaluated value rather than a string callee.
    value_names: Vec<String>,
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
        let module_id = allocate_module_id();
        let eval_name = "tree-walk";

        // Initial builder with no dependencies. `with_dependency` will
        // replace this with a builder that includes the dependencies.
        let builder: RuntimeBuilder = Arc::new(|| build_runtime(Vec::new(), eval_name, None, None));

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
            global_lookup: None,
            eval_name,
            import_resolver: None,
            source,
            builder,
            module_id,
            value_names: Vec::new(),
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
        let builder: RuntimeBuilder = Arc::new(|| build_runtime(Vec::new(), eval_name, None, None));

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
            global_lookup: None,
            eval_name,
            import_resolver: None,
            source,
            builder,
            module_id,
            value_names: Vec::new(),
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

    /// Attach a global [`ModuleLookup`] layered under the dependency
    /// registry.
    ///
    /// When a string callee `"mod.fn"` is not found among the registered
    /// dependencies, it is resolved through this lookup. This is how stdlib
    /// packages reach the whole module table without pre-listing `deps`
    /// (see `plugin-lib-stdlib`'s table lookup).
    pub fn with_global_lookup(mut self, lookup: Arc<dyn ModuleLookup + Send + Sync>) -> Self {
        self.global_lookup = Some(lookup);
        self.rebuild_functions();
        self
    }

    /// Attach the [`ImportResolver`] used to evaluate `import`/`from`
    /// statements found in the (prepared) `.hnl` source.
    ///
    /// The runtime calls this resolver through `prim_import` to obtain a
    /// module's bindings; see [`DependencyImportResolver`] for the binding
    /// shape (string callees that dispatch through the module registry,
    /// identical to how `mod.fn` resolves today).
    ///
    /// When not set, `build_runtime` derives a [`DependencyImportResolver`]
    /// from the module's resolved dependencies -- sufficient whenever every
    /// import path names a registered dependency. This explicit hook is for
    /// resolvers that *build* modules from a table rather than reuse the
    /// dependency list (e.g. `plugin-lib-stdlib`'s `TableImportResolver`).
    pub fn with_import_resolver(mut self, resolver: Arc<dyn ImportResolver + Send + Sync>) -> Self {
        self.import_resolver = Some(resolver);
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
        let import_resolver = self.import_resolver.clone();
        let global_lookup = self.global_lookup.clone();
        let builder: RuntimeBuilder = Arc::new(move || {
            build_runtime(
                deps.clone(),
                eval_name,
                import_resolver.clone(),
                global_lookup.clone(),
            )
        });
        let module_id = allocate_module_id();
        for func in self.functions.values_mut() {
            func.builder = Arc::clone(&builder);
            func.module_id = module_id;
        }
        self.builder = builder;
        self.module_id = module_id;
    }

    /// Declare `names` as value exports — record fields exposed as plain
    /// evaluated values rather than string callees.
    ///
    /// `from "<mod>" import <name>` binds the field's value (e.g. `bool`'s
    /// `true`/`false` bind `Bool`s), while function exports bind the
    /// `mod.fn` string callee. The names are declared by the host because
    /// distinguishing arrow fields from value fields requires evaluating
    /// the record — explicit names keep enumeration cheap.
    pub fn with_value_names(mut self, names: &[&str]) -> Self {
        self.value_names = names.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Evaluate `(source).<name>` and return the field's value.
    ///
    /// Projection happens inside the module's own cached runtime: the
    /// record is constructed in the engine and only the requested field
    /// crosses the `serde_json::Value` boundary, so arrow fields never
    /// materialize (the exponential-env blowup documented on `AST_CACHE`
    /// only applies to whole-record eval).
    fn eval_value(&self, name: &str) -> Option<serde_json::Value> {
        let module_id = self.module_id;
        let builder = Arc::clone(&self.builder);
        let runtime = RUNTIME_CACHE.with(|cache| {
            std::rc::Rc::clone(cache.borrow_mut().entry(module_id).or_insert_with(|| {
                std::rc::Rc::new(builder().expect("canonical kit assembly should not fail"))
            }))
        });
        let ast_key = (module_id, name.to_string());
        let ast = VALUE_AST_CACHE.with(|cache| cache.borrow().get(&ast_key).cloned());
        let ast = match ast {
            Some(ast) => ast,
            None => {
                let source = format!("({}).{}", self.source, name);
                let toks = runtime.lex(&handl_language::Source::new(&source)).ok()?;
                let parsed = std::rc::Rc::new(runtime.parse(&toks).ok()?);
                VALUE_AST_CACHE.with(|cache| {
                    cache
                        .borrow_mut()
                        .insert(ast_key, std::rc::Rc::clone(&parsed))
                });
                parsed
            }
        };
        runtime.eval_shared_with_call_args(&ast, &[]).ok()
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

    fn value_names(&self) -> Vec<String> {
        self.value_names.clone()
    }

    fn value(&self, name: &str) -> Option<serde_json::Value> {
        if !self.value_names.iter().any(|n| n == name) {
            return None;
        }
        self.eval_value(name)
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
            HandlModule::new_raw("loader", LOADER_HNL, LOADER_FUNCS).with_eval_engine("bytecode");
        for dep in deps {
            module = module.with_dependency_arc(dep);
        }
        Self { module }
    }

    /// Invoke a loader transform function on `src`.
    pub fn call(&self, func: &str, src: &str) -> Result<serde_json::Value, EvalError> {
        let f = self
            .module
            .function(func)
            .ok_or_else(|| EvalError::Other(format!("loader: no transform function '{func}'")))?;
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
    let result = LOADER.set(loader);
    result
}

/// The installed [`HandlLoader`], if any.
pub fn loader() -> Option<&'static HandlLoader> {
    LOADER.get()
}

/// Call a transform on the installed loader; `None` when no loader is
/// installed or the call fails (callers fall back to `bootstrap`).
fn loader_call(func: &str, src: &str) -> Option<serde_json::Value> {
    match loader()?.call(func, src) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("loader_call {func} failed: {e}");
            None
        }
    }
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

/// The `str-handl` package sources: `(basename, source, exported names)`
/// for every per-arrow and `internal/` helper file. The basename is the
/// module name a `from "./x" import` (or `from "./internal/x" import`)
/// spec normalizes to, so sibling files resolve each other by basename.
/// Rows are in dependency order — a file may only import rows above it
/// (plus `char`). [`STR_HANDL_HNL`] is the aggregator index on top.
pub const STR_HANDL_SOURCES: &[(&str, &str, &[&str])] = &[
    // -- internal/ generic list helpers (no `char`) -------------------------
    ("rev", include_str!("../../../handl-packages/str-handl/internal/rev.hnl"), &["rev"]),
    ("len", include_str!("../../../handl-packages/str-handl/internal/len.hnl"), &["len"]),
    ("head", include_str!("../../../handl-packages/str-handl/internal/head.hnl"), &["head"]),
    ("drop", include_str!("../../../handl-packages/str-handl/internal/drop.hnl"), &["drop"]),
    ("member", include_str!("../../../handl-packages/str-handl/internal/member.hnl"), &["member"]),
    ("prefix_match", include_str!("../../../handl-packages/str-handl/internal/prefix_match.hnl"), &["prefix_match"]),
    ("take", include_str!("../../../handl-packages/str-handl/internal/take.hnl"), &["take"]),
    ("append", include_str!("../../../handl-packages/str-handl/internal/append.hnl"), &["append"]),
    ("last", include_str!("../../../handl-packages/str-handl/internal/last.hnl"), &["last"]),
    ("find_idx", include_str!("../../../handl-packages/str-handl/internal/find_idx.hnl"), &["find_idx"]),
    // -- internal/ string helpers (over `char`) -----------------------------
    ("capitalize_first", include_str!("../../../handl-packages/str-handl/internal/capitalize_first.hnl"), &["capitalize_first"]),
    ("pad_to", include_str!("../../../handl-packages/str-handl/internal/pad_to.hnl"), &["pad_to"]),
    // -- internal/ callback-invoking helpers --------------------------------
    // `map`/`all`/`drop_while` evaluate caller-supplied arrows, whose
    // `char.*`/helper calls resolve in THESE modules' runtimes — they must
    // come after every other internal row (see `STR_HANDL_CALLBACK_SCOPE`).
    ("map", include_str!("../../../handl-packages/str-handl/internal/map.hnl"), &["map"]),
    ("all", include_str!("../../../handl-packages/str-handl/internal/all.hnl"), &["all"]),
    ("drop_while", include_str!("../../../handl-packages/str-handl/internal/drop_while.hnl"), &["drop_while"]),
    ("split_words", include_str!("../../../handl-packages/str-handl/internal/split_words.hnl"), &["split_words"]),
    // -- `str` contract arrows (one file per arrow) -------------------------
    ("join", include_str!("../../../handl-packages/str-handl/join.hnl"), &["join"]),
    ("concat", include_str!("../../../handl-packages/str-handl/concat.hnl"), &["concat"]),
    ("is_empty", include_str!("../../../handl-packages/str-handl/is_empty.hnl"), &["is_empty"]),
    ("equals", include_str!("../../../handl-packages/str-handl/equals.hnl"), &["equals"]),
    ("repeat", include_str!("../../../handl-packages/str-handl/repeat.hnl"), &["repeat"]),
    ("upper", include_str!("../../../handl-packages/str-handl/upper.hnl"), &["upper"]),
    ("lower", include_str!("../../../handl-packages/str-handl/lower.hnl"), &["lower"]),
    ("trim", include_str!("../../../handl-packages/str-handl/trim.hnl"), &["trim"]),
    ("trim_start", include_str!("../../../handl-packages/str-handl/trim_start.hnl"), &["trim_start"]),
    ("trim_end", include_str!("../../../handl-packages/str-handl/trim_end.hnl"), &["trim_end"]),
    ("trim_matches", include_str!("../../../handl-packages/str-handl/trim_matches.hnl"), &["trim_matches"]),
    ("reverse", include_str!("../../../handl-packages/str-handl/reverse.hnl"), &["reverse"]),
    ("length", include_str!("../../../handl-packages/str-handl/length.hnl"), &["length"]),
    ("char_at", include_str!("../../../handl-packages/str-handl/char_at.hnl"), &["char_at"]),
    ("slice", include_str!("../../../handl-packages/str-handl/slice.hnl"), &["slice"]),
    ("split_at", include_str!("../../../handl-packages/str-handl/split_at.hnl"), &["split_at"]),
    ("is_alpha", include_str!("../../../handl-packages/str-handl/is_alpha.hnl"), &["is_alpha"]),
    ("is_numeric", include_str!("../../../handl-packages/str-handl/is_numeric.hnl"), &["is_numeric"]),
    ("is_alphanumeric", include_str!("../../../handl-packages/str-handl/is_alphanumeric.hnl"), &["is_alphanumeric"]),
    ("capitalize", include_str!("../../../handl-packages/str-handl/capitalize.hnl"), &["capitalize"]),
    ("title_case", include_str!("../../../handl-packages/str-handl/title_case.hnl"), &["title_case"]),
    ("snake_case", include_str!("../../../handl-packages/str-handl/snake_case.hnl"), &["snake_case"]),
    ("camel_case", include_str!("../../../handl-packages/str-handl/camel_case.hnl"), &["camel_case"]),
    ("pascal_case", include_str!("../../../handl-packages/str-handl/pascal_case.hnl"), &["pascal_case"]),
    ("kebab_case", include_str!("../../../handl-packages/str-handl/kebab_case.hnl"), &["kebab_case"]),
    ("contains", include_str!("../../../handl-packages/str-handl/contains.hnl"), &["contains"]),
    ("starts_with", include_str!("../../../handl-packages/str-handl/starts_with.hnl"), &["starts_with"]),
    ("ends_with", include_str!("../../../handl-packages/str-handl/ends_with.hnl"), &["ends_with"]),
    ("count", include_str!("../../../handl-packages/str-handl/count.hnl"), &["count"]),
    ("find", include_str!("../../../handl-packages/str-handl/find.hnl"), &["find"]),
    ("rfind", include_str!("../../../handl-packages/str-handl/rfind.hnl"), &["rfind"]),
    ("levenshtein", include_str!("../../../handl-packages/str-handl/levenshtein.hnl"), &["levenshtein"]),
    ("similarity", include_str!("../../../handl-packages/str-handl/similarity.hnl"), &["similarity"]),
    ("split", include_str!("../../../handl-packages/str-handl/split.hnl"), &["split"]),
    ("replace", include_str!("../../../handl-packages/str-handl/replace.hnl"), &["replace"]),
    ("replace_first", include_str!("../../../handl-packages/str-handl/replace_first.hnl"), &["replace_first"]),
    ("pad_left", include_str!("../../../handl-packages/str-handl/pad_left.hnl"), &["pad_left"]),
    ("pad_right", include_str!("../../../handl-packages/str-handl/pad_right.hnl"), &["pad_right"]),
    ("strip_prefix", include_str!("../../../handl-packages/str-handl/strip_prefix.hnl"), &["strip_prefix"]),
    ("strip_suffix", include_str!("../../../handl-packages/str-handl/strip_suffix.hnl"), &["strip_suffix"]),
    ("lines", include_str!("../../../handl-packages/str-handl/lines.hnl"), &["lines"]),
    ("split_whitespace", include_str!("../../../handl-packages/str-handl/split_whitespace.hnl"), &["split_whitespace"]),
    ("truncate", include_str!("../../../handl-packages/str-handl/truncate.hnl"), &["truncate"]),
    ("slugify", include_str!("../../../handl-packages/str-handl/slugify.hnl"), &["slugify"]),
    ("word_wrap", include_str!("../../../handl-packages/str-handl/word_wrap.hnl"), &["word_wrap"]),
];

/// Internal helpers that invoke caller-supplied arrows (`f`/`p`).
const STR_HANDL_CALLBACK_INVOKERS: &[&str] = &["map", "all", "drop_while"];

/// The module names a caller-supplied arrow may legitimately reach:
/// `char` (wired via each invoker's own `from "char" import`) plus every
/// internal helper. Host/module-function calls resolve **by name in the
/// invoking runtime's module chain** — not the closure's — so when e.g.
/// `trim_matches` passes `c ~> member(mcs, c)` to `drop_while`, the
/// `member.member` call resolves only if `member` is a dep of
/// `drop_while`'s module. [`STR_HANDL_SOURCES`] orders these before the
/// invokers so each is already built when wired here.
const STR_HANDL_CALLBACK_SCOPE: &[&str] = &[
    "rev",
    "len",
    "head",
    "drop",
    "take",
    "append",
    "last",
    "member",
    "prefix_match",
    "find_idx",
    "capitalize_first",
    "pad_to",
    "map",
    "all",
    "drop_while",
];

/// The first `"..."`-quoted spec of each top-level `from "..."`/`import
/// "..."` line in `src`, in order. Used to wire [`HandlModule`] deps so
/// the Rust table above can never drift from the `.hnl` imports.
fn str_handl_import_specs(src: &str) -> Vec<String> {
    let mut specs = Vec::new();
    for line in src.lines() {
        let t = line.trim_start();
        if !(t.starts_with("from ") || t.starts_with("import ")) {
            continue;
        }
        let rest = t[t.find(' ').unwrap() + 1..].trim_start();
        if let Some(inner) = rest.strip_prefix('"').and_then(|r| r.split('"').next()) {
            specs.push(inner.to_string());
        }
    }
    specs
}

/// Build every `str-handl` sibling file as a [`HandlModule`], wiring each
/// file's `from "..."` imports as dep modules. Returns
/// `(basename -> module, index module)`; the index is [`STR_HANDL_HNL`]
/// re-exporting the 45 `str` arrows under the name `"str"`.
fn str_handl_modules(
    resolver: &dyn ModuleResolver,
    eval_name: &'static str,
) -> (HashMap<String, Arc<dyn handl_platform::Module + Send + Sync>>, HandlModule) {
    let mut siblings: HashMap<String, Arc<dyn handl_platform::Module + Send + Sync>> =
        HashMap::new();
    for &(name, src, funcs) in STR_HANDL_SOURCES {
        let mut module = HandlModule::new(name, src, funcs).with_eval_engine(eval_name);
        for spec in str_handl_import_specs(src) {
            let dep_name = normalize_import_name(&spec);
            let dep = siblings
                .get(dep_name)
                .cloned()
                .or_else(|| resolver.resolve(dep_name))
                .unwrap_or_else(|| {
                    panic!("str-handl: unresolved import '{spec}' in {name}.hnl")
                });
            module = module.with_dependency_arc(dep);
        }
        // Callback-invoking helpers additionally depend on every internal
        // module a caller-supplied arrow may reach (see
        // `STR_HANDL_CALLBACK_SCOPE`). Names not yet built (the invokers
        // themselves, or `name`) are skipped — rows are ordered so all
        // reachable helpers precede them.
        if STR_HANDL_CALLBACK_INVOKERS.contains(&name) {
            for &dep_name in STR_HANDL_CALLBACK_SCOPE {
                if dep_name == name {
                    continue;
                }
                if let Some(dep) = siblings.get(dep_name) {
                    module = module.with_dependency_arc(Arc::clone(dep));
                }
            }
        }
        siblings.insert(name.to_string(), Arc::new(module));
    }
    let mut index =
        HandlModule::new("str", STR_HANDL_HNL, STR_HANDL_FUNCS).with_eval_engine(eval_name);
    for spec in str_handl_import_specs(STR_HANDL_HNL) {
        let dep_name = normalize_import_name(&spec);
        let dep = siblings.get(dep_name).cloned().unwrap_or_else(|| {
            panic!("str-handl: index imports '{spec}' but no such arrow file exists")
        });
        index = index.with_dependency_arc(dep);
    }
    (siblings, index)
}

/// Build the [`HandlModule`] for `str-handl`.
///
/// `str-handl` is a package: [`STR_HANDL_HNL`] is an aggregator index and
/// each `str` arrow lives in its own file ([`STR_HANDL_SOURCES`]). The
/// files import `char`, so the supplied resolver must map `"char"` to a
/// `char` backend (typically
/// [`CharModule`](handl_plugin_lib_char_native::CharModule)); sibling
/// `from "./x"` imports are wired from the sources table itself. Runs on
/// the bytecode engine — the loader's `str` dep path.
pub fn build_str_handl_module(resolver: &dyn ModuleResolver) -> HandlModule {
    str_handl_modules(resolver, "bytecode").1
}

/// Build the [`HandlModule`] for `str-handl` on `eval_name` ("tree-walk"
/// or "bytecode"), for kits that pick the engine themselves.
pub fn build_str_handl_module_with(
    resolver: &dyn ModuleResolver,
    eval_name: &'static str,
) -> HandlModule {
    str_handl_modules(resolver, eval_name).1
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
