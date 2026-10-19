//! # handl-kit-data
//!
//! The **data-driven kit assembly** framework for ADR-0044 Phase 5.
//!
//! ADR-0044 §1 realizes that "selection and merge are the composition; the
//! registry is replaced by a `Language` built from merged fragments." This
//! crate replaces `PluginRegistry`-based kit assembly with a data-driven
//! pipeline:
//!
//! ```text
//! manifest → fragment sources → merge → Language + Runtime selection
//! ```
//!
//! A [`DataKitManifest`] is an ordered list of data-file fragment sources
//! (ADR §6 end state) plus include/exclude filters and a [`RuntimeConfig`]
//! for engine selection (ADR §7). A caller that already holds a [`Language`]
//! — e.g. `handl-kit-data-prelude::language_from_yaml` — passes it to
//! [`DataKit::assemble`] as `base_language`: the kit stack merges on top of
//! it. The manifest-named Rust emitters of the ADR §6 migration scaffold
//! are retired (see `project/parked.yaml`).
//!
//! ## Data-file sources
//!
//! A fragment source is a serialized [`Language`] on disk, read at assembly
//! time ([`FragmentSourceEntry::source`]). [`DataKitManifest::load`] /
//! [`DataKitManifest::load_from_text`] do the same for a kit manifest. Both
//! dispatch on extension — JSON, YAML (canonical) or TOML — and fail with an
//! error naming the file. This lets the module that owns a set of fragments
//! or a kit manifest *ship it as data* instead of `include_str!`-pinning it
//! into another crate (ADR-0128 §Deferred).
//!
//! ## Include/exclude
//!
//! Per ADR §1, include/exclude operate at the **fragment level** (by fragment
//! `id`), not at the plugin/entry level. The assembler applies the filter
//! *during* merge — fragments whose `id` is in `exclude` (or not in `include`
//! when `include` is non-empty) are skipped. This is a real operation, not a
//! silent no-op (ADR §5): excluding a fragment that another fragment declares
//! `replaces` causes the dependent's merge to fail with
//! [`LangError::UnknownReplace`], surfacing the broken override chain
//! explicitly.
//!
//! ## `extends` chain
//!
//! A kit may declare `extends: "parent-kit-name"`. The parent is assembled
//! first (resolved via [`KitRegistry::resolve_kit`]), then this kit's
//! fragments merge on top. Phase 5 supports one level of `extends`
//! (parent → child); deeper chains are a straightforward recursive extension.
//!
//! The legacy `kit-config` / `kit-server` / `kit-cli` / `kit-hello` /
//! `kit-stdlib` Rust-monolith kits have been **deleted** — kit composition is
//! now data-driven via the `data/kits/*.yaml` manifests in
//! `handl-kit-data-prelude` (resolved through [`LibraryRegistry`] into
//! `ModuleKit`s). `kit-canonical` remains as the canonical runtime assembly.

#![warn(missing_docs)]

pub mod library_registry;
pub use library_registry::{LibraryPlugin, LibraryRegistry};

use std::collections::HashMap;
use std::path::PathBuf;

use handl_language::{CapabilityDecl, Fragment, LangError, Language};
use handl_runtime::{EngineRegistry, Runtime, RuntimeConfig};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

/// A named fragment source entry in a [`DataKitManifest`].
///
/// The `name` labels the source for documentation and `extends` resolution;
/// include/exclude filtering operates on fragment `id`s (ADR §1), not on this
/// name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FragmentSourceEntry {
    /// A human-readable label for this fragment source.
    pub name: String,
    /// Path to a data file (JSON/YAML/TOML) holding a serialized [`Language`]
    /// to load at assembly time.
    pub source: PathBuf,
}

impl FragmentSourceEntry {
    /// Create a new named fragment source entry referencing a data file.
    pub fn data_file(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            source: path.into(),
        }
    }
}

