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
//! `scope.unused-import`, `naming-convention`, `source.missing-module-doc`,
//! `source.fmt` (a no-op overridden by hosts with a formatter), and the
//! `builtin.*` per-statement AST checks (`empty-block`, `unreachable-code`,
//! `shadowed-binding`, `unused-binding`). Hosts register their own natives on
//! top (e.g. `plugin-ext-lint` overrides `source.fmt` with the real
//! formatter-backed implementation).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use handl_language::{Ast, AstRef, LangError, Language, Span};

use crate::LintFixOut;
use crate::NodeClassifier;
use crate::ScopeStack;
use crate::module::{LintStatement, ModuleAnalysis, absolute, child_indices};

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

/// A name resolver consulted when `name` has no [`NativeLintRegistry`]
/// entry.
///
/// Resolvers are the generic seam a host uses to dispatch a whole name
/// *prefix* without enumerating names in the map — e.g. an adapter that
/// routes `handl:<module>.<fn>` rules to `.hnl`-authored lint functions
/// (the lint analogue of `HandlNativeRewrite`, ADR-0094). Returning
/// `None` declines the name; returning a [`NativeLintFn`] that errors
/// lets the resolver report name-resolution failures (unknown module,
/// missing function) at lint time.
pub type NativeLintResolver = Arc<dyn Fn(&str) -> Option<NativeLintFn> + Send + Sync>;

/// A named registry of native lint rules.
///
/// Registered functions receive a [`LintCtx`] and return the violations they
/// find. An unregistered `!Native` name produces [`LangError`] at lint
/// time — a rule whose native no host wired is a configuration bug, not a
/// pass. Names not in the map are offered to the registered
/// [resolvers](NativeLintResolver) in order before giving up.
#[derive(Default, Clone)]
pub struct NativeLintRegistry {
    map: HashMap<String, NativeLintFn>,
    resolvers: Vec<NativeLintResolver>,
}

impl std::fmt::Debug for NativeLintRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut names: Vec<_> = self.map.keys().collect();
        names.sort();
        f.debug_struct("NativeLintRegistry")
            .field("names", &names)
            .field("resolvers", &self.resolvers.len())
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

    /// Merge another registry's entries and resolvers into this one
    /// (their map entries win; their resolvers are appended).
    pub fn extend(&mut self, other: &NativeLintRegistry) {
        self.map
            .extend(other.map.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.resolvers.extend(other.resolvers.iter().cloned());
    }

    /// Add a [`NativeLintResolver`] consulted (in registration order) for
    /// `!Native` names not in the map. This is how a host dispatches
    /// dynamic prefixes — e.g. `handl:<mod>.<fn>` rules — while keeping
    /// the engine ignorant of the prefix scheme.
    pub fn add_resolver(&mut self, resolver: NativeLintResolver) {
        self.resolvers.push(resolver);
    }

    /// Look up a native by name: the map first, then the resolvers.
    pub fn get(&self, name: &str) -> Option<NativeLintFn> {
        if let Some(f) = self.map.get(name) {
            return Some(Arc::clone(f));
        }
        self.resolvers.iter().find_map(|r| r(name))
    }

    /// Whether `name` resolves (map hit or a resolver accepts it).
    pub fn contains(&self, name: &str) -> bool {
        self.map.contains_key(name) || self.resolvers.iter().any(|r| r(name).is_some())
    }
}

/// The canonical lint-native names [`canonical_lint_natives`] registers.
pub const CANONICAL_LINT_NATIVES: &[&str] = &[
    "scope.unbound-wire",
    "scope.shadowed-binding",
    "scope.unused-import",
    "naming-convention",
    "source.missing-module-doc",
    "builtin.empty-block",
    "builtin.unreachable-code",
    "builtin.shadowed-binding",
    "builtin.unused-binding",
];

