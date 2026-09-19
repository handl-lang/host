//! # handl-plugin-engine-exec-cranelift
//!
//! The **cranelift eval engine** for the eval operation (ADR-0044 §4, §7).
//!
//! This engine compiles [`handl_language::Term`] data to **native code** via
//! [cranelift] and executes it via an in-memory JIT. It shares the same
//! `Term` vocabulary as the
//! [`handl_plugin_engine_exec_tree_walk`](handl_plugin_engine_exec_tree_walk)
//! reference interpreter and the
//! [`handl_plugin_engine_exec_bytecode`](handl_plugin_engine_exec_bytecode)
//! bytecode engine; conformance tests verify all three produce **identical
//! results**.
//!
//! ## Compilation strategy: boxed values + native control flow
//!
//! The handl value space is [`serde_json::Value`] — dynamically typed. A full
//! type-inferred native compiler is out of scope for Phase 7. The realistic
//! approach (and what ADR-0044 §7 "compilers erase data dispatch" means in
//! practice):
//!
//! **Cranelift generates native functions where:**
//!
//! - **Control flow is native**: [`Term::Choice`] compiles to native
//!   `brif`/`jump` instructions. `extend_env`/`proj_env` scope push/pop is
//!   native. [`Term::App`] closure dispatch is a native call to a helper.
//! - **Value operations call Rust helpers**: native `Arr` ops
//!   ([`Term::Arr`] with `ArrSource::Native`), foreign `Arr` ops
//!   ([`Term::Arr`] with `ArrSource::Foreign`), `proj_child`, variable
//!   load/store, and payload access are native calls into Rust functions
//!   that operate on [`Value`]
//!   (boxed as a pointer). The native code passes/receives `*mut Value`
//!   handles.
//! - **The per-node scheme lookup is erased**: at compile time, each node's
//!   [`Term`] is lowered to a fixed sequence of cranelift IR instructions +
//!   helper calls. The compiled function has no `lang.eval[node.kind]` hash
//!   lookup at execution time — the scheme is baked into the native code. This
//!   is the ADR §7 property.
//!
//! This is a legitimate JIT: native control flow + erased data dispatch +
//! helper calls for value ops. It is faster than the bytecode VM (native jumps
//! vs interpreted dispatch) and demonstrates the compilation pipeline that
//! enables the self-hosting bootstrap (`compile(handlLanguage, native)`).
//!
//! ## Per-node compilation
//!
//! Every node in the [`Ast`] arena is compiled to its own native function with
//! the signature `fn(*mut EvalCtx) -> *mut Value`. The scheme for each node is
//! resolved at **compile time** (one-time `lang.eval[node.kind]` lookup) and
//! baked into the IR. At execution time, `proj_child` dispatches to the
//! child's pre-compiled native function via a function-pointer cache — no
//! scheme lookup occurs.
//!
//! ## Constant pools
//!
//! Literal values and string names (variable names, primitive operators,
//! foreign function names) encountered during compilation are collected into
//! constant pools stored in [`EvalCtx`]. The compiled code references them by
//! index, avoiding raw string pointers in native code.
//!
//! ## Conventions (matched to the tree-walk and bytecode)
//!
//! - `Child(n)` = nth child of current node: reads `node.payload["children"][n]`
//!   to get the absolute [`AstRef`], then calls the child's compiled function.
//! - `Var("$payload")` reads `node.payload` as the variable name.
//! - `Bind{name: "$payload", ...}` reads `node.payload["name"]` as the bind name.
//! - No-scheme fallback: if a node kind has no eval scheme, returns
//!   `node.payload` directly.
//! - Closure representation: `Value::Object` with
//!   `{"__arrow__": true, "params": [...], "body": <arena_idx>, "env": {...}}`,
//!   created by the `"closure"` primitive.
//! - Primitives: `add, sub, mul, div, mod, eq, lt, gt, not, and, or, neg,
//!   closure, bind` — identical semantics to the tree-walk.
//! - Scope: `Vec<HashMap<String, Value>>` scope stack; `Bind` pushes a child
//!   scope; lookup searches innermost-first. Closure calls swap in the
//!   captured env, bind params, eval body, restore.
//!
//! ## Safety model
//!
//! Raw pointers + JIT is inherently unsafe. The safety is encapsulated:
//!
//! - The public API ([`CraneliftEngine`], [`EvalEngine`] impl) is **safe**.
//! - The [`EvalCtx`] is owned by the engine on the Rust side; native code
//!   receives a raw pointer. The engine guarantees the [`EvalCtx`] outlives
//!   all native code execution.
//! - [`Value`] handles are heap-allocated `Box<Value>` leaked into pointers
//!   (`Box::into_raw`). Helpers that consume a value pointer reclaim the box
//!   (`Box::from_raw` + `mem::take`). The final return value is reclaimed by
//!   the engine. Intermediate values that are checked but not consumed (e.g.
//!   a `Branch` condition) leak for the duration of evaluation — acceptable
//!   for Phase 7, documented here. On error, the scope stack and any leaked
//!   values are dropped with the [`EvalCtx`].
//! - `Bind` scope pop (`pop_scope`) is only called on the success path. On
//!   error, the native code returns immediately, skipping pending
//!   `pop_scope` calls. The scope stack is dropped with the [`EvalCtx`] on
//!   error — no memory corruption, just extra scopes that are never popped.
//!   This is a documented safety tradeoff for Phase 7.

#![warn(missing_docs)]

mod compile;
mod standalone;

pub use compile::{
    CompileMode, CompiledBackend, CompiledFn, CompiledResult, ObjectResult, compile_all_nodes,
    compile_all_nodes_jit, compile_all_nodes_object,
};

use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// Null-incoming sentinel (Fix #8)
// ---------------------------------------------------------------------------

/// Return a null pointer to use as the `incoming` value for closure-body and
/// child-node calls (Fix #8).
///
/// Previously, every closure invocation allocated a fresh
/// `Box::new(TaggedValue::Null)` — for deeply recursive code like ackermann
/// this is thousands of heap allocations.  All helpers that receive the
/// `incoming` pointer already check `ptr.is_null()` and handle it gracefully
/// (treating null as `TaggedValue::Null`), so passing a raw null pointer is
/// semantically identical to passing a boxed `Null` but avoids the allocation
/// entirely.
#[inline]
fn null_incoming_ptr() -> *mut TaggedValue {
    std::ptr::null_mut()
}

use handl_language::{Ast, AstNode, AstRef, LangError, Language, SchemeVersion, Term, Value};
use handl_plugin_engine_exec::native_ops::{
    self, NativeOpCtx, OpService, build_loop_step_args, is_truthy, loop_step_outcome, step_decision,
};
use handl_plugin_engine_exec::pattern_match::{self, PatternMatchCtx};
use handl_plugin_engine_exec::{EvalEngine, EvalRequest, ImportResolver};
use handl_value::{TaggedValue, either_value, is_either_left, is_either_right, tagged_to_value};

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// The cranelift eval engine.
///
/// Compiles `Term` data to native code via cranelift and executes via an
/// in-memory JIT. Construct with [`CraneliftEngine::new`] and optionally pass
/// a module registry via [`CraneliftEngine::with_modules`].
///
/// The engine checks `lang.scheme_version` against `SchemeVersion::v2()` and
/// rejects mismatched versions.
#[derive(Clone)]
pub struct CraneliftEngine {
    /// The module registry for `Foreign` dispatch (may be `None`).
    modules: Option<Rc<dyn handl_platform::ModuleLookup>>,
    /// Global variables injected by the host, pre-populated into the root
    /// scope on every eval call (e.g. `self`, and the `Language::globals`
    /// ambient values like `true`/`false`).
    globals: HashMap<String, Value>,
    /// The import resolver for `import "path"` statements (Phase 9.5).
    import_resolver: Option<Rc<dyn ImportResolver>>,
    /// FFI bindings for direct C ABI dispatch (ADR-0083 §4).
    ///
    /// Maps Foreign op names to FFI bindings. When a `Foreign` op name
    /// matches an entry here, the compiler emits a call to
    /// `helper_v2_call_ffi_raw` (direct C ABI) instead of
    /// `helper_v2_call_foreign` (ModuleLookup trait object dispatch).
    ffi_ops: HashMap<String, handl_plugin_ffi::FfiOpFunction>,
}

impl CraneliftEngine {
    /// Create a new cranelift engine with no module registry.
    pub fn new() -> Self {
        Self {
            modules: None,
            globals: HashMap::new(),
            import_resolver: None,
            ffi_ops: HashMap::new(),
        }
    }

    /// Attach a module registry (builder style).
    pub fn with_modules(modules: Rc<dyn handl_platform::ModuleLookup>) -> Self {
        Self {
            modules: Some(modules),
            globals: HashMap::new(),
            import_resolver: None,
            ffi_ops: HashMap::new(),
        }
    }

    /// Inject a global variable visible to all eval calls (builder style).
    /// Mirrors `TreeWalkEngine::with_global` — the value is pre-populated
    /// into the scope stack's globals on every eval.
    pub fn with_global(mut self, name: impl Into<String>, value: Value) -> Self {
        self.globals.insert(name.into(), value);
        self
    }

    /// Register an FFI binding for direct C ABI dispatch (ADR-0083 §4).
    ///
    /// When a `Foreign` op name matches `op_name`, the compiler will emit
    /// a call to `helper_v2_call_ffi_raw` (direct C ABI via `dlsym`) instead
    /// of the general `helper_v2_call_foreign` path (ModuleLookup trait
    /// object dispatch). This is the "raw C ABI / dlsym" fast path.
    ///
    /// Non-FFI Foreign ops continue to use the existing `ModuleLookup` path,
    /// preserving backward compatibility.
    pub fn register_ffi(&mut self, op_name: &str, op: handl_plugin_ffi::FfiOpFunction) {
        self.ffi_ops.insert(op_name.to_string(), op);
    }

    /// Attach FFI ops (builder style).
    pub fn with_ffi_ops(mut self, ops: HashMap<String, handl_plugin_ffi::FfiOpFunction>) -> Self {
        self.ffi_ops = ops;
        self
    }

    /// Compile the language + AST to an on-disk object file.
    ///
    /// This is the `compile(handlLanguage, native)` step from ADR-0044 Phase 8.
    /// The emitted `.o` file contains native code for every node; helper
    /// functions are declared as external imports resolved at link time
    /// against a runtime stub library that exports the `helper_*` symbols.
    /// No function pointers are returned — the code lives in the file.
    /// Linking + running the object file is a separate future step.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on unsupported scheme version, cranelift setup or
    /// compilation failure, or object-file emission/write failure.
    pub fn compile_to_object(
        &self,
        lang: &Language,
        ast: &Ast,
        output_path: &Path,
    ) -> Result<ObjectResult, LangError> {
        compile_all_nodes_object(lang, ast, output_path)
    }

    /// Compile the language + AST to a **standalone executable**.
    ///
    /// This is the full ADR-0044 Phase 8 `compile(handlLanguage, native)`
    /// step: compiles the AST to a relocatable `.o` file, then links it
    /// with a generated C `main` driver and this crate's static library
    /// (which exports the `helper_*` runtime stub and the
    /// `handl_standalone_*` entry points) to produce a standalone
    /// executable.
    ///
    /// The resulting executable, when run, evaluates the AST and prints the
    /// result (as JSON) to stdout — producing the same output as the
    /// host-built handl.
    ///
    /// # Foreign functions
    ///
    /// The standalone executable does **not** support foreign `Arr` ops
    /// (`Term::Arr` with `ArrSource::Foreign`) — there is no host functions
    /// registry at runtime. Corpus cases that use foreign functions will fail
    /// at evaluation time; this
    /// is a documented limitation of the Phase 8 proof-of-concept.
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on compilation, serialization, driver
    /// generation, staticlib discovery, or linking failure.
    pub fn compile_to_executable(
        &self,
        lang: &Language,
        ast: &Ast,
        output_path: &Path,
    ) -> Result<(), LangError> {
        standalone::compile_to_executable(lang, ast, output_path)
    }

    /// Run a pre-compiled [`CompiledResult`] against `lang` and `ast`.
    ///
    /// This is the execution half of `compile` + `run`: a [`CompiledResult`]
    /// produced by [`compile_all_nodes_jit`] is executed without
    /// recompiling. The `lang` and `ast` must be the same values used to
    /// produce `result` (the compiled function pointers reference constant
    /// pools and per-node functions keyed by arena index). The
    /// [`CompiledResult`] must outlive this call — its backend keeps the
    /// JIT-allocated native code in memory, so the function pointers in
    /// `result.compiled` remain valid as long as `result` is alive.
    ///
    /// The engine's module registry (set via [`with_modules`]) is used
    /// for foreign `Arr` op dispatch, matching [`eval`].
    ///
    /// [`with_modules`]: CraneliftEngine::with_modules
    /// [`eval`]: EvalEngine::eval
    ///
    /// # Errors
    ///
    /// Returns [`LangError`] on evaluation failure (deferred native error,
    /// null return, missing root function).
    pub fn run_compiled(
        &self,
        lang: &Language,
        ast: &Ast,
        result: &CompiledResult,
    ) -> Result<Value, LangError> {
        if ast.is_empty() {
            return Ok(Value::Null);
        }
        run_native(
            lang,
            ast,
            &result.compiled,
            &result.literals,
            &result.names,
            self.modules.as_ref(),
            self.import_resolver.clone(),
            &self.ffi_ops,
            &self.globals,
        )
    }
}

impl Default for CraneliftEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl EvalEngine for CraneliftEngine {
    fn eval(&self, lang: &Language, req: &EvalRequest) -> Result<Value, LangError> {
        let ast = &req.ast;
        if ast.is_empty() {
            return Ok(Value::Null);
        }
        // Term version check — the engine supports v2 only.
        if !SchemeVersion::v2().supports(&lang.scheme_version) {
            return Err(LangError::engine(format!(
                "unsupported scheme version: {} (engine supports v2)",
                lang.scheme_version
            )));
        }

        // Compile all nodes in the AST to native functions.
        // The scheme lookup (lang.eval[node.kind]) happens here, at compile
        // time — it is erased from the compiled code (ADR §7).
        let compile_result = compile_all_nodes_jit(lang, ast, &self.ffi_ops)?;

        run_native(
            lang,
            ast,
            &compile_result.compiled,
            &compile_result.literals,
            &compile_result.names,
            self.modules.as_ref(),
            self.import_resolver.clone(),
            &self.ffi_ops,
            &self.globals,
        )
    }

    fn box_clone(&self) -> Box<dyn EvalEngine> {
        Box::new(self.clone())
    }

    fn with_modules(&self, modules: Rc<dyn handl_platform::ModuleLookup>) -> Box<dyn EvalEngine> {
        Box::new(CraneliftEngine::with_modules(modules))
    }

    fn with_global(&self, name: &str, val: Value) -> Box<dyn EvalEngine> {
        Box::new(self.clone().with_global(name, val))
    }

    fn with_import_resolver(&self, resolver: Rc<dyn ImportResolver>) -> Box<dyn EvalEngine> {
        let mut engine = self.clone();
        engine.import_resolver = Some(resolver);
        Box::new(engine)
    }
}

/// Alias for backward compatibility (ADR-0052 cutover).
pub type CraneliftEngineV2 = CraneliftEngine;

/// Execute pre-compiled native functions against `lang` and `ast`.
///
/// Builds an [`EvalCtx`] from the compiled function-pointer map and constant
/// pools, then calls the root node's native function. Shared by
/// [`CraneliftEngine::eval`] (after compilation) and
/// [`CraneliftEngine::run_compiled`] (with a pre-compiled result).
///
/// # Safety
///
/// The function pointers in `compiled` must point to valid native code that
/// remains alive for the duration of this call (e.g. held by a
/// [`CompiledResult`]'s backend). `lang` and `ast` must outlive the call.
fn run_native(
    lang: &Language,
    ast: &Ast,
    compiled: &HashMap<usize, CompiledFn>,
    literals: &[Value],
    names: &[String],
    modules: Option<&Rc<dyn handl_platform::ModuleLookup>>,
    import_resolver: Option<Rc<dyn ImportResolver>>,
    ffi_ops: &HashMap<String, handl_plugin_ffi::FfiOpFunction>,
    globals: &HashMap<String, Value>,
) -> Result<Value, LangError> {
    // Build the evaluation context.
    // Convert literals from serde_json::Value to TaggedValue at the boundary.
    let literals: Vec<TaggedValue> = literals.iter().cloned().map(TaggedValue::from).collect();

    // Build the FFI table from the registered ops, in a deterministic
    // order (sorted by op name) so indices are stable.
    let mut ffi_table: Vec<(&String, &handl_plugin_ffi::FfiOpFunction)> = ffi_ops.iter().collect();
    ffi_table.sort_by(|a, b| a.0.cmp(b.0));
    let ffi_table: Vec<handl_plugin_ffi::FfiOpFunction> =
        ffi_table.iter().map(|(_, op)| (*op).clone()).collect();

    // Seed host-provided globals (`self`, language-data ambient values like
    // `true`/`false`) into the scope stack's globals map — searched after
    // local bindings so user code shadows them.
    let global_scope: FxHashMap<Rc<str>, TaggedValue> = globals
        .iter()
        .map(|(name, val)| (Rc::from(name.as_str()), TaggedValue::from(val.clone())))
        .collect();

    let mut ctx = EvalCtx {
        lang: lang as *const Language,
        ast: ast as *const Ast,
        env: ScopeStack::new(),
        current_ref: ast.len() - 1,
        modules: modules.map(|m| Rc::as_ptr(m) as *const dyn handl_platform::ModuleLookup),
        import_resolver,
        compiled: compiled.clone(),
        literals,
        names: names.to_vec(),
        error: None,
        ffi_table,
    };
    ctx.env.set_globals(Rc::new(global_scope));

    // Call the root node's compiled native function.
    // The root is the last node in the arena (Pratt convention).
    let root_idx = ast.len() - 1;
    let root_fn = ctx
        .compiled
        .get(&root_idx)
        .copied()
        .ok_or_else(|| LangError::engine("no compiled function for root".to_string()))?;

    // Top-level incoming is a boxed Null (reused sentinel — Fix #8).
    let null_incoming = null_incoming_ptr();
    // SAFETY: root_fn is a valid function pointer into JIT-allocated memory
    // that remains alive for the duration of this call (held by the
    // CompiledResult's backend). ctx is a valid &mut EvalCtx on the stack.
    let result_ptr: *mut TaggedValue = unsafe {
        root_fn(
            &mut ctx as *mut EvalCtx as *mut u8,
            null_incoming as *mut u8,
        ) as *mut TaggedValue
    };

    // Check for deferred error.
    if let Some(err) = ctx.error.take() {
        if !result_ptr.is_null() {
            // SAFETY: result_ptr was boxed by a helper via box_value; reclaim
            // it to avoid leaking on the error path.
            unsafe {
                let _ = take_value(result_ptr);
            }
        }
        return Err(err);
    }

    // Reclaim the result value.
    if result_ptr.is_null() {
        return Err(LangError::engine(
            "native evaluation returned null without error".to_string(),
        ));
    }
    // SAFETY: the helper boxed this TaggedValue with Box::into_raw. We reclaim it.
    let value = unsafe { take_value(result_ptr) };
    // Convert the TaggedValue result back to serde_json::Value at the boundary.
    Ok(tagged_to_value(&value))
}

// ---------------------------------------------------------------------------
// Evaluation context (passed to native code as a raw pointer)
// ---------------------------------------------------------------------------

