//! # Scheme → cranelift IR compilation
//!
//! This module compiles [`handl_language::Scheme`] data to native code via
//! cranelift. Each node in the [`Ast`] arena is compiled to its own native
//! function with the signature `fn(*mut EvalCtx) -> *mut Value`.
//!
//! The scheme for each node is resolved at **compile time** (one-time
//! `lang.eval[node.kind]` lookup) and baked into the IR. At execution time,
//! `Scheme::Child` dispatches to the child's pre-compiled native function via
//! a function-pointer cache — no scheme lookup occurs (ADR-0044 §7).
//!
//! ## Compilation strategy
//!
//! - **Control flow is native**: `Scheme::Branch` compiles to native
//!   `brif`/`jump` instructions. `Scheme::Bind` scope push/pop is native.
//! - **Value operations call Rust helpers**: `Scheme::Prim`,
//!   `Scheme::Foreign`, `Scheme::Child`, variable load/store, and payload
//!   access are native calls into `extern "C"` helper functions defined in
//!   [`crate`].
//! - **Constant pools**: Literal values and string names are collected into
//!   pools stored in [`crate::EvalCtx`]. The compiled code references them by
//!   index, avoiding raw string pointers in native code.
//!
//! ## Two compilation modes
//!
//! The core compilation logic is generic over the cranelift [`Module`] trait,
//! enabling two backends that share the same IR-generation code:
//!
//! - **In-memory JIT** ([`CompileMode::Jit`]): uses [`JITModule`]. Helper
//!   functions are registered with the JIT builder via `builder.symbol(...)`
//!   so the compiled code can call them directly. After finalization,
//!   [`CompiledResult::compiled`] holds callable function pointers. This is
//!   the path used by [`crate::CraneliftEngine`] for in-process evaluation.
//! - **On-disk object file** ([`CompileMode::Object`]): uses
//!   [`ObjectModule`]. Helpers are declared as external imports
//!   ([`Linkage::Import`]) with no addresses — the linker resolves them at
//!   link time against a runtime stub library that exports the
//!   `helper_*` symbols. After finalization, `ObjectModule::finish` produces
//!   a relocatable `.o` file written to `output_path`. There are no function
//!   pointers; the code lives in the file. This is the first step toward the
//!   ADR-0044 Phase 8 `compile(handlLanguage, native)` vision.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cranelift::prelude::*;
use cranelift::codegen::ir::{FuncRef, UserFuncName};
use cranelift::codegen::isa::TargetIsa;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use handl_language::{Ast, LangError, Language, Scheme, SchemeVersion};
use handl_language::Value as JsonValue;

/// Type alias for a compiled native function pointer.
///
/// The function signature is `fn(*mut u8) -> *mut u8`, where the actual types
/// are `*mut EvalCtx` and `*mut Value`. The raw `*mut u8` is used because
/// cranelift works with opaque pointers.
pub type CompiledFn = unsafe extern "C" fn(*mut u8) -> *mut u8;

/// Compilation mode: in-memory JIT or on-disk object file.
///
/// Selects which cranelift backend the core compilation logic targets. See the
/// [module docs](self) for the differences between the two modes.
#[derive(Debug, Clone)]
pub enum CompileMode {
    /// In-memory JIT — compile and execute via function pointers (existing
    /// behavior). Helper addresses are registered with the JIT builder; after
    /// finalization, function pointers are available for direct calls.
    Jit,
    /// On-disk object file — compile to a relocatable `.o` file for linking
    /// (new). Helpers are declared as external imports resolved by the linker
    /// at link time. The emitted file contains native code for every node;
    /// there are no in-memory function pointers.
    Object {
        /// Path to write the emitted object file to.
        output_path: PathBuf,
    },
}

/// Backend-specific state kept alive inside a [`CompiledResult`].
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

/// Result of compilation: function pointers + constant pools + the backend
/// (kept alive to ensure JIT-compiled code remains in memory).
///
/// For [`CompileMode::Object`], [`compiled`](Self::compiled) is empty (there
/// are no function pointers — the code is in the emitted file).
pub struct CompiledResult {
    /// Per-node compiled function pointers, keyed by arena index.
    ///
    /// Empty for [`CompileMode::Object`] (object files have no in-memory
    /// function pointers).
    pub compiled: HashMap<usize, CompiledFn>,
    /// Literal constant pool: values referenced by `Scheme::Lit`.
    pub literals: Vec<JsonValue>,
    /// Name constant pool: variable names, primitive ops, foreign names.
    pub names: Vec<String>,
    /// The compilation mode that produced this result.
    pub mode: CompileMode,
    /// Backend state (kept alive for JIT; unit for Object).
    _backend: CompiledBackend,
}

/// Metadata returned by object-file compilation ([`compile_all_nodes_object`]).
///
/// The emitted `.o` file contains native code for every node; helper functions
/// are declared as external imports resolved at link time. This struct carries
/// the constant pools and per-node function names needed to link and drive the
/// emitted code, but no function pointers (the code lives in the file).
pub struct ObjectResult {
    /// Literal constant pool: values referenced by `Scheme::Lit`.
    pub literals: Vec<JsonValue>,
    /// Name constant pool: variable names, primitive ops, foreign names.
    pub names: Vec<String>,
    /// Path the object file was written to.
    pub output_path: PathBuf,
    /// Per-node function symbol names (`node_N`), keyed by arena index, for
    /// linking. The linker exposes these symbols; a runtime stub library
    /// provides the `helper_*` imports.
    pub function_names: HashMap<usize, String>,
}

