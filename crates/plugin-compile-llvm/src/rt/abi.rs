//! Runtime helper imports for the LLVM backend.

#![allow(unsafe_code)]

#[cfg(feature = "inkwell")]
use handl_plugin_compile::CompileError;
#[cfg(feature = "inkwell")]
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};

/// Re-export the `helper_v2_*` name constants from the shared value ABI.
pub use handl_plugin_compile::value_abi::helpers;

#[cfg(feature = "inkwell")]
use crate::lower::LlvmContext;

/// Build a call to `helper_v2_clone`.
#[cfg(feature = "inkwell")]
pub fn build_clone_call<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    value: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let fn_type = i8_ptr.fn_type(&[i8_ptr.into(), i8_ptr.into()], false);
    let function = get_or_declare(ctx, helpers::HELPER_V2_CLONE, fn_type)?;
    let eval_ctx = current_eval_ctx(ctx)?;
    let call = ctx
        .builder
        .build_call(function, &[eval_ctx.into(), value.into()], "clone")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    call.try_as_basic_value()
        .left()
        .ok_or_else(|| CompileError::Lower("helper_v2_clone returned void".to_string()))
}

/// Build a call to `helper_v2_int_arith`.
#[cfg(feature = "inkwell")]
pub fn build_int_arith_call<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    left: PointerValue<'ctx>,
    right: PointerValue<'ctx>,
    input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let usize_type = ctx.context.i64_type();
    let fn_type = i8_ptr.fn_type(
        &[
            i8_ptr.into(),
            usize_type.into(),
            usize_type.into(),
            i8_ptr.into(),
            i8_ptr.into(),
            i8_ptr.into(),
        ],
        false,
    );
    let function = get_or_declare(ctx, helpers::HELPER_V2_INT_ARITH, fn_type)?;
    let eval_ctx = current_eval_ctx(ctx)?;
    let op = usize_type.const_int(0, false);
    let call = ctx
        .builder
        .build_call(
            function,
            &[
                eval_ctx.into(),
                op.into(),
                op.into(),
                left.into(),
                right.into(),
                input.into(),
            ],
            "int_arith",
        )
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    call.try_as_basic_value()
        .left()
        .ok_or_else(|| CompileError::Lower("helper_v2_int_arith returned void".to_string()))
}

/// Build a call to `helper_v2_pair`.
#[cfg(feature = "inkwell")]
pub fn build_pair_call<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    left: PointerValue<'ctx>,
    right: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let fn_type = i8_ptr.fn_type(&[i8_ptr.into(), i8_ptr.into(), i8_ptr.into()], false);
    let function = get_or_declare(ctx, helpers::HELPER_V2_PAIR, fn_type)?;
    let eval_ctx = current_eval_ctx(ctx)?;
    let call = ctx
        .builder
        .build_call(
            function,
            &[eval_ctx.into(), left.into(), right.into()],
            "pair",
        )
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    call.try_as_basic_value()
        .left()
        .ok_or_else(|| CompileError::Lower("helper_v2_pair returned void".to_string()))
}

/// Build a call to `helper_v2_unpair_fst`.
#[cfg(feature = "inkwell")]
pub fn build_unpair_fst_call<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    value: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let fn_type = i8_ptr.fn_type(&[i8_ptr.into(), i8_ptr.into()], false);
    let function = get_or_declare(ctx, helpers::HELPER_V2_UNPAIR_FST, fn_type)?;
    let eval_ctx = current_eval_ctx(ctx)?;
    let call = ctx
        .builder
        .build_call(function, &[eval_ctx.into(), value.into()], "unpair_fst")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    call.try_as_basic_value()
        .left()
        .ok_or_else(|| CompileError::Lower("helper_v2_unpair_fst returned void".to_string()))
}

/// Build a call to `helper_v2_unpair_snd`.
#[cfg(feature = "inkwell")]
pub fn build_unpair_snd_call<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    value: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let fn_type = i8_ptr.fn_type(&[i8_ptr.into(), i8_ptr.into()], false);
    let function = get_or_declare(ctx, helpers::HELPER_V2_UNPAIR_SND, fn_type)?;
    let eval_ctx = current_eval_ctx(ctx)?;
    let call = ctx
        .builder
        .build_call(function, &[eval_ctx.into(), value.into()], "unpair_snd")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    call.try_as_basic_value()
        .left()
        .ok_or_else(|| CompileError::Lower("helper_v2_unpair_snd returned void".to_string()))
}

