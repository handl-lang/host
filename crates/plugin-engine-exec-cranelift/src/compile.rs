//! # `Term` -> cranelift IR compilation (ADR-0052)
//!
//! This module compiles
//! [`Term`](handl_language::Term) data — the arrow-calculus kernel — to
//! native code via cranelift, targeting the six combinators (`Arr`, `Seq`,
//! `First`, `Choice`, `Loop`, `App`).
//!
//! ## Compilation strategy
//!
//! Each node's `Term` is baked into the IR at **compile time** — no
//! runtime scheme lookup occurs. The compiled function has signature
//! `fn(*mut EvalCtx, *mut Value) -> *mut Value`, where the second parameter
//! is the `incoming` value flowing through the arrow (top-level calls pass
//! a boxed `Null`).
//!
//! - **Control flow is native**: `Choice` compiles to native `brif`/`jump`
//!   with a merge block; `Loop` compiles to native back-edge blocks with a
//!   max-iteration counter in a stack slot.
//! - **Value operations call Rust helpers**: pair/Either construction and
//!   extraction, native op dispatch, variable load/store, child evaluation,
//!   and closure application are native calls into `extern "C"` `helper_v2_*`
//!   functions defined in [`crate`].
//! - **`First` is inline**: unlike the bytecode VM's `FirstOp` sub-program,
//!   cranelift has native control flow, so `First` unpairs the incoming pair,
//!   compiles `inner` inline with `fst` as the incoming value, then re-pairs
//!   the result with `snd`.
//!
//! ## Loop native-code model
//!
//! `Loop { body }` compiles to a native loop with three exit conditions. The
//! incoming value is saved in a local; the initial pair `(incoming, Null)` is
//! constructed and fed to `body`. After each body iteration, the result is
//! inspected by `helper_v2_loop_check`, which returns a tag: `0` = Either
//! left (terminate with the carried value), `1` = Either right (loop back with
//! the new state), `2` = pair (single-step — terminate with `fst`). For the
//! loop-back case, a new pair `(saved_incoming, new_state)` is constructed and
//! control jumps back to the body block. A max-iteration counter (100_000) in
//! a stack slot prevents infinite loops, matching the tree-walk and bytecode
//! VM guards.
//!
//! ## Derived step ops
//!
//! All four derived step ops from ADR-0052 are supported:
//!
//! - **`loop_step`** (the ergonomic `loop` surface lowering): its carried
//!   sub-scheme arg (the body closure) is compiled inline like any other
//!   `Arr.arg`; the `Loop` combinator passes the current pair `(in, state)`
//!   as `incoming` to `apply_native`, which dispatches to
//!   `prim_v2_loop_step`. That helper applies the body closure via
//!   `apply_closure_value` (a recursive call into the closure body's
//!   compiled native function).
//!
//! - **`while_step`/`for_step`/`fix_step`** (built by
//!   `plugin-lib-stdlib::arrow_iter::{while_loop,for_loop,fix_loop}`):
//!   these carry cond/body sub-schemes that must be re-evaluated **per
//!   iteration** with the current value (extracted from the loop pair) as
//!   incoming. The compiler calls `helper_v2_derived_step_arg_incoming` to
//!   extract the appropriate incoming value from the pair at runtime, then
//!   compiles the args inline with that value as incoming. The compiled args
//!   run each iteration (they're inline in the loop body block), using the
//!   runtime arg-incoming value. `apply_native` still receives the full pair
//!   as incoming (the op handlers extract current/state from it).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cranelift::codegen::ir::{FuncRef, UserFuncName};
use cranelift::codegen::isa::TargetIsa;
use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};
use handl_language::Value as JsonValue;
use handl_language::{ArrSource, Ast, LangError, Language, SchemeVersion, Term};
use plugin_engine::TermData;

/// Type alias for a compiled native function pointer.
///
/// The function signature is `fn(*mut u8, *mut u8) -> *mut u8`, where the
/// actual types are `*mut EvalCtx`, `*mut Value` (incoming), and `*mut Value`
/// (result). The raw `*mut u8` is used because cranelift works with opaque
/// pointers. Arrows flow an `incoming` value through the computation.
pub type CompiledFn = unsafe extern "C" fn(*mut u8, *mut u8) -> *mut u8;

/// The backend holding compiled code alive.
///
/// For JIT mode, the [`JITModule`] must be kept alive so the compiled code
/// remains in memory (the function pointers point into JIT-allocated memory).
/// For Object mode, the code has already been written to disk by
/// `ObjectModule::finish` (which consumes the module), so there is nothing to
/// keep alive — the variant is unit.
pub enum CompiledBackend {
    /// The JIT module, kept alive so compiled code remains in memory.
    Jit(JITModule),
    /// Object mode — the module was consumed by `finish` to emit the file.
    Object,
}

/// Result of compilation: function pointers + constant pools + the
/// backend (kept alive to ensure JIT-compiled code remains in memory).
///
/// The `compiled` map holds function pointers (signature
/// `fn(*mut EvalCtx, *mut Value) -> *mut Value`).
pub struct CompiledResult {
    /// Per-node compiled function pointers, keyed by arena index.
    pub compiled: HashMap<usize, CompiledFn>,
    /// Literal constant pool: values referenced by `const` ops.
    pub literals: Vec<JsonValue>,
    /// Name constant pool: variable names, native op names, foreign names.
    pub names: Vec<String>,
    /// Backend state (kept alive for JIT; unit for Object).
    _backend: CompiledBackend,
}

/// Metadata returned by object-file compilation.
///
/// The emitted `.o` file contains native code for every node; helper functions
/// are declared as external imports resolved at link time. This struct carries
/// the constant pools and per-node function names needed to link and drive the
/// emitted code, but no function pointers (the code lives in the file).
pub struct ObjectResult {
    /// Literal constant pool: values referenced by `const` ops.
    pub literals: Vec<JsonValue>,
    /// Name constant pool: variable names, native op names, foreign names.
    pub names: Vec<String>,
    /// Path the object file was written to.
    pub output_path: PathBuf,
    /// Per-node function symbol names (`node_N`), keyed by arena index, for
    /// linking.
    pub function_names: HashMap<usize, String>,
}

/// Compilation mode for the compiler: in-memory JIT or on-disk object file.
///
/// Selects which cranelift backend the compilation logic targets. Mirrors
/// the [`crate::compile::CompileMode`] but produces function pointers.
#[derive(Debug, Clone)]
pub enum CompileMode {
    /// In-memory JIT — compile and execute via function pointers. Helper
    /// addresses are registered with the JIT builder; after finalization,
    /// function pointers are available for direct calls.
    Jit,
    /// On-disk object file — compile to a relocatable `.o` file for linking.
    /// Helpers are declared as external imports resolved by the linker at
    /// link time. The emitted file contains native code for every node; there
    /// are no in-memory function pointers.
    Object {
        /// Path to write the emitted object file to.
        output_path: PathBuf,
    },
}

// ---------------------------------------------------------------------------
// Constant pools (v2: co-located with the engine helpers)
// ---------------------------------------------------------------------------

/// Accumulates literal values and string names during compilation.
struct ConstantPools {
    /// Literal values referenced by `const` ops.
    literals: Vec<JsonValue>,
    /// String names: variable names, native op names, foreign names.
    names: Vec<String>,
}

impl ConstantPools {
    fn new() -> Self {
        Self {
            literals: Vec::new(),
            names: Vec::new(),
        }
    }

    /// Add a literal to the pool and return its index.
    fn add_literal(&mut self, v: JsonValue) -> usize {
        let idx = self.literals.len();
        self.literals.push(v);
        idx
    }

    /// Add a name to the pool and return its index (deduplicated).
    fn add_name(&mut self, name: &str) -> usize {
        if let Some(idx) = self.names.iter().position(|n| n == name) {
            return idx;
        }
        let idx = self.names.len();
        self.names.push(name.to_string());
        idx
    }
}

// ---------------------------------------------------------------------------
// Helper function references (v2)
// ---------------------------------------------------------------------------

/// `FuncId`s for the helper imports, declared in the module.
struct HelperIds {
    push_literal: FuncId,
    load_var: FuncId,
    load_payload_var: FuncId,
    load_payload: FuncId,
    eval_child: FuncId,
    extend_env: FuncId,
    extend_env_payload: FuncId,
    pop_scope: FuncId,
    drop_value: FuncId,
    apply_closure: FuncId,
    call_foreign: FuncId,
    call_ffi_raw: FuncId,
    pair: FuncId,
    unpair_fst: FuncId,
    unpair_snd: FuncId,
    either_left: FuncId,
    either_right: FuncId,
    either_is_left: FuncId,
    either_value: FuncId,
    apply_native: FuncId,
    int_arith: FuncId,
    loop_check: FuncId,
    derived_step_arg_incoming: FuncId,
    clone: FuncId,
}