// ---------------------------------------------------------------------------
// Constant pools
// ---------------------------------------------------------------------------

/// Accumulates literal values and string names during compilation.
struct ConstantPools {
    /// Literal values referenced by `Scheme::Lit`.
    literals: Vec<JsonValue>,
    /// String names: variable names, primitive operators, foreign names.
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

    /// Add a name to the pool and return its index.
    fn add_name(&mut self, name: &str) -> usize {
        // Check if the name already exists (dedup).
        if let Some(idx) = self.names.iter().position(|n| n == name) {
            return idx;
        }
        let idx = self.names.len();
        self.names.push(name.to_string());
        idx
    }
}

// ---------------------------------------------------------------------------
// Helper function references
// ---------------------------------------------------------------------------

/// `FuncId`s for the helper imports, declared in the module.
struct HelperIds {
    push_literal: FuncId,
    load_var: FuncId,
    load_payload_var: FuncId,
    load_payload: FuncId,
    eval_child: FuncId,
    store: FuncId,
    store_payload: FuncId,
    pop_scope: FuncId,
    drop_value: FuncId,
    is_truthy: FuncId,
    apply_closure: FuncId,
    apply_prim: FuncId,
    call_foreign: FuncId,
    vau_enter: FuncId,
    vau_bind: FuncId,
}

