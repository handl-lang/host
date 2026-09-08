//! Native lint rules — the disciplined escape hatch (ADR-0097 §3).
//!
//! A [`LintRuleDef`](handl_language::LintRuleDef) whose `check` is
//! `!Native "name"` defers detection to a registered function. Natives see
//! the whole module — all parsed statements plus the source text — so they
//! can express scope analysis (`scope.*`), convention checks that need
//! renaming (`naming-convention`), and whole-source checks (`source.*`) that
//! the node-local [`LintCheck::Match`](handl_language::LintCheck::Match)
//! predicate vocabulary cannot. This mirrors `TransformRule::Native` /
//! `with_desugar_natives` (ADR-0094): dispatch stays in data, only the
//! implementation is Rust.
//!
//! [`canonical_lint_natives`] registers the canonical surface's semantic
//! residue: `scope.unbound-wire`, `scope.shadowed-binding`,
//! `scope.unused-import`, `naming-convention`, and
//! `source.missing-module-doc`. Hosts register their own natives on top
//! (e.g. `plugin-ext-lint` registers `source.fmt`, which needs the
//! formatter).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use handl_language::{LangError, Language, Span};

use crate::LintFixOut;
use crate::module::{LintStatement, ModuleAnalysis};

/// The context passed to a native lint rule.
pub struct LintCtx<'a> {
    /// The language being linted.
    pub lang: &'a Language,
    /// The module's source text.
    pub src: &'a str,
    /// The parsed statements.
    pub module: &'a [LintStatement],
    /// The shared per-statement analysis (bindings / references / params /
    /// imports), aligned with `module`.
    pub analysis: &'a ModuleAnalysis,
}

/// One violation a native reports: template arguments for the rule's
/// `message`, an absolute byte span, and an optional fix.
#[derive(Debug, Clone, Default)]
pub struct NativeHit {
    /// `{key}` placeholders for the rule's message template (e.g. `name`).
    pub args: BTreeMap<String, String>,
    /// Absolute byte range the diagnostic points at.
    pub span: Option<Span>,
    /// Optional fix (absolute byte range + replacement text).
    pub fix: Option<LintFixOut>,
}

impl NativeHit {
    /// A hit with a `name` argument and a span.
    pub fn named(name: impl Into<String>, span: Span) -> Self {
        let mut args = BTreeMap::new();
        args.insert("name".to_string(), name.into());
        Self {
            args,
            span: Some(span),
            fix: None,
        }
    }

    /// Attach a fix.
    pub fn with_fix(mut self, fix: LintFixOut) -> Self {
        self.fix = Some(fix);
        self
    }
}

/// The shared function signature of a native lint rule.
pub type NativeLintFn =
    Arc<dyn Fn(&LintCtx<'_>) -> Result<Vec<NativeHit>, LangError> + Send + Sync>;

/// A named registry of native lint rules.
///
/// Registered functions receive a [`LintCtx`] and return the violations they
/// find. An unregistered `!Native` name produces [`LangError`] at lint
/// time — a rule whose native no host wired is a configuration bug, not a
/// pass.
#[derive(Default, Clone)]
pub struct NativeLintRegistry {
    map: HashMap<String, NativeLintFn>,
}

impl std::fmt::Debug for NativeLintRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut names: Vec<_> = self.map.keys().collect();
        names.sort();
        f.debug_struct("NativeLintRegistry")
            .field("names", &names)
            .finish()
    }
}

impl NativeLintRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `f` under `name`, replacing any existing entry.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        f: impl Fn(&LintCtx<'_>) -> Result<Vec<NativeHit>, LangError> + Send + Sync + 'static,
    ) {
        self.map.insert(name.into(), Arc::new(f));
    }

    /// Merge another registry's entries into this one (theirs wins).
    pub fn extend(&mut self, other: &NativeLintRegistry) {
        self.map
            .extend(other.map.iter().map(|(k, v)| (k.clone(), v.clone())));
    }

    /// Look up a native by name.
    pub fn get(&self, name: &str) -> Option<&NativeLintFn> {
        self.map.get(name)
    }

    /// Whether `name` is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }
}

/// The canonical lint-native names [`canonical_lint_natives`] registers.
pub const CANONICAL_LINT_NATIVES: &[&str] = &[
    "scope.unbound-wire",
    "scope.shadowed-binding",
    "scope.unused-import",
    "naming-convention",
    "source.missing-module-doc",
];

/// Build a registry holding the canonical surface's native lint rules
/// (ADR-0097 §5).
///
/// These are the rules that need cross-statement scope analysis or raw
/// source access — the semantic residue that stays Rust, mirroring the
/// `desugar.*` natives in `plugin-engine-transform-rewrite`. `source.fmt`
/// is deliberately absent: it needs the formatter, so the host that owns
/// that dependency (e.g. `plugin-ext-lint`) registers it.
pub fn canonical_lint_natives() -> NativeLintRegistry {
    let mut reg = NativeLintRegistry::new();
    reg.register("scope.unbound-wire", |ctx| Ok(unbound_wire(ctx)));
    reg.register("scope.shadowed-binding", |ctx| Ok(shadowed_binding(ctx)));
    reg.register("scope.unused-import", |ctx| Ok(unused_import(ctx)));
    reg.register("naming-convention", |ctx| Ok(naming_convention(ctx)));
    reg.register("source.missing-module-doc", |ctx| {
        Ok(missing_module_doc(ctx))
    });
    reg
}

