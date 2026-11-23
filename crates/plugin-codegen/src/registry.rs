//! Named codegen *targets* — the link-time registry ADR-0128 puts
//! behind `hnl codegen <name>` (§"Registration: link-time first"), now
//! driving the desired-state lifecycle kernel (P-001 / PL-004).
//!
//! A **target** is a data record — `name`, accepted flags, a source id,
//! an emitter id, an optional doc pass, an optional non-backend spec
//! input — not a code path in the CLI extension. Modules in the target
//! repos (`c/modules/codegen-c`, `web/modules/codegen-webidl`,
//! `python/modules/codegen-python`, …) each export a `register` function
//! that populates a [`CodegenRegistry`]; `plugin-ext-codegen` parses the
//! CLI and collapses to a lookup over it.
//!
//! The registry key is deliberately a *name*: a target composes a
//! source and an emitter that may come from different repositories —
//! `hnl codegen python` is `source: "c"` + `emitter: "python"`, so the
//! `python` repo depends on `c/`'s module at *resolution* time, never at
//! the crate level. A missing component is a link-time error message,
//! not an unresolved import.
//!
//! # Lifecycle
//!
//! [`CodegenRegistry::run`] is the `Reconcile` stage of the pipeline:
//! `Source::parse` lifts the spec into IR (pure), the optional `docs`
//! pass layers external documentation, `Emitter::materialize` renders
//! the desired [`Change`] set (pure — no fs writes), `Emitter::verify`
//! is the honest-decline gate, and `Engine::apply` over an
//! [`FsStore`] rooted at `req.output` converges the store — manifest +
//! journal under `.handl/codegen/`, drift-checked and reversible.
//!
//! Sources and emitters are stored **type-erased** ([`AnySource`] /
//! [`AnyEmitter`]) so one registry serves every IR family
//! ([`SdkgenModule`], [`SpecModule`], …). Pairing is compile-time via
//! [`CodegenRegistry::register_typed`]; the name-based record path
//! resolves ids at run time exactly like the old `SdkgenRegistry`.

use std::any::Any;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::Path;

use handl_language::Language;

use hnl_engine_codegen::{
    ApplyOptions, ArtifactStore, Change, CodegenError, EmitContext, Emitter, Engine, FsStore,
    IrKind, IrModule, Manifest, RunMeta, Source,
};

use crate::{SdkgenModule, SpecModule};

/// Everything a target's factories need from the parsed `hnl codegen`
/// CLI.
///
/// Built by `plugin-ext-codegen` after flag parsing and `--from-url`
/// resolution; `spec` is always a local path by then.
pub struct CodegenRequest<'a> {
    /// The resolved input spec/header path.
    pub spec: &'a Path,
    /// `--from-url`/`--source-url` provenance. `Source::parse` stamps it
    /// on `module.source` (the registry only back-fills when a source
    /// forgot — P-001 inventory §4).
    pub source_url: Option<&'a str>,
    /// `--output` directory — the [`FsStore`] root.
    pub output: &'a Path,
    /// `--target` triple / backend-target selector.
    pub target: Option<&'a str>,
    /// `--include` glob list (only targets with `source_filters`).
    pub include: Option<&'a str>,
    /// `--exclude` glob list.
    pub exclude: Option<&'a str>,
    /// `--config` override file.
    pub config: Option<&'a Path>,
    /// `--emit-spec <path|->` — a backend hint to dump the resolved
    /// spec after input processing. The `cli` backend folds it into
    /// its `CliConfig.emit_spec` after the config loads (the flag
    /// wins); `-` writes the spec to stdout. Backends without an
    /// emit-spec concept ignore it.
    pub emit_spec: Option<&'a Path>,
    /// The language emitted `.hnl` sources are canonicalized against —
    /// consumed by emitter factories such as `hnl-file` (cloned into an
    /// `Arc` at `make` time; it never crosses the kernel boundary).
    pub lang: &'a Language,
    /// Cache directory for doc/spec caches (e.g. webidl's MDN
    /// `.mdn-cache.json`) — request-provided, deliberately *outside*
    /// the output tree (P-001 inventory §3 side-effect policy).
    pub cache_dir: Option<&'a Path>,
}

