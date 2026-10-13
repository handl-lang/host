//! Lowering for proof-relevant `Term` combinators.
//!
//! Only `Rewrite` is a variant of the closed `Term` vocabulary. `Path` /
//! `Equiv` (with `Refine`, `CellId`, `JElim`, …) are *derived* operators that
//! ADR-0119 §7 folded into `Term::Opaque`, so a backend never receives them
//! structurally — the former `lower_path` / `lower_equiv` stubs could not be
//! reached and were removed.

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
