//! Lowering for control-flow `Term` combinators.

use handl_plugin_compile::CompileError;
use plugin_engine::Term;

use crate::lower::LlvmContext;

#[cfg(feature = "inkwell")]
use inkwell::values::{BasicValueEnum, PointerValue};

#[cfg(feature = "inkwell")]
use crate::lower::lower_term;

/// Lower `First` by unpairing, applying `inner` to the first component, and
/// re-pairing with the second component.
#[cfg(feature = "inkwell")]
pub fn lower_first<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    inner: &Term,
    input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let fst = crate::rt::abi::build_unpair_fst_call(ctx, input)?.into_pointer_value();
    let snd = crate::rt::abi::build_unpair_snd_call(ctx, input)?.into_pointer_value();
    let result = lower_term(ctx, inner, fst)?;
    crate::rt::abi::build_pair_call(ctx, result.into_pointer_value(), snd)
}

/// Lower `Choice` by inspecting the `__either__` tag and branching.
#[cfg(feature = "inkwell")]
pub fn lower_choice<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    l: &Term,
    r: &Term,
    input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let block = ctx
        .builder
        .get_insert_block()
        .ok_or_else(|| CompileError::Lower("no active basic block".to_string()))?;
    let function = block
        .get_parent()
        .ok_or_else(|| CompileError::Lower("basic block has no parent".to_string()))?;

    let then_bb = ctx.context.append_basic_block(function, "choice_l");
    let else_bb = ctx.context.append_basic_block(function, "choice_r");
    let merge_bb = ctx.context.append_basic_block(function, "choice_merge");

    let is_left = crate::rt::abi::build_either_is_left_call(ctx, input)?.into_int_value();
    ctx.builder
        .build_conditional_branch(is_left, then_bb, else_bb)
        .map_err(|e| CompileError::Lower(e.to_string()))?;

    ctx.builder.position_at_end(then_bb);
    let payload = crate::rt::abi::build_either_value_call(ctx, input)?.into_pointer_value();
    let left_result = lower_term(ctx, l, payload)?;
    ctx.builder
        .build_unconditional_branch(merge_bb)
        .map_err(|e| CompileError::Lower(e.to_string()))?;

    ctx.builder.position_at_end(else_bb);
    let payload = crate::rt::abi::build_either_value_call(ctx, input)?.into_pointer_value();
    let right_result = lower_term(ctx, r, payload)?;
    ctx.builder
        .build_unconditional_branch(merge_bb)
        .map_err(|e| CompileError::Lower(e.to_string()))?;

    ctx.builder.position_at_end(merge_bb);
    let phi = ctx
        .builder
        .build_phi(ctx.i8_ptr_type(), "choice_res")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    phi.add_incoming(&[(&left_result, then_bb), (&right_result, else_bb)]);
    Ok(phi.as_basic_value())
}

/// Lower `Loop` as a bounded `while` loop with `helper_v2_loop_check` semantics.
#[cfg(feature = "inkwell")]
pub fn lower_loop<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    body: &Term,
    input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let i64_type = ctx.context.i64_type();

    let block = ctx
        .builder
        .get_insert_block()
        .ok_or_else(|| CompileError::Lower("no active basic block".to_string()))?;
    let function = block
        .get_parent()
        .ok_or_else(|| CompileError::Lower("basic block has no parent".to_string()))?;

    let state_alloca = ctx
        .builder
        .build_alloca(i8_ptr, "loop_state")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    let counter_alloca = ctx
        .builder
        .build_alloca(i64_type, "loop_counter")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    ctx.builder
        .build_store(state_alloca, input)
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    ctx.builder
        .build_store(counter_alloca, i64_type.const_int(0, false))
        .map_err(|e| CompileError::Lower(e.to_string()))?;

    let header_bb = ctx.context.append_basic_block(function, "loop_header");
    let body_bb = ctx.context.append_basic_block(function, "loop_body");
    let exit_bb = ctx.context.append_basic_block(function, "loop_exit");

    ctx.builder
        .build_unconditional_branch(header_bb)
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    ctx.builder.position_at_end(header_bb);

    let state = ctx
        .builder
        .build_load(state_alloca, "loop_state")
        .map_err(|e| CompileError::Lower(e.to_string()))?
        .into_pointer_value();
    let counter = ctx
        .builder
        .build_load(counter_alloca, "loop_counter")
        .map_err(|e| CompileError::Lower(e.to_string()))?
        .into_int_value();

    let check = crate::rt::abi::build_loop_check_call(ctx, state)?.into_int_value();
    let max = i64_type.const_int(1_000_000, false);
    let not_exceeded = ctx
        .builder
        .build_int_compare(inkwell::IntPredicate::ULT, counter, max, "counter_ok")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    let cond = ctx
        .builder
        .build_and(check, not_exceeded, "loop_cond")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    ctx.builder
        .build_conditional_branch(cond, body_bb, exit_bb)
        .map_err(|e| CompileError::Lower(e.to_string()))?;

    ctx.builder.position_at_end(body_bb);
    let next = lower_term(ctx, body, state)?;
    let next_ptr = next.into_pointer_value();
    ctx.builder
        .build_store(state_alloca, next_ptr)
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    let next_counter = ctx
        .builder
        .build_int_add(counter, i64_type.const_int(1, false), "next_counter")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    ctx.builder
        .build_store(counter_alloca, next_counter)
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    ctx.builder
        .build_unconditional_branch(header_bb)
        .map_err(|e| CompileError::Lower(e.to_string()))?;

    ctx.builder.position_at_end(exit_bb);
    let result = ctx
        .builder
        .build_load(state_alloca, "loop_result")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    Ok(result)
}