/// `FuncRef`s for the helpers, declared in a specific function.
///
/// Some fields (`drop_value`, `either_left`, `either_right`) are declared but
/// not currently emitted in IR — they are available for future optimizations
/// (e.g. explicit drop calls, direct Either construction from native ops).
/// They are kept to ensure all helpers are declared in every function,
/// maintaining a consistent import set.
#[allow(dead_code)]
struct HelperRefs {
    push_literal: FuncRef,
    load_var: FuncRef,
    load_payload_var: FuncRef,
    load_payload: FuncRef,
    eval_child: FuncRef,
    extend_env: FuncRef,
    extend_env_payload: FuncRef,
    pop_scope: FuncRef,
    drop_value: FuncRef,
    apply_closure: FuncRef,
    call_foreign: FuncRef,
    call_ffi_raw: FuncRef,
    pair: FuncRef,
    unpair_fst: FuncRef,
    unpair_snd: FuncRef,
    either_left: FuncRef,
    either_right: FuncRef,
    either_is_left: FuncRef,
    either_value: FuncRef,
    apply_native: FuncRef,
    int_arith: FuncRef,
    loop_check: FuncRef,
    derived_step_arg_incoming: FuncRef,
    clone: FuncRef,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Compile all nodes in the AST using the given [`CompileMode`].
///
/// This is the unified entry point dispatching to the JIT or object backend.
/// For [`CompileMode::Jit`], the result holds callable function pointers.
/// For [`CompileMode::Object`], the object file is written to `output_path`
/// and the result's [`compiled`](CompiledResult::compiled) map is empty.
///
/// # Errors
///
/// Returns [`LangError`] on unsupported scheme version, cranelift setup or
/// compilation failure, or object-file emission/write failure (Object mode).
pub fn compile_all_nodes(
    lang: &Language,
    ast: &Ast,
    mode: &CompileMode,
    ffi_ops: &HashMap<String, handl_plugin_ffi::FfiOpFunction>,
) -> Result<CompiledResult, LangError> {
    match mode {
        CompileMode::Jit => compile_all_nodes_jit(lang, ast, ffi_ops),
        CompileMode::Object { output_path } => {
            let obj = compile_all_nodes_object(lang, ast, output_path)?;
            Ok(CompiledResult {
                compiled: HashMap::new(),
                literals: obj.literals,
                names: obj.names,
                _backend: CompiledBackend::Object,
            })
        }
    }
}

/// Compile all nodes in the AST to in-memory native functions (JIT).
///
/// Each node is compiled to a function with signature `fn(*mut EvalCtx, *mut
/// Value) -> *mut Value`. The scheme for each node is lowered to `Term`
/// at compile time and baked into the IR. V2 helper functions are registered
/// with the JIT builder so the compiled code can call them directly.
///
/// # Errors
///
/// Returns [`LangError`] on unsupported scheme version or cranelift JIT
/// setup/compilation failure.
pub fn compile_all_nodes_jit(
    lang: &Language,
    ast: &Ast,
    ffi_ops: &HashMap<String, handl_plugin_ffi::FfiOpFunction>,
) -> Result<CompiledResult, LangError> {
    let mut builder = JITBuilder::new(default_libcall_names())
        .map_err(|e| LangError::engine(format!("JIT setup error: {}", e)))?;
    // Register all helper symbols.
    builder.symbol(
        "helper_v2_push_literal",
        crate::helper_v2_push_literal as *const u8,
    );
    builder.symbol("helper_v2_load_var", crate::helper_v2_load_var as *const u8);
    builder.symbol(
        "helper_v2_load_payload_var",
        crate::helper_v2_load_payload_var as *const u8,
    );
    builder.symbol(
        "helper_v2_load_payload",
        crate::helper_v2_load_payload as *const u8,
    );
    builder.symbol(
        "helper_v2_eval_child",
        crate::helper_v2_eval_child as *const u8,
    );
    builder.symbol(
        "helper_v2_extend_env",
        crate::helper_v2_extend_env as *const u8,
    );
    builder.symbol(
        "helper_v2_extend_env_payload",
        crate::helper_v2_extend_env_payload as *const u8,
    );
    builder.symbol(
        "helper_v2_pop_scope",
        crate::helper_v2_pop_scope as *const u8,
    );
    builder.symbol(
        "helper_v2_drop_value",
        crate::helper_v2_drop_value as *const u8,
    );
    builder.symbol(
        "helper_v2_apply_closure",
        crate::helper_v2_apply_closure as *const u8,
    );
    builder.symbol(
        "helper_v2_call_foreign",
        crate::helper_v2_call_foreign as *const u8,
    );
    builder.symbol(
        "helper_v2_call_ffi_raw",
        crate::helper_v2_call_ffi_raw as *const u8,
    );
    builder.symbol("helper_v2_pair", crate::helper_v2_pair as *const u8);
    builder.symbol(
        "helper_v2_unpair_fst",
        crate::helper_v2_unpair_fst as *const u8,
    );
    builder.symbol(
        "helper_v2_unpair_snd",
        crate::helper_v2_unpair_snd as *const u8,
    );
    builder.symbol(
        "helper_v2_either_left",
        crate::helper_v2_either_left as *const u8,
    );
    builder.symbol(
        "helper_v2_either_right",
        crate::helper_v2_either_right as *const u8,
    );
    builder.symbol(
        "helper_v2_either_is_left",
        crate::helper_v2_either_is_left as *const u8,
    );
    builder.symbol(
        "helper_v2_either_value",
        crate::helper_v2_either_value as *const u8,
    );
    builder.symbol(
        "helper_v2_apply_native",
        crate::helper_v2_apply_native as *const u8,
    );
    builder.symbol(
        "helper_v2_int_arith",
        crate::helper_v2_int_arith as *const u8,
    );
    builder.symbol(
        "helper_v2_loop_check",
        crate::helper_v2_loop_check as *const u8,
    );
    builder.symbol(
        "helper_v2_derived_step_arg_incoming",
        crate::helper_v2_derived_step_arg_incoming as *const u8,
    );
    builder.symbol("helper_v2_clone", crate::helper_v2_clone as *const u8);

    let mut module = JITModule::new(builder);
    let (func_ids, pools) = compile_core_v2(&mut module, lang, ast, Linkage::Local, ffi_ops)?;

    module
        .finalize_definitions()
        .map_err(|e| LangError::engine(format!("finalize error: {}", e)))?;

    let mut compiled = HashMap::new();
    for (i, func_id) in &func_ids {
        let ptr = module.get_finalized_function(*func_id);
        // SAFETY: ptr is the start address of JIT-compiled machine code with
        // signature fn(*mut u8, *mut u8) -> *mut u8, as defined by the
        // cranelift signature above. The JITModule is kept alive in
        // CompiledBackend::Jit so the code remains valid.
        compiled.insert(*i, unsafe {
            std::mem::transmute::<*const u8, CompiledFn>(ptr)
        });
    }

    Ok(CompiledResult {
        compiled,
        literals: pools.literals,
        names: pools.names,
        _backend: CompiledBackend::Jit(module),
    })
}

/// Compile all nodes in the AST to an on-disk relocatable object file (v2).
///
/// Uses [`ObjectModule`] instead of [`JITModule`]. V2 helpers are declared as
/// external imports resolved by the linker at link time. After compilation and
/// finalization, `ObjectModule::finish` produces the object file, which is
/// written to `output_path`.
///
/// # Errors
///
/// Returns [`LangError`] on unsupported scheme version, cranelift object
/// setup/compilation failure, or object-file emission/write failure.
pub fn compile_all_nodes_object(
    lang: &Language,
    ast: &Ast,
    output_path: &Path,
) -> Result<ObjectResult, LangError> {
    let isa = make_host_isa()?;
    let builder = ObjectBuilder::new(isa, "handl_compiled_v2.o", default_libcall_names())
        .map_err(|e| LangError::engine(format!("object builder error: {}", e)))?;
    let mut module = ObjectModule::new(builder);

    let (func_ids, pools) = compile_core_v2(
        &mut module,
        lang,
        ast,
        Linkage::Export,
        &std::collections::HashMap::new(),
    )?;

    let function_names: HashMap<usize, String> = func_ids
        .iter()
        .map(|(i, _)| (*i, format!("node_{}", i)))
        .collect();

    let product = module.finish();
    let bytes = product
        .emit()
        .map_err(|e| LangError::engine(format!("object emit error: {}", e)))?;

    std::fs::write(output_path, &bytes)
        .map_err(|e| LangError::engine(format!("write object file error: {}", e)))?;

    Ok(ObjectResult {
        literals: pools.literals,
        names: pools.names,
        output_path: output_path.to_path_buf(),
        function_names,
    })
}

// ---------------------------------------------------------------------------
// Shared core compilation (generic over Module)
// ---------------------------------------------------------------------------

/// Build a host-target ISA for the object backend.
fn make_host_isa() -> Result<Arc<dyn TargetIsa>, LangError> {
    let isa_builder = cranelift::native::builder()
        .map_err(|e| LangError::engine(format!("native isa builder error: {}", e)))?;
    let flags = settings::Flags::new(settings::builder());
    isa_builder
        .finish(flags)
        .map_err(|e| LangError::engine(format!("isa finish error: {}", e)))
}

/// The backend-agnostic v2 core: declare helpers + node functions, and compile
/// (define) each node by compiling its `Term`.
///
/// Generic over `M: Module` so the same IR-generation code drives both
/// [`JITModule`] and [`ObjectModule`]. Returns the per-node `FuncId`s and the
/// accumulated constant pools.
///
/// # Errors
///
/// Returns [`LangError`] on unsupported scheme version or any cranelift
/// declare/compile failure.
fn compile_core_v2<M: Module>(
    module: &mut M,
    lang: &Language,
    ast: &Ast,
    node_linkage: Linkage,
    ffi_ops: &HashMap<String, handl_plugin_ffi::FfiOpFunction>,
) -> Result<(HashMap<usize, FuncId>, ConstantPools), LangError> {
    // Term version check — v2 only.
    if !SchemeVersion::v2().supports(&lang.scheme_version) {
        return Err(LangError::engine(format!(
            "unsupported scheme version: {} (v2 only)",
            lang.scheme_version
        )));
    }

    let ptr_type = module.isa().pointer_type();
    let helper_ids = declare_helper_imports_v2(module, ptr_type)?;

    // Declare all node functions first (signature: (ptr, ptr) -> ptr).
    let mut func_ids = HashMap::new();
    for i in 0..ast.len() {
        let name = format!("node_{}", i);
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(ptr_type));
        sig.params.push(AbiParam::new(ptr_type));
        sig.returns.push(AbiParam::new(ptr_type));
        let func_id = module
            .declare_function(&name, node_linkage, &sig)
            .map_err(|e| LangError::engine(format!("declare error: {}", e)))?;
        func_ids.insert(i, func_id);
    }