/// The evaluation context shared between the Rust helpers and the native
/// compiled code.
///
/// This struct is owned by the engine on the Rust side. A raw pointer to it
/// is passed to the cranelift-compiled functions, which pass it back to the
/// `extern "C"` helper functions. All access is single-threaded.
///
/// # Safety
///
/// The `lang`, `ast`, and `modules` raw pointers are valid for the lifetime of
/// the `eval` call (the engine borrows them from the `EvalEngine::eval`
/// parameters). The `compiled` map holds function pointers into JIT-allocated
/// memory that remains valid as long as the `JITModule` is alive — the
/// compile module keeps the module alive via the `CompiledResult`.
pub struct EvalCtx {
    /// The language definition (raw pointer; valid for the eval call).
    #[allow(dead_code)]
    lang: *const Language,
    /// The AST being evaluated (raw pointer; valid for the eval call).
    ast: *const Ast,
    /// The scope stack.
    env: ScopeStack,
    /// The arena index of the node currently being evaluated.
    current_ref: usize,
    /// The module registry (raw trait-object pointer; may be null).
    modules: Option<*const dyn handl_platform::ModuleLookup>,
    /// The import resolver for `import "path"` statements (Phase 9.5).
    import_resolver: Option<Rc<dyn ImportResolver>>,
    /// Per-node compiled function pointers, keyed by arena index.
    /// Used by helpers ([`helper_v2_eval_child`], [`helper_v2_apply_closure`]).
    compiled: HashMap<usize, CompiledFn>,
    /// Literal constant pool: values referenced by `const` ops.
    literals: Vec<TaggedValue>,
    /// Name constant pool: variable names, primitive ops, foreign names.
    names: Vec<String>,
    /// Deferred error: set by a helper on failure, checked after native
    /// execution returns.
    error: Option<LangError>,
    /// FFI binding table (ADR-0083 §4 direct C ABI fast path).
    ///
    /// Populated at engine construction time from registered FFI bindings.
    /// When a `Foreign` op name matches an entry in this table, the compiled
    /// code calls `helper_v2_call_ffi_raw` with the binding's index instead
    /// of the general `helper_v2_call_foreign` path (which goes through
    /// `ModuleLookup` trait object dispatch).
    ffi_table: Vec<handl_plugin_ffi::FfiOpFunction>,
}

impl EvalCtx {
    /// Get the current node being evaluated.
    fn current_node(&self) -> Result<&AstNode, LangError> {
        // SAFETY: self.ast is a valid pointer set by the engine; it outlives
        // the EvalCtx (guaranteed by run_native / for_standalone).
        unsafe { (*self.ast).at(AstRef::new(self.current_ref)) }
    }

    /// Get a node by arena index (ADR-0060).
    fn ast_at(&self, node_ref: AstRef) -> Result<&AstNode, LangError> {
        // SAFETY: self.ast is a valid pointer set by the engine; it outlives
        // the EvalCtx (guaranteed by run_native / for_standalone).
        unsafe { (*self.ast).at(node_ref) }
    }

    /// Set the deferred error and return a null pointer (for helper use).
    fn set_error(&mut self, err: LangError) -> *mut TaggedValue {
        self.error = Some(err);
        std::ptr::null_mut()
    }

    /// Resolve a child arena index for the named role on `node`, using the
    /// language's declared child roles. Resolution is **arity-aware**
    /// (`Language::child_role_index_for`): roles declared after an
    /// `optional` slot shift when the slot is absent — e.g. `match`'s
    /// `block` resolves to index 1 without a motive and 2 with one.
    /// Returns `Ok(None)` when the role is not declared, its optional slot
    /// is absent at this arity, or the payload is malformed.
    fn role_child_arena(&self, node: &AstNode, role: &str) -> Result<Option<usize>, LangError> {
        if self.lang.is_null() {
            return Ok(None);
        }
        // SAFETY: self.lang is a valid pointer for the duration of run_native.
        let idx = unsafe { (*self.lang).child_role_index_for(node, role)? };
        let Some(idx) = idx else {
            return Ok(None);
        };
        let arena = node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.get(idx))
            .and_then(|v| v.as_u64())
            .map(|i| i as usize);
        Ok(arena)
    }

    /// Construct an [`EvalCtx`] for standalone execution.
    ///
    /// This is the entry point used by the standalone bootstrap driver
    /// (ADR-0044 Phase 8): an external driver links the cranelift-emitted
    /// `.o` file (which exports `node_N` symbols) against this crate's
    /// static library (which exports the `helper_*` symbols), builds the
    /// per-node function-pointer map from the `node_N` addresses, and
    /// constructs the evaluation context via this constructor.
    ///
    /// The `ast` must outlive the returned [`EvalCtx`] — a raw pointer to
    /// it is stored and dereferenced by the helpers during evaluation.
    /// `lang` is not needed (the helpers resolve schemes from `ast` at
    /// runtime via `current_node`), so a null pointer is stored. The
    /// `compiled` map carries the `node_N` function pointers keyed by
    /// arena index; `literals` and `names` are the constant pools emitted
    /// alongside the object file (see [`ObjectResult`]).
    ///
    /// # Safety
    ///
    /// The caller must ensure `ast` outlives the [`EvalCtx`] and that the
    /// function pointers in `compiled` point to valid native code
    /// (e.g. linked from the emitted `.o` file) that remains alive for
    /// the duration of evaluation.
    pub fn for_standalone(
        ast: &Ast,
        compiled: HashMap<usize, CompiledFn>,
        literals: Vec<TaggedValue>,
        names: Vec<String>,
    ) -> Self {
        Self {
            lang: std::ptr::null(),
            ast: ast as *const Ast,
            env: ScopeStack::new(),
            current_ref: if ast.is_empty() { 0 } else { ast.len() - 1 },
            modules: None,
            import_resolver: None,
            compiled,

            literals,
            names,
            error: None,
            ffi_table: Vec::new(),
        }
    }
}

/// `PatternMatchCtx` impl for the cranelift eval context (ADR-0061).
///
/// `define_binding` delegates to the scope stack; `eval_pattern_node`
/// delegates to `eval_child_node` (which uses the compiled function
/// pointer cache). The `ast` parameter from the trait is the same AST
/// pointed to by `ctx.ast`.
impl PatternMatchCtx for EvalCtx {
    fn define_binding(&mut self, name: &str, value: Value) {
        self.env.define(name, TaggedValue::from(value));
    }

    fn eval_pattern_node(&mut self, _ast: &Ast, node_ref: AstRef) -> Result<Value, LangError> {
        let val = eval_child_node(self, node_ref.index())?;
        Ok(tagged_to_value(&val))
    }

    fn global_value(&self, name: &str) -> Option<Value> {
        self.env.globals.get(name).map(tagged_to_value)
    }

    fn lang(&self) -> Option<&Language> {
        // SAFETY: `self.lang` is a valid pointer for the duration of
        // evaluation (run_native); it is null for the standalone driver,
        // which then keeps the role-less kind-name fallbacks.
        unsafe { self.lang.as_ref() }
    }
}

// ---------------------------------------------------------------------------
// Scope stack (identical semantics to tree-walk)
// ---------------------------------------------------------------------------

/// A scope stack: `Vec<Rc<FxHashMap<Rc<str>, TaggedValue>>>` searched
/// innermost-first.
///
/// Scope frames are `Rc`-backed so that a closure's captured environment can be
/// shared by reference (`Rc::clone`) instead of deep-cloned on every closure
/// call (Fix #8). The innermost scope is mutated via `Rc::make_mut`, which is
/// cheap because it is always freshly pushed (refcount 1).
struct ScopeStack {
    /// The stack of scope frames.
    scopes: Vec<Rc<FxHashMap<Rc<str>, TaggedValue>>>,
    /// Set when `assign` updates a binding in an *enclosing* scope (not the
    /// innermost one) with a different value. Read and cleared by `eval_rec`
    /// fixpoint detection (the shared `eval_rec` op) — such writes are the
    /// ADR-0074 feedback wires: the `while`/`do-while`/`for` desugars update
    /// `__continue`/`__xs` (and user bindings like `x = x + 1`) from inside
    /// `if`/`match` branch blocks, and a pass that produces any such write
    /// must not count as converged. Mirrors the tree-walk
    /// `ScopeStack::enclosing_updated`.
    enclosing_updated: bool,
    /// Host-provided globals (e.g. `self` and language-data ambient values
    /// like `true`/`false`), searched after local/captured bindings so
    /// user-defined scopes shadow them. Held in a cheaply-cloned `Rc` so it
    /// is excluded from `capture()` and shared across closure calls.
    /// Mirrors the bytecode `ScopeStack::globals`.
    globals: Rc<FxHashMap<Rc<str>, TaggedValue>>,
}

impl ScopeStack {
    fn new() -> Self {
        Self {
            scopes: vec![Rc::new(FxHashMap::default())],
            enclosing_updated: false,
            globals: Rc::new(FxHashMap::default()),
        }
    }

    /// Set the host-provided globals map (an `Rc` clone, cheap). Globals
    /// are accessible via `lookup()` but excluded from `capture()`.
    fn set_globals(&mut self, globals: Rc<FxHashMap<Rc<str>, TaggedValue>>) {
        self.globals = globals;
    }

    /// Look up a variable, searching innermost scope first, then the
    /// host-provided globals.
    fn lookup(&self, name: &str) -> Option<&TaggedValue> {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.get(name))
            .or_else(|| self.globals.get(name))
    }

    /// Define a variable in the current (innermost) scope.
    fn define(&mut self, name: &str, value: TaggedValue) {
        // SAFETY: `ScopeStack::new` always pushes one frame and `pop_scope`
        // only pops when `len > 1`, so the stack is never empty.
        let last = self.scopes.last_mut().expect("scope stack is never empty");
        Rc::make_mut(last).insert(Rc::from(name), value);
    }

    /// Assign `name = value`: update the nearest existing binding in the
    /// enclosing local scopes (innermost-first), or define it in the
    /// innermost scope when the name is not already bound.
    ///
    /// This is the `x = y` statement semantics for blocks (ADR-0074): a
    /// binding that shadows a name bound in an enclosing scope *updates*
    /// that binding — this is what lets the `while`/`do-while` desugar's
    /// feedback wires (`__continue = cond` inside the `if` branch block)
    /// reach the `rec` region's bindings, and what lets loop-body
    /// rebindings such as `x = x + 1` be observed after the loop. Mirrors
    /// the tree-walk `ScopeStack::assign` exactly, including the
    /// `enclosing_updated` fixpoint flag (only cross-scope writes that
    /// change the value set it).
    fn assign(&mut self, name: &str, value: TaggedValue) {
        let last = self.scopes.len() - 1;
        for (i, scope) in self.scopes.iter_mut().enumerate().rev() {
            if scope.contains_key(name) {
                // SAFETY: each scope frame is freshly pushed / uniquely
                // owned (refcount 1) during block eval, so `Rc::make_mut`
                // is a cheap in-place mutation.
                let slot = Rc::make_mut(scope).get_mut(name);
                if let Some(slot) = slot {
                    if !handl_value::eq(slot, &value) {
                        *slot = value;
                        // Only cross-scope writes are fixpoint-relevant: an
                        // update to a binding defined earlier in the *same*
                        // innermost scope is ordinary sequencing and must
                        // not keep a `rec` pass alive.
                        if i < last {
                            self.enclosing_updated = true;
                        }
                    }
                }
                return;
            }
        }
        self.define(name, value);
    }

    /// Push a new empty scope.
    fn push_scope(&mut self) {
        self.scopes.push(Rc::new(FxHashMap::default()));
    }

    /// Pop the innermost scope.
    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Capture all visible variables into a single flat map.
    fn capture(&self) -> FxHashMap<Rc<str>, TaggedValue> {
        let mut captured = FxHashMap::default();
        for scope in &self.scopes {
            for (k, v) in scope.iter() {
                captured.insert(k.clone(), v.clone());
            }
        }
        captured
    }
}

// ---------------------------------------------------------------------------
// Value boxing helpers
// ---------------------------------------------------------------------------

// Debug-mode allocation registry: tracks every live `box_value` pointer so
// `take_value` can detect double-frees and use-after-frees. Zero overhead in
// release builds. The registry is thread-local (evaluation is single-threaded)
// and only holds currently-live addresses, so it stays bounded.
#[cfg(debug_assertions)]
thread_local! {
    static LIVE_VALUES: std::cell::RefCell<rustc_hash::FxHashSet<usize>> =
        std::cell::RefCell::new(rustc_hash::FxHashSet::default());
}

#[cfg(debug_assertions)]
fn debug_track_alloc(ptr: *mut TaggedValue) {
    if !ptr.is_null() {
        let addr = ptr as usize;
        LIVE_VALUES.with(|s| {
            if !s.borrow_mut().insert(addr) {
                panic!(
                    "box_value: pointer {:p} already live — possible use-after-free \
                     (allocator reused a freed address without a matching take_value)",
                    ptr
                );
            }
        });
    }
}

#[cfg(debug_assertions)]
fn debug_track_free(ptr: *mut TaggedValue) {
    if !ptr.is_null() {
        let addr = ptr as usize;
        LIVE_VALUES.with(|s| {
            if !s.borrow_mut().remove(&addr) {
                panic!(
                    "take_value: pointer {:p} not in live set — double-free or \
                     use-after-free",
                    ptr
                );
            }
        });
    }
}

#[cfg(not(debug_assertions))]
#[inline]
fn debug_track_alloc(_ptr: *mut TaggedValue) {}

#[cfg(not(debug_assertions))]
#[inline]
fn debug_track_free(_ptr: *mut TaggedValue) {}

/// Box a [`TaggedValue`] into a heap-allocated pointer for passing to native code.
///
/// # Safety
///
/// The returned pointer must be reclaimed with `Box::from_raw` (either by a
/// consuming helper or by the engine for the final result).
unsafe fn box_value(value: TaggedValue) -> *mut TaggedValue {
    let ptr = Box::into_raw(Box::new(value));
    debug_track_alloc(ptr);
    ptr
}

/// Reclaim a boxed [`TaggedValue`] pointer, moving the value out and freeing the
/// allocation.
///
/// # Safety
///
/// `ptr` must be a valid pointer returned by [`box_value`] (or
/// `Box::into_raw`).
unsafe fn take_value(ptr: *mut TaggedValue) -> TaggedValue {
    debug_track_free(ptr);
    // SAFETY: ptr was produced by box_value (Box::into_raw) and has not been
    // freed yet. Box::from_raw reclaims the allocation exactly once.
    unsafe { std::mem::take(&mut *Box::from_raw(ptr)) }
}

/// Clone a boxed [`TaggedValue`] without consuming the original pointer.
///
/// Returns a freshly boxed clone. The input pointer is **not** consumed and
/// remains valid. If the input is null, returns a boxed `Null`.
///
/// Used by the `Loop` combinator to save a clone of the incoming value before
/// `pair` consumes the original (Fix #8: pre-existing loop double-free fix).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_clone(
    _ctx: *mut EvalCtx,
    val: *mut TaggedValue,
) -> *mut TaggedValue {
    unsafe {
        if val.is_null() {
            return box_value(TaggedValue::Null);
        }
        // SAFETY: val is a valid non-null pointer to a boxed TaggedValue. We
        // only read (clone) it — the original is NOT consumed, preventing
        // double-free when the caller later frees val.
        box_value((*val).clone())
    }
}

// ---------------------------------------------------------------------------
// Helper functions (extern "C", called from native code)
// ---------------------------------------------------------------------------

/// Push a literal value (from the constant pool) as a boxed clone.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_push_literal(
    ctx: *mut EvalCtx,
    lit_idx: usize,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        if lit_idx >= ctx.literals.len() {
            return ctx.set_error(LangError::engine(format!(
                "literal index {} out of bounds",
                lit_idx
            )));
        }
        box_value(ctx.literals[lit_idx].clone())
    }
}

/// Load a variable by name index from the scope stack.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_load_payload(ctx: *mut EvalCtx) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        match ctx.current_node() {
            Ok(node) => box_value(TaggedValue::from(node.payload.clone())),
            Err(e) => ctx.set_error(e),
        }
    }
}

/// Evaluate the nth child of the current node.
///
/// Reads `node.payload["children"][n]` to get the child's `AstRef`, then
/// calls the child's pre-compiled native function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_pop_scope(ctx: *mut EvalCtx) {
    unsafe {
        let ctx = &mut *ctx;
        ctx.env.pop_scope();
    }
}

/// Push a new empty scope for the `vau` parameter bindings (ADR-0041 §6).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_drop_value(val: *mut TaggedValue) {
    unsafe {
        if !val.is_null() {
            let _ = take_value(val);
        }
    }
}

/// Check if a value is truthy.
///
/// `false`, `null`, `0`, and `""` are falsy; everything else is truthy.
/// Returns 1 for truthy, 0 for falsy.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_is_truthy(val: *const TaggedValue) -> u8 {
    unsafe {
        if val.is_null() {
            return 0;
        }
        let v = &*val;
        if is_truthy(v) { 1 } else { 0 }
    }
}

/// Apply a closure value to argument values.
///
/// The callee and all argument value pointers are consumed (reclaimed).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_apply_closure(
    ctx: *mut EvalCtx,
    callee: *mut TaggedValue,
    args: *const *mut TaggedValue,
    n: usize,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        if callee.is_null() {
            return ctx.set_error(LangError::engine("closure callee is null".to_string()));
        }
        let closure_val = take_value(callee);

        // Collect arg values.
        let mut arg_vals = Vec::with_capacity(n);
        for i in 0..n {
            let arg_ptr = *args.add(i);
            if arg_ptr.is_null() {
                // Error already set; consume remaining args.
                for j in (i + 1)..n {
                    let p = *args.add(j);
                    if !p.is_null() {
                        let _ = take_value(p);
                    }
                }
                return std::ptr::null_mut();
            }
            arg_vals.push(take_value(arg_ptr));
        }

        // Parse closure. Share the captured env via Rc::clone instead of
        // deep-cloning the HashMap (Fix #8).
        let (params, rest_param, body_idx, captured_env) = match &closure_val {
            TaggedValue::Arrow {
                params,
                rest,
                body,
                env,
                ..
            } => (params.clone(), rest.clone(), *body, Rc::clone(env)),
            _ => {
                return ctx.set_error(LangError::engine(
                    "callee is not an arrow-value".to_string(),
                ));
            }
        };

        // Rest param (`[a, ...rest] ~> body`): surplus args bind to the
        // rest name as an array, so `arg_vals.len() >= params.len()`.
        let arity_ok = if rest_param.is_some() {
            arg_vals.len() >= params.len()
        } else {
            params.len() == arg_vals.len()
        };
        if !arity_ok {
            return ctx.set_error(LangError::engine(format!(
                "arrow-value expects {} args, got {}",
                params.len(),
                arg_vals.len()
            )));
        }

        // Look up the compiled function for the body node.
        let func = match ctx.compiled.get(&body_idx).copied() {
            Some(f) => f,
            None => {
                return ctx.set_error(LangError::engine(format!(
                    "no compiled function for closure body {}",
                    body_idx
                )));
            }
        };

        // Swap in the captured env + new scope with params.
        let saved_env = std::mem::replace(&mut ctx.env, ScopeStack::new());
        ctx.env.scopes[0] = captured_env;
        ctx.env.push_scope();
        for (param, arg) in params.iter().zip(arg_vals.iter()) {
            ctx.env.define(param, arg.clone());
        }
        // Bind the rest param to the surplus args as an array.
        if let Some(rest_name) = &rest_param {
            ctx.env.define(
                rest_name.as_ref(),
                TaggedValue::Arr(Rc::from(arg_vals[params.len()..].to_vec())),
            );
        }

        // Save and set current_ref for the body.
        let saved_ref = ctx.current_ref;
        ctx.current_ref = body_idx;

        let null_incoming = null_incoming_ptr();
        let result =
            func(ctx as *mut EvalCtx as *mut u8, null_incoming as *mut u8) as *mut TaggedValue;

        // Restore the original scope stack and current_ref.
        ctx.current_ref = saved_ref;
        ctx.env = saved_env;

        result
    }
}