/// Adapt a [`CodegenRequest`] into the kernel-side [`EmitContext`]
/// passed to `Source::parse` / `Emitter::materialize` / `verify`.
///
/// `spec`, `output`, `source_url`, and `cache_dir` map to typed fields;
/// the remaining request flags (`target`, `include`, `exclude`,
/// `config`, `emit-spec`) travel as `config` entries — sources that
/// need them read them back by key. `language` stays unset: `Language`
/// has no name and emitters capture `Arc<Language>` at `make` time.
pub fn emit_context(req: &CodegenRequest<'_>) -> EmitContext {
    let mut ctx = EmitContext::new()
        .with_output_dir(req.output)
        .with_spec_path(req.spec);
    if let Some(url) = req.source_url {
        ctx = ctx.with_source_url(url);
    }
    if let Some(dir) = req.cache_dir {
        ctx = ctx.with_cache_dir(dir);
    }
    for (key, value) in [
        ("target", req.target),
        ("include", req.include),
        ("exclude", req.exclude),
    ] {
        if let Some(v) = value {
            ctx = ctx.with_config(key, v);
        }
    }
    if let Some(p) = req.config {
        ctx = ctx.with_config("config", p.display().to_string());
    }
    if let Some(p) = req.emit_spec {
        ctx = ctx.with_config("emit-spec", p.display().to_string());
    }
    ctx
}

// ------------------------------------------------------------------
// Type erasure — `AnySource` / `AnyEmitter` / `ErasedModule`
// ------------------------------------------------------------------

/// An IR module with its type erased for registry storage. `kind` is
/// the [`IrModule::kind`] tag; `downcast_ref`/`into_inner` recover the
/// concrete IR for docs passes and typed emitters.
pub struct ErasedModule {
    kind: IrKind,
    inner: Box<dyn Any>,
}

impl ErasedModule {
    fn new<I: IrModule + 'static>(module: I) -> Self {
        Self {
            kind: module.kind(),
            inner: Box::new(module),
        }
    }

    /// The IR family tag (`"sdkgen"`, `"spec"`, …).
    pub fn kind(&self) -> &IrKind {
        &self.kind
    }

    /// Borrow the concrete IR as `I`, or `None` on family mismatch.
    pub fn downcast_ref<I: 'static>(&self) -> Option<&I> {
        self.inner.downcast_ref()
    }

    /// Mutably borrow the concrete IR as `I` (provenance stamping).
    pub fn downcast_mut<I: 'static>(&mut self) -> Option<&mut I> {
        self.inner.downcast_mut()
    }

    /// Consume into the concrete IR `I`.
    pub fn into_inner<I: 'static>(self) -> Result<I, CodegenError> {
        let kind = self.kind.clone();
        self.inner
            .downcast::<I>()
            .map(|b| *b)
            .map_err(|_| CodegenError::Emit(format!("IR kind mismatch: {}", kind.as_str())))
    }
}

/// Type-erased [`Source<I>`] — the registry stores these under source
/// ids; `parse_erased` lifts `EmitContext` into an [`ErasedModule`].
pub trait AnySource {
    fn parse_erased(&self, req: &EmitContext) -> Result<ErasedModule, CodegenError>;
}

/// Type-erased [`Emitter<I>`] — `materialize_erased` downcasts the IR
/// and returns the desired [`Change`] set; `verify_erased` is the
/// honest-decline gate (default: accept).
pub trait AnyEmitter {
    fn materialize_erased(
        &self,
        ir: &ErasedModule,
        req: &EmitContext,
    ) -> Result<Vec<Change>, CodegenError>;

    fn verify_erased(&self, ir: &ErasedModule, changes: &[Change]) -> Result<(), CodegenError>;
}

struct SourceAdapter<S, I> {
    source: S,
    _ir: PhantomData<fn() -> I>,
}

