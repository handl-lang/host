//! Object emission trait and default implementation.

#![allow(unsafe_code)]

use std::fmt::Debug;

use handl_plugin_compile::CompileError;

/// Writes LLVM IR text to a native object file.
pub trait ObjectWriter: Debug {
    /// Emit a `.o` file from `ir` for the given `triple` after running `passes`.
    fn write(&self, triple: &str, ir: &str, passes: &[String]) -> Result<Vec<u8>, CompileError>;
}

/// The default [`ObjectWriter`].
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultObjectWriter;

impl ObjectWriter for DefaultObjectWriter {
    #[cfg(not(feature = "inkwell"))]
    fn write(&self, _triple: &str, _ir: &str, _passes: &[String]) -> Result<Vec<u8>, CompileError> {
        Ok(b"<inkwell disabled>".to_vec())
    }

    #[cfg(feature = "inkwell")]
    fn write(&self, triple: &str, ir: &str, passes: &[String]) -> Result<Vec<u8>, CompileError> {
        use inkwell::OptimizationLevel;
        use inkwell::context::Context;
        use inkwell::memory_buffer::MemoryBuffer;
        use inkwell::passes::PassManager;
        use inkwell::targets::{
            CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetTriple,
        };

        let context = Context::create();
        let buffer = MemoryBuffer::create_from_memory_range_copy(ir.as_bytes(), "ir");
        let module = context
            .create_module_from_ir(buffer)
            .map_err(|e| CompileError::Emit(e.to_string()))?;

        Target::initialize_all(&InitializationConfig::default());

        let triple = TargetTriple::create(triple);
        let target = Target::from_triple(&triple).map_err(|e| {
            CompileError::unsupported_target(&format!(
                "LLVM target '{triple}' is not available: {e}"
            ))
        })?;
        let target_machine = target
            .create_target_machine(
                &triple,
                "",
                "",
                OptimizationLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| {
                CompileError::unsupported_target(&format!(
                    "LLVM TargetMachine for '{triple}' could not be created; ensure the target is built into your LLVM installation"
                ))
            })?;

        let pm: PassManager<inkwell::module::Module> = PassManager::create(());
        for name in passes {
            match name.as_str() {
                "instcombine" => pm.add_instruction_combining_pass(),
                "reassociate" => pm.add_reassociate_pass(),
                "gvn" => pm.add_gvn_pass(),
                "simplifycfg" => pm.add_cfg_simplification_pass(),
                "mem2reg" => pm.add_promote_memory_to_register_pass(),
                other => return Err(CompileError::Emit(format!("unknown pass: {other}"))),
            }
        }
        pm.run_on(&module);

        let object = target_machine
            .write_to_memory_buffer(&module, FileType::Object)
            .map_err(|e| CompileError::Emit(e.to_string()))?;

        Ok(object.as_slice().to_vec())
    }
}

/// Create a boxed default [`ObjectWriter`].
pub fn default_object_writer() -> Box<dyn ObjectWriter> {
    Box::new(DefaultObjectWriter)
}
