use crate::CompileError;

/// A source-map generator for a compiled target.
pub trait SourceMap: std::fmt::Debug {
    /// Generate the source map representation.
    fn generate(&self) -> Result<String, CompileError>;
}

/// Placeholder for a V3 source map (ADR-0114 Phase 2).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct V3 {
    /// Version marker.
    pub version: i32,
    /// Suggested file name.
    pub file: Option<String>,
}

impl V3 {
    /// Create a new, empty V3 source map placeholder.
    pub fn new() -> Self {
        Self {
            version: 3,
            file: None,
        }
    }
}

impl SourceMap for V3 {
    fn generate(&self) -> Result<String, CompileError> {
        Ok(serde_json::json!({ "version": self.version, "file": self.file }).to_string())
    }
}
