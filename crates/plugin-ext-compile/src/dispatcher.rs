//! Compile dispatcher that wires the concrete `js` and `llvm` compilers into
//! a shared `CompilerRegistry` (ADR-0114 Phase 14).

use handl_plugin_compile::{
    CompileError, CompilerConfig, CompilerInput, CompilerOutput, CompilerRegistry,
};
use handl_plugin_compile_js::JsCompiler;
use handl_plugin_compile_llvm::LlvmCompiler;

/// A dispatcher over the built-in `js` and `llvm` compilers.
#[derive(Debug)]
pub struct CompileDispatcher {
    registry: CompilerRegistry,
}

impl CompileDispatcher {
    /// Create a new dispatcher with `JsCompiler` and `LlvmCompiler` registered.
    pub fn new() -> Self {
        let mut registry = CompilerRegistry::new();
        registry.register(Box::new(JsCompiler));
        registry.register(Box::new(LlvmCompiler::new()));
        Self { registry }
    }

    /// Compile `input` to `target` using `config`.
    pub fn compile(
        &self,
        target: &str,
        input: CompilerInput,
        config: &dyn CompilerConfig,
    ) -> Result<Box<dyn CompilerOutput>, CompileError> {
        let compiler = self
            .registry
            .get(target)
            .ok_or_else(|| CompileError::target(target))?;
        compiler.compile(&input, config)
    }
}

impl Default for CompileDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