    // Compile each node.
    let mut pools = ConstantPools::new();
    let mut func_ctx = FunctionBuilderContext::new();
    for i in 0..ast.len() {
        let node = &ast.nodes()[i];
        let scheme_opt = lang.eval.get_by_identity(&node.kind);
        let func_id = func_ids[&i];
        compile_node(
            module,
            func_id,
            scheme_opt.map(|d| &d.scheme),
            &helper_ids,
            &mut pools,
            ptr_type,
            &mut func_ctx,
            ffi_ops,
        )?;
    }

    Ok((func_ids, pools))
}

// ---------------------------------------------------------------------------
// Helper declaration
// ---------------------------------------------------------------------------

/// Declare all helper functions as imports in the module.
///
/// Generic over `M: Module` so the same declarations work for both
/// [`JITModule`] (resolved via registered symbols) and [`ObjectModule`]
/// (resolved by the linker at link time). The symbol names (`helper_v2_*`)
/// must match the `extern "C"` helper function names in [`crate`] exactly.
fn declare_helper_imports_v2<M: Module>(
    module: &mut M,
    ptr_type: types::Type,
) -> Result<HelperIds, LangError> {
    // (ctx, lit_idx) -> ptr
    let mut sig_push_literal = module.make_signature();
    sig_push_literal.params.push(AbiParam::new(ptr_type));
    sig_push_literal.params.push(AbiParam::new(types::I64));
    sig_push_literal.returns.push(AbiParam::new(ptr_type));

    // (ctx) -> ptr
    let mut sig_1arg_ret = module.make_signature();
    sig_1arg_ret.params.push(AbiParam::new(ptr_type));
    sig_1arg_ret.returns.push(AbiParam::new(ptr_type));

    // (ctx) -> void
    let mut sig_1arg_void = module.make_signature();
    sig_1arg_void.params.push(AbiParam::new(ptr_type));

    // (ctx, name_idx, val) -> ptr  (extend_env)
    let mut sig_extend_env = module.make_signature();
    sig_extend_env.params.push(AbiParam::new(ptr_type));
    sig_extend_env.params.push(AbiParam::new(types::I64));
    sig_extend_env.params.push(AbiParam::new(ptr_type));
    sig_extend_env.returns.push(AbiParam::new(ptr_type));

    // (val) -> void  (drop_value)
    let mut sig_drop = module.make_signature();
    sig_drop.params.push(AbiParam::new(ptr_type));

    // (ctx, val) -> ptr  (extend_env_payload, unpair_fst, unpair_snd,
    //   either_left, either_right, either_value)
    let mut sig_2arg_ret = module.make_signature();
    sig_2arg_ret.params.push(AbiParam::new(ptr_type));
    sig_2arg_ret.params.push(AbiParam::new(ptr_type));
    sig_2arg_ret.returns.push(AbiParam::new(ptr_type));

    // (ctx, val) -> i8  (either_is_left)
    let mut sig_is_left = module.make_signature();
    sig_is_left.params.push(AbiParam::new(ptr_type));
    sig_is_left.params.push(AbiParam::new(ptr_type));
    sig_is_left.returns.push(AbiParam::new(types::I8));

    // (ctx, a, b) -> ptr  (pair)
    let mut sig_pair = module.make_signature();
    sig_pair.params.push(AbiParam::new(ptr_type));
    sig_pair.params.push(AbiParam::new(ptr_type));
    sig_pair.params.push(AbiParam::new(ptr_type));
    sig_pair.returns.push(AbiParam::new(ptr_type));

    // (ctx, child_n, incoming, or_null) -> ptr  (eval_child)
    let mut sig_eval_child = module.make_signature();
    sig_eval_child.params.push(AbiParam::new(ptr_type));
    sig_eval_child.params.push(AbiParam::new(types::I64));
    sig_eval_child.params.push(AbiParam::new(ptr_type));
    sig_eval_child.params.push(AbiParam::new(types::I64));
    sig_eval_child.returns.push(AbiParam::new(ptr_type));

    // (ctx, callee, args, n) -> ptr  (apply_closure)
    let mut sig_apply_closure = module.make_signature();
    sig_apply_closure.params.push(AbiParam::new(ptr_type));
    sig_apply_closure.params.push(AbiParam::new(ptr_type));
    sig_apply_closure.params.push(AbiParam::new(ptr_type));
    sig_apply_closure.params.push(AbiParam::new(types::I64));
    sig_apply_closure.returns.push(AbiParam::new(ptr_type));

    // (ctx, op_idx, args, n, incoming) -> ptr  (apply_native)
    let mut sig_apply_native = module.make_signature();
    sig_apply_native.params.push(AbiParam::new(ptr_type));
    sig_apply_native.params.push(AbiParam::new(types::I64));
    sig_apply_native.params.push(AbiParam::new(ptr_type));
    sig_apply_native.params.push(AbiParam::new(types::I64));
    sig_apply_native.params.push(AbiParam::new(ptr_type));
    sig_apply_native.returns.push(AbiParam::new(ptr_type));

    // (ctx, op_code, op_idx, a_ptr, b_ptr, incoming) -> ptr  (int_arith)
    let mut sig_int_arith = module.make_signature();
    sig_int_arith.params.push(AbiParam::new(ptr_type));
    sig_int_arith.params.push(AbiParam::new(types::I64));
    sig_int_arith.params.push(AbiParam::new(types::I64));
    sig_int_arith.params.push(AbiParam::new(ptr_type));
    sig_int_arith.params.push(AbiParam::new(ptr_type));
    sig_int_arith.params.push(AbiParam::new(ptr_type));
    sig_int_arith.returns.push(AbiParam::new(ptr_type));

    // (ctx, result) -> i8  (loop_check)
    let mut sig_loop_check = module.make_signature();
    sig_loop_check.params.push(AbiParam::new(ptr_type));
    sig_loop_check.params.push(AbiParam::new(ptr_type));
    sig_loop_check.returns.push(AbiParam::new(types::I8));

    // (ctx, pair, op_idx) -> *mut Value  (derived_step_arg_incoming)
    let mut sig_derived_step = module.make_signature();
    sig_derived_step.params.push(AbiParam::new(ptr_type));
    sig_derived_step.params.push(AbiParam::new(ptr_type));
    sig_derived_step.params.push(AbiParam::new(types::I64));
    sig_derived_step.returns.push(AbiParam::new(ptr_type));

    let declare = |m: &mut M, name: &str, sig: &Signature| -> Result<FuncId, LangError> {
        m.declare_function(name, Linkage::Import, sig)
            .map_err(|e| LangError::engine(format!("declare import error: {}", e)))
    };

    Ok(HelperIds {
        push_literal: declare(module, "helper_v2_push_literal", &sig_push_literal)?,
        load_var: declare(module, "helper_v2_load_var", &sig_push_literal)?,
        load_payload_var: declare(module, "helper_v2_load_payload_var", &sig_1arg_ret)?,
        load_payload: declare(module, "helper_v2_load_payload", &sig_1arg_ret)?,
        eval_child: declare(module, "helper_v2_eval_child", &sig_eval_child)?,
        extend_env: declare(module, "helper_v2_extend_env", &sig_extend_env)?,
        extend_env_payload: declare(module, "helper_v2_extend_env_payload", &sig_2arg_ret)?,
        pop_scope: declare(module, "helper_v2_pop_scope", &sig_1arg_void)?,
        drop_value: declare(module, "helper_v2_drop_value", &sig_drop)?,
        apply_closure: declare(module, "helper_v2_apply_closure", &sig_apply_closure)?,
        call_foreign: declare(module, "helper_v2_call_foreign", &sig_apply_closure)?,
        call_ffi_raw: declare(module, "helper_v2_call_ffi_raw", &sig_apply_closure)?,
        pair: declare(module, "helper_v2_pair", &sig_pair)?,
        unpair_fst: declare(module, "helper_v2_unpair_fst", &sig_2arg_ret)?,
        unpair_snd: declare(module, "helper_v2_unpair_snd", &sig_2arg_ret)?,
        either_left: declare(module, "helper_v2_either_left", &sig_2arg_ret)?,
        either_right: declare(module, "helper_v2_either_right", &sig_2arg_ret)?,
        either_is_left: declare(module, "helper_v2_either_is_left", &sig_is_left)?,
        either_value: declare(module, "helper_v2_either_value", &sig_2arg_ret)?,
        apply_native: declare(module, "helper_v2_apply_native", &sig_apply_native)?,
        int_arith: declare(module, "helper_v2_int_arith", &sig_int_arith)?,
        loop_check: declare(module, "helper_v2_loop_check", &sig_loop_check)?,
        derived_step_arg_incoming: declare(
            module,
            "helper_v2_derived_step_arg_incoming",
            &sig_derived_step,
        )?,
        clone: declare(module, "helper_v2_clone", &sig_2arg_ret)?,
    })
}

