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
    pub records: Vec<CRecord>,
    pub functions: Vec<CFunction>,
    pub typedefs: Vec<CTypedef>,
    pub enum_constants: Vec<CEnumConst>,
    pub diagnostics: Vec<String>,
}

/// C record layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CRecord {
    pub name: String,
    pub fields: Vec<CField>,
    pub size: usize,
    pub align: usize,
}

/// C record field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CField {
    pub name: String,
    pub port: String,
    pub descriptor: String,
    pub offset: usize,
    pub size: usize,
    pub ptr_depth: usize,
    pub is_const: bool,
}

/// C function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFunction {
    pub name: String,
    pub port_name: String,
    pub port_args: Vec<CHandlArg>,
    pub arg_descriptors: Vec<String>,
    pub return_port: String,
    pub ret_descriptor: String,
    pub out_struct: Option<String>,
    pub variadic: bool,
    pub c_decl: String,
}

/// Visible Handl argument for a C function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CHandlArg {
    pub name: String,
    pub port: String,
}

/// C typedef.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CTypedef {
    pub name: String,
    pub port: String,
    pub descriptor: String,
}

/// C enum constant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CEnumConst {
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
