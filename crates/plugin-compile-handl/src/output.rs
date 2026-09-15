//! Handl compiler output type.

use std::io::Write;

use handl_plugin_compile::{Artifact, CompileError, CompilerOutput};

/// The successful result of compiling Handl to Hughes arrow source.
#[derive(Debug, Clone, PartialEq)]
pub struct HandlOutput {
    target: String,
    artifacts: Vec<Artifact>,
}

impl HandlOutput {
    /// Create a new Handl output with a single `main.hnl` artifact.
    pub fn new(src: impl Into<String>) -> Self {
        Self {
            target: "handl".to_string(),
            artifacts: vec![Artifact::new("main.hnl", src.into())],
        }
    }
}

impl CompilerOutput for HandlOutput {
    fn target(&self) -> &str {
        &self.target
    }

    fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }

    fn stream(&self, writer: &mut dyn Write) -> Result<(), CompileError> {
        match self.artifacts().first() {
            Some(artifact) => artifact.stream(writer),
            None => Ok(()),
        }
    }
}
