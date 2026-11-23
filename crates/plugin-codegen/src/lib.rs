//! Handl codegen core: IR families, the `CodegenRegistry`, and the
//! desired-state lifecycle wiring (P-001 / PL-004 — renamed from
//! `plugin-sdkgen`, ADR-0107/ADR-0128).
//!
//! The pipeline is the org-wide `Lift → Transform → Materialize →
//! [Verify] → Reconcile` shape: [`Source<I>`] parses an input spec into
//! a typed [`IrModule`], an [`Emitter<I>`] renders it to a desired
//! [`Change`] set with **no side effects**, and
//! [`CodegenRegistry::run`] reconciles that set against the output
//! directory through the `hnl-tool-codegen` kernel (`FsStore` +
//! `Engine::apply` — manifest + journal under `.handl/codegen/`).
//!
//! `sdkgen` survives only as the *domain/IR* name — [`SdkgenModule`],
//! backend ids (`c`, `webidl`, `cli`), emitter ids (`hnl-file`,
//! `python`) — per the P-001 naming map; the command surface is
//! `codegen`.

mod registry;

pub use registry::{
    AnyEmitter, AnySource, CodegenEmitterDef, CodegenRegistry, CodegenRequest, CodegenSourceDef,
    CodegenTarget, CodegenTargetError, CodegenTypedTarget, ErasedModule, emit_context,
    erase_emitter, erase_source,
};

// The kernel surface registrants implement against — re-exported so
// target modules only depend on this crate.
pub use hnl_tool_codegen::{
    ApplyOptions, ApplyReport, ArtifactRef, ArtifactStat, Change, CodegenError, ContentHash,
    EmitContext, Emitter, Engine, FsStore, IrKind, IrModule, Manifest, MemoryStore, OnError, Plan,
    PlanAction, PlanEntry, RevertReport, RunMeta, Source, StoreError,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Platform-agnostic SDK module produced by a [`Source`] and consumed
/// by an [`Emitter`] — the foreign-binding IR family (kept under the
/// `Sdkgen` name: it names the domain, not the command surface).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SdkgenModule {
    pub client_package: String,
    pub backend_id: String,
    pub target: String,
    pub library: String,
    pub module: String,
    pub records: Vec<Record>,
    pub functions: Vec<Function>,
    pub typedefs: Vec<Typedef>,
    pub enum_constants: Vec<EnumConst>,
    /// Additional packages the emitted port must import — declared by the
    /// backend as data so emitters stay free of backend vocabularies
    /// (e.g. the C backend's `types/c` aliases).
    #[serde(default)]
    pub imports: Vec<ModuleImport>,
    /// Provenance of the input spec — e.g. the URL passed to
    /// `hnl codegen --from-url`. Stamped by `Source::parse` from
    /// `EmitContext::source_url` (P-001 inventory §4) and rendered as a
    /// `--- source:` header line so committed generated files record
    /// the pin they were produced from.
    #[serde(default)]
    pub source: Option<String>,
    /// File layout hint for emitters that support it: when set, the
    /// module is written as one `<Record>.hnl` port/adapter pair per
    /// record plus a `types.hnl` for typedefs, enum constants, and
    /// functions not owned by a record. Keeping each generated file
    /// small keeps it under typechecker node budgets; the C backend
    /// leaves this unset because its records reference each other by
    /// name and must share one module scope.
    #[serde(default)]
    pub per_record_files: bool,
    /// Resolved-spec dump requested via `--emit-spec`/`emit_spec:` —
    /// the cli backend's "probe then hand-tune" payload (PL-004:
    /// `Source::parse` must not write it, so it travels on the IR and
    /// [`CodegenRegistry::run`] writes it as a request-level side
    /// effect, like `--from-url` fetches).
    #[serde(default)]
    pub spec_dump: Option<SpecDump>,
    pub diagnostics: Vec<String>,
}

/// A resolved input-spec dump the source asks the registry to surface:
/// `yaml` is the canonical serialization, `dest` the requested output
/// (`"-"` = stdout, anything else a filesystem path — deliberately
/// *not* an [`ArtifactRef`]: emit-spec destinations are arbitrary
/// request-level paths outside the store root).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecDump {
    /// Canonical spec text (e.g. `CliSpec::dump()`).
    pub yaml: String,
    /// Requested destination: a path, or `"-"` for stdout.
    pub dest: String,
}