// ---------------------------------------------------------------------------
// Per-node compilation
// ---------------------------------------------------------------------------

/// Compile a single node's scheme into a native function.
///
/// Compiles a node's [`Term`] to IR. The function signature is
/// `fn(*mut EvalCtx, *mut Value) -> *mut Value` (ctx, incoming) -> result.
fn compile_node<M: Module>(
    module: &mut M,
    func_id: FuncId,
    scheme: Option<&Term>,
    helper_ids: &HelperIds,
    pools: &mut ConstantPools,
    ptr_type: types::Type,
    func_ctx: &mut FunctionBuilderContext,
    ffi_ops: &HashMap<String, handl_plugin_ffi::FfiOpFunction>,
) -> Result<(), LangError> {
    let mut ctx = module.make_context();
    ctx.func.signature = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(ptr_type));
        sig.params.push(AbiParam::new(ptr_type));
        sig.returns.push(AbiParam::new(ptr_type));
        sig
    };
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, func_ctx);

        let helpers = HelperRefs {
            push_literal: module.declare_func_in_func(helper_ids.push_literal, &mut builder.func),
            load_var: module.declare_func_in_func(helper_ids.load_var, &mut builder.func),
            load_payload_var: module
                .declare_func_in_func(helper_ids.load_payload_var, &mut builder.func),
            load_payload: module.declare_func_in_func(helper_ids.load_payload, &mut builder.func),
            eval_child: module.declare_func_in_func(helper_ids.eval_child, &mut builder.func),
            extend_env: module.declare_func_in_func(helper_ids.extend_env, &mut builder.func),
            extend_env_payload: module
                .declare_func_in_func(helper_ids.extend_env_payload, &mut builder.func),
            pop_scope: module.declare_func_in_func(helper_ids.pop_scope, &mut builder.func),
            drop_value: module.declare_func_in_func(helper_ids.drop_value, &mut builder.func),
            apply_closure: module.declare_func_in_func(helper_ids.apply_closure, &mut builder.func),
            call_foreign: module.declare_func_in_func(helper_ids.call_foreign, &mut builder.func),
            call_ffi_raw: module.declare_func_in_func(helper_ids.call_ffi_raw, &mut builder.func),
            pair: module.declare_func_in_func(helper_ids.pair, &mut builder.func),
            unpair_fst: module.declare_func_in_func(helper_ids.unpair_fst, &mut builder.func),
            unpair_snd: module.declare_func_in_func(helper_ids.unpair_snd, &mut builder.func),
            either_left: module.declare_func_in_func(helper_ids.either_left, &mut builder.func),
            either_right: module.declare_func_in_func(helper_ids.either_right, &mut builder.func),
            either_is_left: module
                .declare_func_in_func(helper_ids.either_is_left, &mut builder.func),
            either_value: module.declare_func_in_func(helper_ids.either_value, &mut builder.func),
            apply_native: module.declare_func_in_func(helper_ids.apply_native, &mut builder.func),
            int_arith: module.declare_func_in_func(helper_ids.int_arith, &mut builder.func),
            loop_check: module.declare_func_in_func(helper_ids.loop_check, &mut builder.func),
            derived_step_arg_incoming: module
                .declare_func_in_func(helper_ids.derived_step_arg_incoming, &mut builder.func),
            clone: module.declare_func_in_func(helper_ids.clone, &mut builder.func),
        };

        // Entry block: two params (ctx, incoming).
        let entry_block = builder.create_block();
        builder.append_block_param(entry_block, ptr_type);
        builder.append_block_param(entry_block, ptr_type);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let ctx_param = builder.block_params(entry_block)[0];
        let incoming_param = builder.block_params(entry_block)[1];

        let result = match scheme {
            Some(s) => {
                // Compile the Term directly (already v2).
                let (val, _pushed) = compile_scheme(
                    &mut builder,
                    s,
                    ctx_param,
                    incoming_param,
                    &helpers,
                    pools,
                    ptr_type,
                    ffi_ops,
                )?;
                val
            }
            None => {
                // No-scheme fallback: return node.payload directly.
                let call = builder.ins().call(helpers.load_payload, &[ctx_param]);
                builder.inst_results(call)[0]
            }
        };

        builder.ins().return_(&[result]);
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| LangError::engine(format!("define function error: {}", e)))?;
    module.clear_context(&mut ctx);
    Ok(())
}

// ---------------------------------------------------------------------------
// Term compilation (recursive)
// ---------------------------------------------------------------------------

/// Compile a [`Term`] into cranelift IR, returning the result `Value` and
/// whether a scope was pushed that the caller is responsible for popping.
///
/// `incoming_val` is the cranelift IR value representing the incoming value
/// pointer flowing through the arrow. After this function returns, the builder
/// is in the block containing the result value.
///
/// This is the `&Term` entry point: it converts the scheme to [`TermData`]
/// (ADR-0103 Phase 6) and delegates to [`compile_scheme_td`], which dispatches
/// on the `__tag__` field instead of matching the `Term` enum.
fn compile_scheme(
    builder: &mut FunctionBuilder,
    scheme: &Term,
    ctx_param: Value,
    incoming_val: Value,
    helpers: &HelperRefs,
    pools: &mut ConstantPools,
    ptr_type: types::Type,
    ffi_ops: &HashMap<String, handl_plugin_ffi::FfiOpFunction>,
) -> Result<(Value, bool), LangError> {
    let td = TermData::from_term(scheme);
    compile_scheme_td(
        builder,
        &td,
        ctx_param,
        incoming_val,
        helpers,
        pools,
        ptr_type,
        ffi_ops,
    )
}