/// Build a registry holding the canonical surface's native lint rules
/// (ADR-0097 §5).
///
/// These are the rules that need cross-statement scope analysis, raw
/// source access, or per-statement AST walks — the semantic residue that
/// stays Rust, mirroring the `desugar.*` natives in
/// `plugin-engine-transform-rewrite`. `source.fmt` is registered here as a
/// no-op so canonical `lints.yaml` resolves cleanly for hosts without a
/// formatter; `plugin-ext-lint` overrides it with the real formatter-backed
/// implementation.
pub fn canonical_lint_natives() -> NativeLintRegistry {
    let mut reg = NativeLintRegistry::new();
    reg.register("scope.unbound-wire", |ctx| Ok(unbound_wire(ctx)));
    reg.register("scope.shadowed-binding", |ctx| Ok(shadowed_binding(ctx)));
    reg.register("scope.unused-import", |ctx| Ok(unused_import(ctx)));
    reg.register("naming-convention", |ctx| Ok(naming_convention(ctx)));
    reg.register("source.missing-module-doc", |ctx| {
        Ok(missing_module_doc(ctx))
    });
    reg.register("source.fmt", |ctx| Ok(fmt_noop(ctx)));
    reg.register("builtin.empty-block", |ctx| Ok(empty_block(ctx)));
    reg.register("builtin.unreachable-code", |ctx| Ok(unreachable_code(ctx)));
    reg.register("builtin.shadowed-binding", |ctx| {
        Ok(shadowed_binding_single(ctx))
    });
    reg.register("builtin.unused-binding", |ctx| {
        Ok(unused_binding_single(ctx))
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

/// `source.fmt` — no-op placeholder overridden by hosts that own the
/// formatter (`plugin-ext-lint`).
fn fmt_noop(_ctx: &LintCtx<'_>) -> Vec<NativeHit> {
    Vec::new()
}

// ---------------------------------------------------------------------------
// `builtin.*` — per-statement AST checks (formerly `LintEngine::lint`)
// ---------------------------------------------------------------------------

/// `builtin.empty-block` — a variadic `statements` block with no children.
fn empty_block(ctx: &LintCtx<'_>) -> Vec<NativeHit> {
    let classifier = NodeClassifier::build(ctx.lang);
    let mut hits = Vec::new();
    for stmt in ctx.module {
        walk_empty_blocks(
            &stmt.ast,
            stmt.root,
            ctx.lang,
            &classifier,
            stmt.offset,
            &mut hits,
        );
    }
    hits
}

fn walk_empty_blocks(
    ast: &Ast,
    r: AstRef,
    lang: &Language,
    classifier: &NodeClassifier,
    offset: usize,
    hits: &mut Vec<NativeHit>,
) {
    let Ok(node) = ast.at(r) else { return };
    if classifier.is_block(lang, node) && child_indices(node).is_empty() {
        hits.push(NativeHit {
            args: BTreeMap::new(),
            span: Some(absolute(node.span, offset)),
            fix: None,
        });
    }
    for &c in &child_indices(node) {
        walk_empty_blocks(ast, AstRef::new(c), lang, classifier, offset, hits);
    }
}

/// `builtin.unreachable-code` — code following a terminal statement in a block.
fn unreachable_code(ctx: &LintCtx<'_>) -> Vec<NativeHit> {
    let classifier = NodeClassifier::build(ctx.lang);
    let mut hits = Vec::new();
    for stmt in ctx.module {
        walk_unreachable(
            &stmt.ast,
            stmt.root,
            ctx.lang,
            &classifier,
            stmt.offset,
            &mut hits,
        );
    }
    hits
}

fn walk_unreachable(
    ast: &Ast,
    r: AstRef,
    lang: &Language,
    classifier: &NodeClassifier,
    offset: usize,
    hits: &mut Vec<NativeHit>,
) {
    let Ok(node) = ast.at(r) else { return };
    let children = child_indices(node);
    if classifier.is_block(lang, node) {
        let mut found_terminal = false;
        for &c in &children {
            let Ok(child) = ast.at(AstRef::new(c)) else {
                continue;
            };
            if found_terminal {
                hits.push(NativeHit {
                    args: BTreeMap::new(),
                    span: Some(absolute(child.span, offset)),
                    fix: None,
                });
                break;
            }
            if classifier.is_terminal(child) {
                found_terminal = true;
            }
        }
    }
    for &c in &children {
        walk_unreachable(ast, AstRef::new(c), lang, classifier, offset, hits);
    }
}

/// `builtin.shadowed-binding` — a binding shadows an enclosing one.
fn shadowed_binding_single(ctx: &LintCtx<'_>) -> Vec<NativeHit> {
    let classifier = NodeClassifier::build(ctx.lang);
    let mut hits = Vec::new();
    for stmt in ctx.module {
        let mut scope = ScopeStack::new();
        walk_shadows(
            &stmt.ast,
            stmt.root,
            ctx.lang,
            &classifier,
            stmt.offset,
            &mut scope,
            &mut hits,
        );
    }
    hits
}

fn walk_shadows(
    ast: &Ast,
    r: AstRef,
    lang: &Language,
    classifier: &NodeClassifier,
    offset: usize,
    scope: &mut ScopeStack,
    hits: &mut Vec<NativeHit>,
) {
    let Ok(node) = ast.at(r) else { return };

    // Report a shadow before binding it in the current scope.
    if let Some(name) = classifier.binding_name(lang, ast, node) {
        if scope.shadows(&name) {
            hits.push(NativeHit::named(name, absolute(node.span, offset)));
        }
    }

    let is_block = classifier.is_block(lang, node);
    if is_block {
        scope.push();
    }
    if let Some(name) = classifier.binding_name(lang, ast, node) {
        scope.bind(&name);
    }

    // Arrow parameters (children at a `params` role) are also bindings.
    let params_idx = lang.child_role_index(&node.kind, "params").ok().flatten();
    if let Some(pi) = params_idx {
        let children = child_indices(node);
        if let Some(&pref) = children.get(pi) {
            if let Ok(params_node) = ast.at(AstRef::new(pref)) {
                if let Ok((names, _rest)) = lang.param_names(ast, params_node) {
                    for name in names {
                        if scope.shadows(&name) {
                            hits.push(NativeHit::named(
                                name.clone(),
                                absolute(params_node.span, offset),
                            ));
                        }
                        scope.bind(&name);
                    }
                }
            }
        }
    }

    let target_idx = lang.child_role_index(&node.kind, "target").ok().flatten();
    for (pos, &c) in child_indices(node).iter().enumerate() {
        if Some(pos) == target_idx || Some(pos) == params_idx {
            continue;
        }
        walk_shadows(ast, AstRef::new(c), lang, classifier, offset, scope, hits);
    }

    if is_block {
        scope.pop();
    }
}

/// `builtin.unused-binding` — a binding is never referenced.
fn unused_binding_single(ctx: &LintCtx<'_>) -> Vec<NativeHit> {
    let classifier = NodeClassifier::build(ctx.lang);
    let mut hits = Vec::new();
    for stmt in ctx.module {
        let mut bindings: Vec<(String, Span)> = Vec::new();
        let mut references: HashSet<String> = HashSet::new();
        walk_bindings(
            &stmt.ast,
            stmt.root,
            ctx.lang,
            &classifier,
            stmt.offset,
            true,
            &mut bindings,
            &mut references,
        );
        for (name, span) in bindings {
            if !references.contains(&name) {
                hits.push(NativeHit::named(name, span));
            }
        }
    }
    hits
}

fn walk_bindings(
    ast: &Ast,
    r: AstRef,
    lang: &Language,
    classifier: &NodeClassifier,
    offset: usize,
    is_root: bool,
    bindings: &mut Vec<(String, Span)>,
    references: &mut HashSet<String>,
) {
    let Ok(node) = ast.at(r) else { return };
    // A top-level statement root is the module/export surface; only nested
    // bindings are checked for unused references.
    if !is_root {
        if let Some(name) = classifier.binding_name(lang, ast, node) {
            bindings.push((name, absolute(node.span, offset)));
        }
    }
    if let Some(name) = classifier.reference_name(lang, ast, node) {
        references.insert(name);
    }

    let target_idx = lang.child_role_index(&node.kind, "target").ok().flatten();
    let params_idx = lang.child_role_index(&node.kind, "params").ok().flatten();
    for (pos, &c) in child_indices(node).iter().enumerate() {
        if Some(pos) == target_idx || Some(pos) == params_idx {
            continue;
        }
        walk_bindings(
            ast,
            AstRef::new(c),
            lang,
            classifier,
            offset,
            false,
            bindings,
            references,
        );
    }
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
