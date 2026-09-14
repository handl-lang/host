//! LLVM compiler target and configuration.

use std::any::Any;
use std::fmt;

use handl_plugin_compile::CompilerConfig;

/// LLVM target triple abstraction.
pub trait LlvmTarget: fmt::Debug + Send + Sync {
    /// Canonical target name, e.g. `x86_64` or `aarch64`.
    fn name(&self) -> &str;

    /// LLVM target triple string.
    fn triple(&self) -> &str;
}

/// x86-64 LLVM target.
#[derive(Debug, Clone, Copy, Default)]
pub struct X86_64;

/// AArch64 LLVM target.
#[derive(Debug, Clone, Copy, Default)]
pub struct AArch64;

/// Configuration for the LLVM compiler.
#[derive(Debug)]
pub struct LlvmConfig {
    /// The target triple to compile for.
    pub target: Box<dyn LlvmTarget>,
    /// Whether debug information should be emitted.
    pub debug: bool,
    /// Whether to emit a native object file instead of LLVM IR text.
    pub emit_object: bool,
    /// LLVM optimization/verification passes to run before object emission.
    pub passes: Vec<String>,
}

impl LlvmConfig {
    /// Create a new LLVM config for `target`.
    pub fn new(target: Box<dyn LlvmTarget>) -> Self {
        Self {
            target,
            debug: false,
            emit_object: false,
            passes: Vec::new(),
        }
    }

    /// Add a named LLVM pass to the pipeline.
    pub fn with_pass(&mut self, name: &str) {
        self.passes.push(name.to_string());
    }

    /// The configured LLVM pass pipeline.
    pub fn passes(&self) -> &[String] {
        &self.passes
    }

    /// A default optimized configuration for `x86_64`.
    pub fn default_optimized() -> Self {
        let mut cfg = Self::new(Box::new(X86_64));
        cfg.with_pass("instcombine");
        cfg.with_pass("reassociate");
        cfg.with_pass("gvn");
        cfg
    }
}

impl CompilerConfig for LlvmConfig {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn target(&self) -> &str {
        "llvm"
    }

    fn debug(&self) -> bool {
        self.debug
    }

    fn source_map(&self) -> bool {
        false
    }
}