/// A data-driven kit manifest: an ordered list of fragment sources plus
/// include/exclude filters, plus runtime engine selection (ADR §1, §7).
///
/// This is pure serializable data — a kit author writes it as TOML/JSON and
/// the assembler ([`DataKit::assemble`]) turns it into a [`Language`] +
/// [`RuntimeConfig`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DataKitManifest {
    /// Ordered fragment sources — merged in declared order. Later fragments
    /// may `replaces` earlier ones (ADR §5).
    #[serde(default)]
    pub fragments: Vec<FragmentSourceEntry>,
    /// Fragment ids to include. If empty, all fragments are included.
    #[serde(default)]
    pub include: Vec<String>,
    /// Fragment ids to exclude.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Optional parent kit this `extends`. The parent is assembled first, then
    /// this kit's fragments merge on top (ADR §1, §5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,
    /// Runtime engine selection (ADR §7).
    #[serde(default)]
    pub runtime: RuntimeConfig,
    /// Capability declarations contributed directly by this kit. Merged into
    /// the assembled `Language` using the same `lang.caps.merge` path as
    /// fragment-sourced capabilities (ADR-0044 §5).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caps: Vec<CapabilityDecl>,
    /// Ordered library module plugin names, resolved at load time via a
    /// [`LibraryRegistry`] populated by the host (e.g. `handl-plugin-ext-kit`).
    /// When `extends` is set, the parent's module list comes first and this
    /// list appends to it, preserving contribution order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<String>,
    /// The base module names the kit's module registry builds (module-level,
    /// not library-level — drives `build_stdlib_registry`'s set). Empty =
    /// the host's compiled-in default set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub base_modules: Vec<String>,
}

impl DataKitManifest {
    /// Create a new empty manifest with no fragments and a default
    /// [`RuntimeConfig`].
    pub fn new() -> Self {
        Self {
            fragments: Vec::new(),
            include: Vec::new(),
            exclude: Vec::new(),
            extends: None,
            runtime: RuntimeConfig::new(),
            caps: Vec::new(),
            modules: Vec::new(),
            base_modules: Vec::new(),
        }
    }

    /// Add a fragment source entry (builder style).
    pub fn with_fragment(mut self, entry: FragmentSourceEntry) -> Self {
        self.fragments.push(entry);
        self
    }

    /// Set the include filter (builder style).
    pub fn with_include(mut self, ids: Vec<String>) -> Self {
        self.include = ids;
        self
    }

    /// Set the exclude filter (builder style).
    pub fn with_exclude(mut self, ids: Vec<String>) -> Self {
        self.exclude = ids;
        self
    }

    /// Set the parent kit this extends (builder style).
    pub fn with_extends(mut self, parent: impl Into<String>) -> Self {
        self.extends = Some(parent.into());
        self
    }

    /// Set the runtime config (builder style).
    pub fn with_runtime(mut self, config: RuntimeConfig) -> Self {
        self.runtime = config;
        self
    }

    /// Set the direct capability declarations (builder style).
    pub fn with_caps(mut self, caps: Vec<CapabilityDecl>) -> Self {
        self.caps = caps;
        self
    }

    /// Set the ordered library module plugin names (builder style).
    pub fn with_modules(mut self, modules: Vec<String>) -> Self {
        self.modules = modules;
        self
    }

    /// Load a kit manifest from a data file at `path`.
    ///
    /// This is the manifest half of the data-file seam
    /// (ADR-0128 §Deferred): a module repo can *ship* its kit manifest
    /// (`kits/<name>.yaml`, say) instead of `include_str!`-pinning it into
    /// another crate. Format is chosen by extension — see
    /// [`DataKitManifest::load_from_text`] for the parser rules.
    ///
    /// The returned manifest's relative [`FragmentSourceEntry::source`]
    /// paths are *not* resolved here; callers resolve them against the
    /// manifest's directory when assembling (see [`DataKit::assemble`]).
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Engine`] if the file cannot be read or parsed.
    pub fn load(path: &std::path::Path) -> Result<Self, LangError> {
        read_data_file(path)
    }

    /// Parse a kit manifest from `text`, choosing the parser from `ext`
    /// (`json`, `yaml`/`yml`, `toml`; anything else is tried as JSON then
    /// YAML). Exposed for callers that already hold the manifest source —
    /// e.g. `include_str!`-embedded kit data.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Engine`] if no parser accepts `text`.
    pub fn load_from_text(text: &str, ext: &str) -> Result<Self, LangError> {
        parse_data::<Self>(text, ext, "<inline>")
    }
}

