//! Compile dispatcher that wires the concrete `js`, `llvm`, `handl`, `term`,
//! `dot`, and `sexp` compilers into a shared `CompilerRegistry` (ADR-0114 Phase 14).

use handl_plugin_compile::{
    CompileError, CompilerConfig, CompilerInput, CompilerOutput, CompilerRegistry,
};
use handl_plugin_compile_dot::DotCompiler;
use handl_plugin_compile_handl::HandlCompiler;
use handl_plugin_compile_js::JsCompiler;
use handl_plugin_compile_llvm::LlvmCompiler;
use handl_plugin_compile_sexp::SexpCompiler;
use handl_plugin_compile_term::TermCompiler;

/// A dispatcher over the built-in `js`, `llvm`, `handl`, `term`, `dot`, and `sexp` compilers.
#[derive(Debug)]
pub struct CompileDispatcher {
    registry: CompilerRegistry,
}

impl CompileDispatcher {
    /// Create a new dispatcher with the built-in compilers registered.
    pub fn new() -> Self {
        let mut registry = CompilerRegistry::new();
        registry.register(Box::new(JsCompiler));
        registry.register(Box::new(LlvmCompiler::new()));
        registry.register(Box::new(HandlCompiler));
        registry.register(Box::new(TermCompiler));
        registry.register(Box::new(DotCompiler));
        registry.register(Box::new(SexpCompiler));
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
