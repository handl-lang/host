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

pub mod model;
pub mod op_registry;
pub mod transform;
pub use model::{HandlModel, HandlModelAdapter, HandlModelEngine, term_to_value};
pub use op_registry::{OpFunction, OpRegistry, RuntimePredicateChecker};
pub use transform::{HANDL_PREFIX, HandlNativeRewrite, KEEP_KEY};

/// Re-export of the engine's [`ImportResolver`] trait so `.hnl` library
/// builders (e.g. `plugin-lib-stdlib`) can supply an import resolver
/// without taking a direct `hnl-engine-exec` dependency.
pub use hnl_engine_exec::ImportResolver;

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
/// which asks the resolver for the target module's bindings record.
/// Since ADR-0155 Phase 1 the payload is a **callable record** — one
/// [`wrap_dep_fn`] wrapper arrow per function export (plus plain value
/// exports) via [`module_bindings_tagged`]. Applying such a binding
/// (`m.f(x)`, or a bare named import `f(x)`) routes the arrow through
/// the ordinary `!App` path, and the wrapper's `callv` body lands on the
/// same `call_host` leaf a `"<module>.<field>"` string callee did —
/// resolving through the importer's [`ModuleLookup`] registry, whose
/// `<module>` entry comes from the dependency chain or, failing that,
/// the injected global module lookup.
///
/// The string-callee encoding remains available via
/// [`module_bindings_object`] for hosts that pin it — both encodings
/// apply (ADR-0155 §2).
pub struct DependencyImportResolver {
    modules: Vec<Arc<dyn Module + Send + Sync>>,
}

impl DependencyImportResolver {
    /// Wrap the resolved dependency modules of a [`HandlModule`].
    pub fn new(modules: Vec<Arc<dyn Module + Send + Sync>>) -> Self {
        Self { modules }
    }
}

/// Build the bindings object a [`resolve_import`] returns for a module,
/// in the **legacy string-callee encoding**: function exports as `mod.fn`
/// string callees plus *value* exports ([`Module::value_names`]) as their
/// evaluated values — so `from "bool" import true` binds a `Bool`, not
/// the string `"bool.true"`.
///
/// ADR-0155 Phase 1 supersedes this payload with callable records —
/// [`module_bindings_tagged`]/[`wrap_dep_module`] emit a wrapper arrow
/// per function export. Both encodings apply (a `Str` field routes to
/// `call_host`, an `Arrow` field to `!App`); this emitter remains for
/// hosts that pin the old encoding.
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

// ---------------------------------------------------------------------------
// Scope-bound env wrappers (ADR-0155 Phase 0)
// ---------------------------------------------------------------------------

/// The `rest`-param name a dep wrapper arrow binds its whole argument
/// array to — and the `var` name its `callv` body reads it back through.
/// Kept in one constant so the minted arena and the arrow record cannot
/// drift apart.
const DEP_WRAP_REST: &str = "args";

/// Mint a **wrapper arrow** for a dep function (ADR-0155 §1): the
/// callable-record payload a scope-bound env binds for `name`.
///
/// The arrow is `params: [], rest: "args"` over a `callv` body
/// (`data/eval.yaml` `eval.callv`): applying it collects every call
/// argument into `args` as an `Arr[…]` and hands that array to
/// `call_host` with `name` as the incoming callee string — the same
/// `(name, packed-args-array)` leaf an ordinary `mod.fn(x)` call lands
/// on. Unlike a `call`-node body, `callv` forwards the evaluated array
/// itself rather than re-packing it through `proj_rest`, so the leaf
/// receives `args[0]` = the whole argument list regardless of arity
/// (`Function::call` has no arity metadata — `platform/src/eval.rs`
/// `Function::call`).
///
/// The `body_def` arena is built directly (`Ast::push` +
/// [`handl_language::ast_value`], the `def_with_body` precedent) — **not**
/// via `Ast::instantiate`: the qualified name must stay an ambient `lit`
/// string so it re-resolves through the *applying* runtime's module
/// chain at `call_host` (ADR-0155 §"Wrappers are ambient re-resolution,
/// not pinning"). Nothing is captured — `binds`/`handlers` are empty —
/// and construction forces no evaluation.
///
/// `name` is the qualified leaf name the chain resolves (`"mod.fn"`).
/// Mint-time resolution is deliberately *not* checked — there is no
/// chain in scope here; a wrapper carried into a runtime whose chain
/// lacks `mod` fails at call time (`call_host: function '…' not found`),
/// the same failure an unbound dotted callee produces today.
pub fn wrap_dep_fn(name: &str) -> handl_value::TaggedValue {
    // `callv` children: [lit "mod.fn", var "args"] — child 0 evals to the
    // callee name (`LIT_KIND` declares no eval scheme, so it returns its
    // payload on every engine), child 1 evals to the rest-bound args
    // array. Layout must match `eval.callv`'s scheme.
    let mut body = handl_language::Ast::new();
    let callee = body.push(
        handl_language::AstNode::new(handl_language::LIT_KIND)
            .with_payload(serde_json::json!(name)),
    );
    let args = body.push(
        handl_language::AstNode::new("var")
            .with_payload(serde_json::json!(DEP_WRAP_REST)),
    );
    body.push(
        handl_language::AstNode::new("callv").with_payload(serde_json::json!({
            "children": [callee.index(), args.index()],
        })),
    );
    handl_value::TaggedValue::Arrow {
        params: Rc::from(Vec::<String>::new()),
        rest: Some(Rc::from(DEP_WRAP_REST)),
        array_pattern: false,
        body_def: Rc::new(handl_value::TaggedValue::from(
            handl_language::ast_value(&body),
        )),
        binds: Default::default(),
        handlers: Default::default(),
    }
}