impl Default for DataKitManifest {
    fn default() -> Self {
        Self::new()
    }
}

/// A registry mapping kit names to their [`DataKitManifest`]s — how the
/// `extends` chain in a manifest resolves.
///
/// This is the bridge between **data** (manifest `extends:` names) and
/// **Rust** (the manifests themselves). Fragment sources are data files
/// (ADR-0044 §6 end state); a caller that already holds a [`Language`]
/// passes it to [`DataKit::assemble`] as `base_language`.
pub struct KitRegistry {
    /// Kit manifests keyed by kit name (for `extends` resolution).
    kits: HashMap<String, DataKitManifest>,
}

impl Default for KitRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl KitRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            kits: HashMap::new(),
        }
    }

    /// Register a kit manifest under `name` (for `extends` resolution).
    pub fn register_kit(&mut self, name: &str, manifest: DataKitManifest) {
        self.kits.insert(name.to_string(), manifest);
    }

    /// Resolve a kit name to its manifest (for `extends`).
    pub fn resolve_kit(&self, name: &str) -> Option<&DataKitManifest> {
        self.kits.get(name)
    }
}

/// The assembled data-driven kit: a merged [`Language`] plus the
/// [`RuntimeConfig`] that selects its engines.
///
/// Built by [`DataKit::assemble`] from a [`DataKitManifest`], a
/// [`KitRegistry`], and a base [`Language`]. Call [`DataKit::into_runtime`]
/// to bind the language to concrete engines via [`Runtime::from_config`].
pub struct DataKit {
    /// The merged language.
    lang: Language,
    /// The runtime engine selection.
    runtime_config: RuntimeConfig,
}

impl std::fmt::Debug for DataKit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataKit")
            .field("lang", &self.lang)
            .field("runtime_config", &self.runtime_config)
            .finish()
    }
}

impl DataKit {
    /// Assemble a [`DataKit`] from a manifest, a kit registry, and a base
    /// [`Language`].
    ///
    /// This is the core of ADR-0044 §1: "selection and merge are the
    /// composition." The pipeline is:
    ///
    /// 1. If the manifest declares `extends`, assemble the parent kit first
    ///    (recursively) and use its `Language` as the accumulator base.
    ///    Otherwise `base_language` is the accumulator base — the
    ///    `extends`-recursive call passes it down, so the caller-supplied
    ///    base always lands at the root of the chain. This is how a
    ///    Rust-computed language (e.g. the canonical YAML language) enters
    ///    kit assembly: the kit stack merges on top of it.
    /// 2. For each fragment source entry (in declared order), read the
    ///    data file and deserialize a [`Language`].
    /// 3. Iterate the produced language's fragments across all six fragment
    ///    sets; apply the include/exclude filter by fragment `id`; merge the
    ///    passing fragments into the accumulator in order (tokens, grammar,
    ///    eval, ast, caps, options — preserving `requires` dependencies).
    ///    The filter does *not* apply to `base_language` — the base is the
    ///    accumulator itself, not a fragment source.
    /// 4. Produce a [`DataKit`] holding the merged language and the manifest's
    ///    [`RuntimeConfig`].
    ///
    /// # Include/exclude
    ///
    /// A fragment is merged only if its `id` is not in `exclude` **and**
    /// (`include` is empty or its `id` is in `include`). Excluding a fragment
    /// that another fragment declares `replaces` causes the dependent's merge
    /// to fail with [`LangError::UnknownReplace`] — the override chain breaks
    /// explicitly, never silently (ADR §5).
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on: unknown `extends` kit, an unreadable or
    /// unparseable data-file source, fragment merge failure (collision,
    /// unknown replace target, missing requires), or a broken override chain
    /// caused by an exclude.
    pub fn assemble(
        manifest: &DataKitManifest,
        registry: &KitRegistry,
        base_language: Language,
    ) -> Result<Self, LangError> {
        // 1. Resolve the `extends` chain: assemble the parent first (one level
        //    supported in Phase 5), passing `base_language` to the root of
        //    the chain.
        let (mut lang, parent_runtime) = if let Some(parent_name) = &manifest.extends {
            let parent_manifest = registry
                .resolve_kit(parent_name)
                .ok_or_else(|| LangError::engine(format!("unknown parent kit: {}", parent_name)))?;
            let parent = DataKit::assemble(parent_manifest, registry, base_language)?;
            (parent.lang, parent.runtime_config)
        } else {
            (base_language, RuntimeConfig::default())
        };

        // 2 + 3. Merge each data-file source's fragments, applying the filter.
        for entry in &manifest.fragments {
            let fragment_lang: Language = read_data_file(&entry.source)?;
            merge_filtered(
                &mut lang,
                &fragment_lang,
                &manifest.include,
                &manifest.exclude,
            )?;
        }

        // 4. Merge direct capability declarations contributed by this kit
        //    using the same `lang.caps.merge` path as fragment-sourced caps.
        for cap in &manifest.caps {
            lang.caps.merge(cap.clone())?;
        }

        // Runtime config: a child with no explicit selection inherits its
        // parent's `RuntimeConfig` so that `extends: canonical` preserves e.g.
        // `runtime.typecheck: nbe`.
        let runtime_config = if manifest.runtime == RuntimeConfig::default() {
            parent_runtime
        } else {
            manifest.runtime.clone()
        };

        Ok(Self {
            lang,
            runtime_config,
        })
    }

