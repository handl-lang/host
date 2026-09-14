//! LLVM compiler output type.

use std::io::Write;

use handl_plugin_compile::{Artifact, CompileError, CompilerOutput};

/// The successful result of compiling Handl to LLVM.
#[derive(Debug, Clone, PartialEq)]
pub struct LlvmOutput {
    target: String,
    triple: String,
    artifacts: Vec<Artifact>,
    object: Vec<u8>,
}

impl LlvmOutput {
    /// Create a placeholder output when `inkwell` is not enabled.
    pub fn placeholder(triple: impl Into<String>) -> Self {
        let triple = triple.into();
        let object = b"<inkwell disabled>".to_vec();
        Self::from_object(triple, object)
    }

    /// Create an output from a compiled object byte buffer.
    pub fn from_object(triple: impl Into<String>, object: Vec<u8>) -> Self {
        let triple = triple.into();
        let name = format!("{}.o", triple.replace('-', "_"));
        Self {
            target: "llvm".to_string(),
            triple,
            artifacts: vec![Artifact::new(name, object.clone())],
            object,
        }
    }

    /// The target triple this output was built for.
    pub fn triple(&self) -> &str {
        &self.triple
    }

    /// The emitted object bytes.
    pub fn object(&self) -> &[u8] {
        &self.object
    }
}

impl CompilerOutput for LlvmOutput {
    fn target(&self) -> &str {
        &self.target
    }

    fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }

    fn stream(&self, writer: &mut dyn Write) -> Result<(), CompileError> {
        writer
            .write_all(&self.object)
            .map_err(|e| CompileError::Io(e.to_string()))
    }
}
