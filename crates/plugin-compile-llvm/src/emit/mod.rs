//! IR -> .o (or placeholder) emission.

#![allow(unsafe_code)]

pub mod debug_info;
pub mod object;

pub use debug_info::{DebugInfoBuilder, DwarfLineTable};
pub use object::{DefaultObjectWriter, ObjectWriter, default_object_writer};

#[cfg(feature = "inkwell")]
use handl_plugin_compile::CompileError;

#[cfg(feature = "inkwell")]
use crate::lower::LlvmContext;

/// Emit the current LLVM module as IR text.
#[cfg(feature = "inkwell")]
pub fn emit_ir<'ctx>(ctx: &LlvmContext<'ctx>) -> Result<String, CompileError> {
    let s = ctx.module.print_to_string().to_string();
    if s.is_empty() {
        Err(CompileError::Emit("empty LLVM IR".to_string()))
    } else {
        Ok(s)
    }
}

/// Placeholder for object emission when `inkwell` is unavailable.
#[cfg(not(feature = "inkwell"))]
pub fn emit_placeholder() -> &'static str {
    "LLVM object emission is only available with the `inkwell` feature."
}