impl<S, I> AnySource for SourceAdapter<S, I>
where
    S: Source<I>,
    I: IrModule + 'static,
{
    fn parse_erased(&self, req: &EmitContext) -> Result<ErasedModule, CodegenError> {
        Ok(ErasedModule::new(self.source.parse(req)?))
    }
}

struct EmitterAdapter<E, I> {
    emitter: E,
    _ir: PhantomData<fn() -> I>,
}

impl<E, I> AnyEmitter for EmitterAdapter<E, I>
where
    E: Emitter<I>,
    I: IrModule + 'static,
{
    fn materialize_erased(
        &self,
        ir: &ErasedModule,
        req: &EmitContext,
    ) -> Result<Vec<Change>, CodegenError> {
        let module = ir.downcast_ref::<I>().ok_or_else(|| {
            CodegenError::Emit(format!("IR kind mismatch: {}", ir.kind().as_str()))
        })?;
        self.emitter.materialize(module, req)
    }

    fn verify_erased(&self, ir: &ErasedModule, changes: &[Change]) -> Result<(), CodegenError> {
        let module = ir.downcast_ref::<I>().ok_or_else(|| {
            CodegenError::Emit(format!("IR kind mismatch: {}", ir.kind().as_str()))
        })?;
        self.emitter.verify(module, changes)
    }
}

/// Box a concrete [`Source<I>`] for registry storage.
pub fn erase_source<S, I>(source: S) -> Box<dyn AnySource>
where
    S: Source<I> + 'static,
    I: IrModule + 'static,
{
    Box::new(SourceAdapter {
        source,
        _ir: PhantomData,
    })
}

/// Box a concrete [`Emitter<I>`] for registry storage.
pub fn erase_emitter<E, I>(emitter: E) -> Box<dyn AnyEmitter>
where
    E: Emitter<I> + 'static,
    I: IrModule + 'static,
{
    Box::new(EmitterAdapter {
        emitter,
        _ir: PhantomData,
    })
}

// ------------------------------------------------------------------
// Registry records
// ------------------------------------------------------------------

/// A source entry: `make` builds the [`Source`] for one request
/// (config-driven inputs like webidl's `includes:` are wired inside
/// `make`, before `parse` runs).
pub struct CodegenSourceDef {
    /// The source id target records reference (e.g. `"c"`, `"webidl"`).
    pub name: &'static str,
    /// Build the source for this request.
    pub make: fn(&CodegenRequest<'_>) -> Result<Box<dyn AnySource>, String>,
}

/// An emitter entry keyed by id (e.g. `"hnl-file"`, `"python"`).
pub struct CodegenEmitterDef {
    /// The emitter id target records reference.
    pub name: &'static str,
    /// Build the emitter for this request.
    pub make: fn(&CodegenRequest<'_>) -> Result<Box<dyn AnyEmitter>, String>,
}

/// A non-backend input path on a target — e.g. the python target's
/// spec-driven (yaml/json) generation.
///
/// **Transitional** (P-001 inventory §2): this hook bypasses the
/// `Source`/`Emitter` pipeline and still writes files directly. New
/// spec inputs should register a typed [`SpecModule`] target via
/// [`CodegenRegistry::register_typed`] (or `source`/`emitter` ids) once
/// `engine-codegen-python` wires its renderer — at which point this
/// variant is deleted.
pub struct CodegenSpecInput {
    /// Whether this input handles `req.spec` (checked before the backend
    /// pipeline runs).
    pub matches: fn(&Path) -> bool,
    /// Generate output directly from the request (legacy emit-to-disk).
    pub run: fn(&CodegenRequest<'_>) -> Result<(), String>,
}

/// A named `hnl codegen` target — the record a module registers.
pub struct CodegenTarget {
    /// The name typed after `hnl codegen` (`c`, `webidl`, `python`, …).
    pub name: &'static str,
    /// Source id resolved through the registry.
    pub source: &'static str,
    /// Emitter id resolved through the registry.
    pub emitter: &'static str,
    /// Whether `--include`/`--exclude` source filters are accepted.
    pub source_filters: bool,
    /// Optional post-parse doc pass (doc-source layering). `python`'s
    /// header path deliberately sets `None` — the ctypes projection does
    /// not layer external docs. Applies to [`SdkgenModule`] IR only.
    pub docs: Option<fn(&CodegenRequest<'_>, &mut SdkgenModule) -> Result<(), String>>,
    /// Optional non-backend spec input, tried before the pipeline.
    pub spec_input: Option<CodegenSpecInput>,
}

/// A self-contained target registered with concrete, compile-time-paired
/// `Source<I>`/`Emitter<I>` — the P-001 `register_target<S,E,I>`
/// mechanic: a source `I` that doesn't match its emitter `I` is a
/// compile error, and dispatch downcasts once per run.
pub struct CodegenTypedTarget {
    source: Box<dyn AnySource>,
    emitter: Box<dyn AnyEmitter>,
}

/// The link-time target registry: sources, emitters, and the target
/// records that compose them by name — plus typed self-contained
/// targets.
#[derive(Default)]
pub struct CodegenRegistry {
    sources: HashMap<&'static str, CodegenSourceDef>,
    emitters: HashMap<&'static str, CodegenEmitterDef>,
    targets: HashMap<&'static str, CodegenTarget>,
    typed: HashMap<&'static str, CodegenTypedTarget>,
}

/// A `hnl codegen` failure, split by the exit code the CLI maps it to.
#[derive(Debug)]
pub enum CodegenTargetError {
    /// CLI-level rejection (unknown target, unsupported flags) — exit 2.
    Usage(String),
    /// Runtime failure (config/source/parse/materialize/verify/apply)
    /// — exit 1.
    Failed(String),
}

impl CodegenRegistry {
    /// Create an empty registry; modules `register` their entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a source under its id.
    pub fn register_source(&mut self, def: CodegenSourceDef) {
        self.sources.insert(def.name, def);
    }