/// The [`TermData`]-driven counterpart of [`compile_scheme`].
///
/// Dispatches on [`TermData::tag`] and reads variant fields via the
/// `get`/`get_str`/`get_term`/`get_term_list` accessors. Derived forms
/// (`CellId`/`Src`/`Tgt`/`CellCompV`/`Whisker`/`Const`/`RecordProj`) are
/// rewritten to their `Op`/`Arr` lowering using [`TermData`] constructors and
/// recompiled recursively, so no `Term` enum value is constructed for them.
fn compile_scheme_td(
    builder: &mut FunctionBuilder,
    td: &TermData,
    ctx_param: Value,
    incoming_val: Value,
    helpers: &HelperRefs,
    pools: &mut ConstantPools,
    ptr_type: types::Type,
    ffi_ops: &HashMap<String, handl_plugin_ffi::FfiOpFunction>,
) -> Result<(Value, bool), LangError> {
    match td.tag() {
        "Arr" => {
            let value = td
                .get("value")
                .cloned()
                .expect("Arr term missing 'value' field");
            let lit_idx = pools.add_literal(value);
            let lit_idx_val = builder.ins().iconst(types::I64, lit_idx as i64);
            let call = builder
                .ins()
                .call(helpers.push_literal, &[ctx_param, lit_idx_val]);
            Ok((builder.inst_results(call)[0], false))
        }

        "Op" => {
            let op = td
                .get_str("name")
                .expect("Op term missing 'name' field")
                .to_string();
            let args = td.get_term_list("args");
            let source = td
                .arr_source()
                .expect("Op term missing or malformed 'source' field");
            // `lit` is stored as `null` when `None`; treat Null as absent.
            let lit = td.get("lit").filter(|v| !v.is_null());
            compile_arr(
                builder,
                &op,
                &args,
                &source,
                lit,
                ctx_param,
                incoming_val,
                helpers,
                pools,
                ptr_type,
                ffi_ops,
            )
        }

        "Seq" => {
            let fst = td.get_term("fst").expect("Seq term missing 'fst' field");
            let snd = td.get_term("snd").expect("Seq term missing 'snd' field");
            let (fst_val, fst_pushed) = compile_scheme_td(
                builder,
                &fst,
                ctx_param,
                incoming_val,
                helpers,
                pools,
                ptr_type,
                ffi_ops,
            )?;
            let (snd_val, snd_pushed) = compile_scheme_td(
                builder, &snd, ctx_param, fst_val, helpers, pools, ptr_type, ffi_ops,
            )?;
            if fst_pushed {
                builder.ins().call(helpers.pop_scope, &[ctx_param]);
            }
            Ok((snd_val, snd_pushed))
        }

        "First" => {
            let inner = td
                .get_term("inner")
                .expect("First term missing 'inner' field");
            let fst = {
                let call = builder
                    .ins()
                    .call(helpers.unpair_fst, &[ctx_param, incoming_val]);
                builder.inst_results(call)[0]
            };
            let snd = {
                let call = builder
                    .ins()
                    .call(helpers.unpair_snd, &[ctx_param, incoming_val]);
                builder.inst_results(call)[0]
            };
            // Now that both components are extracted, drop the original pair.
            builder.ins().call(helpers.drop_value, &[incoming_val]);
            let (inner_result, inner_pushed) = compile_scheme_td(
                builder, &inner, ctx_param, fst, helpers, pools, ptr_type, ffi_ops,
            )?;
            if inner_pushed {
                builder.ins().call(helpers.pop_scope, &[ctx_param]);
            }
            let pair_call = builder
                .ins()
                .call(helpers.pair, &[ctx_param, inner_result, snd]);
            let result = builder.inst_results(pair_call)[0];
            Ok((result, false))
        }

        "Choice" => {
            let l = td.get_term("l").expect("Choice term missing 'l' field");
            let r = td.get_term("r").expect("Choice term missing 'r' field");
            let is_left = builder
                .ins()
                .call(helpers.either_is_left, &[ctx_param, incoming_val]);
            let is_left_val = builder.inst_results(is_left)[0];

            let left_block = builder.create_block();
            let right_block = builder.create_block();
            let merge_block = builder.create_block();
            builder.append_block_param(merge_block, ptr_type);

            builder
                .ins()
                .brif(is_left_val, left_block, &[], right_block, &[]);

            // Left branch.
            builder.switch_to_block(left_block);
            builder.seal_block(left_block);
            let left_val = {
                let call = builder
                    .ins()
                    .call(helpers.either_value, &[ctx_param, incoming_val]);
                builder.inst_results(call)[0]
            };
            let (l_result, l_pushed) = compile_scheme_td(
                builder, &l, ctx_param, left_val, helpers, pools, ptr_type, ffi_ops,
            )?;
            if l_pushed {
                builder.ins().call(helpers.pop_scope, &[ctx_param]);
            }
            builder.ins().jump(merge_block, &[l_result]);

            // Right branch.
            builder.switch_to_block(right_block);
            builder.seal_block(right_block);
            let right_val = {
                let call = builder
                    .ins()
                    .call(helpers.either_value, &[ctx_param, incoming_val]);
                builder.inst_results(call)[0]
            };
            let (r_result, r_pushed) = compile_scheme_td(
                builder, &r, ctx_param, right_val, helpers, pools, ptr_type, ffi_ops,
            )?;
            if r_pushed {
                builder.ins().call(helpers.pop_scope, &[ctx_param]);
            }
            builder.ins().jump(merge_block, &[r_result]);

            builder.seal_block(merge_block);
            builder.switch_to_block(merge_block);
            Ok((builder.block_params(merge_block)[0], false))
        }

        "Loop" => {
            let body = td
                .get_term("body")
                .expect("Loop term missing 'body' field");
            compile_loop(
                builder,
                &body,
                ctx_param,
                incoming_val,
                helpers,
                pools,
                ptr_type,
                ffi_ops,
            )
        }

        "App" => {
            let callee = td
                .get_term("callee")
                .expect("App term missing 'callee' field");
            let args = td.get_term_list("args");
            // Clone incoming for the callee so that the callee and the args
            // don't share the same consumable pointer. Without this, if the
            // callee is a consuming combinator (First/Choice/Loop/extend_env)
            // it frees `incoming_val`, and the args would receive a dangling
            // pointer — double-free. The original `incoming_val` leaks if no
            // arg consumes it, which is acceptable per the Phase 7 safety
            // model (documented leaks for intermediate values).
            let callee_incoming = {
                let call = builder
                    .ins()
                    .call(helpers.clone, &[ctx_param, incoming_val]);
                builder.inst_results(call)[0]
            };
            let (callee_val, c_pushed) = compile_scheme_td(
                builder,
                &callee,
                ctx_param,
                callee_incoming,
                helpers,
                pools,
                ptr_type,
                ffi_ops,
            )?;
            if c_pushed {
                builder.ins().call(helpers.pop_scope, &[ctx_param]);
            }
            let arg_ptrs = compile_args_v2(
                builder,
                &args,
                ctx_param,
                incoming_val,
                helpers,
                pools,
                ptr_type,
                ffi_ops,
            )?;
            let n_val = builder.ins().iconst(types::I64, args.len() as i64);
            let call = builder.ins().call(
                helpers.apply_closure,
                &[ctx_param, callee_val, arg_ptrs, n_val],
            );
            Ok((builder.inst_results(call)[0], false))
        }
        // `handle body with target = impl_` (ADR-0076): eliminate the
        // handler at compile time, then compile the resulting handler-free
        // scheme. Uses `eliminate_and_check` to surface unhandled
        // capabilities as a compile-time error, matching the tree-walk,
        // bytecode, and typecheck engines. `eliminate_and_check` operates on
        // the `Term` enum, so the `TermData` is converted back via
        // [`TermData::to_term`] for this one case.
        "Handle" => {
            let term = td
                .to_term()
                .map_err(|e| LangError::engine(e.to_string()))?;
            let eliminated = handl_language::eliminate_and_check(&term)
                .map_err(|e| LangError::engine(e.to_string()))?;
            compile_scheme(
                builder,
                &eliminated,
                ctx_param,
                incoming_val,
                helpers,
                pools,
                ptr_type,
                ffi_ops,
            )
        }

        "Iter" => Err(LangError::NotImplemented),
        "Child" => Err(LangError::NotImplemented),
        "Path" => Err(LangError::NotImplemented),
        "Equiv" => Err(LangError::NotImplemented),
        // HoTT/cell combinators lower to native `cell2_*` ops. The lowering
        // is expressed with [`TermData`] constructors and recompiled, so no
        // `Term` enum value is built.
        "CellId" => {
            let arrow = td
                .get_term("arrow")
                .expect("CellId term missing 'arrow' field");
            compile_scheme_td(
                builder,
                &TermData::op("cell2_id", vec![arrow], &ArrSource::Native, None),
                ctx_param,
                incoming_val,
                helpers,
                pools,
                ptr_type,
                ffi_ops,
            )
        }
        "Src" => {
            let cell = td
                .get_term("cell")
                .expect("Src term missing 'cell' field");
            compile_scheme_td(
                builder,
                &TermData::op("cell2_src", vec![cell], &ArrSource::Native, None),
                ctx_param,
                incoming_val,
                helpers,
                pools,
                ptr_type,
                ffi_ops,
            )
        }
        "Tgt" => {
            let cell = td
                .get_term("cell")
                .expect("Tgt term missing 'cell' field");
            compile_scheme_td(
                builder,
                &TermData::op("cell2_tgt", vec![cell], &ArrSource::Native, None),
                ctx_param,
                incoming_val,
                helpers,
                pools,
                ptr_type,
                ffi_ops,
            )
        }
        "CellCompV" => {
            let first = td
                .get_term("first")
                .expect("CellCompV term missing 'first' field");
            let second = td
                .get_term("second")
                .expect("CellCompV term missing 'second' field");
            compile_scheme_td(
                builder,
                &TermData::op(
                    "cell2_comp_v",
                    vec![first, second],
                    &ArrSource::Native,
                    None,
                ),
                ctx_param,
                incoming_val,
                helpers,
                pools,
                ptr_type,
                ffi_ops,
            )
        }
        "Whisker" => {
            let arrow = td
                .get_term("arrow")
                .expect("Whisker term missing 'arrow' field");
            let cell = td
                .get_term("cell")
                .expect("Whisker term missing 'cell' field");
            let op = match td.get_str("side").unwrap_or("") {
                "Left" => "cell2_whisker_left",
                "Right" => "cell2_whisker_right",
                other => {
                    return Err(LangError::engine(format!(
                        "Whisker: unknown side '{other}'"
                    )))
                }
            };
            compile_scheme_td(
                builder,
                &TermData::op(op, vec![arrow, cell], &ArrSource::Native, None),
                ctx_param,
                incoming_val,
                helpers,
                pools,
                ptr_type,
                ffi_ops,
            )
        }
        "Const" => {
            let value = td
                .get("value")
                .cloned()
                .expect("Const term missing 'value' field");
            compile_scheme_td(
                builder,
                &TermData::arr(value),
                ctx_param,
                incoming_val,
                helpers,
                pools,
                ptr_type,
                ffi_ops,
            )
        }
        "RecordProj" => {
            let record = td
                .get_term("record")
                .expect("RecordProj term missing 'record' field");
            let field = td
                .get_str("field")
                .expect("RecordProj term missing 'field' field")
                .to_string();
            compile_scheme_td(
                builder,
                &TermData::op(
                    "record_proj",
                    vec![record, TermData::arr(JsonValue::String(field))],
                    &ArrSource::Native,
                    None,
                ),
                ctx_param,
                incoming_val,
                helpers,
                pools,
                ptr_type,
                ffi_ops,
            )
        }
        "RecordWith" => Err(LangError::NotImplemented),
        "Tag" => Err(LangError::NotImplemented),
        "Univalence" => Err(LangError::NotImplemented),
        "Glue" => Err(LangError::NotImplemented),
        "Fibrant" => Err(LangError::NotImplemented),
        "JElim" => Err(LangError::NotImplemented),
        "Refine" => Err(LangError::NotImplemented),
        "Cobind" => Err(LangError::NotImplemented),
        "Rewrite" => Err(LangError::NotImplemented),
        other => Err(LangError::engine(format!(
            "compile: unknown Term tag '{other}'"
        ))),
    }
}