/// `FuncRef`s for the helpers, declared in a specific function.
struct HelperRefs {
    push_literal: FuncRef,
    load_var: FuncRef,
    load_payload_var: FuncRef,
    load_payload: FuncRef,
    eval_child: FuncRef,
    store: FuncRef,
    store_payload: FuncRef,
    pop_scope: FuncRef,
    drop_value: FuncRef,
    is_truthy: FuncRef,
    apply_closure: FuncRef,
    apply_prim: FuncRef,
    call_foreign: FuncRef,
    vau_enter: FuncRef,
    vau_bind: FuncRef,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Compile all nodes in the AST using the given [`CompileMode`].
///
/// This is the unified entry point dispatching to the JIT or object backend.
/// For [`CompileMode::Jit`], the result holds callable function pointers. For
/// [`CompileMode::Object`], the object file is written to `output_path` and
/// the result's [`compiled`](CompiledResult::compiled) map is empty.
///
/// # Errors
///
/// Returns [`LangError`] on:
/// - Unsupported scheme version.
/// - Cranelift setup or compilation failure.
/// - Object-file emission or write failure (Object mode).
pub fn compile_all_nodes(
    lang: &Language,
    ast: &Ast,
    mode: &CompileMode,
) -> Result<CompiledResult, LangError> {
    match mode {
        CompileMode::Jit => compile_all_nodes_jit(lang, ast),
        CompileMode::Object { output_path } => {
            let obj = compile_all_nodes_object(lang, ast, output_path)?;
            Ok(CompiledResult {
                compiled: HashMap::new(),
                literals: obj.literals,
                names: obj.names,
                mode: CompileMode::Object {
                    output_path: output_path.clone(),
                },
                _backend: CompiledBackend::Object,
            })
        }
    }
}

/// Compile all nodes in the AST to in-memory native functions (JIT).
///
/// Each node is compiled to a function with signature `fn(*mut EvalCtx) ->
/// *mut Value`. The scheme for each node is resolved at compile time (one-time
/// `lang.eval[node.kind]` lookup) and baked into the IR. Helper functions are
/// registered with the JIT builder so the compiled code can call them
/// directly. This is the existing JIT path used by [`crate::CraneliftEngine`]
/// for in-process evaluation.
///
/// # Errors
///
/// Returns [`LangError`] on:
/// - Unsupported scheme version.
/// - Cranelift JIT setup or compilation failure.
pub fn compile_all_nodes_jit(lang: &Language, ast: &Ast) -> Result<CompiledResult, LangError> {
    // Set up the JIT builder with helper symbols. The helper addresses are
    // registered here so the JIT can resolve `Linkage::Import` calls to the
    // `extern "C"` helper functions defined in `crate`.
    let mut builder = JITBuilder::new(default_libcall_names())
        .map_err(|e| LangError::Engine(format!("JIT setup error: {}", e)))?;
    builder.symbol("helper_push_literal", crate::helper_push_literal as *const u8);
    builder.symbol("helper_load_var", crate::helper_load_var as *const u8);
    builder.symbol("helper_load_payload_var", crate::helper_load_payload_var as *const u8);
    builder.symbol("helper_load_payload", crate::helper_load_payload as *const u8);
    builder.symbol("helper_eval_child", crate::helper_eval_child as *const u8);
    builder.symbol("helper_store", crate::helper_store as *const u8);
    builder.symbol("helper_store_payload", crate::helper_store_payload as *const u8);
    builder.symbol("helper_pop_scope", crate::helper_pop_scope as *const u8);
    builder.symbol("helper_drop_value", crate::helper_drop_value as *const u8);
    builder.symbol("helper_is_truthy", crate::helper_is_truthy as *const u8);
    builder.symbol("helper_apply_closure", crate::helper_apply_closure as *const u8);
    builder.symbol("helper_apply_prim", crate::helper_apply_prim as *const u8);
    builder.symbol("helper_call_foreign", crate::helper_call_foreign as *const u8);
    builder.symbol("helper_vau_enter", crate::helper_vau_enter as *const u8);
    builder.symbol("helper_vau_bind", crate::helper_vau_bind as *const u8);

    let mut module = JITModule::new(builder);

    // Run the shared, backend-agnostic compilation (declare + compile). The
    // version check happens inside `compile_core`. `finalize_definitions` is
    // JIT-specific (it allocates/executes the compiled code) and is called
    // here, after the shared compilation.
    let (func_ids, pools) = compile_core(&mut module, lang, ast, Linkage::Local)?;

    // Finalize all definitions (compiles + links the functions in memory).
    module
        .finalize_definitions()
        .map_err(|e| LangError::Engine(format!("finalize error: {}", e)))?;

    // Get function pointers (JIT-specific — Object mode has no pointers).
    let mut compiled = HashMap::new();
    for (i, func_id) in &func_ids {
        let ptr = module.get_finalized_function(*func_id);
        compiled.insert(
            *i,
            unsafe { std::mem::transmute::<*const u8, CompiledFn>(ptr) },
        );
    }

    Ok(CompiledResult {
        compiled,
        literals: pools.literals,
        names: pools.names,
        mode: CompileMode::Jit,
        _backend: CompiledBackend::Jit(module),
    })
}

/// Compile all nodes in the AST to an on-disk relocatable object file.
///
/// Uses [`ObjectModule`] instead of [`JITModule`]. Helpers are declared as
/// external imports ([`Linkage::Import`]) with no addresses — the linker
/// resolves them at link time against a runtime stub library that exports the
/// `helper_*` symbols. After compilation and finalization,
/// `ObjectModule::finish` produces the object file, which is written to
/// `output_path`. No function pointers are returned; the code lives in the
/// file. Linking + running the object file is a separate future step.
///
/// # Errors
///
/// Returns [`LangError`] on:
/// - Unsupported scheme version.
/// - Cranelift object setup or compilation failure.
/// - Object-file emission or write failure.
pub fn compile_all_nodes_object(
    lang: &Language,
    ast: &Ast,
    output_path: &Path,
) -> Result<ObjectResult, LangError> {
    // Build a host-target ISA for the object backend.
    let isa = make_host_isa()?;

    // ObjectBuilder takes the ISA, a file name (embedded as a file symbol),
    // and the libcall-name resolver. NO symbol registration — helpers are
    // resolved by the linker at link time.
    let builder = ObjectBuilder::new(isa, "handl_compiled.o", default_libcall_names())
        .map_err(|e| LangError::Engine(format!("object builder error: {}", e)))?;
    let mut module = ObjectModule::new(builder);

    // Run the shared compilation (declare + compile + finalize). The same
    // `declare_helper_imports` is used — it declares `Linkage::Import`
    // functions, which the JIT resolves via registered symbols and the
    // object backend leaves for the linker.
    let (func_ids, pools) = compile_core(&mut module, lang, ast, Linkage::Export)?;

    // Build the per-node function-name map for linking (node_N).
    let function_names: HashMap<usize, String> = func_ids
        .iter()
        .map(|(i, _)| (*i, format!("node_{}", i)))
        .collect();

    // Finalize the object: consume the module and emit bytes.
    let product = module.finish();
    let bytes = product
        .emit()
        .map_err(|e| LangError::Engine(format!("object emit error: {}", e)))?;

    // Write the object file to disk.
    std::fs::write(output_path, &bytes)
        .map_err(|e| LangError::Engine(format!("write object file error: {}", e)))?;

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
///
/// Uses [`cranelift::native::builder`] to detect the host architecture and
/// infer native CPU features, then finishes the ISA with default flags.
fn make_host_isa() -> Result<Arc<dyn TargetIsa>, LangError> {
    let isa_builder = cranelift::native::builder()
        .map_err(|e| LangError::Engine(format!("native isa builder error: {}", e)))?;
    let flags = settings::Flags::new(settings::builder());
    isa_builder
        .finish(flags)
        .map_err(|e| LangError::Engine(format!("isa finish error: {}", e)))
}

/// The backend-agnostic core: declare helpers + node functions, and compile
/// (define) each node.
///
/// Generic over `M: Module` so the same IR-generation code drives both
/// [`JITModule`] and [`ObjectModule`]. The backend-specific finalization
/// steps — JIT `finalize_definitions` (allocates/executes code) and Object
/// `finish` (emits the `.o` file) — live in the entry points, since
/// `finalize_definitions` is an inherent `JITModule` method, not a `Module`
/// trait method. Returns the per-node `FuncId`s and the accumulated constant
/// pools.
///
/// # Errors
///
/// Returns [`LangError`] on unsupported scheme version or any cranelift
/// declare/compile failure.
fn compile_core<M: Module>(
    module: &mut M,
    lang: &Language,
    ast: &Ast,
    node_linkage: Linkage,
) -> Result<(HashMap<usize, FuncId>, ConstantPools), LangError> {
    // Scheme version check.
    if !SchemeVersion::v1().supports(&lang.scheme_version) {
        return Err(LangError::Engine(format!(
            "unsupported scheme version: {} (engine supports v1)",
            lang.scheme_version
        )));
    }

    let ptr_type = module.isa().pointer_type();

    // Declare helper imports. Both backends declare these as
    // `Linkage::Import`; the JIT resolves them via registered symbols, the
    // object backend leaves them for the linker.
    let helper_ids = declare_helper_imports(module, ptr_type)?;

    // Declare all node functions first (so cross-references work).
    let mut func_ids = HashMap::new();
    for i in 0..ast.len() {
        let name = format!("node_{}", i);
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(ptr_type));
        sig.returns.push(AbiParam::new(ptr_type));
        let func_id = module
            .declare_function(&name, node_linkage, &sig)
            .map_err(|e| LangError::Engine(format!("declare error: {}", e)))?;
        func_ids.insert(i, func_id);
    }

    // Compile each node (define_function emits the code into the backend).
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
/// (resolved by the linker at link time). The symbol names (`helper_*`)
/// must match the `extern "C"` helper function names in [`crate`] exactly.
fn declare_helper_imports<M: Module>(
    module: &mut M,
    ptr_type: types::Type,
) -> Result<HelperIds, LangError> {
    // Build all signatures first (each borrows module immutably but sequentially).
    let mut sig_2arg_ret = module.make_signature();
    sig_2arg_ret.params.push(AbiParam::new(ptr_type));
    sig_2arg_ret.params.push(AbiParam::new(types::I64));
    sig_2arg_ret.returns.push(AbiParam::new(ptr_type));

    let mut sig_1arg_ret = module.make_signature();
    sig_1arg_ret.params.push(AbiParam::new(ptr_type));
    sig_1arg_ret.returns.push(AbiParam::new(ptr_type));

    let mut sig_1arg_void = module.make_signature();
    sig_1arg_void.params.push(AbiParam::new(ptr_type));

    let mut sig_2arg_void = module.make_signature();
    sig_2arg_void.params.push(AbiParam::new(ptr_type));
    sig_2arg_void.params.push(AbiParam::new(ptr_type));

    let mut sig_3arg_void = module.make_signature();
    sig_3arg_void.params.push(AbiParam::new(ptr_type));
    sig_3arg_void.params.push(AbiParam::new(types::I64));
    sig_3arg_void.params.push(AbiParam::new(ptr_type));

    let mut sig_ptr_u8 = module.make_signature();
    sig_ptr_u8.params.push(AbiParam::new(ptr_type));
    sig_ptr_u8.returns.push(AbiParam::new(types::I8));

    let mut sig_apply_closure = module.make_signature();
    sig_apply_closure.params.push(AbiParam::new(ptr_type));
    sig_apply_closure.params.push(AbiParam::new(ptr_type));
    sig_apply_closure.params.push(AbiParam::new(ptr_type));
    sig_apply_closure.params.push(AbiParam::new(types::I64));
    sig_apply_closure.returns.push(AbiParam::new(ptr_type));

    let mut sig_apply_prim = module.make_signature();
    sig_apply_prim.params.push(AbiParam::new(ptr_type));
    sig_apply_prim.params.push(AbiParam::new(types::I64));
    sig_apply_prim.params.push(AbiParam::new(ptr_type));
    sig_apply_prim.params.push(AbiParam::new(types::I64));
    sig_apply_prim.returns.push(AbiParam::new(ptr_type));

    // helper_vau_enter: (ctx) -> void
    // helper_pop_scope signature (sig_1arg_void) is reused.
    // helper_vau_bind: (ctx, name_idx, child_n) -> void
    let mut sig_vau_bind = module.make_signature();
    sig_vau_bind.params.push(AbiParam::new(ptr_type));
    sig_vau_bind.params.push(AbiParam::new(types::I64));
    sig_vau_bind.params.push(AbiParam::new(types::I64));

    let declare = |m: &mut M, name: &str, sig: &Signature| -> Result<FuncId, LangError> {
        m.declare_function(name, Linkage::Import, sig)
            .map_err(|e| LangError::Engine(format!("declare import error: {}", e)))
    };

    Ok(HelperIds {
        push_literal: declare(module, "helper_push_literal", &sig_2arg_ret)?,
        load_var: declare(module, "helper_load_var", &sig_2arg_ret)?,
        load_payload_var: declare(module, "helper_load_payload_var", &sig_1arg_ret)?,
        load_payload: declare(module, "helper_load_payload", &sig_1arg_ret)?,
        eval_child: declare(module, "helper_eval_child", &sig_2arg_ret)?,
        store: declare(module, "helper_store", &sig_3arg_void)?,
        store_payload: declare(module, "helper_store_payload", &sig_2arg_void)?,
        pop_scope: declare(module, "helper_pop_scope", &sig_1arg_void)?,
        drop_value: declare(module, "helper_drop_value", &sig_1arg_void)?,
        is_truthy: declare(module, "helper_is_truthy", &sig_ptr_u8)?,
        apply_closure: declare(module, "helper_apply_closure", &sig_apply_closure)?,
        apply_prim: declare(module, "helper_apply_prim", &sig_apply_prim)?,
        call_foreign: declare(module, "helper_call_foreign", &sig_apply_prim)?,
        vau_enter: declare(module, "helper_vau_enter", &sig_1arg_void)?,
        vau_bind: declare(module, "helper_vau_bind", &sig_vau_bind)?,
    })
}

// ---------------------------------------------------------------------------
// Per-node compilation
// ---------------------------------------------------------------------------

/// Compile a single node's scheme into a native function.
///
/// Generic over `M: Module` so it works with both backends.
fn compile_node<M: Module>(
    module: &mut M,
    func_id: FuncId,
    scheme: Option<&Scheme>,
    helper_ids: &HelperIds,
    pools: &mut ConstantPools,
    ptr_type: types::Type,
    func_ctx: &mut FunctionBuilderContext,
) -> Result<(), LangError> {
    // Create a compilation context.
    let mut ctx = module.make_context();
    ctx.func.signature = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(ptr_type));
        sig.returns.push(AbiParam::new(ptr_type));
        sig
    };
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    // Build the IR.
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, func_ctx);

        // Declare helper func refs in this function.
        let helpers = HelperRefs {
            push_literal: module.declare_func_in_func(helper_ids.push_literal, &mut builder.func),
            load_var: module.declare_func_in_func(helper_ids.load_var, &mut builder.func),
            load_payload_var: module.declare_func_in_func(helper_ids.load_payload_var, &mut builder.func),
            load_payload: module.declare_func_in_func(helper_ids.load_payload, &mut builder.func),
            eval_child: module.declare_func_in_func(helper_ids.eval_child, &mut builder.func),
            store: module.declare_func_in_func(helper_ids.store, &mut builder.func),
            store_payload: module.declare_func_in_func(helper_ids.store_payload, &mut builder.func),
            pop_scope: module.declare_func_in_func(helper_ids.pop_scope, &mut builder.func),
            drop_value: module.declare_func_in_func(helper_ids.drop_value, &mut builder.func),
            is_truthy: module.declare_func_in_func(helper_ids.is_truthy, &mut builder.func),
            apply_closure: module.declare_func_in_func(helper_ids.apply_closure, &mut builder.func),
            apply_prim: module.declare_func_in_func(helper_ids.apply_prim, &mut builder.func),
            call_foreign: module.declare_func_in_func(helper_ids.call_foreign, &mut builder.func),
            vau_enter: module.declare_func_in_func(helper_ids.vau_enter, &mut builder.func),
            vau_bind: module.declare_func_in_func(helper_ids.vau_bind, &mut builder.func),
        };

        // Create the entry block.
        let entry_block = builder.create_block();
        builder.append_block_param(entry_block, ptr_type);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let ctx_param = builder.block_params(entry_block)[0];

        // Compile the scheme (or no-scheme fallback).
        // Null checks are NOT emitted in the IR — helpers handle null inputs
        // gracefully (returning null, setting ctx.error). The engine checks
        // ctx.error after native execution returns.
        let result = match scheme {
            Some(s) => compile_scheme(
                &mut builder,
                s,
                ctx_param,
                &helpers,
                pools,
                ptr_type,
            ),
            None => {
                // No-scheme fallback: return node.payload directly.
                let call = builder.ins().call(helpers.load_payload, &[ctx_param]);
                builder.inst_results(call)[0]
            }
        };

        // Return the result from the current block.
        builder.ins().return_(&[result]);

        // Finalize the builder.
        builder.finalize();
    }

    // Define the function in the module.
    module
        .define_function(func_id, &mut ctx)
        .map_err(|e| LangError::Engine(format!("define function error: {}", e)))?;

    // Clear the context for reuse.
    module.clear_context(&mut ctx);

    Ok(())
}