    /// The merged language.
    pub fn lang(&self) -> &Language {
        &self.lang
    }

    /// The runtime engine selection.
    pub fn runtime_config(&self) -> &RuntimeConfig {
        &self.runtime_config
    }

    /// Consume the kit and build a [`Runtime`] bound to its language and
    /// engine selection via [`Runtime::from_registry`] (ADR-0047 §2).
    ///
    /// The caller supplies an [`EngineRegistry`] populated with the available
    /// engine impls; `into_runtime` resolves `self.runtime_config` against it.
    /// If `modules` is `Some`, it is passed to the eval engine for
    /// foreign (`ArrSource::Foreign`) dispatch.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Engine`] for any unknown engine name.
    pub fn into_runtime(
        self,
        registry: &Rc<EngineRegistry>,
        modules: Option<Rc<dyn handl_platform::ModuleLookup>>,
    ) -> Result<Runtime, LangError> {
        Runtime::from_registry_with_globals(
            self.lang,
            registry,
            &self.runtime_config,
            modules,
            None,
        )
    }
}

/// Read and deserialize the data file at `path` into `T`.
///
/// The parser is chosen by extension — see [`parse_data`] for the rules. A
/// read or parse failure surfaces as [`LangError::Engine`] naming the file, so
/// a misconfigured kit manifest or fragment source fails loudly rather than
/// silently assembling an empty language.
///
/// This is the loading half of the data-file seam
/// (ADR-0128 §Deferred): fragments and kit manifests can be *shipped as data*
/// by the module that owns them, rather than `include_str!`-pinned into a
/// crate. See also [`DataKitManifest::load`].
///
/// # Errors
///
/// Returns [`LangError::Engine`] if the file cannot be read, or if no parser
/// accepts its contents.
fn read_data_file<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, LangError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        LangError::engine(format!("cannot read data file {}: {e}", path.display()))
    })?;
    parse_data(&text, &extension_of(path), &path.display().to_string())
}

