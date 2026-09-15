//! S-expression compiler output type.

use std::io::Write;

use handl_plugin_compile::{Artifact, CompileError, CompilerOutput};

/// The successful result of compiling a `Term` to an S-expression.
#[derive(Debug, Clone, PartialEq)]
pub struct SexpOutput {
    target: String,
    artifacts: Vec<Artifact>,
}

impl SexpOutput {
    /// Create a new S-expression output with a single `main.sexp` artifact.
    pub fn new(src: impl Into<String>) -> Self {
        Self {
            target: "sexp".to_string(),
            artifacts: vec![Artifact::new("main.sexp", src.into())],
        }
    }
}

impl CompilerOutput for SexpOutput {
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