/// Apply a primitive operation (by name index).
///
/// All argument value pointers are consumed (reclaimed).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_call_foreign(
    ctx: *mut EvalCtx,
    name_idx: usize,
    args: *const *mut TaggedValue,
    n: usize,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        if name_idx >= ctx.names.len() {
            for i in 0..n {
                let p = *args.add(i);
                if !p.is_null() {
                    let _ = take_value(p);
                }
            }
            return ctx.set_error(LangError::engine(format!(
                "name index {} out of bounds",
                name_idx
            )));
        }
        let name = ctx.names[name_idx].clone();

        // Collect arg values.
        let mut arg_vals = Vec::with_capacity(n);
        for i in 0..n {
            let arg_ptr = *args.add(i);
            if arg_ptr.is_null() {
                for j in (i + 1)..n {
                    let p = *args.add(j);
                    if !p.is_null() {
                        let _ = take_value(p);
                    }
                }
                return std::ptr::null_mut();
            }
            arg_vals.push(take_value(arg_ptr));
        }

        // Look up the module function.
        let modules_ptr = match ctx.modules {
            Some(ptr) => ptr,
            None => {
                return ctx.set_error(LangError::engine(format!("unregistered foreign: {}", name)));
            }
        };
        // SAFETY: modules_ptr is a valid trait-object pointer set by the engine.
        let modules: &dyn handl_platform::ModuleLookup = &*modules_ptr;
        let func = match modules.lookup(&name) {
            Some(f) => f,
            None => {
                return ctx.set_error(LangError::engine(format!("unregistered foreign: {}", name)));
            }
        };

        // Convert TaggedValue args to serde_json::Value for the host boundary.
        let val_args: Vec<Value> = arg_vals.iter().map(tagged_to_value).collect();
        // Call the module function.
        let mut pctx = handl_platform::EvalContext::new();
        match func.call(&val_args, &mut pctx) {
            Ok(v) => box_value(TaggedValue::from(v)),
            Err(e) => ctx.set_error(LangError::engine(e.to_string())),
        }
    }
}

/// Call a raw C function by FFI binding index (ADR-0083 §4 direct FFI
/// dispatch).
///
/// This is the fast path that bypasses the `ModuleLookup` trait object
/// indirection: the cranelift engine calls the C symbol directly via
/// `dlsym`, marshal, call, unmarshal — all in native code.
///
/// The `ffi_idx` is an index into `ctx.ffi_table`, which is built at engine
/// construction time from the registered FFI bindings. All argument value
/// pointers are consumed (reclaimed).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_call_ffi_raw(
    ctx: *mut EvalCtx,
    ffi_idx: usize,
    args: *const *mut TaggedValue,
    n: usize,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;

        // Collect arg values.
        let mut arg_vals = Vec::with_capacity(n);
        for i in 0..n {
            let arg_ptr = *args.add(i);
            if arg_ptr.is_null() {
                for j in (i + 1)..n {
                    let p = *args.add(j);
                    if !p.is_null() {
                        let _ = take_value(p);
                    }
                }
                return std::ptr::null_mut();
            }
            arg_vals.push(take_value(arg_ptr));
        }

        // Look up the FFI op from the table.
        let ffi_op = match ctx.ffi_table.get(ffi_idx) {
            Some(op) => op.clone(),
            None => {
                return ctx.set_error(LangError::engine(format!(
                    "ffi: op index {} out of bounds (table has {} entries)",
                    ffi_idx,
                    ctx.ffi_table.len()
                )));
            }
        };

        // Convert TaggedValue args to serde_json::Value for the FFI boundary.
        let val_args: Vec<Value> = arg_vals.iter().map(tagged_to_value).collect();

        // Call the FfiOpFunction directly (raw C ABI / dlsym internally).
        let mut pctx = handl_platform::EvalContext::new();
        use handl_platform::Function;
        match ffi_op.call(&val_args, &mut pctx) {
            Ok(v) => box_value(TaggedValue::from(v)),
            Err(e) => ctx.set_error(LangError::engine(e.to_string())),
        }
    }
}

/// Evaluate a child node by arena index from within a Rust helper (e.g.
/// `apply_prim`).
///
/// Looks up the pre-compiled native function for the node, saves/restores
/// `current_ref`, calls the function with a boxed `Null` incoming (fresh
/// computation — these ops do not feed an incoming value to children), and
/// reclaims the boxed result. Returns the evaluated [`TaggedValue`] or sets
/// `ctx.error` and returns `Err`.
fn eval_child_node(ctx: &mut EvalCtx, arena_idx: usize) -> Result<TaggedValue, LangError> {
    let func = match ctx.compiled.get(&arena_idx).copied() {
        Some(f) => f,
        None => {
            return Err(LangError::engine(format!(
                "no compiled function for node {}",
                arena_idx
            )));
        }
    };
    let saved_ref = ctx.current_ref;
    ctx.current_ref = arena_idx;
    let null_incoming = null_incoming_ptr();
    // SAFETY: func is a valid compiled function pointer for node arena_idx,
    // held alive by the CompiledResult's backend. ctx is a valid &mut EvalCtx.
    let result_ptr = unsafe {
        func(ctx as *mut EvalCtx as *mut u8, null_incoming as *mut u8) as *mut TaggedValue
    };
    ctx.current_ref = saved_ref;
    if result_ptr.is_null() {
        return Err(ctx
            .error
            .take()
            .unwrap_or_else(|| LangError::engine("eval_child_node: null result".to_string())));
    }
    // SAFETY: result_ptr is non-null and was boxed by the child function via
    // box_value. We reclaim it to avoid leaking.
    Ok(unsafe { take_value(result_ptr) })
}

/// `match` prim: pattern-match a scrutinee against a list of arms.
///
/// The node payload is `{"children": [scrut_idx, block_idx]}`; the block node
/// holds the arm indices. Each arm is `{"children": [pattern_idx, body_idx]}`.
/// The first matching arm's body is evaluated in a scope holding the pattern
/// bindings. If no arm matches, an error is returned.
fn prim_match(ctx: &mut EvalCtx) -> *mut TaggedValue {
    let node = match ctx.current_node() {
        Ok(n) => n,
        Err(e) => return ctx.set_error(e),
    };
    let children = match node.payload.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => {
            return ctx.set_error(LangError::engine(
                "match: missing children in payload".to_string(),
            ));
        }
    };
    // `scrutinee`/`block` positions resolve by named role, arity-aware:
    // the optional `motive` slot (ADR-0081 optional roles) shifts `block`
    // to index 2 when present. Positional 0 / `len >= 3 ? 2 : 1` is the
    // role-less fallback.
    let scrut_idx = match ctx
        .role_child_arena(node, "scrutinee")
        .ok()
        .flatten()
        .or_else(|| {
            children
                .first()
                .and_then(|v| v.as_u64())
                .map(|i| i as usize)
        }) {
        Some(i) => i,
        None => {
            return ctx.set_error(LangError::engine(
                "match: missing scrutinee child".to_string(),
            ));
        }
    };
    let block_idx = match ctx
        .role_child_arena(node, "block")
        .ok()
        .flatten()
        .or_else(|| {
            children
                .get(if children.len() >= 3 { 2 } else { 1 })
                .and_then(|v| v.as_u64())
                .map(|i| i as usize)
        }) {
        Some(i) => i,
        None => {
            return ctx.set_error(LangError::engine_node(
                "match: missing block child".to_string(),
                node,
            ));
        }
    };
    let scrut_val = match eval_child_node(ctx, scrut_idx) {
        Ok(v) => v,
        Err(e) => return ctx.set_error(e),
    };
    let ast = unsafe { &*ctx.ast };
    let block_node = match ast.at(AstRef::new(block_idx)) {
        Ok(n) => n,
        Err(e) => return ctx.set_error(e),
    };
    let arm_indices = match block_node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
    {
        Some(c) => c,
        None => {
            return ctx.set_error(LangError::engine(
                "match: block node missing children".to_string(),
            ));
        }
    };
    for arm_entry in arm_indices {
        let arm_idx = match arm_entry.as_u64() {
            Some(i) => i as usize,
            None => {
                return ctx.set_error(LangError::engine(
                    "match: arm index must be a number".to_string(),
                ));
            }
        };
        let arm_node = match ast.at(AstRef::new(arm_idx)) {
            Ok(n) => n,
            Err(e) => return ctx.set_error(e),
        };
        let arm_children = match arm_node.payload.get("children").and_then(|c| c.as_array()) {
            Some(c) => c,
            None => {
                return ctx.set_error(LangError::engine(
                    "match: arm node missing children".to_string(),
                ));
            }
        };
        // `pattern`/`body` positions resolve by named role (ADR-0081);
        // positional 0/1 is the role-less fallback.
        let pattern_idx = match ctx
            .role_child_arena(arm_node, "pattern")
            .ok()
            .flatten()
            .or_else(|| {
                arm_children
                    .first()
                    .and_then(|v| v.as_u64())
                    .map(|i| i as usize)
            }) {
            Some(i) => i,
            None => {
                return ctx.set_error(LangError::engine(
                    "match: arm missing pattern child".to_string(),
                ));
            }
        };
        let body_idx = match ctx
            .role_child_arena(arm_node, "body")
            .ok()
            .flatten()
            .or_else(|| {
                arm_children
                    .get(1)
                    .and_then(|v| v.as_u64())
                    .map(|i| i as usize)
            }) {
            Some(i) => i,
            None => {
                return ctx.set_error(LangError::engine(
                    "match: arm missing body child".to_string(),
                ));
            }
        };
        ctx.env.push_scope();
        let ast = unsafe { &*ctx.ast };
        let scrut_value = tagged_to_value(&scrut_val);
        let matched =
            match pattern_match::try_pattern(ast, AstRef::new(pattern_idx), &scrut_value, ctx) {
                Ok(b) => b,
                Err(e) => {
                    ctx.env.pop_scope();
                    return ctx.set_error(e);
                }
            };
        if matched {
            let result = eval_child_node(ctx, body_idx);
            ctx.env.pop_scope();
            return match result {
                Ok(v) => unsafe { box_value(v) },
                Err(e) => ctx.set_error(e),
            };
        }
        ctx.env.pop_scope();
    }
    ctx.set_error(LangError::engine("match: no arm matched".to_string()))
}

/// `handle` prim: evaluate a body, then dispatch the result to the first
/// matching handler clause.
///
/// Structurally identical to `match` (children = [body_idx, block_idx], block
/// contains `arm` nodes with [pattern_idx, handler_idx]). The body is
/// evaluated first; its result is matched against clause patterns. If no
/// clause matches, the body value is returned as-is (not an error).
fn prim_handle(ctx: &mut EvalCtx) -> *mut TaggedValue {
    let node = match ctx.current_node() {
        Ok(n) => n,
        Err(e) => return ctx.set_error(e),
    };
    let children = match node.payload.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => {
            return ctx.set_error(LangError::engine(
                "handle: missing children in payload".to_string(),
            ));
        }
    };
    // `target`/`impl` positions resolve by named role (ADR-0081);
    // positional 0/1 is the role-less fallback.
    let body_idx = match ctx
        .role_child_arena(node, "target")
        .ok()
        .flatten()
        .or_else(|| {
            children
                .first()
                .and_then(|v| v.as_u64())
                .map(|i| i as usize)
        }) {
        Some(i) => i,
        None => {
            return ctx.set_error(LangError::engine_node(
                "handle: missing body child".to_string(),
                node,
            ));
        }
    };
    let block_idx = match ctx
        .role_child_arena(node, "impl")
        .ok()
        .flatten()
        .or_else(|| children.get(1).and_then(|v| v.as_u64()).map(|i| i as usize))
    {
        Some(i) => i,
        None => {
            return ctx.set_error(LangError::engine_node(
                "handle: missing block child".to_string(),
                node,
            ));
        }
    };
    let body_val = match eval_child_node(ctx, body_idx) {
        Ok(v) => v,
        Err(e) => return ctx.set_error(e),
    };
    // Convert to Value for pattern matching (which works on serde_json::Value).
    let body_value = tagged_to_value(&body_val);
    let ast = unsafe { &*ctx.ast };
    let block_node = match ast.at(AstRef::new(block_idx)) {
        Ok(n) => n,
        Err(e) => return ctx.set_error(e),
    };
    let clause_indices = match block_node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
    {
        Some(c) => c,
        None => {
            return ctx.set_error(LangError::engine(
                "handle: block node missing children".to_string(),
            ));
        }
    };
    for clause_entry in clause_indices {
        let clause_idx = match clause_entry.as_u64() {
            Some(i) => i as usize,
            None => {
                return ctx.set_error(LangError::engine(
                    "handle: clause index must be a number".to_string(),
                ));
            }
        };
        let clause_node = match ast.at(AstRef::new(clause_idx)) {
            Ok(n) => n,
            Err(e) => return ctx.set_error(e),
        };
        let clause_children = match clause_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
        {
            Some(c) => c,
            None => {
                return ctx.set_error(LangError::engine(
                    "handle: clause node missing children".to_string(),
                ));
            }
        };
        // `pattern`/`body` (the handler position) resolve by named role
        // (ADR-0081); positional 0/1 is the role-less fallback.
        let pattern_idx = match ctx
            .role_child_arena(clause_node, "pattern")
            .ok()
            .flatten()
            .or_else(|| {
                clause_children
                    .first()
                    .and_then(|v| v.as_u64())
                    .map(|i| i as usize)
            }) {
            Some(i) => i,
            None => {
                return ctx.set_error(LangError::engine(
                    "handle: clause missing pattern child".to_string(),
                ));
            }
        };
        let handler_idx = match ctx
            .role_child_arena(clause_node, "body")
            .ok()
            .flatten()
            .or_else(|| {
                clause_children
                    .get(1)
                    .and_then(|v| v.as_u64())
                    .map(|i| i as usize)
            }) {
            Some(i) => i,
            None => {
                return ctx.set_error(LangError::engine(
                    "handle: clause missing handler child".to_string(),
                ));
            }
        };
        ctx.env.push_scope();
        let ast = unsafe { &*ctx.ast };
        let matched =
            match pattern_match::try_pattern(ast, AstRef::new(pattern_idx), &body_value, ctx) {
                Ok(b) => b,
                Err(e) => {
                    ctx.env.pop_scope();
                    return ctx.set_error(e);
                }
            };
        if matched {
            let result = eval_child_node(ctx, handler_idx);
            ctx.env.pop_scope();
            return match result {
                Ok(v) => unsafe { box_value(v) },
                Err(e) => ctx.set_error(e),
            };
        }
        ctx.env.pop_scope();
    }
    // No clause matched — return the body value as-is.
    unsafe { box_value(body_val) }
}

// ---------------------------------------------------------------------------
// CompileEngine / CompiledArtifact impls (ADR-0047 §3)
// ---------------------------------------------------------------------------

/// A compiled native (cranelift) artifact. Held in [`Rc`] so the
/// `CompiledResult` (and its JIT module) can be referenced during
/// execution without cloning (JITModule is not Clone).
#[derive(Clone)]
pub struct CraneliftArtifact {
    result: Option<Rc<CompiledResult>>,
    lang: Language,
    ast: Ast,
}

impl handl_runtime::CompiledArtifact for CraneliftArtifact {
    fn run(
        &self,
        modules: Option<Rc<dyn handl_platform::ModuleLookup>>,
    ) -> Result<Value, LangError> {
        if self.ast.is_empty() {
            return Ok(Value::Null);
        }
        let result = self
            .result
            .as_ref()
            .ok_or_else(|| LangError::engine("run: no compiled result (empty program)"))?;
        let engine = match modules {
            Some(m) => CraneliftEngine::with_modules(m),
            None => CraneliftEngine::new(),
        };
        engine.run_compiled(&self.lang, &self.ast, result)
    }

    fn box_clone(&self) -> Box<dyn handl_runtime::CompiledArtifact> {
        Box::new(self.clone())
    }
}

/// The cranelift compile engine.
#[derive(Clone, Default)]
pub struct CraneliftCompileEngine;

impl CraneliftCompileEngine {
    /// Create a new cranelift compile engine.
    pub fn new() -> Self {
        Self
    }
}

impl handl_runtime::CompileEngine for CraneliftCompileEngine {
    fn compile(
        &self,
        lang: &Language,
        ast: &Ast,
        _target: &str,
    ) -> Result<Box<dyn handl_runtime::CompiledArtifact>, LangError> {
        if ast.is_empty() {
            return Ok(Box::new(CraneliftArtifact {
                result: None,
                lang: lang.clone(),
                ast: ast.clone(),
            }));
        }
        if !handl_language::SchemeVersion::v2().supports(&lang.scheme_version) {
            return Err(LangError::engine(format!(
                "compile: unsupported scheme version: {} (engine supports v2)",
                lang.scheme_version
            )));
        }
        let result = compile_all_nodes_jit(lang, ast, &std::collections::HashMap::new())?;
        Ok(Box::new(CraneliftArtifact {
            result: Some(Rc::new(result)),
            lang: lang.clone(),
            ast: ast.clone(),
        }))
    }

    fn box_clone(&self) -> Box<dyn handl_runtime::CompileEngine> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// V2 helpers (extern "C", called from v2 native code)
// ---------------------------------------------------------------------------
//
// These are separate from the v1 helpers (`helper_*`) to keep v1 completely
// untouched. Where a v2 helper is identical to a v1 helper, it delegates to
// the v1 implementation internally. V2-specific behavior (load_var returning
// the name as a string when unbound, eval_child passing the incoming value,
// apply_native with the v2 op set, pair/Either construction) is implemented
// fresh.

/// Push a literal value (from the constant pool) as a boxed clone.
///
/// Pushes a literal value (from the constant pool) as a boxed clone.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_push_literal(
    ctx: *mut EvalCtx,
    lit_idx: usize,
) -> *mut TaggedValue {
    unsafe { helper_push_literal(ctx, lit_idx) }
}

/// Load a variable by name index from the scope stack.
///
/// Loads a variable from the environment. An unbound variable returns the name as
/// a string value (matching the v2 tree-walk convention), not an error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_load_var(
    ctx: *mut EvalCtx,
    name_idx: usize,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        if name_idx >= ctx.names.len() {
            return ctx.set_error(LangError::engine(format!(
                "name index {} out of bounds",
                name_idx
            )));
        }
        let name = &ctx.names[name_idx];
        match ctx.env.lookup(name) {
            Some(v) => box_value(v.clone()),
            None => box_value(TaggedValue::Str(Rc::from(name.as_str()))),
        }
    }
}

/// Load the payload variable: reads `node.payload` as a string (the variable
/// name) and looks it up in the scope.
///
/// Loads the current node's payload as a variable name. An unbound variable returns the
/// name as a string value (matching the v2 tree-walk convention).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_load_payload_var(ctx: *mut EvalCtx) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        match ctx.current_node() {
            Ok(node) => match node.payload.as_str() {
                Some(var_name) => match ctx.env.lookup(var_name) {
                    Some(v) => box_value(v.clone()),
                    None => box_value(TaggedValue::Str(Rc::from(var_name))),
                },
                None => ctx.set_error(LangError::engine(
                    "expected string payload for $payload var".to_string(),
                )),
            },
            Err(e) => ctx.set_error(e),
        }
    }
}

/// Load the current node's payload directly (no-scheme fallback).
///
/// Loads the current node's payload as a value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_load_payload(ctx: *mut EvalCtx) -> *mut TaggedValue {
    unsafe { helper_load_payload(ctx) }
}

/// Evaluate the nth child of the current node.
///
/// Reads `node.payload["children"][n]` to get the child's `AstRef`, then
/// calls the child's pre-compiled v2 native function. Child-node evaluation
/// is a fresh computation: the child receives a null `incoming` — mirroring
/// the tree-walk `eval_node_inner` and the bytecode `exec_node_inner`, which
/// both pass `None`, and this backend's own [`eval_child_node`]. The caller's
/// `incoming` operand is consumed (reclaimed) here because `proj_child`
/// discards it — the flow value never crosses the node boundary. This is what
/// lets a first-class `arr <op>` child (`eval.arr`, `Op { name: "$payload" }`)
/// see an absent incoming and produce the `__arr__` dispatch object that
/// `apply_closure_value_opts` applies later.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_eval_child(
    ctx: *mut EvalCtx,
    child_n: usize,
    incoming: *mut TaggedValue,
    or_null: i64,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        let node = match ctx.current_node() {
            Ok(n) => n,
            Err(e) => return ctx.set_error(e),
        };
        // `or_null` (from `proj_child_or`): an absent child — missing
        // children array or out-of-range index — yields null instead of an
        // error (e.g. the missing `else` of an else-less `if`).
        let tolerate_missing = or_null != 0;
        let children = match node.payload.get("children").and_then(|c| c.as_array()) {
            Some(arr) => arr,
            None if tolerate_missing => return box_value(TaggedValue::Null),
            None => {
                return ctx.set_error(LangError::engine(
                    "expected children array in payload".to_string(),
                ));
            }
        };
        let arena_idx = match children.get(child_n).and_then(|v| v.as_u64()) {
            Some(idx) => idx as usize,
            None if tolerate_missing => return box_value(TaggedValue::Null),
            None => {
                return ctx.set_error(LangError::engine_node(
                    format!("child {} not found", child_n),
                    node,
                ));
            }
        };
        let func = match ctx.compiled.get(&arena_idx).copied() {
            Some(f) => f,
            None => {
                return ctx.set_error(LangError::engine(format!(
                    "no compiled v2 function for node {}",
                    arena_idx
                )));
            }
        };
        // The child is a fresh computation: it gets a null `incoming`, and
        // the caller's `incoming` operand is consumed here (`proj_child`
        // discards it) so the pointer is not leaked or double-forwarded.
        if !incoming.is_null() {
            let _ = take_value(incoming);
        }
        let saved_ref = ctx.current_ref;
        ctx.current_ref = arena_idx;
        let result = func(
            ctx as *mut EvalCtx as *mut u8,
            null_incoming_ptr() as *mut u8,
        ) as *mut TaggedValue;
        ctx.current_ref = saved_ref;
        result
    }
}