impl IrModule for SdkgenModule {
    fn kind(&self) -> IrKind {
        IrKind::new("sdkgen")
    }
}

/// IR for the spec-input path (P-001 inventory §2): a parsed yaml/json
/// spec document — the python target's spec-driven generation — so the
/// registry's spec hook becomes a normal `Source`/`Emitter` pairing
/// instead of an IR-free bypass. The `hnl` renderer lives in
/// `tool-codegen-python` (`handl-plugin-codegen-emitter-python`); this
/// crate defines the IR + [`Source`]/[`Emitter`] surface only.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpecModule {
    /// Spec format tag: `"yaml"` or `"json"`.
    pub format: String,
    /// Package/target name derived from the spec (file stem).
    pub name: String,
    /// The parsed spec document. YAML is normalised to a JSON value at
    /// parse time so emitters see one shape regardless of format.
    pub document: serde_json::Value,
    /// Provenance (spec path / `--from-url` / `--source-url`).
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

impl IrModule for SpecModule {
    fn kind(&self) -> IrKind {
        IrKind::new("spec")
    }
}

impl SpecModule {
    /// Detect the spec format from a path's extension
    /// (`.yaml`/`.yml`/`.json`); `None` for anything else.
    pub fn format_for(path: &Path) -> Option<&'static str> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("yaml" | "yml") => Some("yaml"),
            Some("json") => Some("json"),
            _ => None,
        }
    }

    /// Parse a yaml/json spec file into [`SpecModule`] — the reference
    /// [`Source<SpecModule>`] body for spec inputs. Pure: reads `path`,
    /// writes nothing. `name` defaults to the file stem.
    pub fn load(path: &Path, source: Option<String>) -> Result<Self, CodegenError> {
        let format = Self::format_for(path).ok_or_else(|| {
            CodegenError::Parse(format!("unsupported spec format: {}", path.display()))
        })?;
        let text = std::fs::read_to_string(path)
            .map_err(|e| CodegenError::Parse(format!("failed to read {}: {e}", path.display())))?;
        let document: serde_json::Value = if format == "json" {
            serde_json::from_str(&text)
                .map_err(|e| CodegenError::Parse(format!("invalid json spec: {e}")))?
        } else {
            // serde_yaml deserializes YAML mappings/sequences into
            // serde_json::Value directly for JSON-compatible docs.
            serde_yaml::from_str(&text)
                .map_err(|e| CodegenError::Parse(format!("invalid yaml spec: {e}")))?
        };
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("spec")
            .to_string();
        Ok(Self {
            format: format.to_string(),
            name,
            document,
            source,
            diagnostics: Vec::new(),
        })
    }
}

/// A named import the emitted port needs: `from "<package>" import { names }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleImport {
    pub package: String,
    pub names: Vec<String>,
}

/// Record (struct/union) layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub name: String,
    pub fields: Vec<Field>,
    pub size: usize,
    pub align: usize,
    pub doc: Option<String>,
}

/// Record field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub port: String,
    pub descriptor: String,
    pub offset: usize,
    pub size: usize,
    pub ptr_depth: usize,
    pub is_const: bool,
    /// Doc comment captured from the spec, if any.
    #[serde(default)]
    pub doc: Option<String>,
}

/// Foreign function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub port_name: String,
    pub port_args: Vec<Arg>,
    pub arg_descriptors: Vec<String>,
    pub return_port: String,
    pub ret_descriptor: String,
    pub out_struct: Option<String>,
    pub variadic: bool,
    /// Whether the emitter wraps the call in the POSIX `ret < 0 ->
    /// err(errno())` guard. Libraries that signal errors out-of-band
    /// (ICU's `UErrorCode*`, Win32 `GetLastError`) set this false.
    #[serde(default = "default_true")]
    pub errno_guard: bool,
    pub c_decl: String,
    pub doc: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Visible Handl argument for a foreign function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Arg {
    pub name: String,
    pub port: String,
}

/// Typedef alias.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Typedef {
    pub name: String,
    pub port: String,
    pub descriptor: String,
    pub doc: Option<String>,
}

/// Enum / macro integer constant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumConst {
    pub name: String,
    pub value: i64,
    /// Doc comment captured from the spec, if any.
    #[serde(default)]
    pub doc: Option<String>,
}

