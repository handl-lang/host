//! Named sdkgen *targets* — the link-time registry ADR-0128 puts behind
//! `hnl sdkgen <name>` (§"Registration: link-time first").
//!
//! A **target** is a data record — `name`, accepted flags, a backend id,
//! an emitter id, an optional doc pass, an optional non-backend spec
//! input — not a code path in the CLI extension. Modules in the target
//! repos (`c/modules/sdkgen-c`, `web/modules/sdkgen-webidl`,
//! `python/modules/sdkgen-python`, …) each export a `register` function
//! that populates a [`SdkgenRegistry`]; `plugin-ext-sdkgen` parses the
//! CLI and collapses to a lookup over it.
//!
//! The registry key is deliberately a *name*: a target composes a backend
//! and an emitter that may come from different repositories —
//! `hnl sdkgen python` is `backend: "c"` + `emitter: "python"`, so the
//! `python` repo depends on `c/`'s module at *resolution* time, never at
//! the crate level. A missing component is a link-time error message,
//! not an unresolved import.

use std::collections::HashMap;
use std::path::Path;

use handl_language::Language;

use crate::{Emitter, SdkgenBackend, SdkgenModule};

/// Everything a target's factories need from the parsed `hnl sdkgen` CLI.
///
/// Built by `plugin-ext-sdkgen` after flag parsing and `--from-url`
/// resolution; `spec` is always a local path by then.
pub struct SdkgenRequest<'a> {
    /// The resolved input spec/header path.
    pub spec: &'a Path,
    /// `--from-url` provenance, stamped on `module.source`.
    pub source_url: Option<&'a str>,
    /// `--output` directory.
    pub output: &'a Path,
    /// `--target` triple / backend-target selector.
    pub target: Option<&'a str>,
    /// `--include` glob list (only targets with `source_filters`).
    pub include: Option<&'a str>,
    /// `--exclude` glob list.
    pub exclude: Option<&'a str>,
    /// `--config` override file.
    pub config: Option<&'a Path>,
    /// The language emitted `.hnl` sources are canonicalized against —
    /// consumed by emitter factories such as `hnl-file`.
    pub lang: &'a Language,
}

/// A backend entry: `make` loads its own config and returns the parser
/// for one request (config-driven inputs like webidl's `includes:` are
/// wired inside `make`, before `parse_header` runs).
pub struct SdkgenBackendDef {
    /// The backend id target records reference (e.g. `"c"`, `"webidl"`).
    pub name: &'static str,
    /// Build the backend for this request.
    pub make: fn(&SdkgenRequest<'_>) -> Result<Box<dyn SdkgenBackend>, String>,
}

/// An emitter entry keyed by id (e.g. `"hnl-file"`, `"python"`).
pub struct SdkgenEmitterDef {
    /// The emitter id target records reference.
    pub name: &'static str,
    /// Build the emitter for this request.
    pub make: fn(&SdkgenRequest<'_>) -> Result<Box<dyn Emitter>, String>,
}

/// A non-backend input path on a target — e.g. the python target's
/// spec-driven (yaml/json) generation, which bypasses backend+emitter.
pub struct SdkgenSpecInput {
    /// Whether this input handles `req.spec` (checked before the backend
    /// pipeline runs).
    pub matches: fn(&Path) -> bool,
    /// Generate output directly from the request.
    pub run: fn(&SdkgenRequest<'_>) -> Result<(), String>,
}

/// A named `hnl sdkgen` target — the record a module registers.
pub struct SdkgenTarget {
    /// The name typed after `hnl sdkgen` (`c`, `webidl`, `python`, …).
    pub name: &'static str,
    /// Backend id resolved through the registry.
    pub backend: &'static str,
    /// Emitter id resolved through the registry.
    pub emitter: &'static str,
    /// Whether `--include`/`--exclude` source filters are accepted.
    pub source_filters: bool,
    /// Optional post-parse doc pass (doc-source layering). `python`'s
    /// header path deliberately sets `None` — the ctypes projection does
    /// not layer external docs.
    pub docs: Option<fn(&SdkgenRequest<'_>, &mut SdkgenModule) -> Result<(), String>>,
    /// Optional non-backend spec input, tried before the pipeline.
    pub spec_input: Option<SdkgenSpecInput>,
}

/// The link-time target registry: backends, emitters, and the target
/// records that compose them by name.
#[derive(Default)]
pub struct SdkgenRegistry {
    backends: HashMap<&'static str, SdkgenBackendDef>,
    emitters: HashMap<&'static str, SdkgenEmitterDef>,
    targets: HashMap<&'static str, SdkgenTarget>,
}

/// A `hnl sdkgen` failure, split by the exit code the CLI maps it to.
#[derive(Debug)]
pub enum SdkgenTargetError {
    /// CLI-level rejection (unknown target, unsupported flags) — exit 2.
    Usage(String),
    /// Runtime failure (config/backend/parse/emit) — exit 1.
    Failed(String),
}

impl SdkgenRegistry {
    /// Create an empty registry; modules `register` their entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a backend under its id.
    pub fn register_backend(&mut self, def: SdkgenBackendDef) {
        self.backends.insert(def.name, def);
    }

    /// Register an emitter under its id.
    pub fn register_emitter(&mut self, def: SdkgenEmitterDef) {
        self.emitters.insert(def.name, def);
    }

    /// Register a target record under its name.
    pub fn register_target(&mut self, target: SdkgenTarget) {
        self.targets.insert(target.name, target);
    }

    /// The target record named `name`, if linked in.
    pub fn target(&self, name: &str) -> Option<&SdkgenTarget> {
        self.targets.get(name)
    }

