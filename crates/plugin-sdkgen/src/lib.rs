//! Handl SDK generator core IR and traits (ADR-0107).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Platform-agnostic SDK module produced by a backend and consumed by an emitter.
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
    /// `hnl sdkgen --from-url`. Rendered as a `--- source:` header line so
    /// committed generated files record the pin they were produced from.
    #[serde(default)]
    pub source: Option<String>,
    pub diagnostics: Vec<String>,
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
    pub c_decl: String,
    pub doc: Option<String>,
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

/// Backend-specific parser from a foreign API description to [`SdkgenModule`].
pub trait SdkgenBackend {
    fn parse_header(&self, path: &Path) -> Result<SdkgenModule, SdkgenError>;
}

/// Output-channel-specific emitter for [`SdkgenModule`].
pub trait Emitter {
    fn emit(&self, module: &SdkgenModule, output: &Path) -> Result<(), SdkgenError>;
}

/// Top-level generation API.
pub fn generate<B: SdkgenBackend, E: Emitter>(
    backend: &B,
    emitter: &E,
    header: &Path,
    output: &Path,
) -> Result<(), SdkgenError> {
    let module = backend.parse_header(header)?;
    emitter.emit(&module, output)
}

/// SDK generation error.
#[derive(Debug)]
pub struct SdkgenError(pub String);

impl std::fmt::Display for SdkgenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SdkgenError {}

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
}
