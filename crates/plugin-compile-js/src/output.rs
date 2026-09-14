//! JS compiler output type.

use std::io::Write;

use handl_plugin_compile::{Artifact, CompileError, CompilerOutput};

/// The successful result of compiling Handl to JavaScript.
#[derive(Debug, Clone, PartialEq)]
pub struct JsOutput {
    target: String,
    artifacts: Vec<Artifact>,
}

impl JsOutput {
    /// Create a new JS output with a single `main.js` artifact.
    pub fn new(js: impl Into<String>) -> Self {
        Self {
            target: "js".to_string(),
            artifacts: vec![Artifact::new("main.js", js.into())],
        }
    }

    /// Create a JS output with both `main.js` and a `.map` artifact.
    pub fn new_with_source_map(
        js: impl Into<String>,
        map: impl Into<String>,
        map_name: impl Into<String>,
    ) -> Self {
        Self {
            target: "js".to_string(),
            artifacts: vec![
                Artifact::new("main.js", js.into()),
                Artifact::new(map_name, map.into()),
            ],
        }
    }
}

impl CompilerOutput for JsOutput {
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
