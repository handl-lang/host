//! Term compiler output type.

use std::io::Write;

use handl_plugin_compile::{Artifact, CompileError, CompilerOutput};

/// The successful result of lowering to Term JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct TermOutput {
    target: String,
    artifacts: Vec<Artifact>,
}

impl TermOutput {
    /// Create a new Term output with a single `term.json` artifact.
    pub fn new(json: impl Into<String>) -> Self {
        Self {
            target: "term".to_string(),
            artifacts: vec![Artifact::new("term.json", json.into())],
        }
    }
}

impl CompilerOutput for TermOutput {
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
