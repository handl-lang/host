//! DOT compiler output type.

use std::io::Write;

use handl_plugin_compile::{Artifact, CompileError, CompilerOutput};

/// The successful result of compiling a `Term` to a Graphviz DOT graph.
#[derive(Debug, Clone, PartialEq)]
pub struct DotOutput {
    target: String,
    artifacts: Vec<Artifact>,
}

impl DotOutput {
    /// Create a new DOT output with a single `main.dot` artifact.
    pub fn new(src: impl Into<String>) -> Self {
        Self {
            target: "dot".to_string(),
            artifacts: vec![Artifact::new("main.dot", src.into())],
        }
    }
}

impl CompilerOutput for DotOutput {
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
