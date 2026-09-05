//! # handl-lib-handl (generic `.hnl` library loader)
//!
//! A generic crate that lets `.hnl` files replace hand-rolled Rust
//! `plugin-lib-*` bridge crates for pure-logic libraries.
//!
//! Each existing pure-logic library (e.g. `plugin-lib-stats`,
//! `plugin-lib-hello`) has ~100+ lines of Rust bridge code that
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
//! per-thread (via [`thread_local!`]), and only the small applied expression
//! `(source).func(args)` is lexed/parsed/evaluated per call.
//!
//! ## Usage
//!
//! ```ignore
//! use handl_plugin_lib_handl::{HandlModule, HandlLibraryPlugin};
//! use handl_plugin_lib_utils::{MathModule, TypeModule};
//!
//! const STATS_HNL: &str = include_str!("../../../handl-packages/stats/stats.hnl");
//! const STATS_FUNCS: &[&str] = &["sum", "avg", "product", "min", "max", "median", "stddev"];
//!
//! let module = HandlModule::new("stats", STATS_HNL, STATS_FUNCS)
//!     .with_dependency(MathModule)
//!     .with_dependency(TypeModule);
//!
//! // Or as a plugin:
//! let plugin = HandlLibraryPlugin::new("stats", "lib.stats", "lib-stats", STATS_HNL, STATS_FUNCS)
//!     .with_dependency(MathModule)
//!     .with_dependency(TypeModule);
//! ```

#![warn(missing_docs)]

pub mod op_registry;
pub use op_registry::{OpFunction, OpRegistry, RuntimePredicateChecker};

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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

