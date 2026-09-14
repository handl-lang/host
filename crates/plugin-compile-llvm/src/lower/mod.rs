//! Term -> LLVM IR dispatcher.

#[cfg(feature = "inkwell")]
pub mod primitive;

pub mod control;
pub mod higher;
pub mod proof;

#[cfg(feature = "inkwell")]
use handl_plugin_compile::CompileError;
#[cfg(feature = "inkwell")]
use inkwell::values::{BasicValueEnum, PointerValue};
#[cfg(feature = "inkwell")]
use plugin_engine::Term;

/// Compilation context for the current LLVM module.
#[cfg(feature = "inkwell")]
pub struct LlvmContext<'ctx> {
    /// The inkwell context.
    pub context: &'ctx inkwell::context::Context,
    /// The LLVM module being built.
    pub module: &'ctx inkwell::module::Module<'ctx>,
    /// The instruction builder.
    pub builder: inkwell::builder::Builder<'ctx>,
    /// Target triple.
    pub triple: String,
}

#[cfg(feature = "inkwell")]
impl<'ctx> LlvmContext<'ctx> {
    /// Create a new lowering context.
    pub fn new(
        context: &'ctx inkwell::context::Context,
        module: &'ctx inkwell::module::Module<'ctx>,
        builder: inkwell::builder::Builder<'ctx>,
        triple: impl Into<String>,
    ) -> Self {
        Self {
            context,
            module,
            builder,
            triple: triple.into(),
        }
    }

    /// The `i8*` type used for opaque `*mut Value` / `*mut EvalCtx`.
    pub fn i8_ptr_type(&self) -> inkwell::types::PointerType<'ctx> {
        self.context
            .i8_type()
            .ptr_type(inkwell::AddressSpace::default())
    }
}

/// Lower the root `term` into a `main` arrow function.
#[cfg(feature = "inkwell")]
pub fn lower_root<'ctx>(ctx: &mut LlvmContext<'ctx>, term: &Term) -> Result<(), CompileError> {
    let i8_ptr = ctx.i8_ptr_type();
    let fn_type = i8_ptr.fn_type(&[i8_ptr.into(), i8_ptr.into()], false);
    let function = ctx.module.add_function("main", fn_type, None);

    let entry = ctx.context.append_basic_block(function, "entry");
    ctx.builder.position_at_end(entry);

    let input = function
        .get_nth_param(1)
        .ok_or_else(|| CompileError::Lower("main input parameter missing".to_string()))?;
    let input = input.into_pointer_value();

    let result = lower_term(ctx, term, input)?;
    ctx.builder
        .build_return(Some(&result))
        .map_err(|e| CompileError::Emit(e.to_string()))?;

    Ok(())
}

/// Lower a single `term` arrow applied to `input`.
#[cfg(feature = "inkwell")]
pub fn lower_term<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    term: &Term,
    input: PointerValue<'ctx>,
) -> Result<BasicValueEnum<'ctx>, CompileError> {
    match term {
        Term::Arr { .. } => primitive::lower_arr(ctx, input),
        Term::Const { .. } => primitive::lower_const(ctx, term),
        Term::Op { .. } => primitive::lower_op(ctx, input, term),
        Term::Seq { fst, snd } => primitive::lower_seq(ctx, input, fst, snd),
        Term::First { inner } => control::lower_first(ctx, inner, input),
        Term::Choice { l, r } => control::lower_choice(ctx, l, r, input),
        Term::Loop { body } => control::lower_loop(ctx, body, input),
        Term::Iter {
            fuel,
            predicate,
            step,
            initial,
        } => control::lower_iter(ctx, fuel, predicate, step, initial, input),
        Term::App { callee, args } => control::lower_app(ctx, callee, args, input),
        Term::Child { role } => higher::lower_child(ctx, role, input),
        Term::Handle {
            body,
            target,
            impl_,
        } => higher::lower_handle(ctx, body, target, impl_, input),
        Term::Rewrite {
            source,
            target,
            witness,
        } => proof::lower_rewrite(ctx, source, target, witness, input),
        Term::Path { endpoints, witness } => proof::lower_path(ctx, endpoints, witness, input),
        Term::Equiv { a, b, path } => proof::lower_equiv(ctx, a, b, path, input),
        _ => Err(CompileError::Lower(format!("unsupported term: {term:?}"))),
    }
}

#[cfg(not(feature = "inkwell"))]
use handl_plugin_compile::CompileError;
#[cfg(not(feature = "inkwell"))]
use plugin_engine::Term;

/// Stub context when `inkwell` is not enabled.
#[cfg(not(feature = "inkwell"))]
pub struct LlvmContext<'ctx> {
    /// The term being lowered.
    pub term: &'ctx Term,
    /// Target triple.
    pub triple: String,
}

#[cfg(not(feature = "inkwell"))]
impl<'ctx> LlvmContext<'ctx> {
    /// Create a stub lowering context.
    pub fn new(term: &'ctx Term, triple: impl Into<String>) -> Self {
        Self {
            term,
            triple: triple.into(),
        }
    }
}

/// Stub lower function when `inkwell` is not enabled.
#[cfg(not(feature = "inkwell"))]
pub fn lower_term<'ctx>(
    ctx: &mut LlvmContext<'ctx>,
    term: &Term,
    _input: (),
) -> Result<(), CompileError> {
    match term {
        Term::Arr { .. }
        | Term::Const { .. }
        | Term::Op { .. }
        | Term::Seq { .. }
        | Term::CellId { .. }
        | Term::Src { .. }
        | Term::Tgt { .. }
        | Term::CellCompV { .. }
        | Term::Whisker { .. }
        | Term::RecordProj { .. }
        | Term::RecordWith { .. }
        | Term::Tag { .. }
        | Term::Univalence { .. }
        | Term::Glue { .. }
        | Term::Fibrant { .. }
        | Term::JElim { .. }
        | Term::Refine { .. }
        | Term::Cobind { .. } => Ok(()),
        Term::First { inner } => control::lower_first(ctx, inner, ()),
        Term::Choice { l, r } => control::lower_choice(ctx, l, r, ()),
        Term::Loop { body } => control::lower_loop(ctx, body, ()),
        Term::Iter {
            fuel,
            predicate,
            step,
            initial,
        } => control::lower_iter(ctx, fuel, predicate, step, initial, ()),
        Term::App { callee, args } => control::lower_app(ctx, callee, args, ()),
        Term::Child { role } => higher::lower_child(ctx, role, ()),
        Term::Handle {
            body,
            target,
            impl_,
        } => higher::lower_handle(ctx, body, target, impl_, ()),
        Term::Rewrite {
            source,
            target,
            witness,
        } => proof::lower_rewrite(ctx, source, target, witness, ()),
        Term::Path { endpoints, witness } => proof::lower_path(ctx, endpoints, witness, ()),
        Term::Equiv { a, b, path } => proof::lower_equiv(ctx, a, b, path, ()),
    }
}

/// Stub root lower function when `inkwell` is not enabled.
#[cfg(not(feature = "inkwell"))]
pub fn lower_root<'ctx>(_ctx: &mut LlvmContext<'ctx>, _term: &Term) -> Result<(), CompileError> {
    Ok(())
}
