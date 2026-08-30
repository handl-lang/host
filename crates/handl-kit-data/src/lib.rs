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
//! A [`DataKitManifest`] is an ordered list of fragment sources (emitters
//! during migration, data files after — ADR §6) plus include/exclude filters
//! and a [`RuntimeConfig`] for engine selection (ADR §7). The
//! [`EmitterRegistry`] bridges data (manifest names) to Rust (emitter
//! constructors), so the assembler never hardcodes a specific emitter.
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
//! first (resolved via [`EmitterRegistry::resolve_kit`]), then this kit's
//! fragments merge on top. Phase 5 supports one level of `extends`
//! (parent → child); deeper chains are a straightforward recursive extension.
//!
//! The existing `kit-canonical` / `kit-config` / `kit-server` / `kit-cli`
//! Rust-monolith kits are **not** modified or deleted — they remain as the
//! pre-migration reference. This crate builds the new data-driven path that
//! runs alongside them.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::path::PathBuf;

use handl_emitters::Emitter;
use handl_language::{Fragment, LangError, Language};
use handl_plugin_engine_eval::HostFunctions;
use handl_runtime::{Runtime, RuntimeConfig};
use serde::{Deserialize, Serialize};

/// A resolved source of `Language` fragments (runtime form).
///
/// Per ADR §6, a fragment source is either a data file (the end state) or a
/// Rust emitter (the migration scaffold). This enum models both at runtime;
/// the serializable manifest form is [`FragmentSourceSerde`].
pub enum FragmentSource {
    /// An emitter that produces a [`Language`] at load time (migration
    /// scaffold, ADR §6).
    Emitter(Box<dyn Emitter>),
    /// A path to a data file (TOML/JSON) loaded at runtime. For Phase 5 the
    /// emitters are the active path; data-file loading is modeled as a type
    /// but returns [`LangError::NotImplemented`] when resolved, since the
    /// authored-data end state arrives after migration completes.
    DataFile(PathBuf),
}

impl std::fmt::Debug for FragmentSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FragmentSource::Emitter(_) => f.debug_struct("FragmentSource::Emitter").finish(),
            FragmentSource::DataFile(path) => f
                .debug_tuple("FragmentSource::DataFile")
                .field(path)
                .finish(),
        }
    }
}

/// The serializable form of a fragment source, used in [`DataKitManifest`].
///
/// A manifest references emitters **by name** (resolved at assembly time via
/// [`EmitterRegistry`]) rather than embedding Rust trait objects, keeping the
/// manifest pure data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FragmentSourceSerde {
    /// Reference to a registered emitter by name (resolved via
    /// [`EmitterRegistry::resolve_emitter`]).
    Emitter(String),
    /// Path to a data file (TOML/JSON) to load at runtime.
    DataFile(PathBuf),
}

/// A named fragment source entry in a [`DataKitManifest`].
///
/// The `name` labels the source for documentation and `extends` resolution;
/// include/exclude filtering operates on fragment `id`s (ADR §1), not on this
/// name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FragmentSourceEntry {
    /// A human-readable label for this fragment source.
    pub name: String,
    /// The serializable source descriptor.
    pub source: FragmentSourceSerde,
}

impl FragmentSourceEntry {
    /// Create a new named fragment source entry referencing an emitter by name.
    pub fn emitter(name: impl Into<String>, emitter: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: FragmentSourceSerde::Emitter(emitter.into()),
        }
    }

    /// Create a new named fragment source entry referencing a data file.
    pub fn data_file(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            source: FragmentSourceSerde::DataFile(path.into()),
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
}

impl Default for DataKitManifest {
    fn default() -> Self {
        Self::new()
    }
}

/// A registry mapping source names (and kit names) to `Box<dyn Emitter>`
/// constructors, so a [`DataKitManifest`] can reference emitters by name
/// without the assembler hardcoding them.
///
/// This is the bridge between **data** (manifest emitter names) and **Rust**
/// (emitter constructors). It also maps kit names to their manifests, which
/// is how the `extends` chain resolves.
pub struct EmitterRegistry {
    /// Constructors keyed by emitter name.
    emitters: HashMap<String, Box<dyn Fn() -> Box<dyn Emitter>>>,
    /// Kit manifests keyed by kit name (for `extends` resolution).
    kits: HashMap<String, DataKitManifest>,
}