    /// All registered target names, sorted — the usage string source.
    pub fn target_names(&self) -> Vec<&'static str> {
        let mut names: Vec<_> = self.targets.keys().copied().collect();
        names.sort_unstable();
        names
    }

    /// Run target `name` over `req`: spec-input path first when it
    /// matches, else backend → parse → provenance → docs → emit.
    pub fn run(&self, name: &str, req: &SdkgenRequest<'_>) -> Result<(), SdkgenTargetError> {
        let target = self.targets.get(name).ok_or_else(|| {
            SdkgenTargetError::Usage(format!("unknown sdkgen target `{name}`"))
        })?;

        if !target.source_filters && (req.include.is_some() || req.exclude.is_some()) {
            return Err(SdkgenTargetError::Usage(format!(
                "--include/--exclude are not supported for `sdkgen {name}`"
            )));
        }

        if let Some(spec_input) = &target.spec_input
            && (spec_input.matches)(req.spec)
        {
            return (spec_input.run)(req).map_err(SdkgenTargetError::Failed);
        }

        let backend_def = self.backends.get(target.backend).ok_or_else(|| {
            SdkgenTargetError::Failed(format!(
                "sdkgen target `{name}` needs backend `{}`, which is not linked in",
                target.backend
            ))
        })?;
        let backend = (backend_def.make)(req).map_err(SdkgenTargetError::Failed)?;

        let mut module = backend
            .parse_header(req.spec)
            .map_err(|e| SdkgenTargetError::Failed(e.to_string()))?;
        module.source = req.source_url.map(str::to_string);
        if let Some(docs) = target.docs {
            docs(req, &mut module).map_err(SdkgenTargetError::Failed)?;
        }
        for d in &module.diagnostics {
            eprintln!("sdkgen: diagnostic: {d}");
        }

        let emitter_def = self.emitters.get(target.emitter).ok_or_else(|| {
            SdkgenTargetError::Failed(format!(
                "sdkgen target `{name}` needs emitter `{}`, which is not linked in",
                target.emitter
            ))
        })?;
        let emitter = (emitter_def.make)(req).map_err(SdkgenTargetError::Failed)?;
        emitter
            .emit(&module, req.output)
            .map_err(|e| SdkgenTargetError::Failed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SdkgenError, SdkgenModule};

    struct StubBackend;
    impl SdkgenBackend for StubBackend {
        fn parse_header(&self, _path: &Path) -> Result<SdkgenModule, SdkgenError> {
            Ok(SdkgenModule::default())
        }
    }

    struct RecordingEmitter;
    impl Emitter for RecordingEmitter {
        fn emit(&self, _module: &SdkgenModule, _output: &Path) -> Result<(), SdkgenError> {
            Ok(())
        }
    }

    fn registry(lang: &Language) -> (SdkgenRegistry, SdkgenRequest<'_>) {
        let mut reg = SdkgenRegistry::new();
        reg.register_backend(SdkgenBackendDef {
            name: "b",
            make: |_| Ok(Box::new(StubBackend)),
        });
        reg.register_emitter(SdkgenEmitterDef {
            name: "e",
            make: |_| Ok(Box::new(RecordingEmitter)),
        });
        reg.register_target(SdkgenTarget {
            name: "t",
            backend: "b",
            emitter: "e",
            source_filters: true,
            docs: None,
            spec_input: None,
        });
        let req = SdkgenRequest {
            spec: Path::new("spec.h"),
            source_url: None,
            output: Path::new("out"),
            target: None,
            include: None,
            exclude: None,
            config: None,
            lang,
        };
        (reg, req)
    }

    #[test]
    fn run_resolves_backend_and_emitter_by_name() {
        let lang = Language::default();
        let (reg, req) = registry(&lang);
        assert!(reg.run("t", &req).is_ok());
    }

    #[test]
    fn unknown_target_is_usage() {
        let lang = Language::default();
        let (reg, req) = registry(&lang);
        assert!(matches!(
            reg.run("nope", &req),
            Err(SdkgenTargetError::Usage(_))
        ));
    }

    #[test]
    fn missing_backend_is_a_link_error() {
        let lang = Language::default();
        let (mut reg, req) = registry(&lang);
        reg.register_target(SdkgenTarget {
            name: "half",
            backend: "absent",
            emitter: "e",
            source_filters: false,
            docs: None,
            spec_input: None,
        });
        assert!(matches!(
            reg.run("half", &req),
            Err(SdkgenTargetError::Failed(_))
        ));
    }

    #[test]
    fn source_filter_flags_are_rejected_per_target() {
        let lang = Language::default();
        let (mut reg, mut req) = registry(&lang);
        reg.register_target(SdkgenTarget {
            name: "nf",
            backend: "b",
            emitter: "e",
            source_filters: false,
            docs: None,
            spec_input: None,
        });
        req.include = Some("*");
        assert!(matches!(
            reg.run("nf", &req),
            Err(SdkgenTargetError::Usage(_))
        ));
    }

    #[test]
    fn spec_input_bypasses_the_backend() {
        let lang = Language::default();
        let (mut reg, req) = registry(&lang);
        reg.register_target(SdkgenTarget {
            name: "spec",
            backend: "absent", // never resolved — spec input wins
            emitter: "absent",
            source_filters: false,
            docs: None,
            spec_input: Some(SdkgenSpecInput {
                matches: |p| p.extension().and_then(|e| e.to_str()) == Some("spec"),
                run: |_| Ok(()),
            }),
        });
        let mut req = req;
        req.spec = Path::new("in.spec");
        assert!(reg.run("spec", &req).is_ok());
    }
}
