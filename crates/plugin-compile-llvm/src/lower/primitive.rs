//! Lowering for primitive `Term` combinators.

use handl_plugin_compile::CompileError;
use inkwell::values::{BasicValueEnum, PointerValue};
use plugin_engine::Term;

use crate::lower::{LlvmContext, lower_term};

/// Lower `Arr` to a function that returns a constant value pointer.
pub fn lower_arr<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    _input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let ptr = ctx
        .builder
        .build_global_string_ptr("arr", "arr_const")
        .map_err(|e| CompileError::Lower(e.to_string()))?;
    Ok(ptr.as_basic_value_enum())
}

/// Lower `Const` to a `serde_json::Value` constant pointer.
pub fn lower_const<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    term: &Term,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    if let Term::Const { value } = term {
        let json = serde_json::to_string(value).map_err(|e| CompileError::Lower(e.to_string()))?;
        let ptr = ctx
            .builder
            .build_global_string_ptr(&json, "const_json")
            .map_err(|e| CompileError::Lower(e.to_string()))?;
        Ok(ptr.as_basic_value_enum())
    } else {
        Err(CompileError::InvalidInput(
            "lower_const called with non-Const term".to_string(),
        ))
    }
}

/// Lower `Op` to a runtime helper call or inline arithmetic.
pub fn lower_op<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    input: PointerValue<'ctx>,
    term: &Term,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    if let Term::Op { name, args, .. } = term {
        if name == "add" && args.len() == 2 {
            let a = lower_term(ctx, &args[0], input)?;
            let b = lower_term(ctx, &args[1], input)?;
            let a_ptr = a.into_pointer_value();
            let b_ptr = b.into_pointer_value();
            return crate::rt::abi::build_int_arith_call(ctx, a_ptr, b_ptr, input);
        }

        // Default: clone the input value.
        crate::rt::abi::build_clone_call(ctx, input)
    } else {
        Err(CompileError::InvalidInput(
            "lower_op called with non-Op term".to_string(),
        ))
    }
}

/// Lower `Seq` by chaining two arrows.
pub fn lower_seq<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    input: PointerValue<'ctx>,
    fst: &Term,
    snd: &Term,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    let mid = lower_term(ctx, fst, input)?;
    let mid = mid.into_pointer_value();
    lower_term(ctx, snd, mid)
}
