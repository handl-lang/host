//! `handl-plugin-codemod` — the host-side codemod wiring (P-002 /
//! PL-004 T-085).
//!
//! `hnl-tool-codemod` is the pure engine: it consumes a validated
//! [`CodemodSpec`], per-target [`Language`] data, glob-resolved
//! [`TargetArtifacts`], and injected resolver/provider seams, and
//! produces a [`CodemodPlan`] — edits plus a materialized
//! [`Change`] set. This crate is everything *around* that engine:
//!
//! - [`Codemod`] — the builder hosts configure once and reuse: a
//!   `language:` name → [`Language`] map (the canonical kit language
//!   registers as `"handl"` via [`Codemod::with_kit_language`]),
//!   `handl:<mod>.<fn>` rewrite modules ([`HandlModule`]s through
//!   [`HandlNativeRewrite`], the same resolver pattern as
//!   `HandlNativeLint`), `native:`/bare rewrite functions
//!   ([`NativeRewriteMap`]), `requires:` engine contexts, and
//!   [`CodemodRuleProvider`] rule packs.
//! - [`resolve_targets`] — `targets[].glob` → [`TargetArtifacts`]
//!   over a filesystem root (`glob` crate; kernel-reserved
//!   `.handl/codegen/` paths are never codemod targets).
//! - [`merge_spec`] — appends provider-supplied rules to the spec's
//!   own `rules:` and re-validates (duplicate ids are reported, never
//!   silently dropped).
//!
//! The lifecycle half (`hnl codemod plan|apply|revert`) lives in
//! `handl-plugin-ext-codemod`; this crate stays command-free so hosts
//! can drive [`Codemod::engine`] programmatically.
//!
//! # `requires:` honesty boundary
//!
//! [`EngineContextProvider`]s are host-supplied — this crate wires the
//! seam (`Codemod::with_engine_context` / provider
//! [`CodemodRuleProvider::engine_contexts`]) but ships **no concrete
//! engine providers**: `requires: [typecheck]` with nothing registered
//! is an honest `EngineUnavailable` diagnostic governed by the rule's
//! `on_engine_error` (the tool-codemod crate documents the same
//! boundary — `hnl-tool-typecheck` pulls host deps the engine must
//! not see).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use glob::Pattern;
use handl_language::{AstRef, LangError, Language};
use handl_plugin_lib_handl::{HandlModule, HandlNativeRewrite};
use hnl_tool_codegen::{ArtifactRef, ArtifactStore, RESERVED_PREFIX};
use hnl_tool_codemod::{
    CodemodEngine, EngineContextProvider, EngineContextReport, EngineContextRequest,
    LanguageResolver, SpecIssue, StaticLanguageResolver, TargetArtifacts,
};
use hnl_tool_transform::native::{ChainedRewrite, NativeRewrite, RewriteCtx};

/// A named native rewrite function — the `Fn` half of
/// `hnl_tool_transform::native::NativeRewriteRegistry::register`,
/// behind `Arc` so registries and providers can be cloned into every
/// per-run engine.
pub type NativeRewriteFn =
    Arc<dyn Fn(&mut RewriteCtx<'_>) -> Result<AstRef, LangError> + Send + Sync>;

/// A [`NativeRewrite`] over a fixed `name → fn` map — the cloneable
/// counterpart of `NativeRewriteRegistry` (whose boxed fns are not
/// `Clone`, which a reusable [`Codemod`] builder needs).
///
/// Unknown names report the same "unknown native rewrite" error text
/// the registry emits, and [`NativeRewrite::provides`] answers from
/// the key set so [`ChainedRewrite`] routing stays exact.
#[derive(Clone, Default)]
pub struct NativeRewriteMap {
    fns: BTreeMap<String, NativeRewriteFn>,
}

impl NativeRewriteMap {
    /// An empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `f` under `name` (builder style).
    pub fn with_rewrite(mut self, name: impl Into<String>, f: NativeRewriteFn) -> Self {
        self.fns.insert(name.into(), f);
        self
    }

    /// Register `f` under `name`, replacing any prior entry.
    pub fn register(&mut self, name: impl Into<String>, f: NativeRewriteFn) {
        self.fns.insert(name.into(), f);
    }

    /// Whether `name` is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.fns.contains_key(name)
    }
}

