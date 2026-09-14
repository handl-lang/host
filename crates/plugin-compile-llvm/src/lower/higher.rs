//! Lowering for higher-order `Term` combinators.

use handl_plugin_compile::CompileError;
use plugin_engine::Term;

use crate::lower::LlvmContext;

#[cfg(feature = "inkwell")]
use inkwell::values::{BasicValueEnum, PointerValue};

#[cfg(feature = "inkwell")]
use crate::lower::lower_term;

/// Lower `Child` to a runtime `helper_v2_eval_child` call.
#[cfg(feature = "inkwell")]
pub fn lower_child<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    role: &str,
    input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    // The role is not yet mapped to an index; pass a placeholder and the
    // original input value through the helper ABI.
    let _ = role;
    crate::rt::abi::build_eval_child_call(ctx, input, 0, 0)
}

/// Lower `Handle` by lowering the body, treating the capability as a
/// placeholder substitution for now.
#[cfg(feature = "inkwell")]
pub fn lower_handle<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    body: &Term,
    _target: &str,
    _impl_: &Term,
    input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    lower_term(ctx, body, input)
}

/// Stub `Child` lowering when `inkwell` is unavailable.
#[cfg(not(feature = "inkwell"))]
pub fn lower_child<'ctx>(
    _ctx: &mut LlvmContext<'ctx>,
    _role: &str,
    _input: (),
) -> Result<(), CompileError> {
    Ok(())
}

/// Stub `Handle` lowering when `inkwell` is unavailable.
#[cfg(not(feature = "inkwell"))]
pub fn lower_handle<'ctx>(
    _ctx: &mut LlvmContext<'ctx>,
    _body: &Term,
    _target: &str,
    _impl_: &Term,
    _input: (),
) -> Result<(), CompileError> {
    Ok(())
}