/// Build a dep module's env record of wrapper arrows (ADR-0155 §3):
/// `{"fn": <wrap_dep_fn "mod.fn">, …}` over [`Module::functions`], with
/// [`Module::value_names`] carried over as plain values — the
/// arrow-encoding counterpart of [`module_bindings_object`].
///
/// The record is pure data: `TaggedValue::Obj` of self-contained arrows,
/// serializable as ordinary `{"__arrow__": …}` wire values.
pub fn wrap_dep_module(module: &dyn Module) -> handl_value::TaggedValue {
    let mut obj = serde_json::Map::new();
    for (fname, _f) in module.functions() {
        obj.insert(
            fname.to_string(),
            handl_language::Value::from(wrap_dep_fn(&format!(
                "{}.{}",
                module.name(),
                fname
            ))),
        );
    }
    for vname in module.value_names() {
        if let Some(v) = module.value(&vname) {
            obj.insert(vname, v);
        }
    }
    handl_value::TaggedValue::from(handl_language::Value::Object(obj))
}

/// The bindings record a resolver hands to the `import` op under
/// ADR-0155 (§3): `module`'s own [`Module::bindings_tagged`] when it
/// provides one — the evaluated export record of a `.hnl`-backed module —
/// else a synthesized [`wrap_dep_module`] record of wrapper arrows over
/// [`Module::functions`].
///
/// This is the callable counterpart of [`module_bindings_object`]: an
/// `Arrow` field applies through the ordinary `!App` path, so
/// `from "<mod>" import f` binds `f` as a real callable value rather
/// than a string callee — while the wrapper's `callv` body still lands
/// on the same `call_host` leaf the string encoding dispatched to.
pub fn module_bindings_tagged(module: &dyn Module) -> Rc<handl_value::TaggedValue> {
    module
        .bindings_tagged()
        .unwrap_or_else(|| Rc::new(wrap_dep_module(module)))
}

impl ImportResolver for DependencyImportResolver {
    fn resolve_import(
        &self,
        path: &str,
    ) -> Result<handl_language::Value, handl_language::LangError> {
        // The `Value` boundary re-encodes the shared tagged record; the
        // `{"__arrow__": …}` wire form decodes back losslessly on any
        // `TaggedValue::from` consumer (ADR-0155 §1 marshal story).
        self.resolve_import_tagged(path)
            .map(|rc| handl_value::tagged_to_value(&rc))
    }