// ---------------------------------------------------------------------------
// Scheme compilation (recursive)
// ---------------------------------------------------------------------------

/// Compile a [`Scheme`] into cranelift IR, returning a `Value` representing
/// the result pointer.
///
/// After this function returns, the builder is in the block containing the
/// result value. Null checks are not emitted — helpers handle null inputs
/// gracefully and set `ctx.error` on failure. The engine checks the error
/// flag after native execution returns.
fn compile_scheme(
    builder: &mut FunctionBuilder,
    scheme: &Scheme,
    ctx_param: Value,
    helpers: &HelperRefs,
    pools: &mut ConstantPools,
    ptr_type: types::Type,
) -> Value {
    match scheme {
        Scheme::Lit(v) => {
            let lit_idx = pools.add_literal(v.clone());
            let lit_idx_val = builder.ins().iconst(types::I64, lit_idx as i64);
            let call = builder.ins().call(helpers.push_literal, &[ctx_param, lit_idx_val]);
            builder.inst_results(call)[0]
        }

        Scheme::Var(name) => {
            if name == "$payload" {
                let call = builder.ins().call(helpers.load_payload_var, &[ctx_param]);
                builder.inst_results(call)[0]
            } else {
                let name_idx = pools.add_name(name);
                let name_idx_val = builder.ins().iconst(types::I64, name_idx as i64);
                let call = builder.ins().call(helpers.load_var, &[ctx_param, name_idx_val]);
                builder.inst_results(call)[0]
            }
        }

        Scheme::Child(child_ref) => {
            let child_idx_val = builder.ins().iconst(types::I64, child_ref.0 as i64);
            let call = builder.ins().call(helpers.eval_child, &[ctx_param, child_idx_val]);
            builder.inst_results(call)[0]
        }

        Scheme::Bind { name, value, body } => {
            // Compile the value scheme.
            let val_ptr = compile_scheme(
                builder, value, ctx_param, helpers, pools, ptr_type,
            );

            // Store the value (pushes scope + defines variable).
            // Helpers handle null val_ptr gracefully.
            if name == "$payload" {
                builder
                    .ins()
                    .call(helpers.store_payload, &[ctx_param, val_ptr]);
            } else {
                let name_idx = pools.add_name(name);
                let name_idx_val = builder.ins().iconst(types::I64, name_idx as i64);
                builder
                    .ins()
                    .call(helpers.store, &[ctx_param, name_idx_val, val_ptr]);
            }

            // Compile the body scheme.
            let body_ptr = compile_scheme(
                builder, body, ctx_param, helpers, pools, ptr_type,
            );

            // Pop the scope (always, even if body failed — matches tree-walk).
            builder.ins().call(helpers.pop_scope, &[ctx_param]);

            // Return the body result (may be null if body failed).
            body_ptr
        }

        Scheme::Branch { cond, then_, else_ } => {
            // Compile the condition.
            let cond_ptr = compile_scheme(
                builder, cond, ctx_param, helpers, pools, ptr_type,
            );

            // Check truthiness (helper handles null cond_ptr → returns 0/falsy).
            let truthy = builder.ins().call(helpers.is_truthy, &[cond_ptr]);
            let truthy_val = builder.inst_results(truthy)[0];

            // Free the condition value (helper handles null).
            builder
                .ins()
                .call(helpers.drop_value, &[cond_ptr]);

            // Create blocks for then, else, and merge.
            let then_block = builder.create_block();
            let else_block = builder.create_block();
            let merge_block = builder.create_block();
            builder.append_block_param(merge_block, ptr_type);

            // Branch on truthiness.
            builder
                .ins()
                .brif(truthy_val, then_block, &[], else_block, &[]);

            // Compile then_ branch.
            builder.switch_to_block(then_block);
            builder.seal_block(then_block);
            let then_ptr = compile_scheme(
                builder, then_, ctx_param, helpers, pools, ptr_type,
            );
            builder.ins().jump(merge_block, &[then_ptr]);

            // Compile else_ branch.
            builder.switch_to_block(else_block);
            builder.seal_block(else_block);
            let else_ptr = compile_scheme(
                builder, else_, ctx_param, helpers, pools, ptr_type,
            );
            builder.ins().jump(merge_block, &[else_ptr]);

            // Seal merge block (both predecessors added) and switch to it.
            builder.seal_block(merge_block);
            builder.switch_to_block(merge_block);
            builder.block_params(merge_block)[0]
        }

        Scheme::App { callee, args } => {
            // Compile the callee.
            let callee_ptr = compile_scheme(
                builder, callee, ctx_param, helpers, pools, ptr_type,
            );

            // Compile args and pack into a stack array.
            let arg_ptrs = compile_args(
                builder, args, ctx_param, helpers, pools, ptr_type,
            );

            // Call helper_apply_closure.
            let n_val = builder.ins().iconst(types::I64, args.len() as i64);
            let call = builder.ins().call(
                helpers.apply_closure,
                &[ctx_param, callee_ptr, arg_ptrs, n_val],
            );
            builder.inst_results(call)[0]
        }

        Scheme::Prim { op, args } => {
            // Compile args and pack into a stack array.
            let arg_ptrs = compile_args(
                builder, args, ctx_param, helpers, pools, ptr_type,
            );

            // Call helper_apply_prim.
            let op_idx = pools.add_name(op);
            let op_idx_val = builder.ins().iconst(types::I64, op_idx as i64);
            let n_val = builder.ins().iconst(types::I64, args.len() as i64);
            let call = builder
                .ins()
                .call(helpers.apply_prim, &[ctx_param, op_idx_val, arg_ptrs, n_val]);
            builder.inst_results(call)[0]
        }

        Scheme::Foreign { name, args } => {
            // Compile args and pack into a stack array.
            let arg_ptrs = compile_args(
                builder, args, ctx_param, helpers, pools, ptr_type,
            );

            // Call helper_call_foreign.
            let name_idx = pools.add_name(name);
            let name_idx_val = builder.ins().iconst(types::I64, name_idx as i64);
            let n_val = builder.ins().iconst(types::I64, args.len() as i64);
            let call = builder.ins().call(
                helpers.call_foreign,
                &[ctx_param, name_idx_val, arg_ptrs, n_val],
            );
            builder.inst_results(call)[0]
        }

        Scheme::Vau { params, body } => {
            // Push a new scope for the vau parameter bindings.
            builder.ins().call(helpers.vau_enter, &[ctx_param]);

            // Bind each param to the unevaluated child subtree.
            for (i, param) in params.iter().enumerate() {
                let name_idx = pools.add_name(param);
                let name_idx_val = builder.ins().iconst(types::I64, name_idx as i64);
                let child_n_val = builder.ins().iconst(types::I64, i as i64);
                builder
                    .ins()
                    .call(helpers.vau_bind, &[ctx_param, name_idx_val, child_n_val]);
            }

            // Compile the body scheme.
            let body_ptr = compile_scheme(
                builder, body, ctx_param, helpers, pools, ptr_type,
            );

            // Pop the scope (always, even if body failed — matches tree-walk).
            builder.ins().call(helpers.pop_scope, &[ctx_param]);

            body_ptr
        }
    }
}