/// Lower `Iter` as a counted loop over `predicate` and `step`.
#[cfg(feature = "inkwell")]
pub fn lower_iter<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    _fuel: &Term,
    predicate: &Term,
    step: &Term,
    initial: &Term,
    input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let i64_type = ctx.context.i64_type();

    let block = ctx
        .builder
        .get_insert_block()
        .ok_or_else(|| CompileError::Lower("no active basic block".to_string()))?;
    let function = block
        .get_parent()
        .ok_or_else(|| CompileError::Lower("basic block has no parent".to_string()))?;

    let initial_val = lower_term(ctx, initial, input)?;
    let state_alloca = ctx
        .builder
        .build_alloca(i8_ptr, "iter_state")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    let counter_alloca = ctx
        .builder
        .build_alloca(i64_type, "iter_counter")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    ctx.builder
        .build_store(state_alloca, initial_val.into_pointer_value())
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    ctx.builder
        .build_store(counter_alloca, i64_type.const_int(0, false))
        .map_err(|e| CompileError::Lower(e.to_string()))?;

    let header_bb = ctx.context.append_basic_block(function, "iter_header");
    let body_bb = ctx.context.append_basic_block(function, "iter_body");
    let exit_bb = ctx.context.append_basic_block(function, "iter_exit");

    ctx.builder
        .build_unconditional_branch(header_bb)
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    ctx.builder.position_at_end(header_bb);

    let state = ctx
        .builder
        .build_load(state_alloca, "iter_state")
        .map_err(|e| CompileError::Lower(e.to_string()))?
        .into_pointer_value();
    let counter = ctx
        .builder
        .build_load(counter_alloca, "iter_counter")
        .map_err(|e| CompileError::Lower(e.to_string()))?
        .into_int_value();

    let pred_val = lower_term(ctx, predicate, state)?;
    let check =
        crate::rt::abi::build_loop_check_call(ctx, pred_val.into_pointer_value())?.into_int_value();
    let max = i64_type.const_int(1_000_000, false);
    let not_exceeded = ctx
        .builder
        .build_int_compare(inkwell::IntPredicate::ULT, counter, max, "iter_counter_ok")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    let cond = ctx
        .builder
        .build_and(check, not_exceeded, "iter_cond")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    ctx.builder
        .build_conditional_branch(cond, body_bb, exit_bb)
        .map_err(|e| CompileError::Lower(e.to_string()))?;

    ctx.builder.position_at_end(body_bb);
    let next = lower_term(ctx, step, state)?;
    let next_ptr = next.into_pointer_value();
    ctx.builder
        .build_store(state_alloca, next_ptr)
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    let next_counter = ctx
        .builder
        .build_int_add(counter, i64_type.const_int(1, false), "next_iter_counter")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    ctx.builder
        .build_store(counter_alloca, next_counter)
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    ctx.builder
        .build_unconditional_branch(header_bb)
        .map_err(|e| CompileError::Lower(e.to_string()))?;

    ctx.builder.position_at_end(exit_bb);
    let result = ctx
        .builder
        .build_load(state_alloca, "iter_result")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    Ok(result)
}

/// Lower `App` by applying `callee` to `args`.
#[cfg(feature = "inkwell")]
pub fn lower_app<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    callee: &Term,
    args: &[Term],
    input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let closure = lower_term(ctx, callee, input)?;
    let mut arg_ptrs = Vec::new();
    for arg in args {
        let a = lower_term(ctx, arg, input)?;
        arg_ptrs.push(a.into_pointer_value());
    }
    crate::rt::abi::build_apply_closure_call(ctx, closure.into_pointer_value(), &arg_ptrs)
}

// Stubs when `inkwell` is disabled.

/// Stub `First` lowering when `inkwell` is unavailable.
#[cfg(not(feature = "inkwell"))]
pub fn lower_first<'ctx>(
    _ctx: &mut LlvmContext<'ctx>,
    _inner: &Term,
    _input: (),
) -> Result<(), CompileError> {
    Ok(())
}

/// Stub `Choice` lowering when `inkwell` is unavailable.
#[cfg(not(feature = "inkwell"))]
pub fn lower_choice<'ctx>(
    _ctx: &mut LlvmContext<'ctx>,
    _l: &Term,
    _r: &Term,
    _input: (),
) -> Result<(), CompileError> {
    Ok(())
}

/// Stub `Loop` lowering when `inkwell` is unavailable.
#[cfg(not(feature = "inkwell"))]
pub fn lower_loop<'ctx>(
    _ctx: &mut LlvmContext<'ctx>,
    _body: &Term,
    _input: (),
) -> Result<(), CompileError> {
    Ok(())
}

/// Stub `Iter` lowering when `inkwell` is unavailable.
#[cfg(not(feature = "inkwell"))]
pub fn lower_iter<'ctx>(
    _ctx: &mut LlvmContext<'ctx>,
    _fuel: &Term,
    _predicate: &Term,
    _step: &Term,
    _initial: &Term,
    _input: (),
) -> Result<(), CompileError> {
    Ok(())
}

/// Stub `App` lowering when `inkwell` is unavailable.
#[cfg(not(feature = "inkwell"))]
pub fn lower_app<'ctx>(
    _ctx: &mut LlvmContext<'ctx>,
    _callee: &Term,
    _args: &[Term],
    _input: (),
) -> Result<(), CompileError> {
    Ok(())
}