// ---------------------------------------------------------------------------
// `scope.*` — cross-statement scope analysis
// ---------------------------------------------------------------------------

/// `scope.unbound-wire` — a reference to a name not bound in scope.
///
/// The global scope is `_` (wildcard) plus every import name and top-level
/// binding in the module; each statement additionally scopes its own arrow
/// parameters.
fn unbound_wire(ctx: &LintCtx<'_>) -> Vec<NativeHit> {
    let mut global: HashSet<&str> = HashSet::new();
    global.insert("_");
    for s in &ctx.analysis.statements {
        if let Some(name) = &s.import_name {
            global.insert(name.as_str());
        }
        if let Some((name, _)) = &s.binding {
            global.insert(name.as_str());
        }
    }

    let mut hits = Vec::new();
    for s in &ctx.analysis.statements {
        let mut scope = global.clone();
        for param in &s.params {
            scope.insert(param.as_str());
        }
        for (name, span) in &s.references {
            if !scope.contains(name.as_str()) {
                hits.push(NativeHit::named(name.clone(), *span));
            }
        }
    }
    hits
}

/// `scope.shadowed-binding` — a top-level binding re-binding a name already
/// in scope (imports and preceding bindings).
fn shadowed_binding(ctx: &LintCtx<'_>) -> Vec<NativeHit> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut hits = Vec::new();
    for s in &ctx.analysis.statements {
        if s.is_import {
            if let Some(name) = &s.import_name {
                seen.insert(name.as_str());
            }
            continue;
        }
        let Some((name, span)) = &s.binding else {
            continue;
        };
        if seen.contains(name.as_str()) {
            hits.push(NativeHit::named(name.clone(), *span));
        }
        seen.insert(name.as_str());
    }
    hits
}

/// `scope.unused-import` — an import projection never referenced. Fixable:
/// the fix removes the import line including its trailing newline.
fn unused_import(ctx: &LintCtx<'_>) -> Vec<NativeHit> {
    let mut referenced: HashSet<&str> = HashSet::new();
    for s in &ctx.analysis.statements {
        if s.is_import {
            continue;
        }
        for (name, _) in &s.references {
            referenced.insert(name.as_str());
        }
    }

    let mut hits = Vec::new();
    for (stmt, s) in ctx.module.iter().zip(ctx.analysis.statements.iter()) {
        if !s.is_import {
            continue;
        }
        let Some(name) = &s.import_name else {
            continue;
        };
        if referenced.contains(name.as_str()) {
            continue;
        }
        let stmt_end = stmt.offset + stmt.text.len();
        let line_end = consume_line_end(ctx.src, stmt_end);
        let span = Span {
            start: stmt.offset,
            end: line_end,
        };
        hits.push(NativeHit::named(name.clone(), span).with_fix(LintFixOut {
            start: span.start,
            end: span.end,
            replacement: String::new(),
        }));
    }
    hits
}

// ---------------------------------------------------------------------------
// `naming-convention` — binding names are `snake_case`
// ---------------------------------------------------------------------------

/// `naming-convention` — top-level binding names must be `snake_case`.
/// Fixable: the fix rewrites the binding name to snake_case.
fn naming_convention(ctx: &LintCtx<'_>) -> Vec<NativeHit> {
    let mut hits = Vec::new();
    for s in &ctx.analysis.statements {
        let Some((name, span)) = &s.binding else {
            continue;
        };
        if is_snake_case(name) {
            continue;
        }
        hits.push(NativeHit::named(name.clone(), *span).with_fix(LintFixOut {
            start: span.start,
            end: span.end,
            replacement: to_snake_case(name),
        }));
    }
    hits
}

/// Whether `name` is `snake_case` (lowercase letters, digits, underscores;
/// not starting with a digit).
fn is_snake_case(name: &str) -> bool {
    if name.is_empty() || name == "_" {
        return true;
    }
    let mut has_alpha = false;
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_digit() {
            if i == 0 {
                return false;
            }
            continue;
        }
        if c == '_' {
            continue;
        }
        if c.is_ascii_lowercase() {
            has_alpha = true;
            continue;
        }
        return false;
    }
    has_alpha || name.chars().all(|c| c == '_' || c.is_ascii_digit())
}

/// Convert a name to `snake_case` (insert `_` before uppercase letters, then
/// lowercase; collapse non-alphanumeric runs to `_`).
fn to_snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_alphanumeric() {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out
}

// ---------------------------------------------------------------------------
// `source.*` — whole-source checks
// ---------------------------------------------------------------------------

/// `source.missing-module-doc` — a module lacks a leading doc comment.
fn missing_module_doc(ctx: &LintCtx<'_>) -> Vec<NativeHit> {
    let trimmed = ctx.src.trim_start();
    if trimmed.is_empty() || trimmed.starts_with("--") {
        return Vec::new();
    }
    vec![NativeHit {
        args: BTreeMap::new(),
        span: Some(Span { start: 0, end: 0 }),
        fix: None,
    }]
}

// ---------------------------------------------------------------------------
// Source helpers
// ---------------------------------------------------------------------------

/// From `from` (just past the last non-whitespace char of a statement), skip
/// trailing whitespace and consume one trailing newline, returning the byte
/// offset just past the newline (or the source end). Lets a fix remove a
/// whole line without leaving a blank line behind.
pub fn consume_line_end(src: &str, from: usize) -> usize {
    let bytes = src.as_bytes();
    let mut i = from.min(bytes.len());
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'\n' {
        i += 1;
    }
    i
}