    /// The callable record arrives pre-tagged (ADR-0155 Phase 1): an
    /// [`Rc`] of wrapper-arrow fields — no `serde_json` round-trip, and
    /// `module`'s bespoke [`Module::bindings_tagged`] record is shared
    /// rather than rebuilt.
    fn resolve_import_tagged(
        &self,
        path: &str,
    ) -> Result<Rc<handl_value::TaggedValue>, handl_language::LangError> {
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
        Ok(module_bindings_tagged(module.as_ref()))
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

    /// Forward the tagged fast path so a wrapped tagged-cache resolver
    /// (e.g. `ModuleLoader`) keeps its `Rc`-sharing behavior.
    fn resolve_import_tagged(
        &self,
        path: &str,
    ) -> Result<Rc<handl_value::TaggedValue>, handl_language::LangError> {
        self.0.resolve_import_tagged(path)
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
                // Strip `handle "op.name" with <impl>` declarations — the
                // contract-decl form (a string literal target), not the
                // dynamic-extent `handle <expr> with <handler>` statement
                // used inside arrows (which doesn't take a string literal).
                if let Some(rest) = trimmed.strip_prefix("handle") {
                    if rest.trim_start().starts_with('"') {
                        return false;
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

    /// Split a module source into its top-level statements, returning each
    /// statement's verbatim text (internal newlines and indentation
    /// preserved) in source order.
    ///
    /// Mirrors `handl_runtime::module_prep::top_level_segments`. A statement
    /// boundary is recognized purely by structure — never by a hardcoded
    /// line shape:
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
    pub(super) fn top_level_segments(src: &str) -> Vec<String> {
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
                    while j < bytes.len()
                        && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_')
                    {
                        j += 1;
                    }
                    let word = &src[i..j];
                    // A word after `.` is a field name (`char.from_chars`,
                    // `x.from`), not a statement keyword.
                    let dotted = i > 0 && bytes[i - 1] == b'.';
                    let kw = if !dotted && (word == "export" || word == "import" || word == "from")
                    {
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
    /// declaration — `None` when `export` does not sit at a word boundary
    /// at the start of the statement (`exporter := …` is a binding, not an
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

    /// Top-level import statements of `src`, in original order. `from "..."
    /// export x, y` re-export lines are rewritten to plain `import ... as`
    /// statements so the hoisted `do { ... }` block binds the local aliases
    /// the record literal references. Statement segmentation mirrors
    /// `handl_runtime::module_prep`: an import is recognized wherever a
    /// statement can begin, not only at column 0 of a line.
    fn collect_import_lines(src: &str) -> Vec<String> {
        top_level_segments(src)
            .iter()
            .map(|seg| seg.trim().to_string())
            .filter(|seg| is_import_line(seg))
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

    /// Local names bound by an import statement: `from "<spec>" import
    /// a, b as c` → `[a, c]`, `* as m` → `[m]`, `import <name> [from
    /// "<spec>"]` → `[name]`. Bare splices (`import "<spec>"`, `from
    /// "<spec>" import *`) bind names that are not statically enumerable
    /// — skipped. Mirrors `handl_runtime::module_prep::import_bound_locals`.
    fn import_bound_locals(line: &str) -> Vec<String> {
        let trimmed = line.trim_start();
        if trimmed.starts_with("from") {
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

    /// `src` with top-level import statements removed (all other
    /// statements kept verbatim). Statement segmentation mirrors
    /// `handl_runtime::module_prep::drop_all_import_lines`.
    fn drop_import_lines(src: &str) -> String {
        top_level_segments(src)
            .iter()
            .filter(|seg| !is_import_line(seg.trim()))
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Transform top-level `export name = expr` declarations into an object
    /// literal `{ name: expr, ... }` so the source evaluates to a record of
    /// exported functions.
    ///
    /// The source is consumed as top-level *statements*
    /// ([`top_level_segments`]) — not lines — so an `export` is recognized
    /// wherever a statement can begin: at column 0, indented, after `;`, or
    /// mid-line after another statement; and its RHS expression is captured
    /// whole regardless of how newlines fall inside `~>`-chained `do {…}` /
    /// `if`/`else` bodies. A declaration that cannot be transformed (an
    /// `export` with no `::`/`:=`/`:`/`=` separator, or an empty binding
    /// name) panics — a module prep must never silently drop an export
    /// into an empty record (mirrors `handl_runtime::module_prep`'s
    /// `LangError`).
    ///
    /// When the source contains top-level `import`/`from` statements they are
    /// hoisted -- in original order -- ahead of the record literal inside a
    /// `do { ... }` block, so the runtime can evaluate them (`prim_import`)
    /// and bind their names for the record. This is required because the
    /// `(source).func(args)` applied form needs a *single* expression; a bare
    /// import statement followed by a record is not one.
    ///
    /// If no `export` statements are found and there are no imports, the
    /// source is returned unchanged (the existing object-literal form is
    /// preserved).
    pub(super) fn transform_export_to_object(src: &str) -> String {
        // Collect (name, expr_lines, exported) triples in source order.
        // All bindings become `do`-block assignments (`name := expr;`)
        // emitted *before* the object so closures can reference siblings
        // — exported or not. The object literal projects the exported
        // names — mirrors `handl_runtime::module_prep`.
        let mut bindings: Vec<(String, Vec<String>, bool)> = Vec::new();
        let mut current: Option<(String, Vec<String>, bool)> = None;
        // Whether the module's last top-level statement is a binding.
        // When it is not (a trailing `do { … }` or record literal) the
        // body's own value is the module record and no name projection
        // is appended — mirrors `handl_runtime::module_prep`.
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
                    panic!(
                        "module prep: `export` declaration has no binding name: {trimmed}"
                    );
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
            // helper functions are preserved instead of dropped — mirrors
            // `handl_runtime::module_prep`.
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

        let has_exports = bindings.iter().any(|(_, _, exported)| *exported);

        let imports = collect_import_lines(src);

        if !has_exports {
            if imports.is_empty() && bindings.is_empty() {
                return src.to_string();
            }
            // No `export` declarations: the module record is *every*
            // top-level bound name — private bindings plus the locals
            // each hoisted import statement binds. The body is wrapped in
            // `do { … }` (imports first, verbatim statements next, the
            // record last) so the whole module evaluates as one
            // expression under the single-statement parser — mirrors
            // `loader.hnl` and `handl_runtime::module_prep`.
            let stripped = drop_import_lines(src);
            // The projection is appended only when the module's last
            // top-level statement is a binding; a trailing `do { … }`/
            // record literal is already the module's interface value.
            let record = if tail_is_binding {
                let mut seen = std::collections::HashSet::new();
                let mut fields: Vec<String> = Vec::new();
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
                    if is_import_line(trimmed) {
                        for local in import_bound_locals(trimmed) {
                            push_field(&local);
                        }
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
            out.push_str(&stripped);
            if !record.is_empty() {
                out.push_str("\n{ ");
                out.push_str(&record);
                out.push_str(" }");
            }
            out.push_str("\n}");
            return out;
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

    /// Whether `rhs` is a top-level arrow *signature* — contract data
    /// whose domain/codomain are *type* expressions that exec must not
    /// evaluate (Agda-style erasure; annotations are static metadata
    /// owned by the typecheck engine). Signature exports
    /// (`export Run := (Params) ~[1]~> Codomain`) mint an `Arrow`
    /// descriptor record — never a callable — and codomains like
    /// `Result<T, E>` are generic applications (`dimension` nodes) that
    /// crash eval, so the binding erases to `null` (the same value
    /// type-only `export Name: Type` gets). A depth-0 `~[` is only a
    /// signature when [`graded_arrow_is_signature`] holds — an n-cell
    /// *value* export like `export alpha := f ~[2]~> g` is kept.
    /// Ungraded `A ~> B` is untouched unless its params carry a bare
    /// list/record type (`closure` binds only `var`/`type-annot` params,
    /// so `(…, [T], …) ~> B` can only be a signature).
    /// Mirrors `handl_runtime::module_prep::is_signature_rhs`.
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
    /// *signature* `Type ~[1]~> Type` (erase) rather than a genuine
    /// n-cell *value* `f ~[N]~> g` (keep) — the
    /// `export alpha := f ~[2]~> g` footgun this guard used to erase to
    /// `null`. Corpus sweep (every `~[` across
    /// lib-*/kit-*/handl-packages, ~7,200 sites): **all** signatures
    /// grade `~[1]~>`, and every `~[2]~>`+/`~[N]~>` is a cell term (the
    /// `effects`/`optimize` law exports, `lib-type`'s dependent
    /// unit/counit). So:
    ///
    /// - any grade other than the literal `1` → a cell value, keep;
    /// - grade `1` → the *domain operand* decides: a signature's domain
    ///   is a type expression — a parenthesized params `(...)`, a
    ///   record `{...}`, a bare list type `[T]`, or a `Capitalized`
    ///   type head (`Int`, `Result<A, B>`, `T`). A
    ///   lowercase/literal/index-led LHS (`f`, `map inc`, `"a >>> b"`,
    ///   `xs[i]`) is a value expression → a cell, keep.
    ///
    /// Documented limits (no corpus instance today): a grade-1 cell over
    /// a parenthesized domain `(f >>> g) ~[1]~> h` still erases; a
    /// grade-1 signature over *lowercase* type vars `a ~[1]~> b` now
    /// evals (a loud unbound-name error) rather than binding `null`; a
    /// symbolic grade (`A ~[n]~> B` contract) is kept and evals — none
    /// exist outside comments.
    /// Mirrors `handl_runtime::module_prep::graded_arrow_is_signature`.
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
    /// generates constructor bindings:
    /// `name = _0 ~> hash_map.set({value: _0}, "__tag__", "name")`.
    /// (The `tag.tag` mint producer was deleted with the `__tag__` bridge
    /// — ADR-0152 §Migration; `hash_map.set` mints the same record shape.)
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
    ///   ok = _0 ~> hash_map.set({value: _0}, "__tag__", "ok")
    ///   err = _0 ~> hash_map.set({value: _0}, "__tag__", "err")
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
                "  {name} :: _0 ~> hash_map.set({{value: _0}}, \"__tag__\", \"{name}\")\n"
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

/// Expand `tag` declarations into `__tag__` constructor bindings wrapped in
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

// ---------------------------------------------------------------------------
// Per-thread runtime cache
// ---------------------------------------------------------------------------

/// Global counter for assigning unique module IDs.
///
/// Each [`HandlModule`] instance gets a unique ID at construction time. This
/// ID is used as a key in the per-thread runtime cache, so that each thread
/// can independently build and cache a `Runtime` for the module.
static NEXT_MODULE_ID: AtomicUsize = AtomicUsize::new(0);

/// Counter backing [`ModuleEnv::env_id`] (ADR-0156) — the identity sharing
/// layers above `HandlModule` (kit caches, stdlib `module_ref`, loaders) key
/// their env-aware entries by. Mirrors [`NEXT_MODULE_ID`].
static NEXT_ENV_ID: AtomicUsize = AtomicUsize::new(0);

/// A shared, thread-safe runtime builder closure.
///
/// The builder captures the module's dependency configuration (which is
/// `Send + Sync`) but produces a non-`Send` `Runtime`. The builder itself
/// is `Send + Sync`, so it can be shared across threads via [`Arc`]; each
/// thread calls it independently to construct its own per-thread `Runtime`.
type RuntimeBuilder = Arc<dyn Fn() -> Result<Runtime, handl_language::LangError> + Send + Sync>;

/// The process-wide default [`Language`](handl_language::Language) for
/// [`HandlModule`] bridge runtimes (ADR-0128 §"The 0.8 seam").
///
/// This crate is the generic `.hnl` module loader — machinery that stays in
/// platform — so it must not link `handl-kit-data-prelude`: the language
/// a `.hnl` module evaluates under is supplied by the host instead. Hosts
/// install it once at startup ([`install_default_language`], e.g. inside
/// `Kit::new`); a [`HandlModule`] may also carry an explicit per-module
/// language via [`HandlModule::with_language`]. This mirrors the
/// [`install_loader`] seam — process-wide, first-install-wins — for the
/// same reason: `.hnl` modules are ambient machinery whose builders run
/// lazily per thread.
static DEFAULT_LANGUAGE: std::sync::OnceLock<Arc<handl_language::Language>> =
    std::sync::OnceLock::new();

/// Install the process-wide default [`Language`](handl_language::Language)
/// used by lazily-built [`HandlModule`] runtimes.
///
/// The first installation wins; later calls return the rejected language.
/// Typically invoked by kit assembly (`kit-canonical`'s `Kit::new` and
/// friends) or by a test harness — e.g.
/// `install_default_language(language_from_yaml()?)`.
pub fn install_default_language(
    lang: handl_language::Language,
) -> Result<(), Arc<handl_language::Language>> {
    let r = DEFAULT_LANGUAGE.set(Arc::new(lang));
    // The canonical `desugar` rewrites are `.hnl`-authored
    // (`handl-packages/desugar/desugar.hnl`, ADR-0094): installing the
    // canonical language also installs the `handl:desugar.*` transform
    // overlay so `Runtime::parse`'s desugar pass resolves them. The Rust
    // `desugar.*` natives remain the bootstrap/fallback layer for hosts
    // that never call this (mirroring loader.hnl vs `bootstrap`).
    crate::transform::install_desugar_overlay();
    r
}

/// The installed default [`Language`](handl_language::Language), if any.
pub fn default_language() -> Option<Arc<handl_language::Language>> {
    DEFAULT_LANGUAGE.get().cloned()
}

/// Resolve the language a module's bridge runtime binds: the module's own
/// [`HandlModule::with_language`] override when set, otherwise the
/// process-wide [`default_language`].
fn module_language(
    override_lang: &Option<Arc<handl_language::Language>>,
) -> Result<handl_language::Language, handl_language::LangError> {
    match override_lang {
        Some(lang) => Ok(lang.as_ref().clone()),
        None => default_language()
            .map(|lang| lang.as_ref().clone())
            .ok_or_else(|| {
                handl_language::LangError::engine(
                    "handl module: no language available — install one with \
                     `handl_plugin_lib_handl::install_default_language` or \
                     `HandlModule::with_language`",
                )
            }),
    }
}

thread_local! {
    /// Per-thread cache of runtimes, keyed by module ID. Each thread
    /// independently builds and caches a `Runtime` for each [`HandlModule`]
    /// on first use. This is necessary because the tree-walk engine stores
    /// the active AST / `Language` in thread-locals, so `Runtime` is not
    /// `Send`.
    static RUNTIME_CACHE: RefCell<HashMap<usize, std::rc::Rc<Runtime>>> = RefCell::new(HashMap::new());

    /// Per-thread cache of evaluated module records (one per module) — the
    /// `__mod` the applied form projects its function from.
    static MODULE_VALUE_CACHE: RefCell<HashMap<usize, serde_json::Value>> =
        RefCell::new(HashMap::new());

    /// Per-thread cache of parsed `(module source)` ASTs, evaluated once
    /// into `MODULE_VALUE_CACHE`.
    static MODULE_SRC_AST_CACHE: RefCell<HashMap<usize, Rc<handl_language::Ast>>> =
        RefCell::new(HashMap::new());

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

/// Allocate a unique env ID for a [`ModuleEnv`] (ADR-0156) — mirrors
/// [`allocate_module_id`].
fn allocate_env_id() -> usize {
    NEXT_ENV_ID.fetch_add(1, Ordering::Relaxed)
}

/// Build a minimal runtime for evaluating `.hnl` sources, optionally with
/// dependency modules registered in the module registry.
///
/// Binds the injected `lang` ([`default_language`] or the module's
/// [`HandlModule::with_language`] override) to the data lexer, Pratt parser, and
/// tree-walk evaluator. If `dependencies` is non-empty, a
/// [`ModuleRegistry`](handl_plugin_module::ModuleRegistry) is wired with the
/// given modules and layered under the canonical `FpHostFunctions` (for
/// field-access support). If no dependencies are needed, no module registry
/// is wired (matching the `hello` bridge pattern).
fn build_runtime(
    lang: handl_language::Language,
    dependencies: Vec<Arc<dyn Module + Send + Sync>>,
    eval_name: &str,
    import_resolver: Option<Arc<dyn ImportResolver + Send + Sync>>,
    global_lookup: Option<Arc<dyn ModuleLookup + Send + Sync>>,
) -> Result<Runtime, handl_language::LangError> {
    let config = RuntimeConfig::new()
        .with_lex("data")
        .with_parse("pratt")
        .with_eval(eval_name);
    let mut registry = EngineRegistry::new();
    registry.register_lex(
        "data",
        Box::new(hnl_engine_lex_pattern::DataLexerEngine::new()),
    );
    registry.register_parse("pratt", hnl_engine_parse_pratt::PrattEngine::new());
    registry.register_eval(
        "tree-walk",
        Box::new(hnl_engine_exec_tree_walk::TreeWalkEngine::new()),
    );
    registry.register_eval(
        "bytecode",
        Box::new(hnl_engine_exec_bytecode::BytecodeEngine::new()),
    );
    registry.register_compile(
        "bytecode",
        Box::new(hnl_engine_exec_bytecode::BytecodeCompileEngine::new()),
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
        // express arrows (`__arrow__` — the object constructor rejects the
        // reserved key) or other reserved-key values (`__tag__`, `__pair__`,
        // `__either__`, `__bytes__`), so values cross the boundary as
        // *values*.
        //
        // ADR-0106: arrows are self-contained (`body_def` + `binds` +
        // `handlers`) — an arrow argument needs no provenance stamping and
        // an arrow result applies in the caller's context directly.
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
            call_args.push((name.clone(), arg.clone()));
            arg_names.push(name);
        }
        let module_id = self.module_id;
        let builder = Arc::clone(&self.builder);
        let module_name = &self.module_name;
        let func_name = &self.func_name;
        // Clone the `Rc<Runtime>` out of the cache and release the
        // `RefCell` borrow *before* `run`: a `.hnl` function may itself call
        // another `HandlFunction` (e.g. `str-handl` → `char.*`), which
        // re-enters `call` and would otherwise hit
        // `RefCell already borrowed`.
        let runtime = RUNTIME_CACHE.with(|cache| {
            std::rc::Rc::clone(cache.borrow_mut().entry(module_id).or_insert_with(|| {
                std::rc::Rc::new(builder().expect("handl module runtime build failed"))
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
        // The module record is arg-independent, so it is evaluated once per
        // (thread, module) and the applied form projects from it —
        // `__mod.func(__arg_0, …)` instead of `(module source).func(…)`,
        // which re-evaluated the whole module on every call. That cost was
        // ~0.33ms of the ~1.15ms fixed per-call overhead (measured with
        // `examples/loader_bench.rs`).
        let mod_val = MODULE_VALUE_CACHE.with(|c| c.borrow().get(&module_id).cloned());
        let mod_val = match mod_val {
            Some(v) => v,
            None => {
                let src_ast =
                    MODULE_SRC_AST_CACHE.with(|cache| cache.borrow().get(&module_id).cloned());
                let src_ast = match src_ast {
                    Some(a) => a,
                    None => {
                        let source = format!("({})", self.source);
                        let toks = runtime.lex(&Source::new(&source)).map_err(errf)?;
                        let parsed = Rc::new(runtime.parse(&toks).map_err(errf)?);
                        MODULE_SRC_AST_CACHE.with(|cache| {
                            cache.borrow_mut().insert(module_id, Rc::clone(&parsed));
                        });
                        parsed
                    }
                };
                let v = runtime
                    .eval_shared_with_call_args(&src_ast, &[])
                    .map_err(errf)?;
                MODULE_VALUE_CACHE.with(|c| c.borrow_mut().insert(module_id, v.clone()));
                v
            }
        };
        let ast_key = (module_id, func_name.clone(), args.len());
        let ast = AST_CACHE.with(|cache| cache.borrow().get(&ast_key).cloned());
        let ast = match ast {
            Some(ast) => ast,
            None => {
                let source = format!("__mod.{}({})", self.func_name, arg_names.join(", "));
                let toks = runtime.lex(&Source::new(&source)).map_err(errf)?;
                let parsed = Rc::new(runtime.parse(&toks).map_err(errf)?);
                AST_CACHE.with(|cache| cache.borrow_mut().insert(ast_key, Rc::clone(&parsed)));
                parsed
            }
        };
        let mut call_args = vec![("__mod".to_string(), mod_val)];
        for (i, arg) in args.iter().enumerate() {
            call_args.push((format!("__arg_{i}"), arg.clone()));
        }
        // Arrows in the result are self-contained (`body_def` + `binds`,
        // ADR-0106) — no home-frame registration needed.
        runtime
            .eval_shared_with_call_args(&ast, &call_args)
            .map_err(errf)
    }
}

// ---------------------------------------------------------------------------
// ModuleEnv (ADR-0156)
// ---------------------------------------------------------------------------

/// A [`HandlModule`]'s dependency environment as a first-class value
/// (ADR-0156).
///
/// The env is the tuple [`build_runtime`] threads through: the dependency
/// modules registered in the bridge runtime, an optional global
/// [`ModuleLookup`] layered under the dependency registry, and an optional
/// [`ImportResolver`] for `import`/`from` statements. Lifting it into a
/// value lets one prepared source run under several dep bindings — see
/// [`HandlModule::instantiate`].
///
/// # Identity (ADR-0156 follow-up)
///
/// An env carries an [`env_id`](ModuleEnv::env_id) allocated from the same
/// kind of global counter as [`HandlModule::module_id`]. The name-keyed
/// singleton caches above `HandlModule` (kit caches, stdlib `module_ref`,
/// loaders) historically collapsed one name to one env; env-aware variants
/// key by `(name, env_id)`, and the id is what makes that keying sound:
///
/// - `new`/`default` mint a fresh id — two independently created envs are
///   distinct even with identical contents;
/// - [`clone`](Clone) *preserves* the id — clones are the same env, so
///   env-keyed caches share one entry across them;
/// - [`fork`](Self::fork) keeps the contents but mints a fresh id — the
///   "same deps, deliberately separate cache" case;
/// - the `with_*` builders re-mint, mirroring how
///   [`HandlModule::with_dependency`] re-allocates its `module_id` so
///   stale cache entries are abandoned when the dependency set changes.
#[derive(Clone)]
pub struct ModuleEnv {
    /// Dependency modules registered in the bridge runtime.
    dependencies: Vec<Arc<dyn Module + Send + Sync>>,
    /// Optional global [`ModuleLookup`] layered under the dependency
    /// registry.
    global_lookup: Option<Arc<dyn ModuleLookup + Send + Sync>>,
    /// Optional explicit [`ImportResolver`]; `None` lets `build_runtime`
    /// derive a [`DependencyImportResolver`] from `dependencies`.
    import_resolver: Option<Arc<dyn ImportResolver + Send + Sync>>,
    /// Identity of this env value (see the section docs): allocated once
    /// per env lineage, preserved by `clone`, re-minted by `fork` and by
    /// the `with_*` builders. Env-aware caches key on it.
    env_id: usize,
}

impl Default for ModuleEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleEnv {
    /// Create an empty environment — no dependencies, no global lookup,
    /// no explicit import resolver — with a fresh env id.
    ///
    /// Equivalent to [`ModuleEnv::default`]; `instantiate`d under an empty
    /// env a module still evaluates, but `mod.fn` calls and `import`s fail
    /// to resolve unless the source's names are reachable some other way.
    pub fn new() -> Self {
        Self {
            dependencies: Vec::new(),
            global_lookup: None,
            import_resolver: None,
            env_id: allocate_env_id(),
        }
    }

    /// Build an env from a dependency manifest (ADR-0156): each
    /// `(name, spec)` pair resolves through `resolver`, and every
    /// declared name must resolve.
    ///
    /// This is the declarative form of
    /// `ModuleEnv::new().with_dependency_arc(a).with_dependency_arc(b)`:
    /// the pairs are a `pkg.hnl` `deps` record (e.g.
    /// `{ deps: { math: "branch:main" } }`), as parsed by
    /// [`handl_platform::pkg_dep_specs`] — see [`from_pkg_hnl`](Self::from_pkg_hnl)
    /// for the one-call form. The spec half (`"branch:main"`) is
    /// declarative data: fetching over git is the package manager's job
    /// (ADR-0010), so the resolver answers by name alone today.
    ///
    /// A name the resolver does not know is an error listing *every*
    /// missing name — nothing is skipped silently. The built env gets
    /// one fresh [`env_id`](Self::env_id) (it is a new env), and the
    /// `with_*` builders still apply for overrides on top.
    pub fn from_manifest(
        deps: &[(String, String)],
        resolver: &dyn ModuleResolver,
    ) -> Result<Self, handl_language::LangError> {
        let mut dependencies = Vec::with_capacity(deps.len());
        let mut missing: Vec<String> = Vec::new();
        for (name, spec) in deps {
            match resolver.resolve(name) {
                Some(module) => dependencies.push(module),
                None => missing.push(format!("`{name}` (spec \"{spec}\")")),
            }
        }
        if !missing.is_empty() {
            return Err(handl_language::LangError::engine(format!(
                "from_manifest: unresolved deps: {}",
                missing.join(", ")
            )));
        }
        Ok(Self {
            dependencies,
            global_lookup: None,
            import_resolver: None,
            env_id: allocate_env_id(),
        })
    }

    /// [`from_manifest`](Self::from_manifest) over a `pkg.hnl` source:
    /// the `deps` table is parsed by [`handl_platform::pkg_dep_specs`] —
    /// the same lenient scan [`resolve_pkg_dependencies`] uses, so no
    /// parsing is duplicated here — and each declared name then resolves
    /// through `resolver`.
    ///
    /// A source without a `deps` table — or with a malformed one —
    /// yields an empty env (not an error): `pkg.hnl` files are authored
    /// by package repos this workspace may not own, and a bad manifest
    /// must not break an unrelated build, the leniency
    /// [`resolve_pkg_dependencies`] already applies.
    ///
    /// [`resolve_pkg_dependencies`]: handl_platform::resolve_pkg_dependencies
    pub fn from_pkg_hnl(
        src: &str,
        resolver: &dyn ModuleResolver,
    ) -> Result<Self, handl_language::LangError> {
        Self::from_manifest(&handl_platform::pkg_dep_specs(src), resolver)
    }

    /// This env's identity (ADR-0156): stable across `clone`, fresh for
    /// `new`/`default`/`fork` and for every `with_*` reconfiguration.
    ///
    /// Sharing layers above `HandlModule` use it to key env-aware cache
    /// entries: `(module name, env_id)` names the module built under this
    /// env, so one name can mean several envs.
    pub fn env_id(&self) -> usize {
        self.env_id
    }

    /// Same contents, fresh identity: the "same deps, deliberately separate
    /// cache" case (ADR-0156).
    ///
    /// `fork` resolves exactly like `self`, but env-aware caches treat the
    /// fork as a distinct env — unlike [`clone`](Clone), which shares the
    /// id and therefore the env-keyed entries.
    pub fn fork(&self) -> Self {
        Self {
            env_id: allocate_env_id(),
            ..self.clone()
        }
    }

    /// Add a dependency module to the environment — the env-side
    /// equivalent of [`HandlModule::with_dependency`].
    ///
    /// Re-mints the env id: the contents changed, so env-keyed caches must
    /// not keep serving entries built for the previous configuration.
    pub fn with_dependency(mut self, module: impl Module + Send + Sync + 'static) -> Self {
        self.dependencies.push(Arc::new(module));
        self.env_id = allocate_env_id();
        self
    }

    /// Add a dependency module (already `Arc`ed) to the environment.
    ///
    /// Re-mints the env id (see [`with_dependency`](Self::with_dependency)).
    pub fn with_dependency_arc(mut self, module: Arc<dyn Module + Send + Sync>) -> Self {
        self.dependencies.push(module);
        self.env_id = allocate_env_id();
        self
    }

    /// Attach a global [`ModuleLookup`] layered under the dependency
    /// registry — see [`HandlModule::with_global_lookup`].
    ///
    /// Re-mints the env id (see [`with_dependency`](Self::with_dependency)).
    pub fn with_global_lookup(mut self, lookup: Arc<dyn ModuleLookup + Send + Sync>) -> Self {
        self.global_lookup = Some(lookup);
        self.env_id = allocate_env_id();
        self
    }

    /// Attach the [`ImportResolver`] for `import`/`from` statements —
    /// see [`HandlModule::with_import_resolver`].
    ///
    /// Re-mints the env id (see [`with_dependency`](Self::with_dependency)).
    pub fn with_import_resolver(mut self, resolver: Arc<dyn ImportResolver + Send + Sync>) -> Self {
        self.import_resolver = Some(resolver);
        self.env_id = allocate_env_id();
        self
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
    /// Optional per-module language override (ADR-0128 §"The 0.8 seam").
    /// `None` resolves the process-wide [`default_language`] lazily at
    /// first use.
    language: Option<Arc<handl_language::Language>>,
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
        let builder: RuntimeBuilder = Arc::new(|| {
            module_language(&None)
                .and_then(|lang| build_runtime(lang, Vec::new(), eval_name, None, None))
        });

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
            language: None,
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
        let builder: RuntimeBuilder = Arc::new(|| {
            module_language(&None)
                .and_then(|lang| build_runtime(lang, Vec::new(), eval_name, None, None))
        });

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
            language: None,
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

    /// Bind this module's bridge runtime to `lang` instead of the
    /// process-wide [`default_language`] (ADR-0128 §"The 0.8 seam").
    ///
    /// Use this for modules authored in a non-canonical language: the
    /// default covers the common case where the host installed the
    /// canonical language once at startup.
    pub fn with_language(mut self, lang: Arc<handl_language::Language>) -> Self {
        self.language = Some(lang);
        self.rebuild_functions();
        self
    }

    /// Select the eval engine this module's bridge runtime binds
    /// (`"tree-walk"`, `"bytecode"`).
    ///
    /// `"bytecode"` runs module bodies on the bytecode VM
    /// (`hnl_engine_exec_bytecode::BytecodeEngine`): the
    /// per-call record construction and function bodies compile to
    /// instructions once per eval instead of paying tree-walk scheme
    /// dispatch per node — measured ~100-400× faster on eval-heavy
    /// transforms (`loader.hnl` `strip_decls` on a 22k-char source:
    /// ~59s tree-walk → ~0.16s bytecode).
    ///
    /// Note: arrows are self-contained data (`body_def` + `binds`,
    /// ADR-0106), so closures crossing the `Function::call` boundary work
    /// under both engines. Modules whose functions take or return closures
    /// can opt into `"bytecode"` as well.
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
        let language = self.language.clone();
        let builder: RuntimeBuilder = Arc::new(move || {
            module_language(&language).and_then(|lang| {
                build_runtime(
                    lang,
                    deps.clone(),
                    eval_name,
                    import_resolver.clone(),
                    global_lookup.clone(),
                )
            })
        });
        let module_id = allocate_module_id();
        for func in self.functions.values_mut() {
            func.builder = Arc::clone(&builder);
            func.module_id = module_id;
        }
        self.builder = builder;
        self.module_id = module_id;
    }

    /// Clone this module's dependency environment out as a [`ModuleEnv`]
    /// (ADR-0156).
    ///
    /// Useful for clone-instantiate patterns: `m.instantiate(m.env())`
    /// produces a sibling identical to `self` except for a fresh
    /// `module_id`, and `m.instantiate(m.env().with_dependency(extra))`
    /// layers a dep onto the existing binding.
    ///
    /// Each call mints a fresh [`ModuleEnv::env_id`]: the snapshot is a new
    /// env value (the module does not hold a `ModuleEnv` itself), so
    /// env-keyed caches treat snapshots as separate envs — [`fork`] them
    /// or share via `clone` if a stable identity is wanted.
    ///
    /// [`fork`]: ModuleEnv::fork
    pub fn env(&self) -> ModuleEnv {
        ModuleEnv {
            dependencies: self.dependencies.clone(),
            global_lookup: self.global_lookup.clone(),
            import_resolver: self.import_resolver.clone(),
            env_id: allocate_env_id(),
        }
    }

    /// The current module ID — the key into the per-thread
    /// `RUNTIME_CACHE`/`AST_CACHE`/`MODULE_VALUE_CACHE` tables.
    ///
    /// Exposed so hosts (and tests) can observe that
    /// [`instantiate`][Self::instantiate] / `with_dependency*` re-keyed
    /// the caches: two [`HandlModule`]s with different module IDs evaluate
    /// against disjoint per-thread cache entries even on the same thread.
    pub fn module_id(&self) -> usize {
        self.module_id
    }

    /// Instantiate a sibling module under a different dependency
    /// environment (ADR-0156).
    ///
    /// The returned [`HandlModule`] shares this module's prepared source,
    /// language, eval engine, op functions and value names, but its env is
    /// `env` — this module's own `dependencies`/`global_lookup`/
    /// `import_resolver` are *not* carried over. A fresh `module_id` is
    /// allocated through the same `rebuild_functions` path, so the
    /// per-thread runtime/AST caches key the two instances apart: each
    /// evaluates the same source against its own dep bindings.
    ///
    /// `self` is unchanged; each instance owns its own [`HandlFunction`]s
    /// bound to its own `module_id`.
    pub fn instantiate(&self, env: ModuleEnv) -> Self {
        let mut module = Self {
            name: self.name.clone(),
            functions: self
                .functions
                .keys()
                .map(|fname| {
                    let func = HandlFunction::new(
                        self.name.clone(),
                        fname.clone(),
                        self.source.clone(),
                        self.module_id,
                        Arc::clone(&self.builder),
                    );
                    (fname.clone(), func)
                })
                .collect(),
            op_functions: self.op_functions.clone(),
            dependencies: env.dependencies,
            global_lookup: env.global_lookup,
            eval_name: self.eval_name,
            import_resolver: env.import_resolver,
            language: self.language.clone(),
            source: self.source.clone(),
            builder: Arc::clone(&self.builder),
            module_id: self.module_id,
            value_names: self.value_names.clone(),
        };
        module.rebuild_functions();
        module
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
                std::rc::Rc::new(builder().expect("handl module runtime build failed"))
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
///
/// Owned by this crate (ADR-0128 §".hnl ownership": one `include_str!` per
/// package; other consumers — e.g. `examples/loader_bench.rs` — take this
/// `pub const`).
pub use handl_lib_str_handl::STR_HANDL_HNL;

/// The canonical text-transform loader source (ADR-0094).
///
/// Loaded raw via [`HandlModule::new_raw`] by [`HandlLoader::new`]; once
/// installed through [`install_loader`], every `.hnl` text transform
/// (declaration/comment stripping, export transformation, import/
/// capability/tag parsing, tag expansion) is evaluated in handl.
const LOADER_HNL: &str = handl_lib_loader::LOADER_HNL;

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

pub use handl_lib_str_handl::STR_HANDL_FUNCS;
pub use handl_lib_str_handl::STR_HANDL_SOURCES;
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
/// "..."` statement in `src`, in order. Used to wire [`HandlModule`] deps so
/// the Rust table above can never drift from the `.hnl` imports. Statement
/// segmentation mirrors the export transform, so an import sharing a line
/// with a sibling statement is still wired.
fn str_handl_import_specs(src: &str) -> Vec<String> {
    let mut specs = Vec::new();
    for seg in bootstrap::top_level_segments(src) {
        let t = seg.trim();
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
) -> (
    HashMap<String, Arc<dyn handl_platform::Module + Send + Sync>>,
    HandlModule,
) {
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
                .unwrap_or_else(|| panic!("str-handl: unresolved import '{spec}' in {name}.hnl"));
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

/// Build a [`HandlModule`] for a multi-file `.hnl` package — the generic
/// form of [`str_handl_modules`] without its callback-invoker special
/// cases. `sources` rows are `(name, src, funcs)` and must be ordered so
/// that every file's `from "./x.hnl"` dep appears earlier in the table;
/// `from "<mod>"` deps resolve through `resolver`.
pub fn handl_module_from_sources(
    mod_name: &'static str,
    index_src: &'static str,
    index_funcs: &[&'static str],
    sources: &[(&'static str, &'static str, &[&'static str])],
    resolver: &dyn ModuleResolver,
    eval_name: &'static str,
) -> HandlModule {
    let mut siblings: HashMap<String, Arc<dyn handl_platform::Module + Send + Sync>> =
        HashMap::new();
    for &(name, src, funcs) in sources {
        let mut module = HandlModule::new(name, src, funcs).with_eval_engine(eval_name);
        for spec in str_handl_import_specs(src) {
            let dep_name = normalize_import_name(&spec);
            let dep = siblings
                .get(dep_name)
                .cloned()
                .or_else(|| resolver.resolve(dep_name))
                .unwrap_or_else(|| panic!("{mod_name}: unresolved import '{spec}' in {name}.hnl"));
            module = module.with_dependency_arc(dep);
        }
        siblings.insert(name.to_string(), Arc::new(module));
    }
    let mut index =
        HandlModule::new(mod_name, index_src, index_funcs).with_eval_engine(eval_name);
    for spec in str_handl_import_specs(index_src) {
        let dep_name = normalize_import_name(&spec);
        let dep = siblings
            .get(dep_name)
            .cloned()
            .or_else(|| resolver.resolve(dep_name))
            .unwrap_or_else(|| panic!("{mod_name}: index imports '{spec}' but no module exists"));
        index = index.with_dependency_arc(dep);
    }
    index
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
