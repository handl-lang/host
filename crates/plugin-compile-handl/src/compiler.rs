//! The `HandlCompiler` implementation.

use handl_plugin_compile::{CompileError, Compiler, CompilerConfig, CompilerInput, CompilerOutput};

use crate::config::HandlConfig;
use crate::emit::HughesEmitter;
use crate::output::HandlOutput;

/// Handl-to-Handl (Hughes arrow dialect) compiler.
#[derive(Debug, Default, Clone, Copy)]
pub struct HandlCompiler;

impl Compiler for HandlCompiler {
    fn compile(
        &self,
        input: &CompilerInput,
        config: &dyn CompilerConfig,
    ) -> Result<Box<dyn CompilerOutput>, CompileError> {
        let cfg = config
            .as_any()
            .downcast_ref::<HandlConfig>()
            .ok_or_else(|| CompileError::InvalidInput("expected HandlConfig".to_string()))?;

        if cfg.to != "hughes" {
            return Err(CompileError::UnsupportedTarget(cfg.to.clone()));
        }

        let src = HughesEmitter.emit(&input.term);
        Ok(Box::new(HandlOutput::new(src)))
    }

    fn target(&self) -> &str {
        "handl"
    }
}