/// Strip `--` comment lines from a `.hnl` source so the embedded source
/// parses as a bare expression.
///
/// Data files often have a `--` header comment that documents the
/// definitions but is not valid inside the applied `(source).func(input)`
/// form the Pratt parser evaluates, so it is removed before evaluation.
fn strip_comment_lines(src: &str) -> String {
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
pub fn strip_declaration_lines(src: &str) -> String {
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

/// Returns true if the line is an import declaration (`import ...` or
/// `from ... import ...`).
fn is_import_line(trimmed: &str) -> bool {
    if let Some(rest) = trimmed.strip_prefix("import") {
        if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') || rest.starts_with('"') || rest.starts_with('\'') {
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

/// A parsed import declaration from a `.hnl` source file.
#[derive(Debug, Clone)]
pub struct ImportDecl {
    /// The module path being imported from (e.g. `"str"`, `"fn"`).
    pub path: String,
    /// The local alias for the imported module (e.g. `"str"`).
    pub alias: String,
}

/// Parse import declarations from a `.hnl` source string.
///
/// Supports: `from 'path' import * as 'alias'` and `import "path" as alias`.
pub fn parse_import_declarations(src: &str) -> Vec<ImportDecl> {
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
    if rest.is_empty() { return None; }
    let (path, after_path) = extract_quoted(rest)?;
    let import_kw = after_path.trim().strip_prefix("import")?.trim();
    let star = import_kw.strip_prefix("*")?.trim();
    let as_kw = star.strip_prefix("as")?.trim();
    if as_kw.is_empty() { return None; }
    let alias = if as_kw.starts_with('"') || as_kw.starts_with('\'') {
        extract_quoted(as_kw)?.0
    } else {
        as_kw.split_whitespace().next()?.to_string()
    };
    if alias.is_empty() { return None; }
    Some(ImportDecl { path, alias })
}

fn parse_legacy_import_line(line: &str) -> Option<ImportDecl> {
    let rest = line.strip_prefix("import")?.trim();
    if rest.is_empty() { return None; }
    let (path, after) = extract_quoted(rest)?;
    let after = after.trim();
    let alias = if let Some(as_part) = after.strip_prefix("as") {
        let a = as_part.trim();
        if a.is_empty() { return None; }
        if a.starts_with('"') || a.starts_with('\'') { extract_quoted(a)?.0 }
        else { a.split_whitespace().next()?.to_string() }
    } else {
        path.rsplit('/').next().unwrap_or(&path).to_string()
    };
    Some(ImportDecl { path, alias })
}

fn extract_quoted(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    let quote = s.chars().next()?;
    if quote != '"' && quote != '\'' { return None; }
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
pub fn parse_cap_declarations(src: &str) -> Vec<String> {
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
pub fn parse_tag_declarations(src: &str) -> Vec<(String, String)> {
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
        // Find the `=` separator.
        if let Some(eq_idx) = rest.find('=') {
            let name = rest[..eq_idx].trim().to_string();
            let payload_type = rest[eq_idx + 1..].trim().to_string();
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
pub fn expand_tag_declarations(src: &str) -> String {
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
                    if let Some(eq_idx) = rest.find('=') {
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
            "  {name} = _0 ~> tag.tag(\"{name}\", {{value: _0}})\n"
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

/// Serialize a JSON value into a handl literal string.
///
/// Handl array/number/string/bool/null literals are JSON-compatible, so
/// `serde_json::to_string` produces valid handl source for the values this
/// loader accepts.
fn value_to_handl_literal(value: &serde_json::Value) -> Result<String, EvalError> {
    serde_json::to_string(value)
        .map_err(|e| EvalError::Other(format!("handl: failed to serialize value: {e}")))
}

/// Serialize a list of JSON argument values into a comma-separated handl
/// argument list string (e.g. `[1, 2]` → `"1, 2"`).
fn args_to_handl_literal(args: &[serde_json::Value]) -> Result<String, EvalError> {
    let mut parts = Vec::with_capacity(args.len());
    for arg in args {
        parts.push(value_to_handl_literal(arg)?);
    }
    Ok(parts.join(", "))
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
    static RUNTIME_CACHE: RefCell<HashMap<usize, Runtime>> = RefCell::new(HashMap::new());
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
) -> Result<Runtime, handl_language::LangError> {
    let kit = handl_kit_data_canonical::assemble()?;
    let lang = kit.lang().clone();
    let config = RuntimeConfig::new()
        .with_lex("data")
        .with_parse("pratt")
        .with_eval("tree-walk");
    let mut registry = EngineRegistry::new();
    registry.register_lex(
        "data",
        Box::new(handl_plugin_engine_lex_pattern::DataLexerEngine::new()),
    );
    registry.register_parse("pratt", handl_plugin_engine_parse_pratt::PrattEngine::new());
    registry.register_eval(
        "tree-walk",
        Box::new(handl_plugin_engine_eval_tree_walk::TreeWalkEngine::new()),
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
/// Each call evaluates `(source).func_name(args)` via a cached per-thread
/// `Runtime`. The `Runtime` is built lazily on first use and reused for
/// subsequent calls on the same thread.
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
        let args_literal = args_to_handl_literal(args)?;
        let source = format!("({}).{}({})", self.source, self.func_name, args_literal);
        let module_id = self.module_id;
        let builder = Arc::clone(&self.builder);
        let module_name = &self.module_name;
        let func_name = &self.func_name;
        RUNTIME_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            let runtime = cache
                .entry(module_id)
                .or_insert_with(|| builder().expect("canonical kit assembly should not fail"));
            runtime
                .run(&Source::new(&source))
                .map_err(|e| EvalError::Other(format!("{module_name}.{func_name}: {e}")))
        })
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
/// Dependency modules (e.g. [`MathModule`](handl_plugin_lib_utils::MathModule),
/// [`TypeModule`](handl_plugin_lib_utils::TypeModule)) can be registered via
/// [`HandlModule::with_dependency`] so that module-qualified calls inside the
/// `.hnl` source (e.g. `math.sqrt`) resolve during evaluation.
///
/// # Example
///
/// ```ignore
/// use handl_plugin_lib_handl::HandlModule;
/// use handl_plugin_lib_utils::{MathModule, TypeModule};
///
/// const STATS_HNL: &str = include_str!("../../../handl-packages/stats/stats.hnl");
///
/// let module = HandlModule::new("stats", STATS_HNL, &["sum", "avg", "stddev"])
///     .with_dependency(MathModule)
///     .with_dependency(TypeModule);
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
    /// The raw `.hnl` source (before stripping), used for parsing imports.
    raw_source: String,
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
        let stripped = strip_declaration_lines(&strip_comment_lines(hnl_source));
        let source = expand_tag_declarations(&stripped);
        let module_id = allocate_module_id();

        // Initial builder with no dependencies. `with_dependency` will
        // replace this with a builder that includes the dependencies.
        let builder: RuntimeBuilder = Arc::new(|| build_runtime(Vec::new()));

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
            raw_source: hnl_source.to_string(),
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
        let imports = parse_import_declarations(&self.raw_source);
        for decl in &imports {
            if let Some(module) = resolver.resolve(&decl.path) {
                self.dependencies.push(module);
            }
        }
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
            self.op_functions
                .insert(local_name.to_string(), OpFunction::new(op_name, Arc::clone(&registry)));
        }
        self
    }

    /// Rebuild all [`HandlFunction`] instances with an updated runtime
    /// builder that includes the current dependencies.
    ///
    /// This is called after `with_dependency` / `with_dependency_arc` to
    /// ensure the builder reflects the latest dependency set. The per-thread
    /// runtime cache uses `module_id` as the key, so any previously cached
    /// runtime for this module ID will be replaced on next access.
    fn rebuild_functions(&mut self) {
        let deps = self.dependencies.clone();
        let builder: RuntimeBuilder = Arc::new(move || build_runtime(deps.clone()));
        for func in self.functions.values_mut() {
            func.builder = Arc::clone(&builder);
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
/// use handl_plugin_lib_utils::{MathModule, TypeModule};
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
            source: expand_tag_declarations(&strip_declaration_lines(&strip_comment_lines(
                hnl_source,
            ))),
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{EvalContext, ModuleLookup};
    use handl_plugin_module::ModuleRegistry;
    use serde_json::json;

    /// A minimal `.hnl` source for testing: a record with `double` and
    /// `inc` fields.
    const TEST_HNL: &str = r#"
-- A test record of arrows.
{
  double: xs ~> xs * 2,
  inc: xs ~> xs + 1
}
"#;

    /// A `.hnl` source that uses `math.sqrt` (requires a dependency module).
    const TEST_DEP_HNL: &str = r#"
-- Uses math.sqrt, requires MathModule dependency.
{
  root: xs ~> math.sqrt(xs)
}
"#;

    #[test]
    fn handl_module_exposes_functions() {
        let module = HandlModule::new("test", TEST_HNL, &["double", "inc"]);
        assert_eq!(module.name(), "test");

        let mut ctx = EvalContext::new();
        let double = module.function("double").expect("double should exist");
        let result = double.call(&[json!(21)], &mut ctx).unwrap();
        assert_eq!(result, json!(42));

        let inc = module.function("inc").expect("inc should exist");
        let result = inc.call(&[json!(41)], &mut ctx).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn handl_module_unknown_function_returns_none() {
        let module = HandlModule::new("test", TEST_HNL, &["double"]);
        assert!(module.function("nope").is_none());
    }

    #[test]
    fn handl_module_functions_enumeration() {
        let module = HandlModule::new("test", TEST_HNL, &["double", "inc"]);
        let fns = module.functions();
        assert_eq!(fns.len(), 2);
        let names: Vec<&str> = fns.iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"double"));
        assert!(names.contains(&"inc"));
    }

    #[test]
    fn handl_module_with_dependency_resolves_module_calls() {
        use handl_plugin_lib_utils::MathModule;
        let module = HandlModule::new("test", TEST_DEP_HNL, &["root"])
            .with_dependency(MathModule);

        let mut ctx = EvalContext::new();
        let root = module.function("root").expect("root should exist");
        let result = root.call(&[json!(16)], &mut ctx).unwrap();
        assert_eq!(result.as_f64().unwrap(), 4.0);
    }

    #[test]
    fn handl_module_works_in_module_registry() {
        let module = HandlModule::new("test", TEST_HNL, &["double", "inc"]);
        let registry = ModuleRegistry::new().with(module);
        let lookup: &dyn ModuleLookup = &registry;

        let func = lookup.lookup("test.double").expect("test.double should resolve");
        let mut ctx = EvalContext::new();
        let result = func.call(&[json!(5)], &mut ctx).unwrap();
        assert_eq!(result, json!(10));
    }

    #[test]
    fn handl_library_plugin_contributes_module() {
        use handl_platform::{PluginRegistry, KitRegistry};
        let plugin = HandlLibraryPlugin::new(
            "test", "lib.test", "lib-test", TEST_HNL, &["double", "inc"],
        );

        assert_eq!(plugin.id(), "lib.test");
        assert_eq!(plugin.name(), "lib-test");

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        plugins.register(ModuleRuntime::new());
        let mut cctx = ContributionContext::new(&mut plugins, &mut kits);
        plugin.contribute(&mut cctx).unwrap();

        let runtime = plugins
            .get_plugin_mut::<ModuleRuntime>()
            .expect("ModuleRuntime should be registered");
        assert_eq!(runtime.module_count(), 1);
        assert!(runtime.lookup("test.double").is_some());
    }

    #[test]
    fn strip_comment_lines_removes_dashed_comments() {
        let src = "-- a comment\nlet x = 1 in\n-- another\nx";
        let stripped = strip_comment_lines(src);
        assert!(!stripped.contains("--"));
        assert!(stripped.contains("let x = 1 in"));
    }

    #[test]
    fn strip_declaration_lines_removes_cap_op_handl() {
        let src = "cap \"hello.greet\"\nop \"hello.greet\" : String -> String\n  guarantee: True\nhandl \"hello.greet\" = rust_greet\nname ~> \"Hello, \" + name + \"!\"";
        let cleaned = strip_declaration_lines(src);
        assert!(!cleaned.contains("cap"));
        assert!(!cleaned.contains("op "));
        assert!(!cleaned.contains("handl "));
        assert!(!cleaned.contains("guarantee"));
        assert!(cleaned.contains("name ~>"));
    }

    #[test]
    fn strip_declaration_lines_preserves_handl_logic() {
        let src = "cap \"test.foo\"\n{\n  foo: x ~> x + 1\n}";
        let cleaned = strip_declaration_lines(src);
        assert!(cleaned.contains("foo: x ~> x + 1"));
        assert!(!cleaned.contains("cap"));
    }

    #[test]
    fn strip_declaration_lines_preserves_indented_logic() {
        // Indented lines that are NOT op continuation lines should be kept.
        let src = "cap \"test.foo\"\n{\n  foo: x ~> {\n    y = x + 1 in\n    y\n  }\n}";
        let cleaned = strip_declaration_lines(src);
        assert!(cleaned.contains("y = x + 1 in"));
        assert!(!cleaned.contains("cap"));
    }

    #[test]
    fn escape_handl_string_escapes_quotes_and_backslashes() {
        assert_eq!(escape_handl_string(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(escape_handl_string("plain"), "plain");
    }

    #[test]
    fn handl_module_with_ops_dispatches_to_registry() {
        use crate::op_registry::{OpFunction, OpRegistry};
        use std::sync::Arc;

        struct Upper;
        impl Function for Upper {
            fn call(&self, args: &[serde_json::Value], _ctx: &mut EvalContext) -> Result<serde_json::Value, EvalError> {
                let s = args[0].as_str().unwrap_or("");
                Ok(json!(s.to_uppercase()))
            }
        }

        let mut registry = OpRegistry::new();
        registry.register("test.upper", Arc::new(Upper));
        let shared = Arc::new(registry);

        let module = HandlModule::new("test", TEST_HNL, &["double"])
            .with_ops(&["test.upper"], shared);

        // The op function should be accessible via the module.
        let upper = module.function("upper").expect("upper should exist");
        let mut ctx = EvalContext::new();
        let result = upper.call(&[json!("hello")], &mut ctx).unwrap();
        assert_eq!(result, json!("HELLO"));

        // The regular function should still work.
        let double = module.function("double").expect("double should exist");
        let result = double.call(&[json!(21)], &mut ctx).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn handl_module_with_ops_works_in_module_registry() {
        use crate::op_registry::OpRegistry;
        use std::sync::Arc;

        struct Upper;
        impl Function for Upper {
            fn call(&self, args: &[serde_json::Value], _ctx: &mut EvalContext) -> Result<serde_json::Value, EvalError> {
                let s = args[0].as_str().unwrap_or("");
                Ok(json!(s.to_uppercase()))
            }
        }

        let mut registry = OpRegistry::new();
        registry.register("test.upper", Arc::new(Upper));
        let shared = Arc::new(registry);

        let module = HandlModule::new("test", TEST_HNL, &["double"])
            .with_ops(&["test.upper"], shared);
        let reg = ModuleRegistry::new().with(module);
        let lookup: &dyn ModuleLookup = &reg;

        // Op function should be resolvable via qualified name.
        let func = lookup.lookup("test.upper").expect("test.upper should resolve");
        let mut ctx = EvalContext::new();
        let result = func.call(&[json!("hi")], &mut ctx).unwrap();
        assert_eq!(result, json!("HI"));

        // Regular function should also work.
        let func = lookup.lookup("test.double").expect("test.double should resolve");
        let result = func.call(&[json!(5)], &mut ctx).unwrap();
        assert_eq!(result, json!(10));
    }

    #[test]
    fn handl_library_plugin_with_ops_contributes_module() {
        use handl_platform::{PluginRegistry, KitRegistry};
        use crate::op_registry::OpRegistry;
        use std::sync::Arc;

        struct Upper;
        impl Function for Upper {
            fn call(&self, args: &[serde_json::Value], _ctx: &mut EvalContext) -> Result<serde_json::Value, EvalError> {
                let s = args[0].as_str().unwrap_or("");
                Ok(json!(s.to_uppercase()))
            }
        }

        let mut registry = OpRegistry::new();
        registry.register("test.upper", Arc::new(Upper));
        let shared = Arc::new(registry);

        let plugin = HandlLibraryPlugin::new(
            "test", "lib.test", "lib-test", TEST_HNL, &["double"],
        )
        .with_ops(&["test.upper"], shared);

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        plugins.register(ModuleRuntime::new());
        let mut cctx = ContributionContext::new(&mut plugins, &mut kits);
        plugin.contribute(&mut cctx).unwrap();

        let runtime = plugins
            .get_plugin_mut::<ModuleRuntime>()
            .expect("ModuleRuntime should be registered");
        assert_eq!(runtime.module_count(), 1);
        assert!(runtime.lookup("test.upper").is_some());
        assert!(runtime.lookup("test.double").is_some());
    }

    // -----------------------------------------------------------------------
    // ADR-0077 Phase 5: cap declaration parsing tests
    // -----------------------------------------------------------------------

    /// A `.hnl` source with `cap` declarations for testing.
    const TEST_CAPS_HNL: &str = r#"
-- A module with capability declarations.
cap "fs.read"
cap "fs.write"
op "fs.read" : Path -> Bytes
op "fs.write" : (Path * Bytes) -> Unit
"#;

    #[test]
    fn parse_cap_declarations_extracts_caps() {
        let caps = parse_cap_declarations(TEST_CAPS_HNL);
        assert_eq!(caps, vec!["fs.read".to_string(), "fs.write".to_string()]);
    }

    #[test]
    fn parse_cap_declarations_skips_comments() {
        let src = "-- cap \"not.a.cap\"\ncap \"real.cap\"\n-- another comment\ncap \"second.cap\"";
        let caps = parse_cap_declarations(src);
        assert_eq!(caps, vec!["real.cap".to_string(), "second.cap".to_string()]);
    }

    #[test]
    fn parse_cap_declarations_ignores_non_cap_lines() {
        let src = r#"
op "test.op" : Int -> Int
handl "test.op" = rust_fn
let x = 1 in x
"#;
        let caps = parse_cap_declarations(src);
        assert!(caps.is_empty());
    }

    #[test]
    fn parse_cap_declarations_empty_source() {
        let caps = parse_cap_declarations("");
        assert!(caps.is_empty());
    }

    #[test]
    fn parse_cap_declarations_supports_single_quotes() {
        let src = "cap 'my.cap'\ncap \"other.cap\"";
        let caps = parse_cap_declarations(src);
        assert_eq!(caps, vec!["my.cap".to_string(), "other.cap".to_string()]);
    }

    #[test]
    fn parse_cap_declarations_ignores_cap_prefix_in_words() {
        // "capacity" should not be matched as a `cap` declaration.
        let src = "capacity 100\ncap \"real.cap\"";
        let caps = parse_cap_declarations(src);
        assert_eq!(caps, vec!["real.cap".to_string()]);
    }

    #[test]
    fn handl_library_plugin_with_capabilities_exposes_them() {
        use handl_platform::Plugin;
        let plugin = HandlLibraryPlugin::new_with_caps(
            "test", "lib.test", "lib-test", TEST_CAPS_HNL, &[],
        );
        let caps = plugin.capabilities();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0].as_str(), "fs.read");
        assert_eq!(caps[1].as_str(), "fs.write");
    }

    #[test]
    fn handl_library_plugin_with_capabilities_manual() {
        use handl_platform::Plugin;
        let plugin = HandlLibraryPlugin::new(
            "test", "lib.test", "lib-test", TEST_HNL, &["double"],
        )
        .with_capabilities(vec!["custom.cap".into(), "other.cap".into()]);
        let caps = plugin.capabilities();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0].as_str(), "custom.cap");
        assert_eq!(caps[1].as_str(), "other.cap");
    }

    #[test]
    fn handl_library_plugin_no_caps_returns_empty() {
        use handl_platform::Plugin;
        let plugin = HandlLibraryPlugin::new(
            "test", "lib.test", "lib-test", TEST_HNL, &["double"],
        );
        assert!(plugin.capabilities().is_empty());
    }

    // -----------------------------------------------------------------------
    // ADR-0077: tag declaration parsing and expansion tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_tag_declarations_extracts_tags() {
        let src = r#"
tag ok = Value
tag err = Error
"#;
        let tags = parse_tag_declarations(src);
        assert_eq!(
            tags,
            vec![
                ("ok".to_string(), "Value".to_string()),
                ("err".to_string(), "Error".to_string()),
            ]
        );
    }

    #[test]
    fn parse_tag_declarations_skips_comments() {
        let src = "-- tag not = Real\ntag ok = Value\n-- another\ntag err = Error";
        let tags = parse_tag_declarations(src);
        assert_eq!(
            tags,
            vec![
                ("ok".to_string(), "Value".to_string()),
                ("err".to_string(), "Error".to_string()),
            ]
        );
    }

    #[test]
    fn parse_tag_declarations_ignores_non_tag_lines() {
        let src = r#"
op "test.op" : Int -> Int
handl "test.op" = rust_fn
let x = 1 in x
"#;
        let tags = parse_tag_declarations(src);
        assert!(tags.is_empty());
    }

    #[test]
    fn parse_tag_declarations_empty_source() {
        let tags = parse_tag_declarations("");
        assert!(tags.is_empty());
    }

    #[test]
    fn parse_tag_declarations_ignores_tag_prefix_in_words() {
        // "tagged" should not be matched as a `tag` declaration.
        let src = "tagged value\ntag ok = Value";
        let tags = parse_tag_declarations(src);
        assert_eq!(tags, vec![("ok".to_string(), "Value".to_string())]);
    }

    #[test]
    fn expand_tag_declarations_no_tags_returns_unchanged() {
        let src = "{ double: xs ~> xs * 2 }";
        let expanded = expand_tag_declarations(src);
        assert_eq!(expanded, src);
    }

    #[test]
    fn expand_tag_declarations_wraps_in_do_block() {
        let src = r#"tag ok = Value
tag err = Error

{
  result_ok: value ~> value >>> ok,
}"#;
        let expanded = expand_tag_declarations(src);
        // Should start with "do {"
        assert!(expanded.starts_with("do {"));
        // Should contain constructor bindings.
        assert!(expanded.contains("ok = _0 ~> tag.tag(\"ok\", {value: _0})"));
        assert!(expanded.contains("err = _0 ~> tag.tag(\"err\", {value: _0})"));
        // Should contain the remaining source (indented).
        assert!(expanded.contains("result_ok: value ~> value >>> ok,"));
        // Should end with "}".
        assert!(expanded.ends_with("}"));
        // Should NOT contain the tag declaration lines.
        assert!(!expanded.contains("tag ok = Value"));
        assert!(!expanded.contains("tag err = Error"));
    }

    #[test]
    fn expand_tag_declarations_single_tag() {
        let src = "tag ok = Value\n{ f: x ~> x }";
        let expanded = expand_tag_declarations(src);
        assert!(expanded.starts_with("do {"));
        assert!(expanded.contains("ok = _0 ~> tag.tag(\"ok\", {value: _0})"));
        assert!(expanded.contains("f: x ~> x"));
        assert!(!expanded.contains("tag ok = Value"));
    }
}