impl Default for EmitterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl EmitterRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            emitters: HashMap::new(),
            kits: HashMap::new(),
        }
    }

    /// Register an emitter constructor under `name`.
    ///
    /// The constructor is called each time the emitter is resolved, producing
    /// a fresh `Box<dyn Emitter>`.
    pub fn register_emitter(
        &mut self,
        name: &str,
        ctor: impl Fn() -> Box<dyn Emitter> + 'static,
    ) {
        self.emitters.insert(name.to_string(), Box::new(ctor));
    }

    /// Register a kit manifest under `name` (for `extends` resolution).
    pub fn register_kit(&mut self, name: &str, manifest: DataKitManifest) {
        self.kits.insert(name.to_string(), manifest);
    }

    /// Resolve an emitter name to a fresh `Box<dyn Emitter>`.
    pub fn resolve_emitter(&self, name: &str) -> Option<Box<dyn Emitter>> {
        self.emitters.get(name).map(|ctor| ctor())
    }

    /// Resolve a kit name to its manifest (for `extends`).
    pub fn resolve_kit(&self, name: &str) -> Option<&DataKitManifest> {
        self.kits.get(name)
    }

    /// Resolve a [`FragmentSourceSerde`] to a runtime [`FragmentSource`].
    ///
    /// `Emitter(name)` looks up the registered constructor; `DataFile(path)`
    /// is returned as-is (loading is deferred to assembly, where it currently
    /// returns [`LangError::NotImplemented`]).
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Engine`] if an emitter name is not registered
    /// (surfaced as a clear config-style error).
    pub fn resolve_source(
        &self,
        serde: &FragmentSourceSerde,
    ) -> Result<FragmentSource, LangError> {
        match serde {
            FragmentSourceSerde::Emitter(name) => {
                self.resolve_emitter(name)
                    .map(FragmentSource::Emitter)
                    .ok_or_else(|| {
                        LangError::Engine(format!("unknown emitter: {}", name))
                    })
            }
            FragmentSourceSerde::DataFile(path) => {
                Ok(FragmentSource::DataFile(path.clone()))
            }
        }
    }
}

/// The assembled data-driven kit: a merged [`Language`] plus the
/// [`RuntimeConfig`] that selects its engines.
///
/// Built by [`DataKit::assemble`] from a [`DataKitManifest`] and an
/// [`EmitterRegistry`]. Call [`DataKit::into_runtime`] to bind the language to
/// concrete engines via [`Runtime::from_config`].
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
    /// Assemble a [`DataKit`] from a manifest and an emitter registry.
    ///
    /// This is the core of ADR-0044 §1: "selection and merge are the
    /// composition." The pipeline is:
    ///
    /// 1. If the manifest declares `extends`, assemble the parent kit first
    ///    (recursively) and use its `Language` as the accumulator base.
    /// 2. For each fragment source entry (in declared order), resolve the
    ///    source to a [`FragmentSource`] and produce a [`Language`].
    /// 3. Iterate the produced language's fragments across all six fragment
    ///    sets; apply the include/exclude filter by fragment `id`; merge the
    ///    passing fragments into the accumulator in order (tokens, grammar,
    ///    eval, ast, caps, options — preserving `requires` dependencies).
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
    /// Returns [`LangError`] on: unknown emitter name, unknown `extends` kit,
    /// data-file source (not yet implemented), fragment merge failure
    /// (collision, unknown replace target, missing requires), or a broken
    /// override chain caused by an exclude.
    pub fn assemble(
        manifest: &DataKitManifest,
        registry: &EmitterRegistry,
    ) -> Result<Self, LangError> {
        // 1. Resolve the `extends` chain: assemble the parent first (one level
        //    supported in Phase 5).
        let mut lang = if let Some(parent_name) = &manifest.extends {
            let parent_manifest = registry.resolve_kit(parent_name).ok_or_else(|| {
                LangError::Engine(format!("unknown parent kit: {}", parent_name))
            })?;
            let parent = DataKit::assemble(parent_manifest, registry)?;
            parent.lang
        } else {
            Language::new()
        };

        // 2 + 3. Merge each fragment source's fragments, applying the filter.
        for entry in &manifest.fragments {
            let source = registry.resolve_source(&entry.source)?;
            let fragment_lang = produce_language(&source)?;
            merge_filtered(
                &mut lang,
                &fragment_lang,
                &manifest.include,
                &manifest.exclude,
            )?;
        }

        Ok(Self {
            lang,
            runtime_config: manifest.runtime.clone(),
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
    /// engine selection via [`Runtime::from_config`] (ADR §7).
    ///
    /// If `host` is `Some`, it is passed to the eval engine for
    /// [`handl_language::Scheme::Foreign`] dispatch.
    ///
    /// # Errors
    ///
    /// Returns [`LangError::Engine`] for any unknown engine name (e.g.
    /// `eval = "bytecode"` before Phase 6 lands).
    pub fn into_runtime<'h>(
        self,
        host: Option<&'h dyn HostFunctions>,
    ) -> Result<Runtime<'h>, LangError> {
        Runtime::from_config(self.lang, &self.runtime_config, host)
    }
}

/// Produce a [`Language`] from a resolved [`FragmentSource`].
///
/// Emitters call `.emit()`; data files are not yet implemented (ADR §6 end
/// state arrives after migration).
fn produce_language(source: &FragmentSource) -> Result<Language, LangError> {
    match source {
        FragmentSource::Emitter(emitter) => emitter.emit(),
        FragmentSource::DataFile(path) => {
            // Phase 5 models the type but does not implement file loading —
            // the emitters are the active path during migration. Surfacing a
            // clear error keeps the data-file branch honest.
            Err(LangError::Engine(format!(
                "data-file fragment sources are not implemented yet: {}",
                path.display()
            )))
        }
    }
}

/// Merge `fragment_lang`'s fragments into `lang`, applying the include/exclude
/// filter by fragment `id`.
///
/// Fragments are iterated in the order tokens → grammar → eval → ast → caps →
/// options, preserving the dependency ordering that `requires` validation
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
    for frag in fragment_lang.ast.fragments() {
        if passes(frag.id()) {
            lang.ast.merge(frag.clone())?;
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
    Ok(())
}

#[cfg(test)]
mod tests;