/// Extend the environment: push a new scope, bind the name (by index) to the
/// incoming value, and return a boxed clone of the incoming value so it flows
/// through to the next combinator in a `Seq`.
///
/// This is the v2 `extend_env` op. The scope is popped by the enclosing `Seq`
/// (or by the top-level eval if bare).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_extend_env(
    ctx: *mut EvalCtx,
    name_idx: usize,
    val: *mut TaggedValue,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        if name_idx >= ctx.names.len() {
            if !val.is_null() {
                let _ = take_value(val);
            }
            return ctx.set_error(LangError::engine(format!(
                "name index {} out of bounds",
                name_idx
            )));
        }
        let name = ctx.names[name_idx].clone();
        if val.is_null() {
            return ctx.set_error(LangError::engine(
                "extend_env: incoming value is null".to_string(),
            ));
        }
        let value = take_value(val);
        ctx.env.push_scope();
        ctx.env.define(&name, value.clone());
        box_value(value)
    }
}

/// Extend the environment with a `$payload` name: reads
/// `node.payload["name"]` as the bind name, pushes a scope, binds the incoming
/// value, and returns a boxed clone of the incoming value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_extend_env_payload(
    ctx: *mut EvalCtx,
    val: *mut TaggedValue,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        let node = match ctx.current_node() {
            Ok(n) => n,
            Err(e) => {
                if !val.is_null() {
                    let _ = take_value(val);
                }
                return ctx.set_error(e);
            }
        };
        let bind_name = match node.payload.get("name").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => {
                if !val.is_null() {
                    let _ = take_value(val);
                }
                return ctx.set_error(LangError::engine_node(
                    "expected name in payload for $payload extend_env",
                    node,
                ));
            }
        };
        if val.is_null() {
            return ctx.set_error(LangError::engine(
                "extend_env: incoming value is null".to_string(),
            ));
        }
        let value = take_value(val);
        ctx.env.push_scope();
        ctx.env.define(&bind_name, value.clone());
        box_value(value)
    }
}

/// Pop the innermost scope.
///
/// Pops the current scope from the environment stack.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_pop_scope(ctx: *mut EvalCtx) {
    unsafe { helper_pop_scope(ctx) }
}

/// Reclaim a boxed [`Value`] pointer, freeing the allocation.
///
/// Drops a boxed value (decrements refcount).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_drop_value(val: *mut TaggedValue) {
    unsafe { helper_drop_value(val) }
}

/// Apply a closure value (or host function) to argument values.
///
/// The callee and all argument value pointers are consumed (reclaimed). This
/// Applies a closure to arguments by calling the child's v2 compiled
/// function (which takes an incoming value parameter). For closure bodies, the
/// incoming value is a boxed `Null` (closure bodies are evaluated as fresh
/// computations). For host-function string callees, dispatches to the module
/// registry.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_apply_closure(
    ctx: *mut EvalCtx,
    callee: *mut TaggedValue,
    args: *const *mut TaggedValue,
    n: usize,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        if callee.is_null() {
            return ctx.set_error(LangError::engine("closure callee is null".to_string()));
        }
        let closure_val = take_value(callee);

        // Collect arg values.
        let mut arg_vals = Vec::with_capacity(n);
        for i in 0..n {
            let arg_ptr = *args.add(i);
            if arg_ptr.is_null() {
                for j in (i + 1)..n {
                    let p = *args.add(j);
                    if !p.is_null() {
                        let _ = take_value(p);
                    }
                }
                return std::ptr::null_mut();
            }
            arg_vals.push(take_value(arg_ptr));
        }

        apply_closure_value(ctx, closure_val, arg_vals)
    }
}

/// Apply a closure value to a list of argument values. This is the shared
/// core of [`helper_v2_apply_closure`] (callee + args arrive as pointers).
/// Returns a boxed result or a null pointer with the error set on `ctx`.
///
/// ADR-0060: host-function string-callee dispatch has been removed from this
/// function. Host dispatch is now handled by the `call_host` native op. This
/// function only handles closure application (`__arrow__` values).
fn apply_closure_value(
    ctx: &mut EvalCtx,
    closure_val: TaggedValue,
    arg_vals: Vec<TaggedValue>,
) -> *mut TaggedValue {
    apply_closure_value_opts(ctx, closure_val, arg_vals, true)
}

/// Shared core of [`apply_closure_value`]. When `unpack_arr` is false the
/// single-argument array unpack is skipped — the `flow` (`>>>`) op relies on
/// this to pass a list through as the callee's single argument (mirroring
/// the v2 tree-walk's `flow` arm).
fn apply_closure_value_opts(
    ctx: &mut EvalCtx,
    closure_val: TaggedValue,
    arg_vals: Vec<TaggedValue>,
    unpack_arr: bool,
) -> *mut TaggedValue {
    // SAFETY: this is the shared core factored out of the `unsafe extern "C"`
    // helper `helper_v2_apply_closure`. It calls the unsafe `box_value`
    // helper. Callers must uphold the same safety contract as that helper:
    // `ctx` is a valid `&mut EvalCtx`.
    unsafe {
        // First-class `arr <op>` dispatch object (`{__arr__: {op}}`):
        // apply the op to the args — the tree-walk `apply_app` `__arr__`
        // arm.
        if let Some(inner) = closure_val.get(ARR_DISPATCH_TAG) {
            let op = match inner.get("op").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return ctx.set_error(LangError::engine(
                        "arr: missing op in dispatch object".to_string(),
                    ));
                }
            };
            let incoming = arg_vals.first().cloned().unwrap_or(TaggedValue::Null);
            return arr_dispatch(ctx, &op, &incoming);
        }
        // Bound-arrow record `{__term__, __bound__}` (ADR-0120 Phase 1 —
        // generated by `desugar.data` for `Type.rec`): applying it runs
        // `__term__` on `__bound__ ++ args`, delivered as ONE array
        // argument — the dispatch is a unary arrow over the flat `__in`
        // tuple `[motive, c0.., params.., v]`.
        if let TaggedValue::Obj(map) = &closure_val {
            if let Some(term_val) = map.get("__term__") {
                let mut elems: Vec<TaggedValue> = Vec::new();
                if let Some(bound) = map.get("__bound__") {
                    match bound {
                        TaggedValue::Arr(items) => {
                            elems.extend(items.iter().cloned());
                        }
                        other => elems.push(other.clone()),
                    }
                }
                elems.extend(arg_vals);
                let term_val = term_val.clone();
                let tuple = TaggedValue::Arr(Rc::from(elems));
                // `unpack_arr = false`: `__in` binds to the whole tuple,
                // not its first element.
                return apply_closure_value_opts(ctx, term_val, vec![tuple], false);
            }
        }
        // Closure application — must be a TaggedValue::Arrow.
        // Share the captured env via Rc::clone instead of deep-cloning the
        // HashMap (Fix #8).
        let (params, rest_param, body_idx, captured_env, array_pattern) = match &closure_val {
            TaggedValue::Arrow {
                params,
                rest,
                body,
                env,
                array_pattern,
                ..
            } => (
                params.clone(),
                rest.clone(),
                *body,
                Rc::clone(env),
                *array_pattern,
            ),
            _ => {
                return ctx.set_error(LangError::engine(
                    "callee is not an arrow-value".to_string(),
                ));
            }
        };

        let func = match ctx.compiled.get(&body_idx).copied() {
            Some(f) => f,
            None => {
                return ctx.set_error(LangError::engine(format!(
                    "no compiled v2 function for closure body {}",
                    body_idx
                )));
            }
        };

        // Array-pattern arrows (`[x, y] ~> body`) consume a single array
        // argument and destructure it by position.
        if array_pattern {
            if arg_vals.len() != 1 {
                return ctx.set_error(LangError::engine(format!(
                    "array-pattern arrow expects 1 array argument, got {}",
                    arg_vals.len()
                )));
            }
            let arr = arg_vals[0].as_array().ok_or_else(|| {
                LangError::engine("array-pattern arrow expects an array argument".to_string())
            });
            let arr = match arr {
                Ok(a) => a,
                Err(e) => return ctx.set_error(e),
            };
            let fixed = params.len();
            if arr.len() < fixed {
                return ctx.set_error(LangError::engine(format!(
                    "array-pattern arrow expects at least {} element(s), got {}",
                    fixed,
                    arr.len()
                )));
            }
            if rest_param.is_none() && arr.len() != fixed {
                return ctx.set_error(LangError::engine(format!(
                    "array-pattern arrow expects exactly {} element(s), got {}",
                    fixed,
                    arr.len()
                )));
            }
            let (head, tail) = arr.split_at(fixed);
            let saved_env = std::mem::replace(&mut ctx.env, ScopeStack::new());
            ctx.env.scopes[0] = captured_env;
            ctx.env.push_scope();
            for (param, arg) in params.iter().zip(head.iter()) {
                ctx.env.define(param, arg.clone());
            }
            if let Some(rest_name) = &rest_param {
                ctx.env.define(
                    rest_name.as_ref(),
                    TaggedValue::Arr(Rc::from(tail.to_vec())),
                );
            }
            let saved_ref = ctx.current_ref;
            ctx.current_ref = body_idx;
            let null_incoming = null_incoming_ptr();
            let result =
                func(ctx as *mut EvalCtx as *mut u8, null_incoming as *mut u8) as *mut TaggedValue;
            ctx.current_ref = saved_ref;
            ctx.env = saved_env;
            return result;
        }

        // Positional arrows: ADR-0060 unpack single array arg for
        // variable-arity call; ADR-0074 flatten a single `Pair`.
        let mut arg_vals = arg_vals;
        if unpack_arr && arg_vals.len() == 1 {
            if let TaggedValue::Arr(arr) = &arg_vals[0] {
                arg_vals = arr.iter().cloned().collect();
            }
        }
        if params.len() > 1 && arg_vals.len() == 1 && matches!(arg_vals[0], TaggedValue::Pair(_)) {
            arg_vals = flatten_pair(&arg_vals[0]);
        }

        let param_names: Vec<String> = params.iter().map(|s| s.to_string()).collect();

        // Rest param (`[a, ...rest] ~> body`): surplus args bind to the
        // rest name as an array, so `arg_vals.len() >= params.len()`.
        let arity_ok = if rest_param.is_some() {
            arg_vals.len() >= params.len()
        } else {
            param_names.len() == arg_vals.len()
        };
        if !arity_ok {
            return ctx.set_error(LangError::engine(format!(
                "arrow-value expects {} args, got {}",
                param_names.len(),
                arg_vals.len()
            )));
        }

        // Swap in the captured env + new scope with params.
        let saved_env = std::mem::replace(&mut ctx.env, ScopeStack::new());
        ctx.env.scopes[0] = captured_env;
        ctx.env.push_scope();
        for (param, arg) in param_names.iter().zip(arg_vals.iter()) {
            ctx.env.define(param, arg.clone());
        }
        // Bind the rest param to the surplus args as an array.
        if let Some(rest_name) = &rest_param {
            ctx.env.define(
                rest_name.as_ref(),
                TaggedValue::Arr(Rc::from(arg_vals[param_names.len()..].to_vec())),
            );
        }

        let saved_ref = ctx.current_ref;
        ctx.current_ref = body_idx;

        // Closure bodies receive a boxed Null as incoming (fresh computation).
        // Reuse the shared sentinel to avoid per-call allocation (Fix #8).
        let null_incoming = null_incoming_ptr();
        let result =
            func(ctx as *mut EvalCtx as *mut u8, null_incoming as *mut u8) as *mut TaggedValue;

        ctx.current_ref = saved_ref;
        ctx.env = saved_env;

        result
    }
}

/// Flatten a right-nested `Pair` tree into a flat argument list (mirrors
/// the v2 tree-walk's `flatten_pair`): `Pair(a, Pair(b, c))` → `[a, b, c]`.
fn flatten_pair(v: &TaggedValue) -> Vec<TaggedValue> {
    match v {
        TaggedValue::Pair(p) => {
            let mut left = flatten_pair(&p.0);
            left.extend(flatten_pair(&p.1));
            left
        }
        other => vec![other.clone()],
    }
}

/// `flow` (`>>>`) native op: function application with a single input
/// argument (ADR-0074 follow-up #3). Mirrors the v2 tree-walk's `flow`
/// arm exactly: pass the left operand through as one argument, only
/// destructuring a categorical `Pair` (`***`) into multiple positional
/// args. String callees dispatch to the host module registry (same path
/// as `call_host`); arrow callees apply as closures — with the
/// single-array unpack *disabled* so a list input stays one argument.
fn prim_v2_flow(ctx: &mut EvalCtx, args: &[TaggedValue]) -> *mut TaggedValue {
    if args.len() != 2 {
        return ctx.set_error(LangError::engine(format!(
            "flow: expected 2 args, got {}",
            args.len()
        )));
    }
    let input = &args[0];
    let callee = &args[1];
    let actual_args: Vec<TaggedValue> = if matches!(input, TaggedValue::Pair(_)) {
        flatten_pair(input)
    } else {
        vec![input.clone()]
    };
    if let TaggedValue::Str(name) = callee {
        let modules_ptr = match ctx.modules {
            Some(p) => p,
            None => {
                return ctx.set_error(LangError::engine("flow: no module registry".to_string()));
            }
        };
        let modules: &dyn handl_platform::ModuleLookup = unsafe { &*modules_ptr };
        let func = match modules.lookup(name.as_ref()) {
            Some(f) => f,
            None => {
                return ctx.set_error(LangError::engine(format!(
                    "flow: function '{}' not found",
                    name
                )));
            }
        };
        let call_args: Vec<Value> = actual_args
            .iter()
            .map(handl_value::tagged_to_value)
            .collect();
        let mut pctx = handl_platform::EvalContext::new();
        match func.call(&call_args, &mut pctx) {
            Ok(v) => unsafe { box_value(TaggedValue::from(v)) },
            Err(e) => ctx.set_error(LangError::engine(e.to_string())),
        }
    } else {
        apply_closure_value_opts(ctx, callee.clone(), actual_args, false)
    }
}

/// `import` native op (Phase 9.5): load a module via the configured
/// [`ImportResolver`] and bind its top-level exports into the current
/// scope. Mirrors the v2 tree-walk's `prim_import` and the bytecode VM's
/// `prim_v2_import`.
fn prim_v2_import(ctx: &mut EvalCtx, args: &[TaggedValue]) -> *mut TaggedValue {
    let node = match ctx.current_node() {
        Ok(n) => n.clone(),
        Err(e) => return ctx.set_error(e),
    };

    let path: String = match args
        .first()
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            node.payload
                .get("path")
                .and_then(|v| v.as_str())
                .map(String::from)
        }) {
        Some(p) => p,
        None => {
            return ctx.set_error(LangError::engine_node(
                "import: missing module path (expected string argument or 'path' payload)"
                    .to_string(),
                &node,
            ));
        }
    };

    let resolver = match ctx.import_resolver.as_ref() {
        Some(r) => r,
        None => {
            return ctx.set_error(LangError::engine_node(
                "import: no import resolver configured".to_string(),
                &node,
            ));
        }
    };

    let bindings = match resolver.resolve_import(&path) {
        Ok(v) => TaggedValue::from(v),
        Err(e) => {
            return ctx.set_error(LangError::engine_node(
                format!("import \"{}\": {}", path, e),
                &node,
            ));
        }
    };

    // Binding semantics mirror the tree-walk `prim_import` (ADR-0096):
    // `names` (non-"*") projects a single export, `alias` alone binds the
    // whole module object, otherwise each export is spliced into scope.
    // A `names` array supports braced import lists such as
    // `from "p" import { a, b as x }`.
    let names_val = node.payload.get("names");
    let alias = node.payload.get("alias").and_then(|v| v.as_str());
    if let Some(arr) = names_val.and_then(|v| v.as_array()) {
        for item in arr {
            let (imported, local) = match item {
                Value::String(s) => (s.as_str(), s.as_str()),
                Value::Object(obj) => {
                    let n = match obj.get("name").and_then(|v| v.as_str()) {
                        Some(s) => s,
                        None => {
                            return ctx.set_error(LangError::engine_node(
                                "import: missing 'name' in braced import entry".to_string(),
                                &node,
                            ));
                        }
                    };
                    let a = obj.get("as").and_then(|v| v.as_str()).unwrap_or(n);
                    (n, a)
                }
                _ => {
                    return ctx.set_error(LangError::engine_node(
                        "import: invalid braced import entry".to_string(),
                        &node,
                    ));
                }
            };
            if imported == "*" {
                return ctx.set_error(LangError::engine_node(
                    "import: cannot use '*' inside a braced import list".to_string(),
                    &node,
                ));
            }
            let exported = bindings
                .as_object()
                .and_then(|map| map.get(imported).cloned());
            match exported {
                Some(value) => ctx.env.define(local, value),
                None => {
                    return ctx.set_error(LangError::engine_node(
                        format!("import \"{}\": module has no export `{}`", path, imported),
                        &node,
                    ));
                }
            }
        }
    } else if let Some(imported) = names_val.and_then(|v| v.as_str()).filter(|n| *n != "*") {
        let exported = bindings
            .as_object()
            .and_then(|map| map.get(imported).cloned());
        match exported {
            Some(value) => ctx.env.define(alias.unwrap_or(imported), value),
            None => {
                return ctx.set_error(LangError::engine_node(
                    format!("import \"{}\": module has no export `{}`", path, imported),
                    &node,
                ));
            }
        }
    } else if let Some(alias) = alias {
        ctx.env.define(alias, bindings);
    } else if let Some(map) = bindings.as_object() {
        for (name, val) in map.iter() {
            ctx.env.define(name.as_ref(), val.clone());
        }
    }

    unsafe { box_value(TaggedValue::Null) }
}

/// `loop_step` derived step op (ADR-0052 §"Derived forms"): the ergonomic
/// `loop` surface lowering. `args = [body_proc]` is the body closure
/// (evaluated from the carried sub-scheme arg); `incoming` is the loop pair
/// `(in, state)` from the enclosing `Loop` combinator.
///
/// The body closure is applied to the current state (`in` on the first step,
/// `state` thereafter) via [`apply_closure_value`]. It returns:
/// - `{ state: v }`  → `either_right(v)` (continue with new state `v`)
/// - `{ out: v }`    → `either_left(v)`  (terminate with `v`, backward compat)
/// - bare value      → `either_left(v)`  (terminate with `v`, v0.1 `return v`)
///
/// Param destructuring: if the body closure has >1 params and the current state
/// is an object, bind each param to the corresponding field by name. Mirrors
/// the v2 tree-walk's `eval_derived_step` `loop_step` arm exactly.
fn prim_v2_loop_step(
    ctx: &mut EvalCtx,
    args: &[TaggedValue],
    incoming: Option<TaggedValue>,
) -> *mut TaggedValue {
    let inc = incoming.unwrap_or(TaggedValue::Null);
    let (in_val, state) = match &inc {
        TaggedValue::Pair(p) => (p.0.clone(), p.1.clone()),
        _ => (TaggedValue::Null, TaggedValue::Null),
    };
    let body_proc = match args.first() {
        Some(b) => b.clone(),
        None => {
            return ctx.set_error(LangError::engine(
                "loop_step: missing body closure".to_string(),
            ));
        }
    };
    let current = if matches!(state, TaggedValue::Null) {
        in_val
    } else {
        state
    };
    // The argument mapping and outcome semantics are shared
    // (`build_loop_step_args`/`loop_step_outcome`); only the closure
    // application is this backend's machinery.
    let call_args = build_loop_step_args(&body_proc, &current);
    let result_ptr = apply_closure_value(ctx, body_proc, call_args);
    if result_ptr.is_null() {
        // Error already set on ctx by apply_closure_value.
        return std::ptr::null_mut();
    }
    let result = unsafe { take_value(result_ptr) };
    let either = loop_step_outcome(result);
    unsafe { box_value(either) }
}

