//! Handl SDK generator core IR and traits (ADR-0107).

use serde::{Deserialize, Serialize};
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