/// Build a call to `helper_v2_either_is_left`.
#[cfg(feature = "inkwell")]
pub fn build_either_is_left_call<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    value: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let i8_type = ctx.context.i8_type();
    let fn_type = i8_type.fn_type(&[i8_ptr.into(), i8_ptr.into()], false);
    let function = get_or_declare(ctx, helpers::HELPER_V2_EITHER_IS_LEFT, fn_type)?;
    let eval_ctx = current_eval_ctx(ctx)?;
    let call = ctx
        .builder
        .build_call(function, &[eval_ctx.into(), value.into()], "either_is_left")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    call.try_as_basic_value()
        .left()
        .ok_or_else(|| CompileError::Lower("helper_v2_either_is_left returned void".to_string()))
}

/// Build a call to `helper_v2_either_value`.
#[cfg(feature = "inkwell")]
pub fn build_either_value_call<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    value: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let fn_type = i8_ptr.fn_type(&[i8_ptr.into(), i8_ptr.into()], false);
    let function = get_or_declare(ctx, helpers::HELPER_V2_EITHER_VALUE, fn_type)?;
    let eval_ctx = current_eval_ctx(ctx)?;
    let call = ctx
        .builder
        .build_call(function, &[eval_ctx.into(), value.into()], "either_value")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    call.try_as_basic_value()
        .left()
        .ok_or_else(|| CompileError::Lower("helper_v2_either_value returned void".to_string()))
}

/// Build a call to `helper_v2_loop_check`.
#[cfg(feature = "inkwell")]
pub fn build_loop_check_call<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    value: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let i8_type = ctx.context.i8_type();
    let fn_type = i8_type.fn_type(&[i8_ptr.into(), i8_ptr.into()], false);
    let function = get_or_declare(ctx, helpers::HELPER_V2_LOOP_CHECK, fn_type)?;
    let eval_ctx = current_eval_ctx(ctx)?;
    let call = ctx
        .builder
        .build_call(function, &[eval_ctx.into(), value.into()], "loop_check")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    call.try_as_basic_value()
        .left()
        .ok_or_else(|| CompileError::Lower("helper_v2_loop_check returned void".to_string()))
}

/// Build a call to `helper_v2_apply_closure`.
#[cfg(feature = "inkwell")]
pub fn build_apply_closure_call<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    closure: PointerValue<'ctx>,
    args: &[PointerValue<'ctx>],
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let usize_type = ctx.context.i64_type();
    let argv_type = i8_ptr.ptr_type(inkwell::AddressSpace::default());
    let fn_type = i8_ptr.fn_type(
        &[
            i8_ptr.into(),
            i8_ptr.into(),
            argv_type.into(),
            usize_type.into(),
        ],
        false,
    );
    let function = get_or_declare(ctx, helpers::HELPER_V2_APPLY_CLOSURE, fn_type)?;
    let eval_ctx = current_eval_ctx(ctx)?;

    let count = ctx.context.i64_type().const_int(args.len() as u64, false);
    let array = ctx
        .builder
        .build_array_alloca(i8_ptr, count, "apply_args")
        .map_err(|e| CompileError::Lower(e.to_string()))?;

    let i32_type = ctx.context.i32_type();
    for (i, arg) in args.iter().enumerate() {
        let zero = i32_type.const_int(0, false);
        let index = i32_type.const_int(i as u64, false);
        let ptr = unsafe {
            ctx.builder
                .build_in_bounds_gep(i8_ptr, array, &[zero, index], "apply_arg")
        }
        .map_err(|e| CompileError::Lower(e.to_string()))?;
        ctx.builder
            .build_store(ptr, *arg)
            .map_err(|e| CompileError::Lower(e.to_string()))?;
    }

    let call = ctx
        .builder
        .build_call(
            function,
            &[eval_ctx.into(), closure.into(), array.into(), count.into()],
            "apply_closure",
        )
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    call.try_as_basic_value()
        .left()
        .ok_or_else(|| CompileError::Lower("helper_v2_apply_closure returned void".to_string()))
}