/// `fold_step` derived step op (ADR-0120 Phase 1): the ArrowLoop protocol
/// used by the `loop` fold node. `args = [step_proc]` is the step closure
/// (evaluated once, hoisted); `incoming` is the loop pair `(in, state)`
/// from the enclosing `Loop` combinator. The step is applied to the pair
/// components as a 2-argument call and its result is itself the Either —
/// `left(result)` terminates, `right(next_state)` feeds back. Mirrors the
/// v2 tree-walk's `eval_derived_step` `fold_step` arm.
fn prim_v2_fold_step(
    ctx: &mut EvalCtx,
    args: &[TaggedValue],
    incoming: Option<TaggedValue>,
) -> *mut TaggedValue {
    let inc = incoming.unwrap_or(TaggedValue::Null);
    let (in_val, state) = match &inc {
        TaggedValue::Pair(p) => (p.0.clone(), p.1.clone()),
        _ => (inc.clone(), TaggedValue::Null),
    };
    let step_proc = match args.first() {
        Some(s) => s.clone(),
        None => {
            return ctx.set_error(LangError::engine(
                "fold_step: missing step closure".to_string(),
            ));
        }
    };
    let result_ptr = apply_closure_value(ctx, step_proc, vec![in_val, state]);
    if result_ptr.is_null() {
        return std::ptr::null_mut();
    }
    result_ptr
}

/// Call a foreign (host) function (by name index).
///
/// Calls a foreign (host) function (identical behavior —
/// foreign dispatch does not use the incoming value).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_call_foreign(
    ctx: *mut EvalCtx,
    name_idx: usize,
    args: *const *mut TaggedValue,
    n: usize,
) -> *mut TaggedValue {
    unsafe { helper_call_foreign(ctx, name_idx, args, n) }
}

/// Construct a pair value `{"__pair__": [a, b]}` from two value pointers.
///
/// Both input pointers are consumed (reclaimed).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_pair(
    _ctx: *mut EvalCtx,
    a: *mut TaggedValue,
    b: *mut TaggedValue,
) -> *mut TaggedValue {
    unsafe {
        let a_val = if a.is_null() {
            TaggedValue::Null
        } else {
            take_value(a)
        };
        let b_val = if b.is_null() {
            TaggedValue::Null
        } else {
            take_value(b)
        };
        box_value(TaggedValue::Pair(Rc::new((a_val, b_val))))
    }
}

/// Extract the first component of a pair value.
///
/// The input pointer is **not** consumed (the caller is responsible for
/// dropping it via `helper_v2_drop_value` when done with both `fst` and
/// `snd`). Returns a boxed clone of the first component. If the input is
/// not a pair, sets an error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_unpair_fst(
    ctx: *mut EvalCtx,
    val: *mut TaggedValue,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        if val.is_null() {
            return ctx.set_error(LangError::engine("unpair_fst: null input".to_string()));
        }
        let v = &*val;
        match v {
            TaggedValue::Pair(p) => box_value(p.0.clone()),
            _ => ctx.set_error(LangError::engine(
                "unpair_fst: input is not a pair".to_string(),
            )),
        }
    }
}

/// Extract the second component of a pair value.
///
/// The input pointer is **not** consumed (the caller is responsible for
/// dropping it via `helper_v2_drop_value` when done with both `fst` and
/// `snd`). Returns a boxed clone of the second component. If the input is
/// not a pair, sets an error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_unpair_snd(
    ctx: *mut EvalCtx,
    val: *mut TaggedValue,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        if val.is_null() {
            return ctx.set_error(LangError::engine("unpair_snd: null input".to_string()));
        }
        let v = &*val;
        match v {
            TaggedValue::Pair(p) => box_value(p.1.clone()),
            _ => ctx.set_error(LangError::engine(
                "unpair_snd: input is not a pair".to_string(),
            )),
        }
    }
}

/// Construct a `left` Either value `{"__either__": "left", "value": v}`.
///
/// The input pointer is consumed (reclaimed).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_either_left(
    _ctx: *mut EvalCtx,
    val: *mut TaggedValue,
) -> *mut TaggedValue {
    unsafe {
        let v = if val.is_null() {
            TaggedValue::Null
        } else {
            take_value(val)
        };
        box_value(TaggedValue::EitherLeft(Rc::new(v)))
    }
}

/// Construct a `right` Either value `{"__either__": "right", "value": v}`.
///
/// The input pointer is consumed (reclaimed).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_either_right(
    _ctx: *mut EvalCtx,
    val: *mut TaggedValue,
) -> *mut TaggedValue {
    unsafe {
        let v = if val.is_null() {
            TaggedValue::Null
        } else {
            take_value(val)
        };
        box_value(TaggedValue::EitherRight(Rc::new(v)))
    }
}

/// Check whether a value is a `left` Either.
///
/// Returns 1 if the value is `{"__either__": "left", ...}`, 0 otherwise. The
/// input pointer is NOT consumed (it is read-only).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_either_is_left(_ctx: *mut EvalCtx, val: *mut TaggedValue) -> u8 {
    unsafe {
        if val.is_null() {
            return 0;
        }
        let v = &*val;
        if matches!(v, TaggedValue::EitherLeft(_)) {
            1
        } else {
            0
        }
    }
}

/// Extract the carried value from an Either.
///
/// The input pointer is consumed (reclaimed). Returns a boxed clone of the
/// carried value. If the input is not an Either, sets an error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_either_value(
    ctx: *mut EvalCtx,
    val: *mut TaggedValue,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        if val.is_null() {
            return ctx.set_error(LangError::engine("either_value: null input".to_string()));
        }
        let v = take_value(val);
        match either_value(&v) {
            Some(inner) => box_value(inner.clone()),
            _ => ctx.set_error(LangError::engine(
                "either_value: input is not an Either".to_string(),
            )),
        }
    }
}

/// Extract the appropriate incoming value for the arg sub-schemes of a
/// derived step op (`while_step`/`for_step`/`fix_step`).
///
/// The `pair` is the loop pair `(in, state)` from the enclosing `Loop`.
/// The `op_idx` is the name pool index of the op name.
///
/// - `while_step`/`fix_step`: returns `current` = `in` on the first step
///   (state is Null), `state` thereafter.
/// - `for_step`: returns `acc` = `snd(in)` on the first step (state is
///   Null), `snd(state)` thereafter.
///
/// The `pair` pointer is NOT consumed (cloned). The returned pointer is
/// a freshly boxed value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_derived_step_arg_incoming(
    ctx: *mut EvalCtx,
    pair: *mut TaggedValue,
    op_idx: usize,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        if op_idx >= ctx.names.len() {
            return ctx.set_error(LangError::engine(format!(
                "derived_step_arg_incoming: op index {} out of bounds",
                op_idx
            )));
        }
        let op = ctx.names[op_idx].clone();
        if pair.is_null() {
            return box_value(TaggedValue::Null);
        }
        let p = (*pair).clone();
        let (in_val, state) = match &p {
            TaggedValue::Pair(p) => (p.0.clone(), p.1.clone()),
            _ => (TaggedValue::Null, TaggedValue::Null),
        };
        let arg_incoming = if op == "for_step" {
            if matches!(state, TaggedValue::Null) {
                // First step: in_val = pair(count, initial). acc = snd(in_val).
                match &in_val {
                    TaggedValue::Pair(p) => p.1.clone(),
                    _ => TaggedValue::Null,
                }
            } else {
                // Later: state = pair(counter, acc). acc = snd(state).
                match &state {
                    TaggedValue::Pair(p) => p.1.clone(),
                    _ => TaggedValue::Null,
                }
            }
        } else {
            // while_step / fix_step: current = in (first step) or state.
            if matches!(state, TaggedValue::Null) {
                in_val
            } else {
                state
            }
        };
        box_value(arg_incoming)
    }
}

/// Apply a v2 native op (by name index) to argument values, with the incoming
/// value as a separate parameter.
///
/// All argument value pointers are consumed (reclaimed). The incoming pointer
/// is NOT consumed (it is cloned by the ops that use it). The native op set
/// matches the v2 tree-walk exactly: arithmetic, comparison, logic, `id`,
/// `tag`, `closure`, and utility ops (`is_pos`/`dec`/`inc`/`half_floor`/
/// `inc_fst`/`countdown_step`/`infinite_step`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_apply_native(
    ctx: *mut EvalCtx,
    op_idx: usize,
    args: *const *mut TaggedValue,
    n: usize,
    incoming: *mut TaggedValue,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        if op_idx >= ctx.names.len() {
            for i in 0..n {
                let p = *args.add(i);
                if !p.is_null() {
                    let _ = take_value(p);
                }
            }
            return ctx.set_error(LangError::engine(format!(
                "op index {} out of bounds",
                op_idx
            )));
        }
        let op = ctx.names[op_idx].clone();

        // Collect arg values.
        let mut arg_vals = Vec::with_capacity(n);
        for i in 0..n {
            let arg_ptr = *args.add(i);
            if arg_ptr.is_null() {
                for j in (i + 1)..n {
                    let p = *args.add(j);
                    if !p.is_null() {
                        let _ = take_value(p);
                    }
                }
                return std::ptr::null_mut();
            }
            arg_vals.push(take_value(arg_ptr));
        }

        // Read the incoming value (read-only — do not consume the pointer).
        let incoming_val = if incoming.is_null() {
            None
        } else {
            Some((*incoming).clone())
        };

        // `$payload` ops (`arr <op>` surface form, `eval.arr`): the op
        // name lives in the current node's payload — resolve it at
        // runtime and dispatch through the handler-impl → native path
        // (mirrors the tree-walk `eval_arr`/`arr_dispatch`).
        if op == "$payload" {
            return apply_payload_op(ctx, &arg_vals, incoming_val);
        }

        apply_native_v2(ctx, &op, &arg_vals, incoming_val)
    }
}

/// Tag for first-class `arr <op>` dispatch objects — mirrors the
/// tree-walk's `ARR_DISPATCH_TAG` (ADR-0103 value shape).
const ARR_DISPATCH_TAG: &str = "__arr__";

/// Resolve and dispatch a `$payload` op (`arr <op>`).
///
/// The op name is read from the current node's payload — a bare string,
/// or the `name`/`op` field of an object payload (same resolution order
/// as the tree-walk `eval_arr`). With no args: a present incoming operand
/// is dispatched through [`arr_dispatch`]; an absent one produces the
/// first-class `{__arr__: {op}}` dispatch object that
/// [`apply_closure_value_opts`] applies later. With args, falls through
/// to [`apply_native_v2`] under the resolved name.
fn apply_payload_op(
    ctx: &mut EvalCtx,
    args: &[TaggedValue],
    incoming: Option<TaggedValue>,
) -> *mut TaggedValue {
    let op = match ctx.current_node() {
        Ok(node) => match node
            .payload
            .as_str()
            .or_else(|| node.payload.get("name").and_then(|v| v.as_str()))
            .or_else(|| node.payload.get("op").and_then(|v| v.as_str()))
        {
            Some(s) => s.to_string(),
            None => {
                return ctx.set_error(LangError::engine(
                    "expected string payload for $payload op".to_string(),
                ));
            }
        },
        Err(e) => return ctx.set_error(e),
    };
    if args.is_empty() {
        match incoming {
            Some(inc) => arr_dispatch(ctx, &op, &inc),
            None => {
                // First-class `arr <op>`: the dispatch object
                // `apply_closure_value_opts` recognizes (mirrors the
                // tree-walk `__arr__` value shape).
                let mut map = FxHashMap::<Rc<str>, TaggedValue>::default();
                map.insert(Rc::from("op"), TaggedValue::Str(Rc::from(op.as_str())));
                let mut obj = FxHashMap::<Rc<str>, TaggedValue>::default();
                obj.insert(Rc::from(ARR_DISPATCH_TAG), TaggedValue::Obj(Rc::new(map)));
                unsafe { box_value(TaggedValue::Obj(Rc::new(obj))) }
            }
        }
    } else {
        apply_native_v2(ctx, &op, args, incoming)
    }
}

/// Dispatch a payload-resolved `arr <op>` against a concrete operand.
/// Mirrors the tree-walk `arr_dispatch`: a handler implementation carried
/// by the operand's `impls` (or a same-named field) wins; otherwise the
/// operand's `Pair` spine is flattened into positional args for the
/// native op.
///
/// The runtime handler stack the tree-walk also consults does not exist
/// in this backend — `Handle` terms are eliminated at compile time by
/// `Term::eliminate_handlers`.
fn arr_dispatch(ctx: &mut EvalCtx, op: &str, incoming: &TaggedValue) -> *mut TaggedValue {
    if let Some(impl_val) = handler_impl(incoming, op) {
        if matches!(impl_val, TaggedValue::Arrow { .. }) {
            return apply_closure_value(ctx, impl_val, vec![incoming.clone()]);
        }
        // Serialized `Term` impl: this backend cannot JIT new code at
        // runtime, so it runs the impl through the shared pure-data term
        // interpreter with the operand as input — the closest analog of
        // the tree-walk's `eval_inner(&impl_term, ..., incoming)`.
        let impl_term: Term = match serde_json::from_value(tagged_to_value(&impl_val)) {
            Ok(t) => t,
            Err(e) => {
                return ctx.set_error(LangError::engine(format!(
                    "handler impl for op '{op}' is not a Term or arrow: {e}"
                )));
            }
        };
        return match plugin_engine::interpret(&impl_term, tagged_to_value(incoming)) {
            Ok(v) => unsafe { box_value(TaggedValue::from(v)) },
            Err(e) => ctx.set_error(LangError::engine(format!(
                "handler impl for op '{op}' failed: {e}"
            ))),
        };
    }
    let op_args = flatten_pair(incoming);
    apply_native_v2(ctx, op, &op_args, Some(incoming.clone()))
}

/// Extract the implementation entry for `op` from a handler-like value
/// (mirrors the tree-walk `handler_impl`).
///
/// A canonical `handler` value is a tagged object
/// `{"__tag__": "handler", "impls": {"<op>": <arrow or Term>, ...}}`.
/// Surface handler records are plain objects whose keys are the operation
/// names: `{"<op>": <arrow or Term>, ...}`. Returns the value for `op`,
/// or `None` when the value has no implementation for `op`.
fn handler_impl(v: &TaggedValue, op: &str) -> Option<TaggedValue> {
    v.get("impls")
        .and_then(|impls| impls.get(op))
        .or_else(|| v.get(op))
        .cloned()
}

/// Fast-path inline arithmetic helper for `Int`/`Int` operands (Fix #7).
///
/// This is a specialised alternative to [`helper_v2_apply_native`] for the
/// common binary arithmetic and comparison ops (`add`, `sub`, `mul`, `div`,
/// `mod`, `eq`, `neq`, `lt`, `lte`, `gt`, `gte`). It avoids the per-call costs
/// of the general helper:
///
/// - **No string dispatch**: `op_code` is a compile-time-baked integer; the
///   general helper looks up `ctx.names[op_idx]` and clones the string.
/// - **No `Vec` allocation**: the two arg pointers are passed directly; the
///   general helper allocates a `Vec` and pushes each arg.
/// - **No incoming clone**: arithmetic ops never use `incoming`; the general
///   helper always clones it.
///
/// **Fast path** (both args are `TaggedValue::Int`): reads the `i64` payloads,
/// performs the operation directly, boxes the result, frees the arg boxes, and
/// returns. This is semantically identical to the general helper for the
/// `Int`/`Int` case.
///
/// **Slow path** (either arg is not `Int`): falls back to the full
/// [`helper_v2_apply_native`] path — looks up the op name from `op_idx`,
/// collects the args into a `Vec`, clones `incoming`, and calls
/// [`apply_native_v2`].
///
/// # Op codes
///
/// | code | op   | | code | op  |
/// |------|------|-|------|-----|
/// | 0    | add  | | 6    | neq |
/// | 1    | sub  | | 7    | lt  |
/// | 2    | mul  | | 8    | lte |
/// | 3    | div  | | 9    | gt  |
/// | 4    | mod  | | 10   | gte |
/// | 5    | eq   | |      |     |
///
/// # Safety
///
/// `ctx` must be a valid `&mut EvalCtx`. `a_ptr` and `b_ptr` must be valid
/// pointers returned by [`box_value`] (or null on error). `incoming` may be
/// null. Both `a_ptr` and `b_ptr` are consumed (freed) on all paths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_int_arith(
    ctx: *mut EvalCtx,
    op_code: usize,
    op_idx: usize,
    a_ptr: *mut TaggedValue,
    b_ptr: *mut TaggedValue,
    incoming: *mut TaggedValue,
) -> *mut TaggedValue {
    unsafe {
        let ctx = &mut *ctx;
        // Fast path: read both args without consuming, check if both Int.
        // Copy the i64 payloads in a separate scope so the borrows are
        // released before we free the arg boxes below.
        if !a_ptr.is_null() && !b_ptr.is_null() {
            let (a_int, b_int): (Option<i64>, Option<i64>) = {
                let a = &*a_ptr;
                let b = &*b_ptr;
                match (a, b) {
                    (TaggedValue::Int(x), TaggedValue::Int(y)) => (Some(*x), Some(*y)),
                    _ => (None, None),
                }
            };
            if let (Some(x), Some(y)) = (a_int, b_int) {
                let result = match op_code {
                    0 => Some(TaggedValue::Int(x.wrapping_add(y))),
                    1 => Some(TaggedValue::Int(x.wrapping_sub(y))),
                    2 => Some(TaggedValue::Int(x.wrapping_mul(y))),
                    3 => {
                        if y == 0 {
                            // Fall back to helper for the error message.
                            None
                        } else {
                            Some(TaggedValue::Int(x.wrapping_div(y)))
                        }
                    }
                    4 => {
                        if y == 0 {
                            // Fall back to helper for the error message.
                            None
                        } else {
                            Some(TaggedValue::Int(x.wrapping_rem(y)))
                        }
                    }
                    5 => Some(TaggedValue::Bool(x == y)),
                    6 => Some(TaggedValue::Bool(x != y)),
                    // Comparisons use f64 semantics to match cmp_num_v2
                    // (which converts via Num::to_f64). For integers within
                    // the f64 safe range (|v| < 2^53) this is identical to
                    // i64 comparison.
                    7 => Some(TaggedValue::Bool((x as f64) < (y as f64))),
                    8 => Some(TaggedValue::Bool((x as f64) <= (y as f64))),
                    9 => Some(TaggedValue::Bool((x as f64) > (y as f64))),
                    10 => Some(TaggedValue::Bool((x as f64) >= (y as f64))),
                    _ => None, // unknown op code — fall back
                };

                if let Some(result) = result {
                    // Free the arg boxes (they are consumed by this helper).
                    let _ = take_value(a_ptr);
                    let _ = take_value(b_ptr);
                    return box_value(result);
                }
            }
        }

        // Slow path: fall back to the general apply_native path.
        // Consume the arg pointers (or free them on error).
        let mut arg_vals = Vec::with_capacity(2);
        for ptr in [a_ptr, b_ptr] {
            if ptr.is_null() {
                // Error already set; consume remaining args.
                for remaining in [b_ptr] {
                    if !remaining.is_null() {
                        let _ = take_value(remaining);
                    }
                }
                return std::ptr::null_mut();
            }
            arg_vals.push(take_value(ptr));
        }

        // Look up the op name for the slow path.
        if op_idx >= ctx.names.len() {
            return ctx.set_error(LangError::engine(format!(
                "op index {} out of bounds",
                op_idx
            )));
        }
        let op = ctx.names[op_idx].clone();

        // Read the incoming value (read-only — do not consume the pointer).
        let incoming_val = if incoming.is_null() {
            None
        } else {
            Some((*incoming).clone())
        };

        apply_native_v2(ctx, &op, &arg_vals, incoming_val)
    }
}