/// Parse `text` as `T`, selecting the parser from `ext`.
///
/// `json` → `serde_json`, `yaml`/`yml` → `serde_yaml` (the canonical
/// fragment/kit format), `toml` → `toml`. An unrecognized extension is tried
/// as JSON, then YAML. `label` names the source in error messages (a path, or
/// `"<inline>"`).
///
/// # Errors
///
/// Returns [`LangError::Engine`] if no parser accepts `text`.
fn parse_data<T: serde::de::DeserializeOwned>(
    text: &str,
    ext: &str,
    label: &str,
) -> Result<T, LangError> {
    let parsed: Result<T, String> = match ext {
        "json" => serde_json::from_str(text).map_err(|e| e.to_string()),
        "yaml" | "yml" => serde_yaml::from_str(text).map_err(|e| e.to_string()),
        "toml" => toml::from_str(text).map_err(|e| e.to_string()),
        _ => serde_json::from_str(text)
            .map_err(|e| e.to_string())
            .or_else(|_| serde_yaml::from_str(text).map_err(|e| e.to_string())),
    };

    parsed.map_err(|e| LangError::engine(format!("cannot parse data file {label}: {e}")))
}

/// The lowercased extension of `path` (`""` when it has none).
fn extension_of(path: &std::path::Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Merge `fragment_lang`'s fragments into `lang`, applying the include/exclude
/// filter by fragment `id`.
///
/// Fragments are iterated in the order tokens → grammar → eval → types → ast →
/// transforms → caps → options → lexer_rules → lints, preserving the dependency
/// ordering that `requires` validation
/// relies on (e.g. token kinds before grammar rows that require them). A
/// fragment is merged only if it passes the filter; skipped fragments are
/// simply absent, so a later fragment declaring `replaces: [skipped_id]` will
/// fail with [`LangError::UnknownReplace`] — the explicit, non-silent
/// behavior required by ADR §5.
fn merge_filtered(
    lang: &mut Language,
    fragment_lang: &Language,
    include: &[String],
    exclude: &[String],
) -> Result<(), LangError> {
    let include_set: Option<HashMap<&str, ()>> = if include.is_empty() {
        None
    } else {
        Some(include.iter().map(|s| (s.as_str(), ())).collect())
    };

    let passes = |id: &str| -> bool {
        if exclude.iter().any(|e| e == id) {
            return false;
        }
        match &include_set {
            Some(set) => set.contains_key(id),
            None => true,
        }
    };

    // Merge each fragment set in dependency-friendly order.
    for frag in fragment_lang.tokens.fragments() {
        if passes(frag.id()) {
            lang.tokens.merge(frag.clone())?;
        }
    }
    for frag in fragment_lang.grammar.fragments() {
        if passes(frag.id()) {
            lang.grammar.merge(frag.clone())?;
        }
    }
    for frag in fragment_lang.eval.fragments() {
        if passes(frag.id()) {
            lang.eval.merge(frag.clone())?;
        }
    }
    for frag in fragment_lang.types.fragments() {
        if passes(frag.id()) {
            lang.types.merge(frag.clone())?;
        }
    }
    for frag in fragment_lang.ast.fragments() {
        if passes(frag.id()) {
            // `merge_ast_fragment` also lowers each shape's `desugar:` rule
            // into a `TransformDef` in `lang.transforms` (ADR-0094 §6).
            lang.merge_ast_fragment(frag.clone())?;
        }
    }
    for frag in fragment_lang.transforms.fragments() {
        if passes(frag.id()) {
            // Skip fragments already present with identical content — e.g. a
            // `transform.desugar.*` rule the ast loop above re-synthesized.
            if lang.transforms.get_by_id(frag.id()) == Some(frag) {
                continue;
            }
            lang.transforms.merge(frag.clone())?;
        }
    }
    for frag in fragment_lang.caps.fragments() {
        if passes(frag.id()) {
            lang.caps.merge(frag.clone())?;
        }
    }
    for frag in fragment_lang.options.fragments() {
        if passes(frag.id()) {
            lang.options.merge(frag.clone())?;
        }
    }
    for frag in fragment_lang.lexer_rules.fragments() {
        if passes(frag.id()) {
            lang.lexer_rules.merge(frag.clone())?;
        }
    }
    for frag in fragment_lang.lints.fragments() {
        if passes(frag.id()) {
            lang.lints.merge(frag.clone())?;
        }
    }
    // Ambient globals (`globals.yaml`) are a plain value map, not a
    // fragment set — union them unconditionally (later sources override).
    for (name, value) in &fragment_lang.globals {
        lang.globals.insert(name.clone(), value.clone());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
