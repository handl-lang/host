//! Compile dispatcher over the `CompilerRegistry` port (ADR-0114 Phase 14).
//!
//! There is no static target table here: each compiler lives in its
//! target repo (`web/modules/compile-js`, `native/modules/compile-llvm`,
//! `dot/`/`term/`/`sexp/`/`handl/` `modules/compile-*`) and is opted in by
//! a Cargo feature on the host's manifest (ADR-0128 §0.3, §"Registration:
//! link-time first"). [`CompileDispatcher::with_registry`] is the
//! injection seam for hosts that assemble their own target set.

use handl_plugin_compile::{
    CompileError, CompilerConfig, CompilerInput, CompilerOutput, CompilerRegistry,
};

/// The `CompilerRegistry` populated with every compiler target this build
/// linked in — one `#[cfg(feature)]` per repo module.
pub fn linked_targets() -> CompilerRegistry {
    // `mut` is exercised only when at least one target feature is on.
    #[allow(unused_mut)]
    let mut registry = CompilerRegistry::new();
    #[cfg(feature = "js")]
    registry.register(Box::new(handl_plugin_compile_js::JsCompiler));
    #[cfg(feature = "llvm")]
    registry.register(Box::new(handl_plugin_compile_llvm::LlvmCompiler::new()));
    #[cfg(feature = "handl")]
    registry.register(Box::new(handl_plugin_compile_handl::HandlCompiler));
    #[cfg(feature = "term")]
    registry.register(Box::new(handl_plugin_compile_term::TermCompiler));
    #[cfg(feature = "dot")]
    registry.register(Box::new(handl_plugin_compile_dot::DotCompiler));
    #[cfg(feature = "sexp")]
    registry.register(Box::new(handl_plugin_compile_sexp::SexpCompiler));
    registry
}

/// A dispatcher over the linked compiler targets.
#[derive(Debug)]
pub struct CompileDispatcher {
    registry: CompilerRegistry,
}

impl CompileDispatcher {
    /// Create a dispatcher with every feature-linked compiler registered.
    pub fn new() -> Self {
        Self::with_registry(linked_targets())
    }

    /// Create a dispatcher over a host-assembled registry — the opt-in
    /// seam for hosts that wire their own target set.
    pub fn with_registry(registry: CompilerRegistry) -> Self {
        Self { registry }
    }

    /// Mutable access to the underlying registry.
    pub fn registry_mut(&mut self) -> &mut CompilerRegistry {
        &mut self.registry
    }

    /// Unwrap into the underlying registry.
    pub fn into_registry(self) -> CompilerRegistry {
        self.registry
    }

    /// The registered target names.
    pub fn targets(&self) -> Vec<&str> {
        self.registry.targets()
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