    /// Register an emitter under its id.
    pub fn register_emitter(&mut self, def: CodegenEmitterDef) {
        self.emitters.insert(def.name, def);
    }

    /// Register a target record under its name.
    pub fn register_target(&mut self, target: CodegenTarget) {
        self.targets.insert(target.name, target);
    }

    /// Register a self-contained typed target: `S: Source<I>` paired
    /// with `E: Emitter<I>` — mismatched IRs are a compile error.
    /// Typed targets carry no `source_filters`/`docs`/`spec_input`
    /// record features; use a [`CodegenTarget`] record for those.
    pub fn register_typed<S, E, I>(&mut self, name: &'static str, source: S, emitter: E)
    where
        S: Source<I> + 'static,
        E: Emitter<I> + 'static,
        I: IrModule + 'static,
    {
        self.typed.insert(
            name,
            CodegenTypedTarget {
                source: erase_source(source),
                emitter: erase_emitter(emitter),
            },
        );
    }

    /// The target record named `name`, if linked in (typed targets are
    /// not records — see [`Self::contains`]).
    pub fn target(&self, name: &str) -> Option<&CodegenTarget> {
        self.targets.get(name)
    }

    /// Whether `name` resolves to any registered target (record or
    /// typed).
    pub fn contains(&self, name: &str) -> bool {
        self.targets.contains_key(name) || self.typed.contains_key(name)
    }

    /// All registered target names, sorted — the usage string source.
    pub fn target_names(&self) -> Vec<&'static str> {
        let mut names: Vec<_> = self
            .targets
            .keys()
            .chain(self.typed.keys())
            .copied()
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Run target `name` over `req`: typed targets dispatch directly;
    /// record targets run spec-input (transitional) → source parse →
    /// provenance → docs → materialize → verify → kernel apply.
    pub fn run(&self, name: &str, req: &CodegenRequest<'_>) -> Result<(), CodegenTargetError> {
        if let Some(typed) = self.typed.get(name) {
            return self.run_typed(name, typed, req);
        }
        let target = self
            .targets
            .get(name)
            .ok_or_else(|| CodegenTargetError::Usage(format!("unknown codegen target `{name}`")))?;

        if !target.source_filters && (req.include.is_some() || req.exclude.is_some()) {
            return Err(CodegenTargetError::Usage(format!(
                "--include/--exclude are not supported for `codegen {name}`"
            )));
        }

        if let Some(spec_input) = &target.spec_input
            && (spec_input.matches)(req.spec)
        {
            return (spec_input.run)(req).map_err(CodegenTargetError::Failed);
        }

        let source_def = self.sources.get(target.source).ok_or_else(|| {
            CodegenTargetError::Failed(format!(
                "codegen target `{name}` needs source `{}`, which is not linked in",
                target.source
            ))
        })?;
        let source = (source_def.make)(req).map_err(CodegenTargetError::Failed)?;

        let ctx = emit_context(req);
        let mut module = source
            .parse_erased(&ctx)
            .map_err(|e| CodegenTargetError::Failed(e.to_string()))?;
        // Provenance back-fill: `Source::parse` impls are expected to
        // stamp `module.source` from `EmitContext::source_url`
        // (P-001 inventory §4); this only covers sources that forgot.
        if let Some(m) = module.downcast_mut::<SdkgenModule>()
            && m.source.is_none()
        {
            m.source = req.source_url.map(str::to_string);
        }
        if let Some(docs) = target.docs {
            let kind = module.kind().as_str().to_string();
            let m = module.downcast_mut::<SdkgenModule>().ok_or_else(|| {
                CodegenTargetError::Failed(format!(
                    "codegen target `{name}` has a docs pass but the source produced non-sdkgen IR ({kind})"
                ))
            })?;
            docs(req, m).map_err(CodegenTargetError::Failed)?;
        }
        emit_diagnostics(&module);

        let emitter_def = self.emitters.get(target.emitter).ok_or_else(|| {
            CodegenTargetError::Failed(format!(
                "codegen target `{name}` needs emitter `{}`, which is not linked in",
                target.emitter
            ))
        })?;
        let emitter = (emitter_def.make)(req).map_err(CodegenTargetError::Failed)?;
        let changes = emitter
            .materialize_erased(&module, &ctx)
            .map_err(|e| CodegenTargetError::Failed(e.to_string()))?;
        emitter
            .verify_erased(&module, &changes)
            .map_err(|e| CodegenTargetError::Failed(e.to_string()))?;

        self.apply(name, req, changes)
    }

    /// Typed-target path: parse → materialize → verify → apply, with no
    /// record features (filters/docs/spec_input).
    fn run_typed(
        &self,
        name: &str,
        typed: &CodegenTypedTarget,
        req: &CodegenRequest<'_>,
    ) -> Result<(), CodegenTargetError> {
        let ctx = emit_context(req);
        let mut module = typed
            .source
            .parse_erased(&ctx)
            .map_err(|e| CodegenTargetError::Failed(e.to_string()))?;
        if let Some(m) = module.downcast_mut::<SdkgenModule>()
            && m.source.is_none()
        {
            m.source = req.source_url.map(str::to_string);
        }
        emit_diagnostics(&module);
        let changes = typed
            .emitter
            .materialize_erased(&module, &ctx)
            .map_err(|e| CodegenTargetError::Failed(e.to_string()))?;
        typed
            .emitter
            .verify_erased(&module, &changes)
            .map_err(|e| CodegenTargetError::Failed(e.to_string()))?;
        self.apply(name, req, changes)
    }

    /// Reconcile the desired `Change` set against `req.output` through
    /// the kernel: `FsStore` rooted at the output dir, prior manifest
    /// read for owned-artifact deletes, `Engine::apply` with manifest +
    /// journal under `.handl/codegen/`.
    fn apply(
        &self,
        name: &str,
        req: &CodegenRequest<'_>,
        changes: Vec<Change>,
    ) -> Result<(), CodegenTargetError> {
        let store = FsStore::new(req.output);
        let engine = Engine::new(&store);

        // Prior manifest supplies deletes for artifacts this target
        // owned last run but the emitter no longer produces.
        let prior = read_prior_manifest(&store, name);

        let mut meta = RunMeta::new(name, env!("CARGO_PKG_VERSION"));
        meta.new_run_id();
        // Record the spec's content hash as run provenance (the spec
        // is the lift input; fetched specs are already local files).
        if let Ok(bytes) = std::fs::read(req.spec) {
            meta.source_hashes.insert(
                req.spec.display().to_string(),
                hnl_engine_codegen::ContentHash::of(&bytes),
            );
        }

        let report = engine
            .apply(&changes, prior.as_ref(), &meta, &ApplyOptions::default())
            .map_err(|e| CodegenTargetError::Failed(e.to_string()))?;
        if report.clean() {
            return Ok(());
        }
        Err(CodegenTargetError::Failed(format!(
            "codegen apply for `{name}` incomplete: {} created, {} updated, {} deleted, \
             {} unchanged, {} skipped, {} failed ({})",
            report.created,
            report.updated,
            report.deleted,
            report.unchanged,
            report.skipped,
            report.failures.len(),
            report
                .failures
                .iter()
                .map(|(a, e)| format!("{a}: {e}"))
                .collect::<Vec<_>>()
                .join("; ")
        )))
    }
}

/// Read the prior manifest for `target` from the store, if present and
/// decodable. A corrupt manifest is treated as absent — apply then
/// owns only what it writes this run (destroy stays manifest-driven).
fn read_prior_manifest(store: &FsStore, target: &str) -> Option<Manifest> {
    let bytes = store.read(&Manifest::artifact_ref(target)).ok()??;
    serde_json::from_slice(&bytes).ok()
}

/// Surface per-family diagnostics on stderr — the observable behavior
/// `SdkgenRegistry::run` had (`sdkgen: diagnostic: …`). Foreign IR
/// families extend this match when they register.
fn emit_diagnostics(module: &ErasedModule) {
    if let Some(m) = module.downcast_ref::<SdkgenModule>() {
        for d in &m.diagnostics {
            eprintln!("codegen: diagnostic: {d}");
        }
    } else if let Some(m) = module.downcast_ref::<SpecModule>() {
        for d in &m.diagnostics {
            eprintln!("codegen: diagnostic: {d}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubSource;
    impl Source<SdkgenModule> for StubSource {
        fn parse(&self, req: &EmitContext) -> Result<SdkgenModule, CodegenError> {
            let mut m = SdkgenModule::default();
            m.module = "stub".to_string();
            m.source = req.source_url.clone();
            Ok(m)
        }
    }

    struct StubEmitter;
    impl Emitter<SdkgenModule> for StubEmitter {
        fn materialize(
            &self,
            _module: &SdkgenModule,
            _req: &EmitContext,
        ) -> Result<Vec<Change>, CodegenError> {
            Ok(vec![
                Change::text("ports/stub.hnl", "port stub"),
                Change::text("adapters/stub.hnl", "adapter stub"),
            ])
        }
    }

    struct SpecStubSource;
    impl Source<SpecModule> for SpecStubSource {
        fn parse(&self, req: &EmitContext) -> Result<SpecModule, CodegenError> {
            SpecModule::load(req.spec_path.as_deref().unwrap(), req.source_url.clone())
        }
    }

    struct SpecStubEmitter;
    impl Emitter<SpecModule> for SpecStubEmitter {
        fn materialize(
            &self,
            module: &SpecModule,
            _req: &EmitContext,
        ) -> Result<Vec<Change>, CodegenError> {
            Ok(vec![Change::text(
                format!("{}.hnl", module.name),
                format!("spec {}", module.format),
            )])
        }
    }

    fn lang() -> Language {
        Language::default()
    }

    fn request<'a>(spec: &'a Path, output: &'a Path, lang: &'a Language) -> CodegenRequest<'a> {
        CodegenRequest {
            spec,
            source_url: None,
            output,
            target: None,
            include: None,
            exclude: None,
            config: None,
            emit_spec: None,
            lang,
            cache_dir: None,
        }
    }

    fn registry(lang: &Language) -> CodegenRegistry {
        let mut reg = CodegenRegistry::new();
        reg.register_source(CodegenSourceDef {
            name: "b",
            make: |_| Ok(erase_source(StubSource)),
        });
        reg.register_emitter(CodegenEmitterDef {
            name: "e",
            make: |_| Ok(erase_emitter(StubEmitter)),
        });
        reg.register_target(CodegenTarget {
            name: "t",
            source: "b",
            emitter: "e",
            source_filters: true,
            docs: None,
            spec_input: None,
        });
        let _ = lang;
        reg
    }

    #[test]
    fn run_applies_changes_through_the_kernel() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        let spec = dir.path().join("spec.h");
        std::fs::write(&spec, "int x;").unwrap();
        let lang = lang();
        let reg = registry(&lang);
        let req = request(&spec, &out, &lang);

        assert!(reg.run("t", &req).is_ok());
        assert_eq!(
            std::fs::read_to_string(out.join("ports/stub.hnl")).unwrap(),
            "port stub"
        );
        assert_eq!(
            std::fs::read_to_string(out.join("adapters/stub.hnl")).unwrap(),
            "adapter stub"
        );
        // Manifest + journal landed under .handl/codegen/.
        assert!(out.join(".handl/codegen/manifest/t.json").exists());
        assert!(out.join(".handl/codegen/journal").exists());

        // Second run converges to unchanged — no error, same bytes.
        assert!(reg.run("t", &req).is_ok());
    }

    #[test]
    fn unknown_target_is_usage() {
        let lang = lang();
        let reg = registry(&lang);
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("s.h");
        std::fs::write(&spec, "").unwrap();
        let req = request(&spec, dir.path(), &lang);
        assert!(matches!(
            reg.run("nope", &req),
            Err(CodegenTargetError::Usage(_))
        ));
    }

    #[test]
    fn missing_source_is_a_link_error() {
        let lang = lang();
        let mut reg = registry(&lang);
        reg.register_target(CodegenTarget {
            name: "half",
            source: "absent",
            emitter: "e",
            source_filters: false,
            docs: None,
            spec_input: None,
        });
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("s.h");
        std::fs::write(&spec, "").unwrap();
        let req = request(&spec, dir.path(), &lang);
        assert!(matches!(
            reg.run("half", &req),
            Err(CodegenTargetError::Failed(_))
        ));
    }

    #[test]
    fn source_filter_flags_are_rejected_per_target() {
        let lang = lang();
        let mut reg = registry(&lang);
        reg.register_target(CodegenTarget {
            name: "nf",
            source: "b",
            emitter: "e",
            source_filters: false,
            docs: None,
            spec_input: None,
        });
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("s.h");
        std::fs::write(&spec, "").unwrap();
        let mut req = request(&spec, dir.path(), &lang);
        req.include = Some("*");
        assert!(matches!(
            reg.run("nf", &req),
            Err(CodegenTargetError::Usage(_))
        ));
    }

    #[test]
    fn spec_input_bypasses_the_pipeline() {
        let lang = lang();
        let mut reg = registry(&lang);
        reg.register_target(CodegenTarget {
            name: "spec",
            source: "absent", // never resolved — spec input wins
            emitter: "absent",
            source_filters: false,
            docs: None,
            spec_input: Some(CodegenSpecInput {
                matches: |p| p.extension().and_then(|e| e.to_str()) == Some("spec"),
                run: |_| Ok(()),
            }),
        });
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("in.spec");
        std::fs::write(&spec, "").unwrap();
        let req = request(&spec, dir.path(), &lang);
        assert!(reg.run("spec", &req).is_ok());
    }

    #[test]
    fn typed_target_pairs_source_and_emitter_at_compile_time() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("demo.yaml");
        std::fs::write(&spec, "name: demo\n").unwrap();
        let out = dir.path().join("out");
        let lang = lang();
        let mut reg = CodegenRegistry::new();
        reg.register_typed("spec-yaml", SpecStubSource, SpecStubEmitter);
        assert!(reg.contains("spec-yaml"));
        assert!(reg.target_names().contains(&"spec-yaml"));
        let req = request(&spec, &out, &lang);
        assert!(reg.run("spec-yaml", &req).is_ok());
        assert_eq!(
            std::fs::read_to_string(out.join("demo.hnl")).unwrap(),
            "spec yaml"
        );
    }
}
