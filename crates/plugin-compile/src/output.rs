use std::io::Write;
use std::path::PathBuf;

use crate::CompileError;

/// A single artifact produced by a [`Compiler`](crate::Compiler).
#[derive(Debug, Clone, PartialEq)]
pub struct Artifact {
    /// The artifact name (e.g. `"main.c"`, `"app.wasm"`).
    pub name: String,
    /// An optional filesystem path.
    pub path: Option<PathBuf>,
    /// The artifact contents.
    pub content: ArtifactContent,
}

impl Artifact {
    /// Create a new artifact from any content that converts into [`ArtifactContent`].
    pub fn new(name: impl Into<String>, content: impl Into<ArtifactContent>) -> Self {
        Self {
            name: name.into(),
            path: None,
            content: content.into(),
        }
    }

    /// Attach a filesystem path.
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Stream this artifact's content to `writer`.
    pub fn stream<W: ?Sized + Write>(&self, writer: &mut W) -> Result<(), CompileError> {
        let bytes = match &self.content {
            ArtifactContent::Bytes(b) => b.as_slice(),
            ArtifactContent::String(s) => s.as_bytes(),
        };
        writer
            .write_all(bytes)
            .map_err(|e| CompileError::Io(e.to_string()))
    }
}

/// Content of an [`Artifact`].
#[derive(Debug, Clone, PartialEq)]
pub enum ArtifactContent {
    /// Raw bytes, e.g. for object files or Wasm binaries.
    Bytes(Vec<u8>),
    /// A UTF-8 string, e.g. for generated source.
    String(String),
}

impl From<Vec<u8>> for ArtifactContent {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

impl From<String> for ArtifactContent {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for ArtifactContent {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

/// The output of a successful compilation.
pub trait CompilerOutput: std::fmt::Debug + Send + Sync {
    /// The target this output represents.
    fn target(&self) -> &str;

    /// All artifacts produced.
    fn artifacts(&self) -> &[Artifact];

    /// The primary output content, if any.
    fn primary_output(&self) -> Option<&[u8]> {
        self.artifacts().first().map(|a| match &a.content {
            ArtifactContent::Bytes(b) => b.as_slice(),
            ArtifactContent::String(s) => s.as_bytes(),
        })
    }

    /// Stream the primary output bytes to `writer`.
    fn stream(&self, writer: &mut dyn Write) -> Result<(), CompileError> {
        if let Some(bytes) = self.primary_output() {
            writer
                .write_all(bytes)
                .map_err(|e| CompileError::Io(e.to_string()))?;
        }
        Ok(())
    }
}