/// Compile a `Loop { body }` combinator to native IR.
///
/// See the module-level "Loop native-code model" docs for the full strategy.
fn compile_loop(
    builder: &mut FunctionBuilder,
    body: &TermData,
    ctx_param: Value,
    incoming_val: Value,
    helpers: &HelperRefs,
    pools: &mut ConstantPools,
    ptr_type: types::Type,
    ffi_ops: &HashMap<String, handl_plugin_ffi::FfiOpFunction>,
) -> Result<(Value, bool), LangError> {
    // Save a **clone** of the original incoming value in a stack slot (needed
    // for loop-back to reconstruct pair(original_input, new_state)).  We must
    // clone because `helper_v2_pair` below **consumes** (frees) `incoming_val`.
    // Saving the raw pointer and then freeing it would leave a dangling pointer
    // in the stack slot, causing a double-free on loop-back when the allocator
    // reuses the address.
    //
    // The clone is done via `helper_v2_clone(ctx, incoming)` which reads the
    // incoming value (clone, not consume) and returns a freshly boxed copy.
    let saved_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        ptr_type.bytes() as u32,
        0,
    ));
    let saved_addr = builder.ins().stack_addr(ptr_type, saved_slot, 0);
    let clone_call = builder
        .ins()
        .call(helpers.clone, &[ctx_param, incoming_val]);
    let cloned_incoming = builder.inst_results(clone_call)[0];
    builder
        .ins()
        .store(MemFlags::trusted(), cloned_incoming, saved_addr, 0);

    // Iteration counter slot.
    let counter_slot =
        builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
    let counter_addr = builder.ins().stack_addr(ptr_type, counter_slot, 0);
    let zero = builder.ins().iconst(types::I64, 0);
    builder
        .ins()
        .store(MemFlags::trusted(), zero, counter_addr, 0);

    // Construct the initial pair (incoming, Null).  This consumes (frees)
    // `incoming_val`, which is safe because we saved a clone above.
    let null_lit_idx = pools.add_literal(JsonValue::Null);
    let null_lit_idx_val = builder.ins().iconst(types::I64, null_lit_idx as i64);
    let null_val = {
        let call = builder
            .ins()
            .call(helpers.push_literal, &[ctx_param, null_lit_idx_val]);
        builder.inst_results(call)[0]
    };
    let initial_pair = {
        let call = builder
            .ins()
            .call(helpers.pair, &[ctx_param, incoming_val, null_val]);
        builder.inst_results(call)[0]
    };

    // Loop body block: receives the current pair as a block param.
    let loop_block = builder.create_block();
    builder.append_block_param(loop_block, ptr_type);

    // Done block: receives the result as a block param.
    let done_block = builder.create_block();
    builder.append_block_param(done_block, ptr_type);

    // Jump into the loop with the initial pair.
    builder.ins().jump(loop_block, &[initial_pair]);

    // --- Loop body ---
    builder.switch_to_block(loop_block);
    // NOTE: loop_block is NOT sealed here — it has a back-edge from
    // `right_block` below that is added later. Sealing now would trigger
    // cranelift-frontend's "block already sealed" assertion when the
    // back-edge is added. Seal after the back-edge is in place.
    let current_pair = builder.block_params(loop_block)[0];

    // Increment the iteration counter.
    let cur_count = builder
        .ins()
        .load(types::I64, MemFlags::trusted(), counter_addr, 0);
    let one = builder.ins().iconst(types::I64, 1);
    let new_count = builder.ins().iadd(cur_count, one);
    builder
        .ins()
        .store(MemFlags::trusted(), new_count, counter_addr, 0);

    // Check max iterations (100_000).
    let max_iters = builder.ins().iconst(types::I64, 100_000);
    let over_limit = builder
        .ins()
        .icmp(IntCC::SignedGreaterThan, new_count, max_iters);
    let overflow_block = builder.create_block();
    let body_cont_block = builder.create_block();
    builder
        .ins()
        .brif(over_limit, overflow_block, &[], body_cont_block, &[]);

    // Overflow: set error and jump to done with null.
    builder.switch_to_block(overflow_block);
    builder.seal_block(overflow_block);
    let err_null = builder.ins().iconst(ptr_type, 0);
    builder.ins().jump(done_block, &[err_null]);

    // Body continuation: compile the body with current_pair as incoming.
    builder.switch_to_block(body_cont_block);
    builder.seal_block(body_cont_block);
    let (body_result, body_pushed) = compile_scheme_td(
        builder,
        body,
        ctx_param,
        current_pair,
        helpers,
        pools,
        ptr_type,
        ffi_ops,
    )?;
    if body_pushed {
        builder.ins().call(helpers.pop_scope, &[ctx_param]);
    }

    // loop_check returns i8: 0 = left (terminate), 1 = right (loop back),
    // 2 = pair (single-step).
    let tag = builder
        .ins()
        .call(helpers.loop_check, &[ctx_param, body_result]);
    let tag_val = builder.inst_results(tag)[0];

    // Compare tag against 0 (left) and 1 (right).
    let is_left = builder.ins().icmp_imm(IntCC::Equal, tag_val, 0);
    let left_block = builder.create_block();
    let check_right_block = builder.create_block();
    builder
        .ins()
        .brif(is_left, left_block, &[], check_right_block, &[]);

    // Left: terminate with either_value(body_result).
    builder.switch_to_block(left_block);
    builder.seal_block(left_block);
    let left_out = {
        let call = builder
            .ins()
            .call(helpers.either_value, &[ctx_param, body_result]);
        builder.inst_results(call)[0]
    };
    builder.ins().jump(done_block, &[left_out]);

    // Check right.
    builder.switch_to_block(check_right_block);
    builder.seal_block(check_right_block);
    let is_right = builder.ins().icmp_imm(IntCC::Equal, tag_val, 1);
    let right_block = builder.create_block();
    let single_step_block = builder.create_block();
    builder
        .ins()
        .brif(is_right, right_block, &[], single_step_block, &[]);

    // Right: loop back with pair(saved_incoming, new_state).
    builder.switch_to_block(right_block);
    builder.seal_block(right_block);
    let new_state = {
        let call = builder
            .ins()
            .call(helpers.either_value, &[ctx_param, body_result]);
        builder.inst_results(call)[0]
    };
    let saved_incoming = builder
        .ins()
        .load(ptr_type, MemFlags::trusted(), saved_addr, 0);
    // Clone the saved incoming before passing it to `pair`, because `pair`
    // consumes (frees) its inputs.  The stack slot must retain a valid
    // pointer for subsequent loop-backs; without this clone, the second
    // loop-back would load a dangling pointer and double-free.
    let saved_clone = {
        let call = builder
            .ins()
            .call(helpers.clone, &[ctx_param, saved_incoming]);
        builder.inst_results(call)[0]
    };
    let new_pair = {
        let call = builder
            .ins()
            .call(helpers.pair, &[ctx_param, saved_clone, new_state]);
        builder.inst_results(call)[0]
    };
    builder.ins().jump(loop_block, &[new_pair]);

    // Now that both predecessors of loop_block are in place (the initial
    // entry jump and this back-edge), seal it.
    builder.seal_block(loop_block);

    // Single-step: terminate with unpair_fst(body_result).
    builder.switch_to_block(single_step_block);
    builder.seal_block(single_step_block);
    let step_out = {
        let call = builder
            .ins()
            .call(helpers.unpair_fst, &[ctx_param, body_result]);
        builder.inst_results(call)[0]
    };
    // Drop the original pair (unpair_fst no longer consumes it).
    builder.ins().call(helpers.drop_value, &[body_result]);
    builder.ins().jump(done_block, &[step_out]);

    // --- Done ---
    builder.seal_block(done_block);
    builder.switch_to_block(done_block);
    Ok((builder.block_params(done_block)[0], false))
}