/// External, best-effort documentation lookup keyed by symbol name
/// (`clock_gettime`), record/member path (`sockaddr.sa_family`), or
/// constant.
pub trait DocSource {
    /// The doc for `key`, or `None` when the source has none.
    fn lookup(&self, key: &str) -> Option<String>;
}

/// Resolve a single `doc` slot from the three doc layers: hand-written
/// `overrides`, a comment already captured into the slot, then an
/// external [`DocSource`].
///
/// `keys` are candidate override keys in priority order (e.g. a record
/// field tries `record.field`); the highest-priority override wins, and
/// the first key with no override stops at the in-source comment.
/// `external_key` is the key handed to `source` — `None` disables
/// external resolution for slots that are not queryable symbols.
pub fn pick_doc(
    slot: &mut Option<String>,
    overrides: &HashMap<String, String>,
    keys: &[&str],
    source: Option<&dyn DocSource>,
    external_key: Option<&str>,
) {
    for key in keys {
        if let Some(doc) = overrides.get(*key) {
            *slot = Some(doc.clone());
            return;
        }
        if slot.is_some() {
            return;
        }
    }
    if let (Some(source), Some(key)) = (source, external_key) {
        *slot = source.lookup(key);
    }
}

/// Blocking HTTP GET with a timeout, used by [`DocSource`] impls that
/// fetch docs over the network and by `hnl codegen --from-url` spec
/// fetching. Returns the response body on a 2xx status; `Err` carries
/// `HTTP <status>` for non-2xx responses or the transport error message.
pub fn http_get(url: &str, timeout_ms: u64) -> Result<String, String> {
    let req = ureq::get(url).timeout(std::time::Duration::from_millis(timeout_ms));
    match req.call() {
        Ok(resp) => resp.into_string().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(code, _)) => Err(format!("HTTP {code}")),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A [`DocSource`] that records every key it is asked about.
    struct RecordingSource(RefCell<Vec<String>>);

    impl DocSource for RecordingSource {
        fn lookup(&self, key: &str) -> Option<String> {
            self.0.borrow_mut().push(key.to_string());
            Some(format!("ext:{key}"))
        }
    }

    #[test]
    fn pick_doc_queries_external_source_once_with_external_key() {
        let src = RecordingSource(RefCell::new(Vec::new()));
        let mut doc = None;
        pick_doc(
            &mut doc,
            &HashMap::new(),
            &["Intf.member", "member", "member_op"],
            Some(&src),
            Some("Intf.member"),
        );
        assert_eq!(doc.as_deref(), Some("ext:Intf.member"));
        // One lookup with the external key — not one per override key.
        assert_eq!(src.0.borrow().as_slice(), &["Intf.member".to_string()]);
    }

    #[test]
    fn pick_doc_skips_external_when_key_absent() {
        let src = RecordingSource(RefCell::new(Vec::new()));
        let mut doc = None;
        pick_doc(&mut doc, &HashMap::new(), &["c"], Some(&src), None);
        assert_eq!(doc, None);
        assert!(src.0.borrow().is_empty());
    }

    #[test]
    fn sdkgen_module_kind_is_stable() {
        assert_eq!(SdkgenModule::default().kind().as_str(), "sdkgen");
        assert_eq!(SpecModule::default().kind().as_str(), "spec");
    }

    #[test]
    fn spec_module_load_json_and_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let json = dir.path().join("in.spec.json");
        std::fs::write(&json, r#"{"name":"demo","items":[1,2]}"#).unwrap();
        let m = SpecModule::load(&json, None).unwrap();
        assert_eq!(m.format, "json");
        assert_eq!(m.name, "in.spec");
        assert_eq!(m.document["items"][1], serde_json::json!(2));

        let yaml = dir.path().join("in.spec.yaml");
        std::fs::write(&yaml, "name: demo\nitems:\n  - 1\n  - 2\n").unwrap();
        let m = SpecModule::load(&yaml, Some("https://example/spec".into())).unwrap();
        assert_eq!(m.format, "yaml");
        assert_eq!(m.document["name"], serde_json::json!("demo"));
        assert_eq!(m.source.as_deref(), Some("https://example/spec"));

        assert!(SpecModule::format_for(&dir.path().join("x.txt")).is_none());
        assert!(SpecModule::load(&dir.path().join("x.txt"), None).is_err());
    }
}