/// Build a call to `helper_v2_apply_native`.
#[cfg(feature = "inkwell")]
pub fn build_apply_native_call<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    index: u64,
    args: &[PointerValue<'ctx>],
    input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let usize_type = ctx.context.i64_type();
    let argv_type = i8_ptr.ptr_type(inkwell::AddressSpace::default());
    let fn_type = i8_ptr.fn_type(
        &[
            i8_ptr.into(),
            usize_type.into(),
            argv_type.into(),
            usize_type.into(),
            i8_ptr.into(),
        ],
        false,
    );
    let function = get_or_declare(ctx, helpers::HELPER_V2_APPLY_NATIVE, fn_type)?;
    let eval_ctx = current_eval_ctx(ctx)?;

    let count = ctx.context.i64_type().const_int(args.len() as u64, false);
    let array = ctx
        .builder
        .build_array_alloca(i8_ptr, count, "native_args")
        .map_err(|e| CompileError::Lower(e.to_string()))?;

    let i32_type = ctx.context.i32_type();
    for (i, arg) in args.iter().enumerate() {
        let zero = i32_type.const_int(0, false);
        let index = i32_type.const_int(i as u64, false);
        let ptr = unsafe {
            ctx.builder
                .build_in_bounds_gep(i8_ptr, array, &[zero, index], "native_arg")
        }
        .map_err(|e| CompileError::Lower(e.to_string()))?;
        ctx.builder
            .build_store(ptr, *arg)
            .map_err(|e| CompileError::Lower(e.to_string()))?;
    }

    let index_val = usize_type.const_int(index, false);
    let call = ctx
        .builder
        .build_call(
            function,
            &[
                eval_ctx.into(),
                index_val.into(),
                array.into(),
                count.into(),
                input.into(),
            ],
            "apply_native",
        )
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    call.try_as_basic_value()
        .left()
        .ok_or_else(|| CompileError::Lower("helper_v2_apply_native returned void".to_string()))
}

/// Build a call to `helper_v2_eval_child`.
#[cfg(feature = "inkwell")]
pub fn build_eval_child_call<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    value: PointerValue<'ctx>,
    role: u64,
    index: i64,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let usize_type = ctx.context.i64_type();
    let i64_type = ctx.context.i64_type();
    let fn_type = i8_ptr.fn_type(
        &[
            i8_ptr.into(),
            usize_type.into(),
            i8_ptr.into(),
            i64_type.into(),
        ],
        false,
    );
    let function = get_or_declare(ctx, helpers::HELPER_V2_EVAL_CHILD, fn_type)?;
    let eval_ctx = current_eval_ctx(ctx)?;
    let role_val = usize_type.const_int(role, false);
    let index_val = i64_type.const_int(index as u64, false);
    let call = ctx
        .builder
        .build_call(
            function,
            &[
                eval_ctx.into(),
                role_val.into(),
                value.into(),
                index_val.into(),
            ],
            "eval_child",
        )
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    call.try_as_basic_value()
        .left()
        .ok_or_else(|| CompileError::Lower("helper_v2_eval_child returned void".to_string()))
}

#[cfg(feature = "inkwell")]
fn get_or_declare<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    name: &str,
    fn_type: inkwell::types::FunctionType<'ctx>,
) -> Result<FunctionValue<'ctx>, CompileError> {
    Ok(ctx
        .module
        .get_function(name)
        .unwrap_or_else(|| ctx.module.add_function(name, fn_type, None)))
}

#[cfg(feature = "inkwell")]
fn current_eval_ctx<'ctx>(ctx: &LlvmContext<'ctx>) -> Result<PointerValue<'ctx>, CompileError> {
    let block = ctx
        .builder
        .get_insert_block()
        .ok_or_else(|| CompileError::Lower("no active basic block".to_string()))?;
    let function = block
        .get_parent()
        .ok_or_else(|| CompileError::Lower("basic block has no parent function".to_string()))?;
    let param = function
        .get_nth_param(0)
        .ok_or_else(|| CompileError::Lower("no eval context parameter".to_string()))?;
    Ok(param.into_pointer_value())
}

/// Return a fresh DWARF line table builder.
pub fn dwarf_line_table() -> crate::emit::DwarfLineTable {
    crate::emit::DwarfLineTable::new()
}