/// Compile an `Arr` combinator. Returns the result `Value` and whether a scope
/// was pushed (`extend_env` pushes; the caller pops).
fn compile_arr(
    builder: &mut FunctionBuilder,
    op: &str,
    args: &[TermData],
    source: &ArrSource,
    lit: Option<&JsonValue>,
    ctx_param: Value,
    incoming_val: Value,
    helpers: &HelperRefs,
    pools: &mut ConstantPools,
    ptr_type: types::Type,
    ffi_ops: &HashMap<String, handl_plugin_ffi::FfiOpFunction>,
) -> Result<(Value, bool), LangError> {
    // Special-case the lit-bearing ops.
    match op {
        "const" => {
            let v = lit.cloned().unwrap_or(JsonValue::Null);
            let lit_idx = pools.add_literal(v);
            let lit_idx_val = builder.ins().iconst(types::I64, lit_idx as i64);
            let call = builder
                .ins()
                .call(helpers.push_literal, &[ctx_param, lit_idx_val]);
            return Ok((builder.inst_results(call)[0], false));
        }
        "proj_env" => {
            let name = lit.and_then(|v| v.as_str()).unwrap_or("");
            if name == "$payload" {
                let call = builder.ins().call(helpers.load_payload_var, &[ctx_param]);
                return Ok((builder.inst_results(call)[0], false));
            }
            let name_idx = pools.add_name(name);
            let name_idx_val = builder.ins().iconst(types::I64, name_idx as i64);
            let call = builder
                .ins()
                .call(helpers.load_var, &[ctx_param, name_idx_val]);
            return Ok((builder.inst_results(call)[0], false));
        }
        "proj_child" | "proj_child_or" => {
            let idx = lit.and_then(|v| v.as_u64()).unwrap_or(0) as i64;
            let idx_val = builder.ins().iconst(types::I64, idx);
            // `proj_child_or` tolerates an absent child (evaluates to null);
            // `proj_child` errors.
            let or_null = builder
                .ins()
                .iconst(types::I64, if op == "proj_child_or" { 1 } else { 0 });
            let call = builder.ins().call(
                helpers.eval_child,
                &[ctx_param, idx_val, incoming_val, or_null],
            );
            return Ok((builder.inst_results(call)[0], false));
        }
        "extend_env" => {
            let name = lit.and_then(|v| v.as_str()).unwrap_or("");
            if name == "$payload" {
                let call = builder
                    .ins()
                    .call(helpers.extend_env_payload, &[ctx_param, incoming_val]);
                return Ok((builder.inst_results(call)[0], true));
            }
            let name_idx = pools.add_name(name);
            let name_idx_val = builder.ins().iconst(types::I64, name_idx as i64);
            let call = builder
                .ins()
                .call(helpers.extend_env, &[ctx_param, name_idx_val, incoming_val]);
            return Ok((builder.inst_results(call)[0], true));
        }
        // ADR-0060: route_on_type takes the value from incoming (or args[0])
        // and a type tag lit. Encode the type tag in the op name so
        // apply_native_v2 can dispatch on it.
        "route_on_type" => {
            let tag = lit.and_then(|v| v.as_str()).unwrap_or("");
            let synthetic_op = format!("route_on_type:{}", tag);
            let op_idx = pools.add_name(&synthetic_op);
            let op_idx_val = builder.ins().iconst(types::I64, op_idx as i64);
            // route_on_type has 0 sub-scheme args; the value comes from
            // incoming. Pass 0 args to apply_native.
            let arg_ptrs = builder.ins().iconst(types::I64, 0);
            let n_val = builder.ins().iconst(types::I64, 0);
            let call = builder.ins().call(
                helpers.apply_native,
                &[ctx_param, op_idx_val, arg_ptrs, n_val, incoming_val],
            );
            return Ok((builder.inst_results(call)[0], false));
        }
        _ => {}
    }

    // General native or foreign op: compile args, then call apply_native /
    // call_foreign. The incoming value is passed as a separate parameter to
    // apply_native (which may or may not use it depending on the op).
    //
    // Derived step ops (`while_step`/`for_step`/`fix_step`) are special:
    // their args (cond/body sub-schemes) must be re-evaluated per iteration
    // with the current value (extracted from the loop pair) as incoming —
    // not the full pair. We call `helper_v2_derived_step_arg_incoming` to
    // extract the appropriate incoming value from the pair at runtime, then
    // compile the args with that value as incoming. The compiled args run
    // each iteration (they're inline in the loop body), using the runtime
    // arg_incoming value. `apply_native` still receives the full pair as
    // incoming (the op handlers extract current/state from it).
    if source == &ArrSource::Native && is_derived_step_op(op) {
        let op_idx = pools.add_name(op);
        let op_idx_val = builder.ins().iconst(types::I64, op_idx as i64);
        let arg_incoming_call = builder.ins().call(
            helpers.derived_step_arg_incoming,
            &[ctx_param, incoming_val, op_idx_val],
        );
        let arg_incoming = builder.inst_results(arg_incoming_call)[0];
        let arg_ptrs = compile_args_v2(
            builder,
            args,
            ctx_param,
            arg_incoming,
            helpers,
            pools,
            ptr_type,
            ffi_ops,
        )?;
        let n_val = builder.ins().iconst(types::I64, args.len() as i64);
        let call = builder.ins().call(
            helpers.apply_native,
            &[ctx_param, op_idx_val, arg_ptrs, n_val, incoming_val],
        );
        return Ok((builder.inst_results(call)[0], false));
    }

    let arg_ptrs = compile_args_v2(
        builder,
        args,
        ctx_param,
        incoming_val,
        helpers,
        pools,
        ptr_type,
        ffi_ops,
    )?;
    let n_val = builder.ins().iconst(types::I64, args.len() as i64);
    match source {
        ArrSource::Native => {
            let op_idx = pools.add_name(op);
            let op_idx_val = builder.ins().iconst(types::I64, op_idx as i64);

            // Fix #7: Fast path for binary arithmetic/comparison ops on
            // integers. When the op is a known arithmetic op and there are
            // exactly 2 args, emit a call to `helper_v2_int_arith` instead of
            // the general `helper_v2_apply_native`. The fast-path helper
            // checks at runtime whether both args are `Int`; if so it does the
            // operation directly (no string dispatch, no Vec allocation, no
            // incoming clone). If either arg is not `Int`, it falls back to
            // the full `apply_native_v2` path.
            //
            // This is a compile-time decision (op name and arg count are
            // known from the Term), so the IR contains only one path.
            if let Some(op_code) = int_arith_op_code(op) {
                if args.len() == 2 {
                    let a_ptr = builder
                        .ins()
                        .load(ptr_type, MemFlags::trusted(), arg_ptrs, 0);
                    let b_ptr = builder.ins().load(
                        ptr_type,
                        MemFlags::trusted(),
                        arg_ptrs,
                        ptr_type.bytes() as i32,
                    );
                    let op_code_val = builder.ins().iconst(types::I64, op_code as i64);
                    let call = builder.ins().call(
                        helpers.int_arith,
                        &[
                            ctx_param,
                            op_code_val,
                            op_idx_val,
                            a_ptr,
                            b_ptr,
                            incoming_val,
                        ],
                    );
                    return Ok((builder.inst_results(call)[0], false));
                }
            }

            let call = builder.ins().call(
                helpers.apply_native,
                &[ctx_param, op_idx_val, arg_ptrs, n_val, incoming_val],
            );
            Ok((builder.inst_results(call)[0], false))
        }
        ArrSource::Foreign(name) => {
            // ADR-0083 §4: If this Foreign op name matches a registered FFI
            // binding, compile to `helper_v2_call_ffi_raw` (direct C ABI via
            // dlsym) instead of `helper_v2_call_foreign` (ModuleLookup trait
            // object dispatch). This is the "raw C ABI / dlsym" fast path.
            //
            // The FFI binding's index in the ffi_table is determined at
            // compile time — the table is built deterministically (sorted by
            // symbol) so the index is stable between compile and run.
            if ffi_ops.contains_key(name) {
                // Compute the FFI table index. The table is sorted by op
                // name at runtime (in run_native), so we replicate that
                // ordering here to find the correct index.
                let mut sorted_names: Vec<&String> = ffi_ops.keys().collect();
                sorted_names.sort();
                let ffi_idx = sorted_names.iter().position(|k| *k == name).unwrap_or(0);

                let ffi_idx_val = builder.ins().iconst(types::I64, ffi_idx as i64);
                let call = builder.ins().call(
                    helpers.call_ffi_raw,
                    &[ctx_param, ffi_idx_val, arg_ptrs, n_val],
                );
                Ok((builder.inst_results(call)[0], false))
            } else {
                // Fall back to the existing ModuleLookup dispatch path.
                let name_idx = pools.add_name(name);
                let name_idx_val = builder.ins().iconst(types::I64, name_idx as i64);
                let call = builder.ins().call(
                    helpers.call_foreign,
                    &[ctx_param, name_idx_val, arg_ptrs, n_val],
                );
                Ok((builder.inst_results(call)[0], false))
            }
        }
        ArrSource::DynamicForeign => {
            // ADR-0060: DynamicForeign ops (e.g. call_host) are dispatched
            // like Native ops — the op handler in apply_native performs the
            // host-function lookup at runtime.
            let op_idx = pools.add_name(op);
            let op_idx_val = builder.ins().iconst(types::I64, op_idx as i64);
            let call = builder.ins().call(
                helpers.apply_native,
                &[ctx_param, op_idx_val, arg_ptrs, n_val, incoming_val],
            );
            Ok((builder.inst_results(call)[0], false))
        }
        ArrSource::Judgment => {
            // Judgment ops (ADR-0054) are typecheck engine ops, not eval
            // engine ops — the eval engine does not dispatch them. This arm
            // should never be reached; return a compile-time error.
            return Err(LangError::engine(format!(
                "compile: judgment op '{}' is not supported by the eval engine (ADR-0054: judgment ops are dispatched by the typecheck engine)",
                op
            )));
        }
    }
}