impl NativeRewrite for NativeRewriteMap {
    fn rewrite(&self, name: &str, ctx: &mut RewriteCtx<'_>) -> Result<AstRef, LangError> {
        match self.fns.get(name) {
            Some(f) => f(ctx),
            None => Err(LangError::engine(format!("unknown native rewrite: {name}"))),
        }
    }

    fn provides(&self, name: &str) -> bool {
        self.fns.contains_key(name)
    }
}

/// A plugin seam for rule packs (P-002: "plugin contributions too —
/// `plugin-codemod` loads `codemod.yaml` plus any registered
/// `CodemodRuleProvider`").
///
/// A provider contributes *data*, never engine behavior: extra rules
/// appended after the spec's own (so spec rules keep declaration-order
/// priority), `native:`/bare rewrite functions, `requires:` engine
/// contexts, and `.hnl` rewrite modules for `handl:<mod>.<fn>` names.
/// Lint rule packs are the intended first consumer — a `lint.*` native
/// exposed here is callable as a codemod rewrite (the `lint --fix` =
/// codemod path).
///
/// All methods default to empty; implement only what the pack has.
pub trait CodemodRuleProvider: Send + Sync {
    /// Stable provider id — used in diagnostics when the provider's
    /// output is rejected (e.g. a duplicate rule id after merging).
    fn id(&self) -> &str;

    /// Rules appended after [`CodemodSpec::rules`], in order. Ids must
    /// be unique across spec + providers — [`Codemod::merge_spec`]
    /// re-validates and reports duplicates.
    fn rules(&self) -> Vec<CodemodRule> {
        Vec::new()
    }

    /// Native rewrite functions the pack registers, under bare names
    /// (`rewrite: "native:<name>"` strips the tag before dispatch;
    /// `rewrite: "<name>"` reaches the same map).
    fn natives(&self) -> Vec<(String, NativeRewriteFn)> {
        Vec::new()
    }

    /// `.hnl` modules the pack exposes to `handl:<mod>.<fn>` rewrite
    /// names — registered on the same [`HandlNativeRewrite`] adapter
    /// kit modules use.
    fn handl_modules(&self) -> Vec<Arc<HandlModule>> {
        Vec::new()
    }

    /// Engine-context providers satisfying this pack's rules'
    /// `requires:` entries.
    fn engine_contexts(&self) -> Vec<Arc<dyn EngineContextProvider>> {
        Vec::new()
    }
}

/// `Arc<dyn EngineContextProvider>` does not itself implement the
/// trait (orphan rule), so providers are re-wrapped per engine build.
struct ArcEngineContext(Arc<dyn EngineContextProvider>);

impl EngineContextProvider for ArcEngineContext {
    fn engine_name(&self) -> &str {
        self.0.engine_name()
    }

    fn engine_version(&self) -> &str {
        self.0.engine_version()
    }

    fn provide(
        &self,
        req: &EngineContextRequest<'_>,
    ) -> Result<EngineContextReport, hnl_tool_codemod::EngineContextError> {
        self.0.provide(req)
    }
}

