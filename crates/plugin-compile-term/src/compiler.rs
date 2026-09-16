//! The `TermCompiler` implementation.

use handl_kit_arrow_lower::lower_ast;
use handl_plugin_compile::{CompileError, Compiler, CompilerConfig, CompilerInput, CompilerOutput};

use crate::config::TermConfig;
use crate::output::TermOutput;

/// Term compiler: lowers canonical Handl to `Term` and serializes it as JSON.
#[derive(Debug, Default, Clone, Copy)]
pub struct TermCompiler;

impl Compiler for TermCompiler {
    fn compile(
        &self,
        input: &CompilerInput,
        config: &dyn CompilerConfig,
    ) -> Result<Box<dyn CompilerOutput>, CompileError> {
        let cfg = config
            .as_any()
            .downcast_ref::<TermConfig>()
            .ok_or_else(|| CompileError::InvalidInput("expected TermConfig".to_string()))?;

        let term = lower_ast(&input.ast, &input.language, input.root)
            .map_err(|e| CompileError::Lower(e))?;

        let json = match cfg.emit.as_str() {
            "json" | "pretty" => serde_json::to_string_pretty(&term)
                .map_err(|e| CompileError::Emit(e.to_string()))?,
            other => return Err(CompileError::UnsupportedTarget(other.to_string())),
        };

        Ok(Box::new(TermOutput::new(json)))
    }

    fn target(&self) -> &str {
        "term"
    }
}