/// `while_step`/`for_step`/`fix_step` — derived step ops that carry
/// cond/body sub-schemes as args and need per-iteration re-evaluation
/// with the current value (extracted from the loop pair) as incoming.
/// (`loop_step` is NOT here — its body arg is a closure evaluated
/// once, then applied per iteration via `apply_closure_value`.)
fn is_derived_step_op(op: &str) -> bool {
    matches!(op, "while_step" | "for_step" | "fix_step")
}

/// Map an arithmetic/comparison op name to its integer op code for the
/// [`helper_v2_int_arith`] fast path (Fix #7). Returns `None` if the op is
/// not a recognised binary arithmetic op.
///
/// The op codes must match the `match op_code` table in
/// [`crate::helper_v2_int_arith`].
fn int_arith_op_code(op: &str) -> Option<u64> {
    match op {
        "add" => Some(0),
        "sub" => Some(1),
        "mul" => Some(2),
        "div" => Some(3),
        "mod" => Some(4),
        "eq" => Some(5),
        "neq" => Some(6),
        "lt" => Some(7),
        "lte" => Some(8),
        "gt" => Some(9),
        "gte" => Some(10),
        _ => None,
    }
}

/// Compile a list of argument sub-schemes, packing the result pointers into a
/// stack-allocated array. Each arg is compiled with the same incoming value.
///
/// Returns a `Value` representing the address of the array (or null if no
/// args).
fn compile_args_v2(
    builder: &mut FunctionBuilder,
    args: &[TermData],
    ctx_param: Value,
    incoming_val: Value,
    helpers: &HelperRefs,
    pools: &mut ConstantPools,
    ptr_type: types::Type,
    ffi_ops: &HashMap<String, handl_plugin_ffi::FfiOpFunction>,
) -> Result<Value, LangError> {
    let n = args.len();
    if n == 0 {
        return Ok(builder.ins().iconst(ptr_type, 0));
    }

    let slot_size = (n * ptr_type.bytes() as usize) as u32;
    let slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        slot_size,
        0,
    ));
    let slot_addr = builder.ins().stack_addr(ptr_type, slot, 0);

    let null_val = builder.ins().iconst(ptr_type, 0);
    for i in 0..n {
        let offset = (i * ptr_type.bytes() as usize) as i32;
        builder
            .ins()
            .store(MemFlags::trusted(), null_val, slot_addr, offset);
    }

    for i in 0..n {
        // Clone incoming for each arg so that multiple args don't share the
        // same consumable pointer. Without this, if two args are consuming
        // combinators (First/Choice/Loop/extend_env) they would both try to
        // free the same `incoming_val` — double-free. The original
        // `incoming_val` is not consumed here; it leaks if unused, which is
        // acceptable per the Phase 7 safety model.
        let arg_incoming = {
            let call = builder
                .ins()
                .call(helpers.clone, &[ctx_param, incoming_val]);
            builder.inst_results(call)[0]
        };
        let (arg_ptr, arg_pushed) = compile_scheme_td(
            builder,
            &args[i],
            ctx_param,
            arg_incoming,
            helpers,
            pools,
            ptr_type,
            ffi_ops,
        )?;
        if arg_pushed {
            builder.ins().call(helpers.pop_scope, &[ctx_param]);
        }
        let offset = (i * ptr_type.bytes() as usize) as i32;
        builder
            .ins()
            .store(MemFlags::trusted(), arg_ptr, slot_addr, offset);
    }

    Ok(slot_addr)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstNode, EvalSchemeDef, Term, Value};
    use serde_json::json;

    /// Build a simple AST for testing.
    fn make_ast(nodes: Vec<AstNode>) -> Ast {
        let mut ast = Ast::new();
        for n in nodes {
            ast.push(n);
        }
        ast
    }

    // --- TermData thin wrappers (ADR-0103 Phase 6 migration) ---

    /// Convert `TermData` back to `Term` for APIs that require `Term`.
    fn td(t: TermData) -> Term {
        t.to_term().expect("TermData -> Term conversion")
    }

    /// Native op with a literal payload (mirrors `Term::arr_native_lit`).
    fn op_lit(name: &str, args: Vec<TermData>, lit: impl Into<Value>) -> TermData {
        if name == "const" && args.is_empty() {
            TermData::arr(lit)
        } else {
            TermData::op(name, args, &ArrSource::Native, Some(lit.into()))
        }
    }

    /// Native op without a literal payload (mirrors `Term::arr_native`).
    fn op_native(name: &str, args: Vec<TermData>) -> TermData {
        if name == "const" && args.is_empty() {
            TermData::arr(Value::Null)
        } else {
            TermData::op(name, args, &ArrSource::Native, None)
        }
    }

    /// Sequential composition (mirrors `Term::seq`).
    fn seq(fst: TermData, snd: TermData) -> TermData {
        TermData::seq(fst, snd)
    }

    /// Choice routing (mirrors `Term::choice`).
    fn choice(l: TermData, r: TermData) -> TermData {
        TermData::choice(l, r)
    }

    /// Application (mirrors `Term::app`).
    fn app(callee: TermData, args: Vec<TermData>) -> TermData {
        TermData::app(callee, args)
    }

    // --- JIT compilation tests (verify every combinator compiles) ---

    #[test]
    fn compile_v2_lit_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.c",
                "c",
                td(op_lit("const", vec![], json!(42))),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("c")]);
        let result = compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn compile_v2_var_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.v",
                "v",
                td(op_lit("proj_env", vec![], Value::String("x".into()))),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("v")]);
        let result = compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn compile_v2_child_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.a",
                "binary.add",
                td(op_native(
                    "add",
                    vec![
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("proj_child", vec![], Value::from(1u64)),
                    ],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let result = compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn compile_v2_bind_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                td(op_lit("proj_env", vec![], Value::String("x".into()))),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                td(seq(
                    seq(
                        op_lit("const", vec![], json!(1)),
                        op_lit("extend_env", vec![], Value::String("x".into())),
                    ),
                    op_lit("proj_env", vec![], Value::String("x".into())),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("let")]);
        let result = compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn compile_v2_branch_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                td(seq(
                    seq(
                        op_lit("const", vec![], json!(true)),
                        op_native("tag", vec![]),
                    ),
                    choice(
                        op_lit("const", vec![], json!(1)),
                        op_lit("const", vec![], json!(2)),
                    ),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("if")]);
        let result = compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn compile_v2_app_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.app",
                "app",
                td(app(
                    op_lit("proj_env", vec![], Value::String("f".into())),
                    vec![op_lit("const", vec![], json!(1))],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("app")]);
        let result = compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn compile_v2_loop_succeeds() {
        // A loop with inc_fst body — compiles to native loop IR.
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.loop",
                "loop",
                td(op_native("inc_fst", vec![])),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("loop")]);
        let result = compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn compile_v2_version_check_rejects_v3() {
        let mut lang = Language::new();
        lang.scheme_version = SchemeVersion::new("v3");
        let ast = make_ast(vec![AstNode::new("c")]);
        let result = compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new());
        assert!(result.is_err());
    }
}