/// The host-side codemod configuration: language map, rewrite
/// resolvers, rule providers, and engine contexts — everything
/// `hnl-tool-codemod` takes as injected seams, in one reusable
/// builder.
///
/// Cheap to clone-configure; [`Codemod::engine`] borrows both the
/// builder and the [`ArtifactStore`] for one `plan` call.
#[derive(Default)]
pub struct Codemod {
    /// `language:` name → `Language` data (ADR-0044 — a new language
    /// is a data change; the map is the whole registry).
    languages: Vec<(String, Arc<Language>)>,
    /// Escape hatch: a fully custom [`LanguageResolver`] (e.g. one that
    /// merges fragments per target). When set it wins over `languages`.
    language_resolver: Option<Arc<dyn LanguageResolver>>,
    /// `.hnl` modules for `handl:<mod>.<fn>` rewrite names.
    handl_modules: Vec<Arc<HandlModule>>,
    /// Bare-name native rewrites (`native:<name>` / `rewrite: "<name>"`).
    natives: BTreeMap<String, NativeRewriteFn>,
    /// Extra [`NativeRewrite`] layers chained under the handl adapter —
    /// e.g. a desugar registry a host wants reachable from codemod
    /// rules.
    extra_rewrites: Vec<Arc<dyn NativeRewrite>>,
    /// Rule-pack providers.
    providers: Vec<Arc<dyn CodemodRuleProvider>>,
    /// `requires:` engine contexts registered directly (provider
    /// contexts are collected at engine build).
    engine_contexts: Vec<Arc<dyn EngineContextProvider>>,
}

impl Codemod {
    /// An empty codemod — register at least one language before any
    /// target can run (unresolved targets are honest
    /// `LanguageUnresolved` diagnostics, never silent skips).
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `lang` under `name` for `targets[].language` lookups.
    pub fn with_language(mut self, name: impl Into<String>, lang: Arc<Language>) -> Self {
        self.languages.push((name.into(), lang));
        self
    }

    /// Register the canonical kit language under `"handl"` — the name
    /// `codemod.yaml` targets conventionally use.
    pub fn with_kit_language(self, lang: &Language) -> Self {
        self.with_language("handl", Arc::new(lang.clone()))
    }

    /// Wire a custom [`LanguageResolver`] (fragment-merging
    /// languages, multi-kit hosts). Overrides `with_language` entries.
    pub fn with_language_resolver(mut self, resolver: Arc<dyn LanguageResolver>) -> Self {
        self.language_resolver = Some(resolver);
        self
    }

    /// Register a `.hnl` module for `handl:<mod>.<fn>` rewrites.
    pub fn with_handl_module(mut self, module: Arc<HandlModule>) -> Self {
        self.handl_modules.push(module);
        self
    }

    /// Register every `.hnl` module a package directory declares
    /// (`pkg.hnl` `modules:` → `src/*.hnl`) for `handl:<mod>.<fn>`
    /// rewrite names — the package-module half of P-002's wiring: a
    /// `codemod.yaml` living in a `lib-*` package resolves its
    /// `handl:` rewrites against that package's own modules
    /// (e.g. lib-codemod's `codemod` module). Intra-package `import`s
    /// between sibling modules are wired; imports of other packages
    /// stay unresolved (the `PackageResolver` boundary).
    ///
    /// # Errors
    ///
    /// `Err` when `pkg_dir` is not a package or a declared module
    /// cannot be read — the CLI surfaces this as a hard error rather
    /// than letting each `handl:` rule fail as a `RewriteError`.
    pub fn with_package_modules(mut self, pkg_dir: &Path) -> Result<Self, LangError> {
        for module in handl_plugin_lib_handl::pkg::handl_modules_in_dir(pkg_dir)? {
            self.handl_modules.push(module);
        }
        Ok(self)
    }