/// Inspect a `Loop` body result and return a tag indicating the loop action.
///
/// Returns:
/// - `0` if the result is an Either **left** (terminate with the carried
///   value).
/// - `1` if the result is an Either **right** (loop back with the new state).
/// - `2` if the result is a pair (single-step mode — terminate with `fst`).
///
/// The input pointer is NOT consumed (the caller uses it to extract the
/// carried value / new state / fst after the tag is checked).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_loop_check(ctx: *mut EvalCtx, result: *mut TaggedValue) -> u8 {
    unsafe {
        let ctx = &mut *ctx;
        if result.is_null() {
            ctx.set_error(LangError::engine("loop_check: null result".to_string()));
            return 2; // arbitrary — error is set
        }
        let v = &*result;
        // `is_either_*` also recognizes the `{"__either__": ...}` object
        // encoding produced by generated fold-machine `object` nodes
        // (ADR-0120 Phase 1).
        if is_either_left(v) {
            0
        } else if is_either_right(v) {
            1
        } else if matches!(v, TaggedValue::Pair(..)) {
            2
        } else {
            ctx.set_error(LangError::engine(
                "Loop: body did not return a pair or Either".to_string(),
            ));
            2
        }
    }
}

/// Resolve arrow-value params and body from the current node's payload.
///
/// Mirrors the v2 tree-walk's `closure` op: reads `params`/`body` from the
/// payload (rich shape), or falls back to the `children` array (Pratt-parsed
/// shape). Returns `(params_as_values, rest, body_arena_index, array_pattern)`.
fn resolve_closure_params_body(
    ctx: &mut EvalCtx,
) -> Result<(Vec<String>, Option<String>, usize, bool), LangError> {
    let node = ctx.current_node()?;
    let payload = &node.payload;
    if let (Some(p), Some(b)) = (
        payload.get("params").and_then(|v| v.as_array()),
        payload.get("body").and_then(|v| v.as_u64()),
    ) {
        let names: Vec<String> = p
            .iter()
            .map(|v| v.as_str().map(String::from).unwrap_or_default())
            .collect();
        let rest_name = payload
            .get("rest")
            .and_then(|v| v.as_str())
            .map(String::from);
        return Ok((names, rest_name, b as usize, false));
    }
    let children = payload
        .get("children")
        .and_then(|c| c.as_array())
        .ok_or_else(|| {
            LangError::engine_node("closure: missing params/body in payload".to_string(), node)
        })?;
    // `params`/`body` child positions resolve by named role (ADR-0081);
    // positional 0/1 is the role-less fallback.
    let param_idx = ctx
        .role_child_arena(node, "params")
        .ok()
        .flatten()
        .or_else(|| {
            children
                .first()
                .and_then(|v| v.as_u64())
                .map(|i| i as usize)
        })
        .ok_or_else(|| LangError::engine_node("closure: missing param child".to_string(), node))?;
    let body_idx = ctx
        .role_child_arena(node, "body")
        .ok()
        .flatten()
        .or_else(|| children.get(1).and_then(|v| v.as_u64()).map(|i| i as usize))
        .ok_or_else(|| LangError::engine_node("closure: missing body child".to_string(), node))?;
    let ast = unsafe { &*ctx.ast };
    let lang = unsafe { &*ctx.lang };
    let param_node = ast.at(AstRef::new(param_idx))?;
    // `param_names` resolves fixed params plus an optional rest param
    // (`...name`) data-driven via `pattern_role`/variadic metadata
    // (ADR-0081).
    let (param_names, rest_name) = lang
        .param_names(ast, param_node)
        .map_err(|e| LangError::engine_node(e.to_string(), node))?;
    let array_pattern = lang.is_array_pattern(param_node);
    Ok((param_names, rest_name, body_idx, array_pattern))
}

/// v2 `array` prim: construct an array value from the current node's payload.
///
/// Two payload shapes for `array`:
/// - Rich: `{"elements": [v1, v2, ...]}` — literal values, returned as-is.
/// - Pratt-parsed: `{"children": [elem_idx, ...]}` — each element is an arena
///   index evaluated via [`eval_child_node`]. A `rest`-roled child
///   (`pattern_role: rest`; from `...expr` syntax) is a spread element: its
///   evaluated value is flattened into the result when it is an array, and
///   pushed as-is otherwise (mirrors the tree-walk and bytecode prims).
fn prim_v2_array(ctx: &mut EvalCtx) -> *mut TaggedValue {
    let node = match ctx.current_node() {
        Ok(n) => n,
        Err(e) => return ctx.set_error(e),
    };
    if let Some(elements) = node.payload.get("elements") {
        return unsafe { box_value(TaggedValue::from(elements.clone())) };
    }
    let children = match node.payload.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => {
            return ctx.set_error(LangError::engine(
                "array: missing elements/children in payload".to_string(),
            ));
        }
    };
    // Collect indices first to release the immutable borrow of `ctx` before
    // the mutable `eval_child_node` calls.
    let indices: Vec<usize> = match children
        .iter()
        .map(|c| c.as_u64().map(|i| i as usize))
        .collect::<Option<Vec<_>>>()
    {
        Some(v) => v,
        None => {
            return ctx.set_error(LangError::engine(
                "array: element index must be a number".to_string(),
            ));
        }
    };
    let mut result = Vec::with_capacity(indices.len());
    for idx in indices {
        // Spread: if the child is a `rest`-roled node (`pattern_role: rest`;
        // from `...expr` syntax), flatten its value into the array if it's
        // an array. `kind == "rest"` is the role-less fallback (ADR-0081).
        // Mirrors the tree-walk and bytecode array prims.
        let is_rest = ctx
            .ast_at(AstRef::new(idx))
            .map(|n| {
                ctx.lang()
                    .and_then(|l| l.pattern_role_is(&n.kind, "rest"))
                    .unwrap_or_else(|| n.kind == "rest")
            })
            .unwrap_or(false);
        match eval_child_node(ctx, idx) {
            Ok(v) => {
                if is_rest {
                    if let TaggedValue::Arr(arr) = v {
                        result.extend(arr.iter().cloned());
                    } else {
                        result.push(v);
                    }
                } else {
                    result.push(v);
                }
            }
            Err(e) => return ctx.set_error(e),
        }
    }
    unsafe { box_value(TaggedValue::Arr(Rc::from(result))) }
}

/// v2 `object` prim: construct an object value from the current node's payload.
///
/// Two payload shapes for `object`:
/// - Rich: `{"entries": [{"key": "k", "value": v}, ...]}`.
/// - Pratt-parsed: `{"children": [entry_idx, ...]}` where each entry node has
///   `{"children": [key_idx, value_idx]}`.
fn prim_v2_object(ctx: &mut EvalCtx) -> *mut TaggedValue {
    let node = match ctx.current_node() {
        Ok(n) => n,
        Err(e) => return ctx.set_error(e),
    };
    if let Some(entries) = node.payload.get("entries").and_then(|v| v.as_array()) {
        let mut obj = FxHashMap::default();
        for entry in entries {
            let key = match entry.get("key").and_then(|v| v.as_str()) {
                Some(k) => k.to_string(),
                None => {
                    return ctx.set_error(LangError::engine_node(
                        "object: entry missing key".to_string(),
                        node,
                    ));
                }
            };
            let value = match entry.get("value") {
                Some(v) => v.clone(),
                None => {
                    return ctx.set_error(LangError::engine_node(
                        "object: entry missing value".to_string(),
                        node,
                    ));
                }
            };
            if handl_language::is_reserved_value_key(&key) {
                return ctx.set_error(LangError::engine(format!(
                    "object: key '{}' is reserved",
                    key
                )));
            }
            obj.insert(Rc::from(key.as_str()), TaggedValue::from(value));
        }
        return unsafe { box_value(TaggedValue::Obj(Rc::new(obj))) };
    }
    let children = match node.payload.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => {
            return ctx.set_error(LangError::engine(
                "object: missing entries/children in payload".to_string(),
            ));
        }
    };
    let lang = match unsafe { ctx.lang.as_ref() } {
        Some(l) => l,
        None => {
            return ctx.set_error(LangError::engine("object: no language".to_string()));
        }
    };
    let start = match lang.child_role_index(&node.kind, "entries") {
        Ok(Some(i)) => i,
        Ok(None) | Err(_) => 0,
    };
    let ast = unsafe { &*ctx.ast };
    // ADR-0074 v0.1: `{ expr }` (bare-expression block) vs `{ key: val }`
    // (object). When ALL children from the `entries` role are non-`entry`
    // nodes (bare expressions), treat as a block: evaluate each and return
    // the last value. When any child is an `entry` node, treat as an
    // object. `kind == "entry"` is the role-less fallback (ADR-0081).
    // Mirrors the v2 tree-walk's object prim.
    let all_bare = children.iter().skip(start).all(|c| {
        c.as_u64()
            .and_then(|idx| ast.at(AstRef::new(idx as usize)).ok())
            .map(|n| {
                !lang
                    .pattern_role_is(&n.kind, "entry")
                    .unwrap_or_else(|| n.kind == "entry")
            })
            .unwrap_or(true)
    });
    // Collect entry indices, then for each entry collect (key_idx, val_idx),
    // releasing borrows before evaluating children.
    let entry_indices: Vec<usize> = match children
        .iter()
        .skip(start)
        .map(|c| c.as_u64().map(|i| i as usize))
        .collect::<Option<Vec<_>>>()
    {
        Some(v) => v,
        None => {
            return ctx.set_error(LangError::engine(
                "object: entry index must be a number".to_string(),
            ));
        }
    };
    if all_bare && !children.is_empty() {
        // Block semantics: evaluate all, return last.
        let mut last = TaggedValue::Null;
        for idx in &entry_indices {
            last = match eval_child_node(ctx, *idx) {
                Ok(v) => v,
                Err(e) => return ctx.set_error(e),
            };
        }
        return unsafe { box_value(last) };
    }
    /// One collected `object` child: either a spread element (a non-`entry`
    /// node evaluated and merged into the object) or a regular `entry`
    /// node's (key, value, literal-key) resolution.
    enum ObjectChild {
        /// `...expr` spread — evaluate the child and merge its fields when
        /// the value is an object.
        Spread(usize),
        /// `key: value` entry — `direct_key` is `Some` when the key child is
        /// declared literal (its payload is the key text, not evaluated).
        Entry(usize, usize, Option<String>),
    }
    // Collect each child's resolution. Releasing borrows before evaluating
    // children below.
    let mut specs: Vec<ObjectChild> = Vec::with_capacity(entry_indices.len());
    for entry_idx in &entry_indices {
        let entry_node = match ast.at(AstRef::new(*entry_idx)) {
            Ok(n) => n,
            Err(e) => return ctx.set_error(e),
        };
        // Spread entry: if the child is not an `entry`-roled node, evaluate
        // it and merge its fields into the object. This implements
        // `{ ...s, field: val }` syntax. `kind == "entry"` is the role-less
        // fallback (ADR-0081). Mirrors the v2 tree-walk's object prim.
        let is_entry = lang
            .pattern_role_is(&entry_node.kind, "entry")
            .unwrap_or_else(|| entry_node.kind == "entry");
        if !is_entry {
            specs.push(ObjectChild::Spread(*entry_idx));
            continue;
        }
        let entry_children = match entry_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
        {
            Some(c) => c,
            None => {
                return ctx.set_error(LangError::engine(
                    "object: entry node missing children".to_string(),
                ));
            }
        };
        let key_role = match lang.child_role_index(&entry_node.kind, "key") {
            Ok(Some(i)) => i,
            Ok(None) | Err(_) => 0,
        };
        let val_role = match lang.child_role_index(&entry_node.kind, "value") {
            Ok(Some(i)) => i,
            Ok(None) | Err(_) => 1,
        };
        let key_idx = match entry_children.get(key_role).and_then(|v| v.as_u64()) {
            Some(i) => i as usize,
            None => {
                return ctx.set_error(LangError::engine(
                    "object: entry missing key child".to_string(),
                ));
            }
        };
        let val_idx = match entry_children.get(val_role).and_then(|v| v.as_u64()) {
            Some(i) => i as usize,
            None => {
                return ctx.set_error(LangError::engine(
                    "object: entry missing value child".to_string(),
                ));
            }
        };
        // Object key: a literal key child (the entry shape declares
        // `literal_children: { key: [...] }`, ADR-0081) uses its payload
        // string directly — don't evaluate it. Otherwise (computed key),
        // evaluate the key expression and use its string representation.
        // Bracket computed keys `[expr]:` are desugared to `paren { expr }`
        // before eval (ADR-0058), so they fall into the `None` (evaluate)
        // branch. Mirrors the v2 tree-walk's object prim. Role-less shapes
        // (or a null lang in the standalone driver) fall back to the
        // legacy `var`/`string` kind heuristic.  Cranelift does not yet
        // opt in to the `lower` pass; see ADR-0094 §"Follow-up: a `lower`
        // pass for codegen backends" for the parity note.
        let key_node = match ast.at(AstRef::new(key_idx)) {
            Ok(n) => n,
            Err(e) => return ctx.set_error(e),
        };
        let is_literal_key = lang
            .literal_child(&entry_node.kind, "key", &key_node.kind)
            .unwrap_or_else(|| matches!(key_node.kind.as_str(), "var" | "string"));
        let direct_key = if is_literal_key {
            key_node.payload.as_str().map(String::from)
        } else {
            None
        };
        specs.push(ObjectChild::Entry(key_idx, val_idx, direct_key));
    }
    let mut obj = FxHashMap::default();
    for spec in specs {
        let (key_idx, val_idx, direct_key) = match spec {
            // Spread element: evaluate the child and merge its fields into
            // the object when the value is an object; a non-object value is
            // skipped (mirrors the v2 tree-walk's object prim).
            ObjectChild::Spread(idx) => {
                let spread_val = match eval_child_node(ctx, idx) {
                    Ok(v) => v,
                    Err(e) => return ctx.set_error(e),
                };
                if let Some(spread_obj) = spread_val.as_object() {
                    for (k, v) in spread_obj {
                        obj.insert(Rc::clone(k), v.clone());
                    }
                }
                continue;
            }
            ObjectChild::Entry(k, v, d) => (k, v, d),
        };
        let key = match direct_key {
            Some(k) => k,
            None => {
                let key_val = match eval_child_node(ctx, key_idx) {
                    Ok(v) => v,
                    Err(e) => return ctx.set_error(e),
                };
                match &key_val {
                    TaggedValue::Str(s) => s.to_string(),
                    _ => handl_value::tagged_to_value(&key_val).to_string(),
                }
            }
        };
        let value = match eval_child_node(ctx, val_idx) {
            Ok(v) => v,
            Err(e) => return ctx.set_error(e),
        };
        if handl_language::is_reserved_value_key(&key) {
            return ctx.set_error(LangError::engine(format!(
                "object: key '{}' is reserved",
                key
            )));
        }
        obj.insert(Rc::from(key.as_str()), value);
    }
    unsafe { box_value(TaggedValue::Obj(Rc::new(obj))) }
}

/// The cranelift backend's [`NativeOpCtx`] — the engine-local globals the
/// shared op table needs: the scope stack, AST-node evaluation (the
/// interpreter fallback `eval_child_node`), and closure application. The
/// op *semantics* live in `plugin-engine-exec::native_ops`; this struct
/// only forwards services.
struct ClCtx<'c> {
    ctx: &'c mut EvalCtx,
}

impl NativeOpCtx for ClCtx<'_> {
    fn supports(&self, _service: OpService) -> bool {
        // Every declared service is available through the interpreter
        // fallback helpers.
        true
    }

    fn node_payload(&self) -> Option<&Value> {
        self.ctx.current_node().ok().map(|n| &n.payload)
    }

    fn node_payload_at(&mut self, r: AstRef) -> Option<Value> {
        self.ctx.ast_at(r).ok().map(|n| n.payload.clone())
    }

    fn child_role_index(&mut self, role: &str) -> Option<usize> {
        if self.ctx.lang.is_null() {
            return None;
        }
        let node = self.ctx.current_node().ok()?;
        // SAFETY: ctx.lang is a valid pointer for the duration of run_native.
        unsafe { (*self.ctx.lang).child_role_index(&node.kind, role) }
            .ok()
            .flatten()
    }

    fn child_role_index_for(&mut self, role: &str) -> Option<usize> {
        if self.ctx.lang.is_null() {
            return None;
        }
        let node = self.ctx.current_node().ok()?;
        // SAFETY: ctx.lang is a valid pointer for the duration of run_native.
        unsafe { (*self.ctx.lang).child_role_index_for(node, role) }
            .ok()
            .flatten()
    }

    fn role_declared_at(&mut self, r: AstRef, role: &str) -> bool {
        if self.ctx.lang.is_null() {
            return false;
        }
        let Some(node) = self.ctx.ast_at(r).ok() else {
            return false;
        };
        // SAFETY: ctx.lang is a valid pointer for the duration of run_native.
        unsafe { (*self.ctx.lang).child_role_index(&node.kind, role) }
            .ok()
            .flatten()
            .is_some()
    }

    fn child_role_index_at(&mut self, r: AstRef, role: &str) -> Option<usize> {
        if self.ctx.lang.is_null() {
            return None;
        }
        let node = self.ctx.ast_at(r).ok()?;
        // SAFETY: ctx.lang is a valid pointer for the duration of run_native.
        unsafe { (*self.ctx.lang).child_role_index_for(node, role) }
            .ok()
            .flatten()
    }

    fn eval_node(&mut self, r: AstRef) -> Result<TaggedValue, LangError> {
        eval_child_node(self.ctx, r.index())
    }

    fn env_lookup(&mut self, name: &str) -> Option<TaggedValue> {
        self.ctx.env.lookup(name).cloned()
    }

    fn env_define(&mut self, name: &str, value: TaggedValue) {
        self.ctx.env.define(name, value);
    }

    fn env_push(&mut self) {
        self.ctx.env.push_scope();
    }

    fn env_pop(&mut self) {
        self.ctx.env.pop_scope();
    }

    fn env_assign(&mut self, name: &str, value: TaggedValue) {
        self.ctx.env.assign(name, value);
    }

    fn env_take_enclosing_updated(&mut self) -> bool {
        std::mem::take(&mut self.ctx.env.enclosing_updated)
    }

    fn clear_eval_error(&mut self) {
        let _ = self.ctx.error.take();
    }

    fn env_capture(&mut self) -> Rc<FxHashMap<Rc<str>, TaggedValue>> {
        Rc::new(self.ctx.env.capture())
    }

    fn apply_arrow(
        &mut self,
        f: &TaggedValue,
        args: &[TaggedValue],
    ) -> Result<TaggedValue, LangError> {
        let ptr = apply_closure_value(self.ctx, f.clone(), args.to_vec());
        if ptr.is_null() {
            Err(self
                .ctx
                .error
                .take()
                .unwrap_or_else(|| LangError::engine("arrow application failed".to_string())))
        } else {
            // SAFETY: `ptr` was produced by `box_value` and is reclaimed
            // exactly once here.
            Ok(unsafe { take_value(ptr) })
        }
    }
}

