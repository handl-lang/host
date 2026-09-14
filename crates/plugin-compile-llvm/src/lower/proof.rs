//! Lowering for proof-relevant `Term` combinators.

use handl_plugin_compile::CompileError;
use plugin_engine::{RewriteWitness, Term};

use crate::lower::LlvmContext;

#[cfg(feature = "inkwell")]
use inkwell::values::{BasicValueEnum, PointerValue};

/// Lower `Rewrite` to an identity/clone of the input.
#[cfg(feature = "inkwell")]
pub fn lower_rewrite<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    _source: &Term,
    _target: &Term,
    _witness: &RewriteWitness,
    input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    crate::rt::abi::build_clone_call(ctx, input)
}

/// Lower `Path` to an identity/clone of the input.
#[cfg(feature = "inkwell")]
pub fn lower_path<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    _endpoints: &(Box<Term>, Box<Term>),
    _witness: &Option<Box<Term>>,
    input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    crate::rt::abi::build_clone_call(ctx, input)
}

/// Lower `Equiv` to an identity/clone of the input.
#[cfg(feature = "inkwell")]
pub fn lower_equiv<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    _a: &Term,
    _b: &Term,
    _path: &Option<Box<Term>>,
    input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    crate::rt::abi::build_clone_call(ctx, input)
}

/// Stub `Rewrite` lowering when `inkwell` is unavailable.
#[cfg(not(feature = "inkwell"))]
pub fn lower_rewrite<'ctx>(
    _ctx: &mut LlvmContext<'ctx>,
    _source: &Term,
    _target: &Term,
    _witness: &RewriteWitness,
    _input: (),
) -> Result<(), CompileError> {
    Ok(())
}

/// Stub `Path` lowering when `inkwell` is unavailable.
#[cfg(not(feature = "inkwell"))]
pub fn lower_path<'ctx>(
    _ctx: &mut LlvmContext<'ctx>,
    _endpoints: &(Box<Term>, Box<Term>),
    _witness: &Option<Box<Term>>,
    _input: (),
) -> Result<(), CompileError> {
    Ok(())
}

/// Stub `Equiv` lowering when `inkwell` is unavailable.
#[cfg(not(feature = "inkwell"))]
pub fn lower_equiv<'ctx>(
    _ctx: &mut LlvmContext<'ctx>,
    _a: &Term,
    _b: &Term,
    _path: &Option<Box<Term>>,
    _input: (),
) -> Result<(), CompileError> {
    Ok(())
}