    /// Register a native rewrite under a bare name — reachable as
    /// `rewrite: "native:<name>"` or `rewrite: "<name>"`.
    pub fn with_native<F>(mut self, name: impl Into<String>, f: F) -> Self
    where
        F: Fn(&mut RewriteCtx<'_>) -> Result<AstRef, LangError> + Send + Sync + 'static,
    {
        self.natives.insert(name.into(), Arc::new(f));
        self
    }

    /// Chain an extra [`NativeRewrite`] layer (consulted for names the
    /// handl adapter and native map do not provide).
    pub fn with_rewrite_layer(mut self, natives: Arc<dyn NativeRewrite>) -> Self {
        self.extra_rewrites.push(natives);
        self
    }

    /// Register a [`CodemodRuleProvider`] rule pack.
    pub fn with_rule_provider(mut self, provider: Arc<dyn CodemodRuleProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    /// Register an [`EngineContextProvider`] for `requires:` entries.
    pub fn with_engine_context(mut self, provider: Arc<dyn EngineContextProvider>) -> Self {
        self.engine_contexts.push(provider);
        self
    }

    /// The spec plus every provider's rules, re-validated.
    ///
    /// Provider rules append after `spec.rules` (declaration order =
    /// fire order). Returns the merged spec, or every
    /// [`SpecIssue`] found — duplicate rule ids across the merged set
    /// are the expected failure.
    pub fn merge_spec(&self, spec: &CodemodSpec) -> Result<CodemodSpec, Vec<SpecIssue>> {
        let mut merged = spec.clone();
        for provider in &self.providers {
            merged.rules.extend(provider.rules());
        }
        merged.validate()?;
        Ok(merged)
    }

    /// All engine contexts: direct registrations, then each provider's.
    fn contexts(&self) -> Vec<Arc<dyn EngineContextProvider>> {
        let mut out = self.engine_contexts.clone();
        for provider in &self.providers {
            out.extend(provider.engine_contexts());
        }
        out
    }

    /// Assemble the `handl:`/`native:` rewrite resolver:
    /// [`HandlNativeRewrite`] (kit/provider `.hnl` modules) over a
    /// [`ChainedRewrite`] fallback of the native map plus any extra
    /// layers. `codemod.delete` never reaches here — the engine chains
    /// its internal tombstone layer ahead of whatever this returns.
    fn rewrite_resolver(&self) -> Arc<dyn NativeRewrite> {
        let mut map = NativeRewriteMap::new();
        for (name, f) in &self.natives {
            map.register(name.clone(), Arc::clone(f));
        }
        for provider in &self.providers {
            for (name, f) in provider.natives() {
                map.register(name, f);
            }
        }
        let mut layers: Vec<Arc<dyn NativeRewrite>> = vec![Arc::new(map)];
        layers.extend(self.extra_rewrites.iter().map(Arc::clone));
        let fallback: Arc<dyn NativeRewrite> = if layers.len() == 1 {
            layers.pop().expect("one layer")
        } else {
            Arc::new(ChainedRewrite::new(layers))
        };

        let mut handl = HandlNativeRewrite::new().with_fallback(fallback);
        for module in &self.handl_modules {
            handl = handl.with_module_arc(Arc::clone(module));
        }
        for provider in &self.providers {
            for module in provider.handl_modules() {
                handl = handl.with_module_arc(module);
            }
        }
        Arc::new(handl)
    }

    /// Build the wired [`CodemodEngine`] over `store` for one `plan`
    /// call. The returned engine borrows `self` and `store`.
    pub fn engine<'s, S: ArtifactStore + ?Sized>(&'s self, store: &'s S) -> CodemodEngine<'s, S> {
        let engine = if let Some(resolver) = &self.language_resolver {
            CodemodEngine::new(store).with_language_resolver_arc(Arc::clone(resolver))
        } else {
            let mut languages = StaticLanguageResolver::new();
            for (name, lang) in &self.languages {
                languages = languages.with_language(name.clone(), Arc::clone(lang));
            }
            CodemodEngine::new(store).with_language_resolver(languages)
        };
        let engine = engine.with_rewrite_resolver(self.rewrite_resolver());
        self.contexts().into_iter().fold(engine, |engine, ctx| {
            engine.with_engine_context(ArcEngineContext(ctx))
        })
    }
}

/// Resolve `spec.targets[].glob` against `root` into
/// [`TargetArtifacts`] — the caller-supplied half of
/// [`CodemodEngine::plan`] (the `ArtifactStore` trait cannot
/// enumerate).
///
/// - `root` is canonicalized when possible; globs are interpreted
///   relative to it (the codemod spec's directory in the CLI flow).
/// - Only regular files become [`ArtifactRef`]s, spelled with `/`
///   separators relative to `root`; paths outside `root` (a `..`
///   glob) and kernel-reserved `.handl/codegen/` paths are skipped.
/// - `extra_glob`, when given, is ANDed with each target's own glob —
///   the `hnl codemod --target <glob>` narrowing flag.
/// - Artifacts are sorted and deduplicated per target; targets with
///   no matches are still returned (the engine reports
///   `NoArtifacts` honestly).
///
/// # Errors
///
/// `Err` on an invalid glob pattern (spec or extra) or a glob
/// iteration I/O failure — surfaced verbatim, never skipped.
pub fn resolve_targets(
    root: &Path,
    spec: &CodemodSpec,
    extra_glob: Option<&str>,
) -> Result<Vec<TargetArtifacts>, String> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let extra = extra_glob
        .map(Pattern::new)
        .transpose()
        .map_err(|e| format!("--target glob {extra_glob:?}: {e}"))?;

    let mut out = Vec::with_capacity(spec.targets.len());
    for (i, target) in spec.targets.iter().enumerate() {
        // `./`-prefixed globs join fine but never match the relative
        // re-check spelling — normalize once.
        let glob_str = target.glob.strip_prefix("./").unwrap_or(&target.glob);
        let pattern = Pattern::new(glob_str)
            .map_err(|e| format!("targets[{i}].glob {:?}: {e}", target.glob))?;
        let full = root.join(glob_str);
        let full = full
            .to_str()
            .ok_or_else(|| format!("targets[{i}].glob {:?}: non-UTF-8 path", target.glob))?;
        let mut refs: Vec<ArtifactRef> = Vec::new();
        for entry in
            glob::glob(full).map_err(|e| format!("targets[{i}].glob {:?}: {e}", target.glob))?
        {
            let path = entry.map_err(|e| format!("targets[{i}].glob {:?}: {e}", target.glob))?;
            if !path.is_file() {
                continue;
            }
            let Ok(rel) = path.strip_prefix(&root) else {
                continue; // glob escaped the root (`..`) — never a target
            };
            // `strip_prefix` is lexical: `root/../sibling/x` still
            // strips, leaving a `..`-carrying rel. Reject anything but
            // normal components so escapes can never become refs.
            if !rel
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
            {
                continue;
            }
            let rel = rel.to_string_lossy().replace('\\', "/");
            if rel.starts_with(RESERVED_PREFIX) {
                continue; // kernel bookkeeping is never codemod input
            }
            if let Some(extra) = &extra
                && !extra.matches(&rel)
                && !extra.matches_path(Path::new(&rel))
            {
                continue;
            }
            // The spec's own glob already matched the absolute path;
            // re-check on the relative spelling so `**` semantics stay
            // identical for callers that passed `./`-prefixed globs.
            if !pattern.matches(&rel) && !pattern.matches_path(Path::new(&rel)) {
                continue;
            }
            refs.push(ArtifactRef::new(rel));
        }
        refs.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        refs.dedup_by(|a, b| a.as_str() == b.as_str());
        out.push(TargetArtifacts {
            target: i,
            artifacts: refs,
        });
    }
    Ok(out)
}

/// Re-export the engine surface hosts drive — spec types, plan
/// output, and the `CodemodEngine`/resolver traits — so consumers
/// depend on one crate.
pub use hnl_tool_codemod::{
    CodemodDiagnostic, CodemodEdit, CodemodPlan, CodemodRewrite, CodemodRule, CodemodSpec,
    CodemodStats, CodemodTarget, DiagnosticKind, EngineErrorPolicy, RuleScope, SpecError,
    UnparseMode,
};
