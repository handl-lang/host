//! The `LlvmCompiler` implementation.

use handl_plugin_compile::{CompileError, Compiler, CompilerConfig, CompilerInput, CompilerOutput};

use crate::config::LlvmConfig;
use crate::emit::{DefaultObjectWriter, ObjectWriter};
use crate::output::LlvmOutput;

/// LLVM compiler for the Handl arrow calculus.
#[derive(Debug, Default)]
pub struct LlvmCompiler<W = DefaultObjectWriter>
where
    W: ObjectWriter,
{
    writer: W,
}

impl LlvmCompiler<DefaultObjectWriter> {
    /// Create a compiler using the default object writer.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<W: ObjectWriter> LlvmCompiler<W> {
    /// Create a compiler with the given object writer.
    pub fn with_writer(writer: W) -> Self {
        Self { writer }
    }
}

impl<W> Compiler for LlvmCompiler<W>
where
    W: ObjectWriter + Send + Sync + 'static,
{
    fn compile(
        &self,
        input: &CompilerInput,
        config: &dyn CompilerConfig,
    ) -> Result<Box<dyn CompilerOutput>, CompileError> {
        let cfg = config
            .as_any()
            .downcast_ref::<LlvmConfig>()
            .ok_or_else(|| CompileError::InvalidInput("expected LlvmConfig".to_string()))?;

        let triple = cfg.target.triple();

        #[cfg(feature = "inkwell")]
        {
            use inkwell::context::Context;

            use crate::emit::emit_ir;
            use crate::lower::LlvmContext;

            let context = Context::create();
            let module = context.create_module("handl");
            let builder = context.create_builder();
            let mut ctx = LlvmContext::new(&context, &module, builder, triple);
            crate::lower::lower_root(&mut ctx, &input.term)?;
            let ir = emit_ir(&ctx)?;
            let object = self.writer.write(triple, &ir, cfg.passes())?;
            Ok(Box::new(LlvmOutput::from_object(triple, object)))
        }

        #[cfg(not(feature = "inkwell"))]
        {
            let _ = input;
            let object = self.writer.write(triple, "", cfg.passes())?;
            Ok(Box::new(LlvmOutput::from_object(triple, object)))
        }
    }

    fn target(&self) -> &str {
        "llvm"
    }
}