/// Apply a v2 native op.
///
/// The op semantics are defined once in
/// `plugin-engine-exec::native_ops` (ADR-0044 §7): this backend supplies
/// only the globals — the `EvalCtx` scope stack, the AST-node interpreter
/// fallback, and closure application — through [`ClCtx`]. The residual
/// `match` below owns ops that still require this engine's machinery
/// (imports, module calls, `eval_block`, closures, FFI) and are migrated
/// to the shared table as their services are declared.
fn apply_native_v2(
    ctx: &mut EvalCtx,
    op: &str,
    args: &[TaggedValue],
    incoming: Option<TaggedValue>,
) -> *mut TaggedValue {
    {
        let mut cl = ClCtx { ctx: &mut *ctx };
        match native_ops::apply_native_op(op, args, None, incoming.clone(), &mut cl) {
            Ok(Some((v, _pushed))) => return unsafe { box_value(v) },
            Ok(None) => {}
            Err(e) => return ctx.set_error(e),
        }
    }
    let result = match op {
        "closure" => match resolve_closure_params_body(ctx) {
            Ok((params, rest, body, array_pattern)) => {
                let captured = ctx.env.capture();
                Ok(TaggedValue::Arrow {
                    params: Rc::from(params),
                    rest: rest.map(Rc::from),
                    body,
                    env: Rc::new(captured),
                    home: None,
                    array_pattern,
                    // ADR-0106 item 2 step 1: cranelift's body is an
                    // instruction index, not an AST node (step 3).
                    body_def: None,
                })
            }
            Err(e) => Err(e),
        },
        // `bind`, `assign`, `eval_block`, and `eval_rec` are owned by the
        // shared op table (native_ops) — EvalNode + Env gated; AST
        // children re-enter through `eval_child_node` via
        // `ClCtx::eval_node`.
        "loop_step" => return prim_v2_loop_step(ctx, args, incoming),
        "fold_step" => return prim_v2_fold_step(ctx, args, incoming),
        // Derived step ops (ADR-0052 §"Derived forms"). This backend's
        // interpretation is eager: compiled code re-evaluates the arg
        // sub-programs per iteration and passes the results in `args`;
        // the step *decision* is the shared [`step_decision`] semantics,
        // with `eval_arg` reading the pre-evaluated args.
        "while_step" | "for_step" | "fix_step" => {
            let inc = incoming.unwrap_or(TaggedValue::Null);
            let mut eval_arg =
                |i: usize, _current: &TaggedValue| -> Result<TaggedValue, LangError> {
                    args.get(i)
                        .cloned()
                        .ok_or_else(|| LangError::engine(format!("{op}: missing arg {i}")))
                };
            step_decision(op, &inc, &mut eval_arg)
        }
        "array" => {
            return prim_v2_array(ctx);
        }
        "object" => {
            return prim_v2_object(ctx);
        }
        // Phase 9.5: `import "path"` loads a module via the configured
        // resolver and binds its top-level exports into the current scope.
        "import" => return prim_v2_import(ctx, args),
        // ADR-0074 follow-up #3: `>>>` (`flow`) — function application with
        // a single input argument (mirrors the v2 tree-walk's `flow` arm).
        "flow" => return prim_v2_flow(ctx, args),
        "call_host" => {
            // Mirror the tree-walk/bytecode convention: the string callee
            // arrives as `incoming` (e.g. ADR-0060 `call` scheme after
            // `route_on_type`), while `args[0]` is the `proj_rest` arg array.
            // Legacy callers put the callee at `args[0]` and the arg array at
            // `args[1]`.
            let (callee, arg_src) = if let Some(TaggedValue::Str(s)) = incoming.as_ref() {
                (s.to_string(), args.first())
            } else if let Some(TaggedValue::Str(s)) = args.first() {
                (s.to_string(), args.get(1))
            } else {
                return ctx.set_error(LangError::engine(
                    "call_host: callee must be a string".to_string(),
                ));
            };
            let modules_ptr = match ctx.modules {
                Some(p) => p,
                None => {
                    return ctx.set_error(LangError::engine(
                        "call_host: no module registry".to_string(),
                    ));
                }
            };
            let modules: &dyn handl_platform::ModuleLookup = unsafe { &*modules_ptr };
            let func = match modules.lookup(&callee) {
                Some(f) => f,
                None => {
                    return ctx.set_error(LangError::engine(format!(
                        "call_host: function '{}' not found",
                        callee
                    )));
                }
            };
            let call_args: Vec<Value> = match arg_src {
                Some(TaggedValue::Arr(arr)) => arr
                    .iter()
                    .map(|v| handl_value::tagged_to_value(v))
                    .collect(),
                _ => Vec::new(),
            };
            let mut pctx = handl_platform::EvalContext::new();
            func.call(&call_args, &mut pctx)
                .map(TaggedValue::from)
                .map_err(|e| LangError::engine(e.to_string()))
        }
        "match" => return prim_match(ctx),
        "handle" => return prim_handle(ctx),
        // `export` and `record_proj` are owned by the shared table.
        // `paren` — parentheses/grouping: evaluate children from the current
        // node's payload, falling back to the supplied args/incoming.
        _ => Err(LangError::engine(format!("unknown v2 native op: {}", op))),
    };

    match result {
        Ok(v) => unsafe { box_value(v) },
        Err(e) => ctx.set_error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstNode, EvalSchemeDef, Term};
    use handl_platform::{EvalContext, Function, Module, ModuleLookup};
    use handl_plugin_engine_exec::EvalRequest;
    use handl_plugin_module::ModuleRegistry;
    use plugin_engine::{ArrSource, TermData};
    use serde_json::json;

    /// Build a simple AST for testing.
    fn make_ast(nodes: Vec<AstNode>) -> Ast {
        let mut ast = Ast::new();
        for n in nodes {
            ast.push(n);
        }
        ast
    }

    /// Construct a pair value `{"__pair__": [a, b]}`.
    fn pair(a: Value, b: Value) -> Value {
        json!({"__pair__": [a, b]})
    }

    // --- TermData thin wrappers (ADR-0103 Phase 6 migration) ---
    // These mirror the Term convenience constructors but produce TermData,
    // which is converted to Term at the call site via `td(...)`.

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

    /// Foreign op (mirrors `Term::arr_foreign`).
    fn op_foreign(name: &str, args: Vec<TermData>) -> TermData {
        TermData::op(name, args, &ArrSource::Foreign(name.to_string()), None)
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

    /// Loop feedback (mirrors `Term::loop_feedback`).
    fn loop_feedback(body: TermData) -> TermData {
        TermData::loop_feedback(body)
    }

    /// A host function that doubles its integer argument.
    struct DoubleHostFn;

    impl Function for DoubleHostFn {
        fn call(
            &self,
            args: &[Value],
            _ctx: &mut EvalContext,
        ) -> Result<Value, handl_platform::EvalError> {
            let n = args[0].as_i64().unwrap();
            Ok(Value::from(n * 2))
        }
    }

    /// A module with "double" registered.
    struct ConformanceModule;

    impl Module for ConformanceModule {
        fn name(&self) -> &str {
            "conformance"
        }
        fn function(&self, name: &str) -> Option<&dyn Function> {
            if name == "double" {
                Some(&DoubleHostFn)
            } else {
                None
            }
        }
    }

    /// Build a module registry with the `double` function registered.
    fn conformance_registry() -> Rc<dyn ModuleLookup> {
        Rc::new(ModuleRegistry::new().with(ConformanceModule))
    }

    // --- Engine-level tests (end-to-end native execution) ---

    #[test]
    fn engine_empty_ast_returns_null() {
        let engine = CraneliftEngine::new();
        let lang = Language::new();
        let req = EvalRequest::new(Ast::new());
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn engine_no_scheme_returns_payload() {
        let engine = CraneliftEngine::new();
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(42))]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn engine_lit_scheme() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.const",
                "const",
                td(op_lit("const", vec![], json!(99))),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("const")]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(99));
    }

    #[test]
    fn engine_lit_null_scheme() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.const",
                "const",
                td(op_lit("const", vec![], Value::Null)),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("const")]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), Value::Null);
    }

    #[test]
    fn engine_var_named() {
        let engine = CraneliftEngine::new();
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
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("extend_env", vec![], Value::String("x".into())),
                    ),
                    op_lit("proj_child", vec![], Value::from(1u64)),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(42)),
            AstNode::new("var"),
            AstNode::new("let").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }

    #[test]
    fn engine_var_payload() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                td(op_lit("proj_env", vec![], Value::String("$payload".into()))),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                td(seq(
                    seq(
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("extend_env", vec![], Value::String("$payload".into())),
                    ),
                    op_lit("proj_child", vec![], Value::from(1u64)),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(42)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }

    #[test]
    fn engine_var_unknown_errors() {
        // In v2, an unbound variable returns its name as a string value
        // (matching the v2 tree-walk convention), not an error.
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                td(op_lit("proj_env", vec![], Value::String("unknown".into()))),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("var")]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!("unknown"));
    }

    #[test]
    fn engine_child_recurses() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
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
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(5));
    }

    #[test]
    fn engine_bind_extends_scope() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                td(op_lit("proj_env", vec![], Value::String("$payload".into()))),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                td(seq(
                    seq(
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("extend_env", vec![], Value::String("$payload".into())),
                    ),
                    op_lit("proj_child", vec![], Value::from(1u64)),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(10)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(10));
    }

    #[test]
    fn engine_bind_scope_does_not_leak() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                td(op_lit("proj_env", vec![], Value::String("$payload".into()))),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                td(seq(
                    seq(
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("extend_env", vec![], Value::String("$payload".into())),
                    ),
                    op_lit("proj_child", vec![], Value::from(1u64)),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(10)),
            AstNode::new("lit").with_payload(json!(20)),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(20));
    }

    #[test]
    fn engine_bind_node_kind() {
        // `x = 5 in x + 1` → bind { name: "x", children: [lit(5), add(x, 1)] }
        // The bind node's scheme is Arr { op: "bind", source: Native }.
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                td(op_lit("proj_env", vec![], Value::String("$payload".into()))),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
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
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.bind",
                "bind",
                td(op_native("bind", vec![])),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(5)),   // 0
            AstNode::new("lit").with_payload(json!(1)),   // 1
            AstNode::new("var").with_payload(json!("x")), // 2
            AstNode::new("binary.add").with_payload(json!({"children": [2, 1]})), // 3
            AstNode::new("bind").with_payload(json!({"name": "x", "children": [0, 3]})), // 4
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(6));
    }

    #[test]
    fn engine_branch_true() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                td(seq(
                    seq(
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_native("tag", vec![]),
                    ),
                    choice(
                        op_lit("proj_child", vec![], Value::from(1u64)),
                        op_lit("proj_child", vec![], Value::from(2u64)),
                    ),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(1));
    }

    #[test]
    fn engine_branch_false() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                td(seq(
                    seq(
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_native("tag", vec![]),
                    ),
                    choice(
                        op_lit("proj_child", vec![], Value::from(1u64)),
                        op_lit("proj_child", vec![], Value::from(2u64)),
                    ),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(false)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(2));
    }

    #[test]
    fn engine_branch_falsy_values() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                td(seq(
                    seq(
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_native("tag", vec![]),
                    ),
                    choice(
                        op_lit("proj_child", vec![], Value::from(1u64)),
                        op_lit("proj_child", vec![], Value::from(2u64)),
                    ),
                )),
            ))
            .unwrap();
        // Test with 0 (falsy)
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(0)),
            AstNode::new("lit").with_payload(json!("then")),
            AstNode::new("lit").with_payload(json!("else")),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!("else"));

        // Test with "" (falsy)
        let ast2 = make_ast(vec![
            AstNode::new("lit").with_payload(json!("")),
            AstNode::new("lit").with_payload(json!("then")),
            AstNode::new("lit").with_payload(json!("else")),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req2 = EvalRequest::new(ast2);
        assert_eq!(engine.eval(&lang, &req2).unwrap(), json!("else"));

        // Test with null (falsy)
        let ast3 = make_ast(vec![
            AstNode::new("lit").with_payload(Value::Null),
            AstNode::new("lit").with_payload(json!("then")),
            AstNode::new("lit").with_payload(json!("else")),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req3 = EvalRequest::new(ast3);
        assert_eq!(engine.eval(&lang, &req3).unwrap(), json!("else"));
    }

    #[test]
    fn engine_prim_add() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
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
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("lit").with_payload(json!(4)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(7));
    }

    #[test]
    fn engine_prim_sub() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.sub",
                "binary.sub",
                td(op_native(
                    "sub",
                    vec![
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("proj_child", vec![], Value::from(1u64)),
                    ],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(10)),
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("binary.sub").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(7));
    }

    #[test]
    fn engine_prim_mul() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.mul",
                "binary.mul",
                td(op_native(
                    "mul",
                    vec![
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("proj_child", vec![], Value::from(1u64)),
                    ],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(6)),
            AstNode::new("lit").with_payload(json!(7)),
            AstNode::new("binary.mul").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }

    #[test]
    fn engine_prim_div() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.div",
                "binary.div",
                td(op_native(
                    "div",
                    vec![
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("proj_child", vec![], Value::from(1u64)),
                    ],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(20)),
            AstNode::new("lit").with_payload(json!(4)),
            AstNode::new("binary.div").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(5));
    }

    #[test]
    fn engine_prim_div_by_zero_errors() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.div",
                "binary.div",
                td(op_native(
                    "div",
                    vec![
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("proj_child", vec![], Value::from(1u64)),
                    ],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(0)),
            AstNode::new("binary.div").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn engine_prim_eq() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.eq",
                "binary.eq",
                td(op_native(
                    "eq",
                    vec![
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("proj_child", vec![], Value::from(1u64)),
                    ],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("binary.eq").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(true));
    }

    #[test]
    fn engine_prim_lt_gt() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.lt",
                "binary.lt",
                td(op_native(
                    "lt",
                    vec![
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("proj_child", vec![], Value::from(1u64)),
                    ],
                )),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.gt",
                "binary.gt",
                td(op_native(
                    "gt",
                    vec![
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("proj_child", vec![], Value::from(1u64)),
                    ],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("binary.lt").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(true));

        let ast2 = make_ast(vec![
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("binary.gt").with_payload(json!({"children": [0, 1]})),
        ]);
        let req2 = EvalRequest::new(ast2);
        assert_eq!(engine.eval(&lang, &req2).unwrap(), json!(false));
    }

    #[test]
    fn engine_prim_not_and_or() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.not",
                "unary.not",
                td(op_native(
                    "not",
                    vec![op_lit("proj_child", vec![], Value::from(0u64))],
                )),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.and",
                "binary.and",
                td(op_native(
                    "and",
                    vec![
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("proj_child", vec![], Value::from(1u64)),
                    ],
                )),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.or",
                "binary.or",
                td(op_native(
                    "or",
                    vec![
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("proj_child", vec![], Value::from(1u64)),
                    ],
                )),
            ))
            .unwrap();

        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("unary.not").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(false));

        let ast2 = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("lit").with_payload(json!(false)),
            AstNode::new("binary.and").with_payload(json!({"children": [0, 1]})),
        ]);
        let req2 = EvalRequest::new(ast2);
        assert_eq!(engine.eval(&lang, &req2).unwrap(), json!(false));

        let ast3 = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("lit").with_payload(json!(false)),
            AstNode::new("binary.or").with_payload(json!({"children": [0, 1]})),
        ]);
        let req3 = EvalRequest::new(ast3);
        assert_eq!(engine.eval(&lang, &req3).unwrap(), json!(true));
    }

    #[test]
    fn engine_prim_unknown_errors() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.weird",
                "weird",
                td(op_native("unknown_op", vec![])),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("weird")]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unknown v2 native op"));
    }

    #[test]
    fn engine_foreign_dispatches_to_registered_host() {
        let engine = CraneliftEngine::with_modules(conformance_registry());
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.dbl",
                "dbl",
                td(op_foreign(
                    "double",
                    vec![op_lit("proj_child", vec![], Value::from(0u64))],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(21)),
            AstNode::new("dbl").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }

    #[test]
    fn engine_foreign_unregistered_errors() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.dbl",
                "dbl",
                td(op_foreign(
                    "double",
                    vec![op_lit("proj_child", vec![], Value::from(0u64))],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(21)),
            AstNode::new("dbl").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unregistered foreign"));
    }

    #[test]
    fn engine_app_calls_closure() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                td(op_native("closure", vec![])),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                td(op_lit("proj_env", vec![], Value::String("$payload".into()))),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
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
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                td(app(
                    op_lit("proj_child", vec![], Value::from(0u64)),
                    vec![op_lit("proj_child", vec![], Value::from(1u64))],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
            AstNode::new("function").with_payload(json!({"params": ["x"], "body": 2})),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("call").with_payload(json!({"children": [3, 4]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(6));
    }

    #[test]
    fn engine_closure_captures_env() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                td(op_lit("proj_env", vec![], Value::String("$payload".into()))),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                td(seq(
                    seq(
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("extend_env", vec![], Value::String("$payload".into())),
                    ),
                    op_lit("proj_child", vec![], Value::from(1u64)),
                )),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
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
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                td(op_native("closure", vec![])),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                td(app(
                    op_lit("proj_child", vec![], Value::from(0u64)),
                    vec![op_lit("proj_child", vec![], Value::from(1u64))],
                )),
            ))
            .unwrap();
        // let x = 10 in let f = fn(y) { x + y } in f(5) → 15
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(10)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("var").with_payload(json!("y")),
            AstNode::new("binary.add").with_payload(json!({"children": [1, 2]})),
            AstNode::new("function").with_payload(json!({"params": ["y"], "body": 3})),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("call").with_payload(json!({"children": [4, 5]})),
            AstNode::new("let").with_payload(json!({"name": "f", "children": [4, 6]})),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 7]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(15));
    }

    #[test]
    fn engine_nested_let_add() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                td(op_lit("proj_env", vec![], Value::String("$payload".into()))),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                td(seq(
                    seq(
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("extend_env", vec![], Value::String("$payload".into())),
                    ),
                    op_lit("proj_child", vec![], Value::from(1u64)),
                )),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
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
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("lit").with_payload(json!(4)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("var").with_payload(json!("y")),
            AstNode::new("binary.add").with_payload(json!({"children": [2, 3]})),
            AstNode::new("let").with_payload(json!({"name": "y", "children": [1, 4]})),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 5]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(7));
    }

    #[test]
    fn engine_version_mismatch_errors() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.scheme_version = SchemeVersion::new("v3");
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(42))]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unsupported scheme version"));
    }

    #[test]
    fn engine_default_constructor() {
        let engine = CraneliftEngine::default();
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(0))]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(0));
    }

    #[test]
    fn engine_trait_dispatch_via_dyn() {
        let engine: Box<dyn EvalEngine> = Box::new(CraneliftEngine::new());
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(99))]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(99));
    }

    // --- Compilation tests (verify every combinator compiles) ---

    #[test]
    fn compile_lit_succeeds() {
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
    fn compile_var_succeeds() {
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
    fn compile_var_payload_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.v",
                "v",
                td(op_lit("proj_env", vec![], Value::String("$payload".into()))),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("v").with_payload(json!("x"))]);
        let result = compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn compile_child_succeeds() {
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
    fn compile_bind_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                td(op_lit("proj_env", vec![], Value::String("$payload".into()))),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                td(seq(
                    seq(
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("extend_env", vec![], Value::String("$payload".into())),
                    ),
                    op_lit("proj_child", vec![], Value::from(1u64)),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
        ]);
        let result = compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn compile_app_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                td(op_native("closure", vec![])),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                td(app(
                    op_lit("proj_child", vec![], Value::from(0u64)),
                    vec![op_lit("proj_child", vec![], Value::from(1u64))],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("function").with_payload(json!({"params": ["x"], "body": 0})),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("call").with_payload(json!({"children": [0, 1]})),
        ]);
        let result = compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn compile_branch_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                td(seq(
                    seq(
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_native("tag", vec![]),
                    ),
                    choice(
                        op_lit("proj_child", vec![], Value::from(1u64)),
                        op_lit("proj_child", vec![], Value::from(2u64)),
                    ),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let result = compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn compile_prim_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
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
    fn compile_foreign_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.dbl",
                "dbl",
                td(op_foreign(
                    "double",
                    vec![op_lit("proj_child", vec![], Value::from(0u64))],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(21)),
            AstNode::new("dbl").with_payload(json!({"children": [0]})),
        ]);
        let result = compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn compile_version_mismatch_errors() {
        let mut lang = Language::new();
        lang.scheme_version = SchemeVersion::new("v3");
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(42))]);
        match compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new()) {
            Ok(_) => panic!("expected version mismatch error, got Ok"),
            Err(e) => {
                let msg = format!("{}", e);
                assert!(msg.contains("unsupported scheme version"));
            }
        }
    }

    // ===================================================================
    // V2 engine tests (CraneliftEngineV2)
    // ===================================================================

    // 1. arr const

    #[test]
    fn v2_arr_const_returns_literal() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.c",
                "c",
                td(op_lit("const", vec![], json!(42))),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("c")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }

    #[test]
    fn v2_arr_const_null_when_no_literal() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.c",
                "c",
                td(op_lit("const", vec![], Value::Null)),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("c")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), Value::Null);
    }

    // 2. arr proj_env

    #[test]
    fn v2_arr_proj_env_bound_lookup() {
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
                        op_lit("const", vec![], json!(7)),
                        op_lit("extend_env", vec![], Value::String("x".into())),
                    ),
                    op_lit("proj_env", vec![], Value::String("x".into())),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("let")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(7));
    }

    #[test]
    fn v2_arr_proj_env_unbound_returns_name() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.v",
                "v",
                td(op_lit("proj_env", vec![], Value::String("unknown".into()))),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("v")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!("unknown"));
    }

    // 3. Seq const >>> id

    #[test]
    fn v2_seq_const_then_id() {
        // let x = 5 in id(x) — lowered to Seq(const 5, id)
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
                        op_lit("const", vec![], json!(5)),
                        op_lit("extend_env", vec![], Value::String("x".into())),
                    ),
                    op_lit("proj_env", vec![], Value::String("x".into())),
                )),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.id",
                "id",
                td(op_native(
                    "id",
                    vec![op_lit("proj_child", vec![], Value::from(0u64))],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
        ]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(5));
    }

    // 4. Bind shape (extend_env + proj_env)

    #[test]
    fn v2_bind_shape_extend_env_then_proj_env() {
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
                        op_lit("const", vec![], json!(5)),
                        op_lit("extend_env", vec![], Value::String("x".into())),
                    ),
                    op_lit("proj_env", vec![], Value::String("x".into())),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("let")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(5));
    }

    // 5. Branch (Choice + tag)

    #[test]
    fn v2_branch_true() {
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
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(1));
    }

    #[test]
    fn v2_branch_false() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                td(seq(
                    seq(
                        op_lit("const", vec![], json!(false)),
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
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(2));
    }

    // 6. Arithmetic parity

    #[test]
    fn v2_native_add() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
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
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(5));
    }

    #[test]
    fn v2_native_eq() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.eq",
                "binary.eq",
                td(op_native(
                    "eq",
                    vec![
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_lit("proj_child", vec![], Value::from(1u64)),
                    ],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("binary.eq").with_payload(json!({"children": [0, 1]})),
        ]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(true));
    }

    // 7. Child recursion (proj_child)

    #[test]
    fn v2_child_recurses() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
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
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(5));
    }

    // 8. No-scheme fallback

    #[test]
    fn v2_no_scheme_returns_payload() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("custom").with_payload(json!("hello"))]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!("hello"));
    }

    // 9. Version checks

    #[test]
    fn v2_engine_accepts_v2_lang() {
        let mut lang = Language::new();
        lang.scheme_version = SchemeVersion::v2();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.c",
                "c",
                td(op_lit("const", vec![], json!(42))),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("c")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }

    #[test]
    fn v2_engine_rejects_v3_lang() {
        let mut lang = Language::new();
        lang.scheme_version = SchemeVersion::new("v3");
        let ast = make_ast(vec![AstNode::new("c")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    // 10. Foreign dispatch

    #[test]
    fn v2_foreign_arr_dispatches_to_module() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.double",
                "double",
                td(op_foreign(
                    "double",
                    vec![op_lit("proj_child", vec![], Value::from(0u64))],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(21)),
            AstNode::new("double").with_payload(json!({"children": [0]})),
        ]);
        let reg = conformance_registry();
        let engine = CraneliftEngineV2::with_modules(reg);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }

    // 11. App with a closure value

    #[test]
    fn v2_app_closure_applies_body() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                td(op_native("closure", vec![])),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                td(op_lit("proj_env", vec![], Value::String("$payload".into()))),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
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
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                td(app(
                    op_lit("proj_child", vec![], Value::from(0u64)),
                    vec![op_lit("proj_child", vec![], Value::from(1u64))],
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
            AstNode::new("function").with_payload(json!({"params": ["x"], "body": 2})),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("call").with_payload(json!({"children": [3, 4]})),
        ]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(6));
    }

    // 11b. `loop_step` derived op — the ergonomic `loop` surface lowering
    // (ADR-0052). Builds the canonical `loop` lowering
    // `Seq(proj_child(0), Loop(loop_step(proj_child(1))))` by hand with a
    // real arrow body, exercising param destructuring, `if`, arithmetic,
    // and object construction under native compilation.

    /// Wire the eval schemes needed for the `loop` surface.
    fn loop_lang() -> Language {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                td(op_lit("proj_env", vec![], Value::String("$payload".into()))),
            ))
            .unwrap();
        for (id, kind, op) in [
            ("e.eq", "binary.eq", "eq"),
            ("e.sub", "binary.sub", "sub"),
            ("e.mul", "binary.mul", "mul"),
        ] {
            lang.eval
                .merge(EvalSchemeDef::new(
                    id,
                    kind,
                    td(op_native(
                        op,
                        vec![
                            op_lit("proj_child", vec![], Value::from(0u64)),
                            op_lit("proj_child", vec![], Value::from(1u64)),
                        ],
                    )),
                ))
                .unwrap();
        }
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                td(seq(
                    seq(
                        op_lit("proj_child", vec![], Value::from(0u64)),
                        op_native("tag", vec![]),
                    ),
                    choice(
                        op_lit("proj_child", vec![], Value::from(1u64)),
                        op_lit("proj_child", vec![], Value::from(2u64)),
                    ),
                )),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.object",
                "object",
                td(op_native("object", vec![])),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.function",
                "function",
                td(op_native("closure", vec![])),
            ))
            .unwrap();
        // return -> proj_child(0) (v0.1: `return v` evaluates to `v`)
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.return",
                "return",
                td(op_lit("proj_child", vec![], Value::from(0u64))),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.loop",
                "loop",
                td(seq(
                    op_lit("proj_child", vec![], Value::from(0u64)),
                    loop_feedback(op_native(
                        "loop_step",
                        vec![op_lit("proj_child", vec![], Value::from(1u64))],
                    )),
                )),
            ))
            .unwrap();
        lang
    }

    /// `loop 42 (x ~> return x)` — single-step terminate, scalar
    /// state, one param. Expected: 42.
    #[test]
    fn v2_loop_step_single_step_terminate() {
        let lang = loop_lang();
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("x")), // 0
            AstNode::new("return").with_payload(json!({"children": [0]})), // 1
            AstNode::new("function").with_payload(json!({"params": ["x"], "body": 1})), // 2
            AstNode::new("lit").with_payload(json!(42)),  // 3
            AstNode::new("loop").with_payload(json!({"children": [3, 2]})), // 4
        ]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }

    /// `loop { n: 3, acc: 1 }([n, acc] ~> if n == 0 then return acc
    /// else { state: { n: n - 1, acc: acc * n } })` — multi-step with param
    /// destructuring, `if`, arithmetic, nested object construction.
    /// Computes 3! = 6.
    #[test]
    fn v2_loop_step_factorial_destructured() {
        let lang = loop_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!("n")),     // 0
            AstNode::new("lit").with_payload(json!("acc")),   // 1
            AstNode::new("lit").with_payload(json!("state")), // 2
            AstNode::new("lit").with_payload(json!(0)),       // 3
            AstNode::new("lit").with_payload(json!(1)),       // 4
            AstNode::new("var").with_payload(json!("n")),     // 5
            AstNode::new("var").with_payload(json!("acc")),   // 6
            AstNode::new("binary.eq").with_payload(json!({"children": [5, 3]})), // 7
            AstNode::new("binary.sub").with_payload(json!({"children": [5, 4]})), // 8
            AstNode::new("binary.mul").with_payload(json!({"children": [6, 5]})), // 9
            AstNode::new("entry").with_payload(json!({"children": [0, 8]})), // 10
            AstNode::new("entry").with_payload(json!({"children": [1, 9]})), // 11
            AstNode::new("object").with_payload(json!({"children": [10, 11]})), // 12
            AstNode::new("var").with_payload(json!("acc")),   // 13
            AstNode::new("return").with_payload(json!({"children": [13]})), // 14
            AstNode::new("entry").with_payload(json!({"children": [2, 12]})), // 15
            AstNode::new("object").with_payload(json!({"children": [15]})), // 16
            AstNode::new("if").with_payload(json!({"children": [7, 14, 16]})), // 17
            AstNode::new("function").with_payload(json!({"params": ["n", "acc"], "body": 17})), // 18
            AstNode::new("lit").with_payload(json!({"n": 3, "acc": 1})), // 19
            AstNode::new("loop").with_payload(json!({"children": [19, 18]})), // 20
        ]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(6));
    }

    // 11c. `while_step`/`for_step`/`fix_step` derived step ops.
    // These carry cond/body sub-schemes that must be re-evaluated per
    // iteration with the current value (extracted from the loop pair) as
    // incoming. The compiler calls `helper_v2_derived_step_arg_incoming`
    // to extract the right incoming value, then compiles the args inline
    // (re-executed each iteration via the loop back-edge).

    /// `while is_pos dec` applied to 5 → countdown to 0.
    #[test]
    fn v2_while_step_countdown() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.while_test",
                "while_test",
                td(seq(
                    op_lit("const", vec![], json!(5)),
                    loop_feedback(op_native(
                        "while_step",
                        vec![op_native("is_pos", vec![]), op_native("dec", vec![])],
                    )),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("while_test")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(0));
    }

    /// `while is_pos dec` applied to 0 → zero iterations.
    #[test]
    fn v2_while_step_zero_iterations() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.while_test",
                "while_test",
                td(seq(
                    op_lit("const", vec![], json!(0)),
                    loop_feedback(op_native(
                        "while_step",
                        vec![op_native("is_pos", vec![]), op_native("dec", vec![])],
                    )),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("while_test")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(0));
    }

    /// `for inc` applied to pair(5, 0) → accumulate 5 increments → 5.
    #[test]
    fn v2_for_step_accumulate() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.for_test",
                "for_test",
                td(seq(
                    op_lit("const", vec![], pair(json!(5), json!(0))),
                    loop_feedback(op_native("for_step", vec![op_native("inc", vec![])])),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("for_test")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(5));
    }

    /// `for inc` applied to pair(0, 99) → zero iterations → 99.
    #[test]
    fn v2_for_step_zero_iterations() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.for_test",
                "for_test",
                td(seq(
                    op_lit("const", vec![], pair(json!(0), json!(99))),
                    loop_feedback(op_native("for_step", vec![op_native("inc", vec![])])),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("for_test")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(99));
    }

    /// `fix half_floor` applied to 100 → halve to 0.
    #[test]
    fn v2_fix_step_halve_to_zero() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fix_test",
                "fix_test",
                td(seq(
                    op_lit("const", vec![], json!(100)),
                    loop_feedback(op_native("fix_step", vec![op_native("half_floor", vec![])])),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("fix_test")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(0));
    }

    /// `fix half_floor` applied to 0 → already at fixpoint.
    #[test]
    fn v2_fix_step_already_at_fixpoint() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fix_test",
                "fix_test",
                td(seq(
                    op_lit("const", vec![], json!(0)),
                    loop_feedback(op_native("fix_step", vec![op_native("half_floor", vec![])])),
                )),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("fix_test")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(0));
    }

    // 12. Empty AST

    #[test]
    fn v2_empty_ast_returns_null() {
        let lang = Language::new();
        let ast = Ast::new();
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), Value::Null);
    }

    // 13. Run pre-compiled (compile + run_compiled separation)

    #[test]
    fn v2_run_compiled_matches_eval() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
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
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let compile_result =
            compile_all_nodes_jit(&lang, &ast, &std::collections::HashMap::new()).unwrap();
        let engine = CraneliftEngineV2::new();
        let result = engine.run_compiled(&lang, &ast, &compile_result).unwrap();
        assert_eq!(result, json!(5));
    }

    // 14. Object compilation (v2)

    #[test]
    fn v2_object_compilation_emits_file() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
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
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let tmp = std::env::temp_dir().join("handl_v2_test.o");
        let mode = CompileMode::Object {
            output_path: tmp.clone(),
        };
        let result =
            compile_all_nodes(&lang, &ast, &mode, &std::collections::HashMap::new()).unwrap();
        assert!(tmp.exists());
        assert!(
            std::fs::metadata(&tmp)
                .map(|m| m.len() > 0)
                .unwrap_or(false)
        );
        // compiled map is empty for Object mode.
        assert!(result.compiled.is_empty());
        let _ = std::fs::remove_file(&tmp);
    }

    // 15. Three-way conformance: v2 cranelift vs v2 tree-walk vs v2 bytecode

    #[test]
    fn v2_three_way_results_match() {
        let cases: Vec<(&str, Language, Ast)> = vec![
            // lit
            {
                let mut lang = Language::new();
                lang.eval
                    .merge(EvalSchemeDef::new(
                        "e.c",
                        "c",
                        td(op_lit("const", vec![], json!(42))),
                    ))
                    .unwrap();
                ("lit", lang, make_ast(vec![AstNode::new("c")]))
            },
            // add
            {
                let mut lang = Language::new();
                lang.eval
                    .merge(EvalSchemeDef::new(
                        "e.add",
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
                (
                    "add",
                    lang,
                    make_ast(vec![
                        AstNode::new("lit").with_payload(json!(2)),
                        AstNode::new("lit").with_payload(json!(3)),
                        AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
                    ]),
                )
            },
            // branch true
            {
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
                ("branch_true", lang, make_ast(vec![AstNode::new("if")]))
            },
            // branch false
            {
                let mut lang = Language::new();
                lang.eval
                    .merge(EvalSchemeDef::new(
                        "e.if",
                        "if",
                        td(seq(
                            seq(
                                op_lit("const", vec![], json!(false)),
                                op_native("tag", vec![]),
                            ),
                            choice(
                                op_lit("const", vec![], json!(1)),
                                op_lit("const", vec![], json!(2)),
                            ),
                        )),
                    ))
                    .unwrap();
                ("branch_false", lang, make_ast(vec![AstNode::new("if")]))
            },
            // bind
            {
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
                                op_lit("const", vec![], json!(10)),
                                op_lit("extend_env", vec![], Value::String("x".into())),
                            ),
                            op_lit("proj_env", vec![], Value::String("x".into())),
                        )),
                    ))
                    .unwrap();
                ("bind", lang, make_ast(vec![AstNode::new("let")]))
            },
        ];

        let tw = handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new();
        let bc = handl_plugin_engine_exec_bytecode::BytecodeEngineV2::new();
        let cl = CraneliftEngineV2::new();

        for (name, lang, ast) in cases {
            let req = EvalRequest::new(ast.clone());
            let tw_result = tw.eval(&lang, &req).unwrap();
            let bc_result = bc.eval(&lang, &req).unwrap();
            let cl_result = cl.eval(&lang, &req).unwrap();
            assert_eq!(
                tw_result, cl_result,
                "case {}: tree-walk != cranelift (tw={}, cl={})",
                name, tw_result, cl_result
            );
            assert_eq!(
                bc_result, cl_result,
                "case {}: bytecode != cranelift (bc={}, cl={})",
                name, bc_result, cl_result
            );
        }
    }

    // -----------------------------------------------------------------------
    // ADR-0083 §4: FFI direct C ABI dispatch fast path tests
    // -----------------------------------------------------------------------

    /// A test C function for FFI dispatch (loaded via Library::this()).
    #[unsafe(no_mangle)]
    pub extern "C" fn ffi_cranelift_test_double_i32(x: i32) -> i32 {
        x * 2
    }

    /// Test that the cranelift engine can dispatch a Foreign op via the
    /// FFI fast path (helper_v2_call_ffi_raw → raw C ABI call).
    ///
    /// This registers an FFI binding for a Foreign op name, compiles and
    /// evaluates a Term that calls that Foreign op, and verifies the
    /// result comes from the raw C function.
    #[test]
    fn ffi_fast_path_dispatches_to_raw_c_symbol() {
        use handl_plugin_ffi::FfiOpFunction;
        use std::collections::HashMap;

        // Build a language with a single node kind "test" whose eval scheme
        // calls the foreign op "ffi.double" with a literal argument.
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.ffi_double",
                "test",
                td(op_foreign(
                    "ffi.double",
                    vec![op_lit("const", vec![], json!(21))],
                )),
            ))
            .unwrap();

        let ast = make_ast(vec![AstNode::new("test")]);

        // Register the FFI op (uses "self" = Library::this() to find the
        // #[no_mangle] test function in the current process).
        let mut ffi_ops = HashMap::new();
        ffi_ops.insert(
            "ffi.double".to_string(),
            FfiOpFunction::new(
                "self",
                "ffi_cranelift_test_double_i32",
                vec!["i32".to_string()],
                "i32",
                false,
            ),
        );

        let engine = CraneliftEngine::new().with_ffi_ops(ffi_ops);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(42));
    }

    /// Test that non-FFI Foreign ops still use the ModuleLookup path
    /// (backward compatibility).
    #[test]
    fn non_ffi_foreign_ops_use_module_lookup_path() {
        use handl_plugin_ffi::FfiOpFunction;
        use std::collections::HashMap;

        // Build a language with a node kind "test" whose eval scheme
        // calls the foreign op "double" (non-FFI, goes through ModuleLookup).
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.double",
                "test",
                td(op_foreign(
                    "double",
                    vec![op_lit("const", vec![], json!(21))],
                )),
            ))
            .unwrap();

        let ast = make_ast(vec![AstNode::new("test")]);

        // Register an FFI op for a DIFFERENT op name — "double" should
        // still go through ModuleLookup.
        let mut ffi_ops = HashMap::new();
        ffi_ops.insert(
            "ffi.other".to_string(),
            FfiOpFunction::new(
                "self",
                "ffi_cranelift_test_double_i32",
                vec!["i32".to_string()],
                "i32",
                false,
            ),
        );

        // Set up the module registry with "double".
        let modules: Rc<dyn handl_platform::ModuleLookup> =
            Rc::new(ModuleRegistry::new().with(ConformanceModule));

        let engine = CraneliftEngine::with_modules(modules).with_ffi_ops(ffi_ops);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(42));
    }

    // --- Phase 9.5 import / eval_block ---

    struct StubImportResolver;
    impl ImportResolver for StubImportResolver {
        fn resolve_import(&self, _path: &str) -> Result<Value, LangError> {
            Ok(json!({"x": 42}))
        }
    }

    fn import_lang() -> Language {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                td(op_lit("proj_env", vec![], Value::String("$payload".into()))),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.import",
                "stmt.import",
                td(op_native(
                    "import",
                    vec![op_lit("proj_child", vec![], json!(0u64))],
                )),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.block",
                "block",
                td(op_native("eval_block", vec![])),
            ))
            .unwrap();
        lang
    }

    #[test]
    fn import_binds_exports_into_scope() {
        let lang = import_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!("m")), // 0
            AstNode::new("var").with_payload(json!("x")),   // 1
            AstNode::new("stmt.import").with_payload(json!({"children": [0]})), // 2
            AstNode::new("block").with_payload(json!({"children": [2, 1]})), // 3
        ]);
        let engine: Box<dyn EvalEngine> =
            CraneliftEngine::new().with_import_resolver(Rc::new(StubImportResolver));
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }

    #[test]
    fn import_binds_alias_as_module_object() {
        let lang = import_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!("m")), // 0
            AstNode::new("var").with_payload(json!("m")),   // 1
            AstNode::new("stmt.import").with_payload(json!({"children": [0], "alias": "m"})), // 2
            AstNode::new("block").with_payload(json!({"children": [2, 1]})), // 3
        ]);
        let engine: Box<dyn EvalEngine> =
            CraneliftEngine::new().with_import_resolver(Rc::new(StubImportResolver));
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!({"x": 42}));
    }

    #[test]
    fn import_errors_without_resolver() {
        let lang = import_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!("m")), // 0
            AstNode::new("stmt.import").with_payload(json!({"children": [0]})), // 1
        ]);
        let engine = CraneliftEngine::new();
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(err.to_string().contains("no import resolver configured"));
    }
}