/// Compile a list of argument schemes, packing the result pointers into a
/// stack-allocated array.
///
/// Returns a `Value` representing the address of the array. Null argument
/// pointers (from failed evaluations) are stored as-is; helpers handle them
/// gracefully.
fn compile_args(
    builder: &mut FunctionBuilder,
    args: &[Scheme],
    ctx_param: Value,
    helpers: &HelperRefs,
    pools: &mut ConstantPools,
    ptr_type: types::Type,
) -> Value {
    let n = args.len();
    if n == 0 {
        // No args: return null pointer for the array.
        return builder.ins().iconst(ptr_type, 0);
    }

    // Create a stack slot for n pointers.
    let slot_size = (n * ptr_type.bytes() as usize) as u32;
    let slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        slot_size,
        0,
    ));
    let slot_addr = builder.ins().stack_addr(ptr_type, slot, 0);

    // Initialize all slots with null.
    let null_val = builder.ins().iconst(ptr_type, 0);
    for i in 0..n {
        let offset = (i * ptr_type.bytes() as usize) as i32;
        builder
            .ins()
            .store(MemFlags::trusted(), null_val, slot_addr, offset);
    }

    // Compile each arg and store its pointer in the stack slot.
    for i in 0..n {
        let arg_ptr = compile_scheme(
            builder,
            &args[i],
            ctx_param,
            helpers,
            pools,
            ptr_type,
        );
        let offset = (i * ptr_type.bytes() as usize) as i32;
        builder
            .ins()
            .store(MemFlags::trusted(), arg_ptr, slot_addr, offset);
    }

    slot_addr
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstNode, EvalSchemeDef, Scheme};
    use serde_json::json;

    /// Build a simple AST for testing.
    fn make_ast(nodes: Vec<AstNode>) -> Ast {
        let mut ast = Ast::new();
        for n in nodes {
            ast.push(n);
        }
        ast
    }

    // --- JIT compilation tests (verify every combinator compiles) ---

    #[test]
    fn compile_lit_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.c", "c", Scheme::lit(json!(42))))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("c")]);
        let result = compile_all_nodes_jit(&lang, &ast);
        assert!(result.is_ok());
    }

    #[test]
    fn compile_var_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.v", "v", Scheme::var("x")))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("v")]);
        let result = compile_all_nodes_jit(&lang, &ast);
        assert!(result.is_ok());
    }

    #[test]
    fn compile_var_payload_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.v", "v", Scheme::var("$payload")))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("v").with_payload(json!("x"))]);
        let result = compile_all_nodes_jit(&lang, &ast);
        assert!(result.is_ok());
    }

    #[test]
    fn compile_child_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.a",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let result = compile_all_nodes_jit(&lang, &ast);
        assert!(result.is_ok());
    }

    #[test]
    fn compile_bind_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::Bind {
                    name: "$payload".into(),
                    value: Box::new(Scheme::child(0)),
                    body: Box::new(Scheme::child(1)),
                },
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
        ]);
        let result = compile_all_nodes_jit(&lang, &ast);
        assert!(result.is_ok());
    }

    #[test]
    fn compile_app_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                Scheme::prim("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                Scheme::App {
                    callee: Box::new(Scheme::child(0)),
                    args: vec![Scheme::child(1)],
                },
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("function").with_payload(json!({"params": ["x"], "body": 0})),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("call").with_payload(json!({"children": [0, 1]})),
        ]);
        let result = compile_all_nodes_jit(&lang, &ast);
        assert!(result.is_ok());
    }

    #[test]
    fn compile_branch_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                Scheme::Branch {
                    cond: Box::new(Scheme::child(0)),
                    then_: Box::new(Scheme::child(1)),
                    else_: Box::new(Scheme::child(2)),
                },
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let result = compile_all_nodes_jit(&lang, &ast);
        assert!(result.is_ok());
    }

    #[test]
    fn compile_prim_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let result = compile_all_nodes_jit(&lang, &ast);
        assert!(result.is_ok());
    }

    #[test]
    fn compile_foreign_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.dbl",
                "dbl",
                Scheme::foreign("double", vec![Scheme::child(0)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(21)),
            AstNode::new("dbl").with_payload(json!({"children": [0]})),
        ]);
        let result = compile_all_nodes_jit(&lang, &ast);
        assert!(result.is_ok());
    }

    #[test]
    fn compile_vau_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.vau",
                "vau",
                Scheme::vau(vec!["x".into()], Scheme::lit(json!(42))),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("vau").with_payload(json!({"children": [0]})),
        ]);
        let result = compile_all_nodes_jit(&lang, &ast);
        assert!(result.is_ok(), "vau compilation failed: {:?}", result.err());
    }

    #[test]
    fn compile_version_mismatch_errors() {
        let mut lang = Language::new();
        lang.scheme_version = SchemeVersion::new("v2");
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(42))]);
        match compile_all_nodes_jit(&lang, &ast) {
            Ok(_) => panic!("expected version mismatch error, got Ok"),
            Err(e) => {
                let msg = format!("{}", e);
                assert!(msg.contains("unsupported scheme version"));
            }
        }
    }

    // --- CompileMode enum tests ---

    #[test]
    fn compile_mode_jit_constructs() {
        let mode = CompileMode::Jit;
        assert!(matches!(mode, CompileMode::Jit));
    }

    #[test]
    fn compile_mode_object_constructs() {
        let mode = CompileMode::Object {
            output_path: PathBuf::from("/tmp/handl_test.o"),
        };
        assert!(matches!(mode, CompileMode::Object { .. }));
    }

    #[test]
    fn compile_mode_clone_works() {
        let mode = CompileMode::Object {
            output_path: PathBuf::from("/tmp/handl_test.o"),
        };
        let cloned = mode.clone();
        assert!(matches!(cloned, CompileMode::Object { .. }));
    }

    // --- Object-file compilation tests ---

    #[test]
    fn object_compile_literal_node_emits_file() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.c", "c", Scheme::lit(json!(42))))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("c")]);
        let path = std::env::temp_dir().join("handl_object_compile_literal.o");
        let _ = std::fs::remove_file(&path);
        let result = compile_all_nodes_object(&lang, &ast, &path);
        assert!(result.is_ok(), "object compilation failed: {:?}", result.err());
        assert!(path.exists(), "object file was not created");
        let metadata = std::fs::metadata(&path).unwrap();
        assert!(
            metadata.len() > 0,
            "object file is empty"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn object_compile_no_scheme_node_emits_file() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(42))]);
        let path = std::env::temp_dir().join("handl_object_compile_noscheme.o");
        let _ = std::fs::remove_file(&path);
        let result = compile_all_nodes_object(&lang, &ast, &path);
        assert!(result.is_ok(), "object compilation failed: {:?}", result.err());
        assert!(path.exists());
        let metadata = std::fs::metadata(&path).unwrap();
        assert!(metadata.len() > 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn object_compile_branch_node_emits_file() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                Scheme::Branch {
                    cond: Box::new(Scheme::child(0)),
                    then_: Box::new(Scheme::child(1)),
                    else_: Box::new(Scheme::child(2)),
                },
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let path = std::env::temp_dir().join("handl_object_compile_branch.o");
        let _ = std::fs::remove_file(&path);
        let result = compile_all_nodes_object(&lang, &ast, &path);
        assert!(result.is_ok(), "object compilation failed: {:?}", result.err());
        assert!(path.exists());
        let metadata = std::fs::metadata(&path).unwrap();
        assert!(metadata.len() > 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn object_compile_returns_function_names() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.c", "c", Scheme::lit(json!(42))))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("c")]);
        let path = std::env::temp_dir().join("handl_object_compile_names.o");
        let _ = std::fs::remove_file(&path);
        let result = compile_all_nodes_object(&lang, &ast, &path).unwrap();
        assert_eq!(result.function_names.len(), 1);
        assert_eq!(result.function_names.get(&0).unwrap(), "node_0");
        assert_eq!(result.output_path, path);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn object_compile_version_mismatch_errors() {
        let mut lang = Language::new();
        lang.scheme_version = SchemeVersion::new("v2");
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(42))]);
        let path = std::env::temp_dir().join("handl_object_compile_version.o");
        match compile_all_nodes_object(&lang, &ast, &path) {
            Ok(_) => panic!("expected version mismatch error, got Ok"),
            Err(e) => {
                let msg = format!("{}", e);
                assert!(msg.contains("unsupported scheme version"));
            }
        }
    }

    #[test]
    fn object_compile_emits_valid_object_magic() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.c", "c", Scheme::lit(json!(42))))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("c")]);
        let path = std::env::temp_dir().join("handl_object_compile_magic.o");
        let _ = std::fs::remove_file(&path);
        compile_all_nodes_object(&lang, &ast, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() > 0);
        // On Linux/ELF, the first 4 bytes are the ELF magic `\x7fELF`.
        // On macOS (Mach-O), the magic is `0xfeedfacf` (64-bit) or
        // `0xfeedface` (32-bit). We check for either so the test is
        // portable across host platforms.
        let is_elf = bytes.len() >= 4 && &bytes[0..4] == b"\x7fELF";
        let is_macho = bytes.len() >= 4
            && (bytes[0..4] == [0xcf, 0xfa, 0xed, 0xfe]
                || bytes[0..4] == [0xce, 0xfa, 0xed, 0xfe]);
        assert!(
            is_elf || is_macho,
            "emitted file is not a recognized relocatable object (first 4 bytes: {:?})",
            &bytes.get(0..4).unwrap_or(&[])
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn compile_all_nodes_dispatches_jit() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.c", "c", Scheme::lit(json!(42))))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("c")]);
        let result = compile_all_nodes(&lang, &ast, &CompileMode::Jit).unwrap();
        assert!(matches!(result.mode, CompileMode::Jit));
        assert!(!result.compiled.is_empty());
    }

    #[test]
    fn compile_all_nodes_dispatches_object() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.c", "c", Scheme::lit(json!(42))))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("c")]);
        let path = std::env::temp_dir().join("handl_object_dispatch.o");
        let _ = std::fs::remove_file(&path);
        let mode = CompileMode::Object {
            output_path: path.clone(),
        };
        let result = compile_all_nodes(&lang, &ast, &mode).unwrap();
        assert!(matches!(result.mode, CompileMode::Object { .. }));
        assert!(result.compiled.is_empty());
        assert!(path.exists());
        let _ = std::fs::remove_file(&path);
    }
}
