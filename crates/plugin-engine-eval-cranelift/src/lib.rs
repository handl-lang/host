//! # handl-plugin-engine-eval-cranelift
//!
//! The **cranelift eval engine** for the eval operation (ADR-0044 §4, §7).
//!
//! This engine compiles [`handl_language::Scheme`] data to **native code** via
//! [cranelift] and executes it via an in-memory JIT. It shares the same
//! `Scheme` vocabulary as the
//! [`handl_plugin_engine_eval_tree_walk`](handl_plugin_engine_eval_tree_walk)
//! reference interpreter and the
//! [`handl_plugin_engine_eval_bytecode`](handl_plugin_engine_eval_bytecode)
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
//! - **Control flow is native**: [`Scheme::Choice`] compiles to native
//!   `brif`/`jump` instructions. `extend_env`/`proj_env` scope push/pop is
//!   native. [`Scheme::App`] closure dispatch is a native call to a helper.
//! - **Value operations call Rust helpers**: native `Arr` ops
//!   ([`Scheme::Arr`] with `ArrSource::Native`), foreign `Arr` ops
//!   ([`Scheme::Arr`] with `ArrSource::Foreign`), `proj_child`, variable
//!   load/store, and payload access are native calls into Rust functions
//!   that operate on [`Value`]
//!   (boxed as a pointer). The native code passes/receives `*mut Value`
//!   handles.
//! - **The per-node scheme lookup is erased**: at compile time, each node's
//!   [`Scheme`] is lowered to a fixed sequence of cranelift IR instructions +
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

use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use handl_language::{Ast, AstNode, AstRef, LangError, Language, SchemeVersion, Value};
use handl_plugin_engine_eval::pattern_match::{self, PatternMatchCtx};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// The cranelift eval engine.
///
/// Compiles `Scheme` data to native code via cranelift and executes via an
/// in-memory JIT. Construct with [`CraneliftEngine::new`] and optionally pass
/// a module registry via [`CraneliftEngine::with_modules`].
///
/// The engine checks `lang.scheme_version` against `SchemeVersion::v2()` and
/// rejects mismatched versions.
#[derive(Clone)]
pub struct CraneliftEngine {
    /// The module registry for `Foreign` dispatch (may be `None`).
    modules: Option<Rc<dyn handl_platform::ModuleLookup>>,
}

impl CraneliftEngine {
    /// Create a new cranelift engine with no module registry.
    pub fn new() -> Self {
        Self { modules: None }
    }

    /// Attach a module registry (builder style).
    pub fn with_modules(modules: Rc<dyn handl_platform::ModuleLookup>) -> Self {
        Self {
            modules: Some(modules),
        }
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
    /// (`Scheme::Arr` with `ArrSource::Foreign`) — there is no host functions
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
        // Scheme version check — the engine supports v2 only.
        if !SchemeVersion::v2().supports(&lang.scheme_version) {
            return Err(LangError::Engine(format!(
                "unsupported scheme version: {} (engine supports v2)",
                lang.scheme_version
            )));
        }

        // Compile all nodes in the AST to native functions.
        // The scheme lookup (lang.eval[node.kind]) happens here, at compile
        // time — it is erased from the compiled code (ADR §7).
        let compile_result = compile_all_nodes_jit(lang, ast)?;

        run_native(
            lang,
            ast,
            &compile_result.compiled,
            &compile_result.literals,
            &compile_result.names,
            self.modules.as_ref(),
        )
    }

    fn box_clone(&self) -> Box<dyn EvalEngine> {
        Box::new(self.clone())
    }

    fn with_modules(&self, modules: Rc<dyn handl_platform::ModuleLookup>) -> Box<dyn EvalEngine> {
        Box::new(CraneliftEngine::with_modules(modules))
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
) -> Result<Value, LangError> {
    // Build the evaluation context.
    let mut ctx = EvalCtx {
        lang: lang as *const Language,
        ast: ast as *const Ast,
        env: ScopeStack::new(),
        current_ref: ast.len() - 1,
        modules: modules.map(|m| Rc::as_ptr(m) as *const dyn handl_platform::ModuleLookup),
        compiled: compiled.clone(),
        literals: literals.to_vec(),
        names: names.to_vec(),
        error: None,
    };

    // Call the root node's compiled native function.
    // The root is the last node in the arena (Pratt convention).
    let root_idx = ast.len() - 1;
    let root_fn = ctx
        .compiled
        .get(&root_idx)
        .copied()
        .ok_or_else(|| LangError::Engine("no compiled function for root".to_string()))?;

    // Top-level incoming is a boxed Null.
    let null_incoming = unsafe { box_value(Value::Null) };
    let result_ptr: *mut Value = unsafe {
        root_fn(
            &mut ctx as *mut EvalCtx as *mut u8,
            null_incoming as *mut u8,
        ) as *mut Value
    };

    // Check for deferred error.
    if let Some(err) = ctx.error.take() {
        if !result_ptr.is_null() {
            unsafe { let _ = take_value(result_ptr); }
        }
        return Err(err);
    }

    // Reclaim the result value.
    if result_ptr.is_null() {
        return Err(LangError::Engine(
            "native evaluation returned null without error".to_string(),
        ));
    }
    // SAFETY: the helper boxed this Value with Box::into_raw. We reclaim it.
    let value = unsafe { take_value(result_ptr) };
    Ok(value)
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
    /// Per-node compiled function pointers, keyed by arena index.
    /// Used by helpers ([`helper_v2_eval_child`], [`helper_v2_apply_closure`]).
    compiled: HashMap<usize, CompiledFn>,
    /// Literal constant pool: values referenced by `const` ops.
    literals: Vec<Value>,
    /// Name constant pool: variable names, primitive ops, foreign names.
    names: Vec<String>,
    /// Deferred error: set by a helper on failure, checked after native
    /// execution returns.
    error: Option<LangError>,
}

impl EvalCtx {
    /// Get the current node being evaluated.
    fn current_node(&self) -> Result<&AstNode, LangError> {
        unsafe { (*self.ast).at(AstRef::new(self.current_ref)) }
    }

    /// Get a node by arena index (ADR-0060).
    fn ast_at(&self, node_ref: AstRef) -> Result<&AstNode, LangError> {
        unsafe { (*self.ast).at(node_ref) }
    }

    /// Set the deferred error and return a null pointer (for helper use).
    fn set_error(&mut self, err: LangError) -> *mut Value {
        self.error = Some(err);
        std::ptr::null_mut()
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
        literals: Vec<Value>,
        names: Vec<String>,
    ) -> Self {
        Self {
            lang: std::ptr::null(),
            ast: ast as *const Ast,
            env: ScopeStack::new(),
            current_ref: if ast.is_empty() { 0 } else { ast.len() - 1 },
            modules: None,
            compiled,
            
            literals,
            names,
            error: None,
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
        self.env.define(name, value);
    }

    fn eval_pattern_node(&mut self, _ast: &Ast, node_ref: AstRef) -> Result<Value, LangError> {
        eval_child_node(self, node_ref.index())
    }
}

// ---------------------------------------------------------------------------
// Scope stack (identical semantics to tree-walk)
// ---------------------------------------------------------------------------

/// A scope stack: `Vec<HashMap<String, Value>>` searched innermost-first.
struct ScopeStack {
    /// The stack of scope frames.
    scopes: Vec<HashMap<String, Value>>,
}

impl ScopeStack {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    /// Look up a variable, searching innermost scope first.
    fn lookup(&self, name: &str) -> Option<&Value> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    /// Define a variable in the current (innermost) scope.
    fn define(&mut self, name: &str, value: Value) {
        let last = self.scopes.last_mut().expect("scope stack is never empty");
        last.insert(name.to_string(), value);
    }

    /// Push a new empty scope.
    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pop the innermost scope.
    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Capture all visible variables into a single flat map.
    fn capture(&self) -> HashMap<String, Value> {
        let mut captured = HashMap::new();
        for scope in &self.scopes {
            for (k, v) in scope {
                captured.insert(k.clone(), v.clone());
            }
        }
        captured
    }
}

// ---------------------------------------------------------------------------
// Number helpers (identical semantics to tree-walk)
// ---------------------------------------------------------------------------

/// A number that can be either integer or float.
#[derive(Debug, Clone, Copy)]
enum Num {
    /// Integer value.
    Int(i64),
    /// Float value.
    Float(f64),
}

impl Num {
    /// Convert to f64.
    fn to_f64(self) -> f64 {
        match self {
            Num::Int(i) => i as f64,
            Num::Float(f) => f,
        }
    }
}

impl std::ops::Add for Num {
    type Output = Num;
    fn add(self, other: Num) -> Num {
        match (self, other) {
            (Num::Int(a), Num::Int(b)) => Num::Int(a + b),
            _ => Num::Float(self.to_f64() + other.to_f64()),
        }
    }
}

impl std::ops::Sub for Num {
    type Output = Num;
    fn sub(self, other: Num) -> Num {
        match (self, other) {
            (Num::Int(a), Num::Int(b)) => Num::Int(a - b),
            _ => Num::Float(self.to_f64() - other.to_f64()),
        }
    }
}

impl std::ops::Mul for Num {
    type Output = Num;
    fn mul(self, other: Num) -> Num {
        match (self, other) {
            (Num::Int(a), Num::Int(b)) => Num::Int(a * b),
            _ => Num::Float(self.to_f64() * other.to_f64()),
        }
    }
}

impl std::ops::Div for Num {
    type Output = Num;
    fn div(self, other: Num) -> Num {
        match (self, other) {
            (Num::Int(a), Num::Int(b)) => {
                if b != 0 {
                    Num::Int(a / b)
                } else {
                    Num::Float(self.to_f64() / other.to_f64())
                }
            }
            _ => Num::Float(self.to_f64() / other.to_f64()),
        }
    }
}

impl std::ops::Rem for Num {
    type Output = Num;
    fn rem(self, other: Num) -> Num {
        match (self, other) {
            (Num::Int(a), Num::Int(b)) => Num::Int(a % b),
            _ => Num::Float(self.to_f64() % other.to_f64()),
        }
    }
}

impl PartialEq for Num {
    fn eq(&self, other: &Num) -> bool {
        self.to_f64() == other.to_f64()
    }
}

impl PartialOrd for Num {
    fn partial_cmp(&self, other: &Num) -> Option<std::cmp::Ordering> {
        self.to_f64().partial_cmp(&other.to_f64())
    }
}

/// Extract a number from a [`Value`].
fn as_num(v: &Value) -> Result<Num, LangError> {
    if let Some(i) = v.as_i64() {
        return Ok(Num::Int(i));
    }
    if let Some(f) = v.as_f64() {
        return Ok(Num::Float(f));
    }
    Err(LangError::Engine(format!("expected number, got {}", v)))
}

/// Convert a [`Num`] back to a [`Value`].
fn num_value(n: Num) -> Value {
    match n {
        Num::Int(i) => Value::from(i),
        Num::Float(f) => Value::from(f),
    }
}

/// Whether a [`Value`] is truthy.
///
/// `false`, `null`, `0`, and `""` are falsy; everything else is truthy.
fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Value boxing helpers
// ---------------------------------------------------------------------------

/// Box a [`Value`] into a heap-allocated pointer for passing to native code.
///
/// # Safety
///
/// The returned pointer must be reclaimed with `Box::from_raw` (either by a
/// consuming helper or by the engine for the final result).
unsafe fn box_value(value: Value) -> *mut Value {
    Box::into_raw(Box::new(value))
}

/// Reclaim a boxed [`Value`] pointer, moving the value out and freeing the
/// allocation.
///
/// # Safety
///
/// `ptr` must be a valid pointer returned by [`box_value`] (or
/// `Box::into_raw`).
unsafe fn take_value(ptr: *mut Value) -> Value {
    unsafe { std::mem::take(&mut *Box::from_raw(ptr)) }
}

// ---------------------------------------------------------------------------
// Helper functions (extern "C", called from native code)
// ---------------------------------------------------------------------------

/// Push a literal value (from the constant pool) as a boxed clone.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_push_literal(ctx: *mut EvalCtx, lit_idx: usize) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        if lit_idx >= ctx.literals.len() {
            return ctx.set_error(LangError::Engine(format!(
                "literal index {} out of bounds",
                lit_idx
            )));
        }
        box_value(ctx.literals[lit_idx].clone())
    }
}

/// Load a variable by name index from the scope stack.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_load_var(ctx: *mut EvalCtx, name_idx: usize) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        if name_idx >= ctx.names.len() {
            return ctx.set_error(LangError::Engine(format!(
                "name index {} out of bounds",
                name_idx
            )));
        }
        let name = &ctx.names[name_idx];
        match ctx.env.lookup(name) {
            Some(v) => box_value(v.clone()),
            None => ctx.set_error(LangError::UnknownVariable { name: name.clone() }),
        }
    }
}

/// Load the payload variable: reads `node.payload` as a string (the variable
/// name) and looks it up in the scope.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_load_payload_var(ctx: *mut EvalCtx) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        match ctx.current_node() {
            Ok(node) => match node.payload.as_str() {
                Some(var_name) => match ctx.env.lookup(var_name) {
                    Some(v) => box_value(v.clone()),
                    None => ctx.set_error(LangError::UnknownVariable {
                        name: var_name.to_string(),
                    }),
                },
                None => ctx.set_error(LangError::Engine(
                    "expected string payload for $payload var".to_string(),
                )),
            },
            Err(e) => ctx.set_error(e),
        }
    }
}

/// Load the current node's payload directly (no-scheme fallback).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_load_payload(ctx: *mut EvalCtx) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        match ctx.current_node() {
            Ok(node) => box_value(node.payload.clone()),
            Err(e) => ctx.set_error(e),
        }
    }
}

/// Evaluate the nth child of the current node.
///
/// Reads `node.payload["children"][n]` to get the child's `AstRef`, then
/// calls the child's pre-compiled native function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_eval_child(ctx: *mut EvalCtx, child_n: usize) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        let node = match ctx.current_node() {
            Ok(n) => n,
            Err(e) => return ctx.set_error(e),
        };
        let children = match node.payload.get("children").and_then(|c| c.as_array()) {
            Some(arr) => arr,
            None => {
                return ctx.set_error(LangError::Engine(
                    "expected children array in payload".to_string(),
                ));
            }
        };
        let arena_idx = match children.get(child_n).and_then(|v| v.as_u64()) {
            Some(idx) => idx as usize,
            None => {
                return ctx.set_error(LangError::Engine(format!("child {} not found", child_n)));
            }
        };
        // Look up the compiled function for the child.
        let func = match ctx.compiled.get(&arena_idx).copied() {
            Some(f) => f,
            None => {
                return ctx.set_error(LangError::Engine(format!(
                    "no compiled function for node {}",
                    arena_idx
                )));
            }
        };
        // Save and set current_ref for the child evaluation.
        let saved_ref = ctx.current_ref;
        ctx.current_ref = arena_idx;
        let null_incoming = box_value(Value::Null);
        let result = func(ctx as *mut EvalCtx as *mut u8, null_incoming as *mut u8) as *mut Value;
        // Restore current_ref.
        ctx.current_ref = saved_ref;
        result
    }
}

/// Store a value in a new scope under the given name (by index).
///
/// Pushes a new scope and defines the variable. The value pointer is
/// consumed (reclaimed).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_store(ctx: *mut EvalCtx, name_idx: usize, val: *mut Value) {
    unsafe {
        let ctx = &mut *ctx;
        if name_idx >= ctx.names.len() {
            ctx.set_error(LangError::Engine(format!(
                "name index {} out of bounds",
                name_idx
            )));
            if !val.is_null() {
                let _ = take_value(val);
            }
            return;
        }
        let name = ctx.names[name_idx].clone();
        let value = if val.is_null() {
            return;
        } else {
            take_value(val)
        };
        ctx.env.push_scope();
        ctx.env.define(&name, value);
    }
}

/// Store a value in a new scope under the name from `node.payload["name"]`.
///
/// Pushes a new scope and defines the variable. The value pointer is
/// consumed (reclaimed).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_store_payload(ctx: *mut EvalCtx, val: *mut Value) {
    unsafe {
        let ctx = &mut *ctx;
        let node = match ctx.current_node() {
            Ok(n) => n,
            Err(e) => {
                ctx.set_error(e);
                if !val.is_null() {
                    let _ = take_value(val);
                }
                return;
            }
        };
        let bind_name = match node.payload.get("name").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => {
                ctx.set_error(LangError::Engine(
                    "expected name in payload for $payload bind".to_string(),
                ));
                if !val.is_null() {
                    let _ = take_value(val);
                }
                return;
            }
        };
        if val.is_null() {
            return;
        }
        let value = take_value(val);
        ctx.env.push_scope();
        ctx.env.define(&bind_name, value);
    }
}

/// Pop the innermost scope.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_pop_scope(ctx: *mut EvalCtx) {
    unsafe {
        let ctx = &mut *ctx;
        ctx.env.pop_scope();
    }
}

/// Push a new empty scope for the `vau` parameter bindings (ADR-0041 §6).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_vau_enter(ctx: *mut EvalCtx) {
    unsafe {
        let ctx = &mut *ctx;
        ctx.env.push_scope();
    }
}

/// Bind the `n`-th child of the current node — unevaluated — to the name
/// (by index) in the current scope (the `vau` escape, ADR-0041 §6).
///
/// Reads `node.payload["children"][child_n]` to get the child's `AstRef`,
/// extracts the child's subtree into a standalone `Ast`, wraps it as a
/// tagged `__ast__` value, and binds it to the name in the current
/// (innermost) scope. The child is **not** evaluated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_vau_bind(ctx: *mut EvalCtx, name_idx: usize, child_n: usize) {
    unsafe {
        let ctx = &mut *ctx;
        if name_idx >= ctx.names.len() {
            ctx.set_error(LangError::Engine(format!(
                "name index {} out of bounds",
                name_idx
            )));
            return;
        }
        let name = ctx.names[name_idx].clone();
        let node = match ctx.current_node() {
            Ok(n) => n,
            Err(e) => {
                ctx.set_error(e);
                return;
            }
        };
        let children = match node.payload.get("children").and_then(|c| c.as_array()) {
            Some(arr) => arr,
            None => {
                ctx.set_error(LangError::Engine(
                    "expected children array in payload".to_string(),
                ));
                return;
            }
        };
        let arena_idx = match children.get(child_n).and_then(|v| v.as_u64()) {
            Some(idx) => idx as usize,
            None => {
                ctx.set_error(LangError::Engine(format!(
                    "vau: child {} not found",
                    child_n
                )));
                return;
            }
        };
        let ast_ref = &*ctx.ast;
        let sub = match ast_ref.subtree(AstRef::new(arena_idx)) {
            Ok(s) => s,
            Err(e) => {
                ctx.set_error(e);
                return;
            }
        };
        let val = handl_language::ast_value(&sub);
        ctx.env.define(&name, val);
    }
}

/// Reclaim a boxed [`Value`] pointer, freeing the allocation.
///
/// This is used to free intermediate values that are checked but not consumed
/// by a helper (e.g. a `Branch` condition).
///
/// # Safety
///
/// `ptr` must be a valid pointer returned by [`box_value`] (or
/// `Box::into_raw`), or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_drop_value(val: *mut Value) {
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
pub unsafe extern "C" fn helper_is_truthy(val: *const Value) -> u8 {
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
    callee: *mut Value,
    args: *const *mut Value,
    n: usize,
) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        if callee.is_null() {
            return ctx.set_error(LangError::Engine("closure callee is null".to_string()));
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

        // Parse closure.
        let obj = match closure_val.as_object() {
            Some(o) => o,
            None => return ctx.set_error(LangError::Engine("callee is not an arrow-value".to_string())),
        };
        if !obj
            .get("__arrow__")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return ctx.set_error(LangError::Engine("callee is not an arrow-value".to_string()));
        }

        let params: Vec<String> = match obj.get("params").and_then(|v| v.as_array()) {
            Some(arr) => {
                let mut params = Vec::with_capacity(arr.len());
                for v in arr {
                    match v.as_str() {
                        Some(s) => params.push(s.to_string()),
                        None => {
                            return ctx.set_error(LangError::Engine(
                                "arrow-value param is not a string".to_string(),
                            ));
                        }
                    }
                }
                params
            }
            None => return ctx.set_error(LangError::Engine("arrow-value missing params".to_string())),
        };

        let body_idx = match obj.get("body").and_then(|v| v.as_u64()) {
            Some(idx) => idx as usize,
            None => return ctx.set_error(LangError::Engine("arrow-value missing body".to_string())),
        };

        let captured_env: HashMap<String, Value> = obj
            .get("env")
            .and_then(|v| v.as_object())
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        if params.len() != arg_vals.len() {
            return ctx.set_error(LangError::Engine(format!(
                "arrow-value expects {} args, got {}",
                params.len(),
                arg_vals.len()
            )));
        }

        // Look up the compiled function for the body node.
        let func = match ctx.compiled.get(&body_idx).copied() {
            Some(f) => f,
            None => {
                return ctx.set_error(LangError::Engine(format!(
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

        // Save and set current_ref for the body.
        let saved_ref = ctx.current_ref;
        ctx.current_ref = body_idx;

        let null_incoming = box_value(Value::Null);
        let result = func(ctx as *mut EvalCtx as *mut u8, null_incoming as *mut u8) as *mut Value;

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
pub unsafe extern "C" fn helper_apply_prim(
    ctx: *mut EvalCtx,
    op_idx: usize,
    args: *const *mut Value,
    n: usize,
) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        if op_idx >= ctx.names.len() {
            // Consume args.
            for i in 0..n {
                let p = *args.add(i);
                if !p.is_null() {
                    let _ = take_value(p);
                }
            }
            return ctx.set_error(LangError::Engine(format!(
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

        apply_prim(ctx, &op, &arg_vals)
    }
}

/// Call a foreign (host) function (by name index).
///
/// All argument value pointers are consumed (reclaimed).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_call_foreign(
    ctx: *mut EvalCtx,
    name_idx: usize,
    args: *const *mut Value,
    n: usize,
) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        if name_idx >= ctx.names.len() {
            for i in 0..n {
                let p = *args.add(i);
                if !p.is_null() {
                    let _ = take_value(p);
                }
            }
            return ctx.set_error(LangError::Engine(format!(
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
                return ctx.set_error(LangError::Engine(format!("unregistered foreign: {}", name)));
            }
        };
        // SAFETY: modules_ptr is a valid trait-object pointer set by the engine.
        let modules: &dyn handl_platform::ModuleLookup = &*modules_ptr;
        let func = match modules.lookup(&name) {
            Some(f) => f,
            None => {
                return ctx.set_error(LangError::Engine(format!("unregistered foreign: {}", name)));
            }
        };

        // Call the module function.
        let mut pctx = handl_platform::EvalContext::new();
        match func.call(&arg_vals, &mut pctx) {
            Ok(v) => box_value(v),
            Err(e) => ctx.set_error(LangError::Engine(e.to_string())),
        }
    }
}

/// Evaluate a child node by arena index from within a Rust helper (e.g.
/// `apply_prim`).
///
/// Looks up the pre-compiled native function for the node, saves/restores
/// `current_ref`, calls the function with a boxed `Null` incoming (fresh
/// computation — these ops do not feed an incoming value to children), and
/// reclaims the boxed result. Returns the evaluated [`Value`] or sets
/// `ctx.error` and returns `Err`.
fn eval_child_node(ctx: &mut EvalCtx, arena_idx: usize) -> Result<Value, LangError> {
    let func = match ctx.compiled.get(&arena_idx).copied() {
        Some(f) => f,
        None => {
            return Err(LangError::Engine(format!(
                "no compiled function for node {}",
                arena_idx
            )))
        }
    };
    let saved_ref = ctx.current_ref;
    ctx.current_ref = arena_idx;
    let null_incoming = unsafe { box_value(Value::Null) };
    let result_ptr = unsafe {
        func(ctx as *mut EvalCtx as *mut u8, null_incoming as *mut u8) as *mut Value
    };
    ctx.current_ref = saved_ref;
    if result_ptr.is_null() {
        return Err(ctx
            .error
            .take()
            .unwrap_or_else(|| {
                LangError::Engine("eval_child_node: null result".to_string())
            }));
    }
    Ok(unsafe { take_value(result_ptr) })
}

/// `match` prim: pattern-match a scrutinee against a list of arms.
///
/// The node payload is `{"children": [scrut_idx, block_idx]}`; the block node
/// holds the arm indices. Each arm is `{"children": [pattern_idx, body_idx]}`.
/// The first matching arm's body is evaluated in a scope holding the pattern
/// bindings. If no arm matches, an error is returned.
fn prim_match(ctx: &mut EvalCtx) -> *mut Value {
    let node = match ctx.current_node() {
        Ok(n) => n,
        Err(e) => return ctx.set_error(e),
    };
    let children = match node.payload.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => {
            return ctx.set_error(LangError::Engine(
                "match: missing children in payload".to_string(),
            ))
        }
    };
    let scrut_idx = match children.get(0).and_then(|v| v.as_u64()) {
        Some(i) => i as usize,
        None => {
            return ctx.set_error(LangError::Engine(
                "match: missing scrutinee child".to_string(),
            ))
        }
    };
    let block_idx = match children.get(1).and_then(|v| v.as_u64()) {
        Some(i) => i as usize,
        None => {
            return ctx.set_error(LangError::Engine(
                "match: missing block child".to_string(),
            ))
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
    let arm_indices = match block_node.payload.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => {
            return ctx.set_error(LangError::Engine(
                "match: block node missing children".to_string(),
            ))
        }
    };
    for arm_entry in arm_indices {
        let arm_idx = match arm_entry.as_u64() {
            Some(i) => i as usize,
            None => {
                return ctx.set_error(LangError::Engine(
                    "match: arm index must be a number".to_string(),
                ))
            }
        };
        let arm_node = match ast.at(AstRef::new(arm_idx)) {
            Ok(n) => n,
            Err(e) => return ctx.set_error(e),
        };
        let arm_children = match arm_node.payload.get("children").and_then(|c| c.as_array()) {
            Some(c) => c,
            None => {
                return ctx.set_error(LangError::Engine(
                    "match: arm node missing children".to_string(),
                ))
            }
        };
        let pattern_idx = match arm_children.get(0).and_then(|v| v.as_u64()) {
            Some(i) => i as usize,
            None => {
                return ctx.set_error(LangError::Engine(
                    "match: arm missing pattern child".to_string(),
                ))
            }
        };
        let body_idx = match arm_children.get(1).and_then(|v| v.as_u64()) {
            Some(i) => i as usize,
            None => {
                return ctx.set_error(LangError::Engine(
                    "match: arm missing body child".to_string(),
                ))
            }
        };
        ctx.env.push_scope();
        let ast = unsafe { &*ctx.ast };
        let matched = match pattern_match::try_pattern(ast, AstRef::new(pattern_idx), &scrut_val, ctx) {
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
    ctx.set_error(LangError::Engine("match: no arm matched".to_string()))
}

/// `handle` prim: evaluate a body, then dispatch the result to the first
/// matching handler clause.
///
/// Structurally identical to `match` (children = [body_idx, block_idx], block
/// contains `arm` nodes with [pattern_idx, handler_idx]). The body is
/// evaluated first; its result is matched against clause patterns. If no
/// clause matches, the body value is returned as-is (not an error).
fn prim_handle(ctx: &mut EvalCtx) -> *mut Value {
    let node = match ctx.current_node() {
        Ok(n) => n,
        Err(e) => return ctx.set_error(e),
    };
    let children = match node.payload.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => {
            return ctx.set_error(LangError::Engine(
                "handle: missing children in payload".to_string(),
            ))
        }
    };
    let body_idx = match children.get(0).and_then(|v| v.as_u64()) {
        Some(i) => i as usize,
        None => {
            return ctx.set_error(LangError::Engine(
                "handle: missing body child".to_string(),
            ))
        }
    };
    let block_idx = match children.get(1).and_then(|v| v.as_u64()) {
        Some(i) => i as usize,
        None => {
            return ctx.set_error(LangError::Engine(
                "handle: missing block child".to_string(),
            ))
        }
    };
    let body_val = match eval_child_node(ctx, body_idx) {
        Ok(v) => v,
        Err(e) => return ctx.set_error(e),
    };
    let ast = unsafe { &*ctx.ast };
    let block_node = match ast.at(AstRef::new(block_idx)) {
        Ok(n) => n,
        Err(e) => return ctx.set_error(e),
    };
    let clause_indices = match block_node.payload.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => {
            return ctx.set_error(LangError::Engine(
                "handle: block node missing children".to_string(),
            ))
        }
    };
    for clause_entry in clause_indices {
        let clause_idx = match clause_entry.as_u64() {
            Some(i) => i as usize,
            None => {
                return ctx.set_error(LangError::Engine(
                    "handle: clause index must be a number".to_string(),
                ))
            }
        };
        let clause_node = match ast.at(AstRef::new(clause_idx)) {
            Ok(n) => n,
            Err(e) => return ctx.set_error(e),
        };
        let clause_children = match clause_node.payload.get("children").and_then(|c| c.as_array()) {
            Some(c) => c,
            None => {
                return ctx.set_error(LangError::Engine(
                    "handle: clause node missing children".to_string(),
                ))
            }
        };
        let pattern_idx = match clause_children.get(0).and_then(|v| v.as_u64()) {
            Some(i) => i as usize,
            None => {
                return ctx.set_error(LangError::Engine(
                    "handle: clause missing pattern child".to_string(),
                ))
            }
        };
        let handler_idx = match clause_children.get(1).and_then(|v| v.as_u64()) {
            Some(i) => i as usize,
            None => {
                return ctx.set_error(LangError::Engine(
                    "handle: clause missing handler child".to_string(),
                ))
            }
        };
        ctx.env.push_scope();
        let ast = unsafe { &*ctx.ast };
        let matched = match pattern_match::try_pattern(ast, AstRef::new(pattern_idx), &body_val, ctx) {
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

/// Apply a primitive operation (shared logic).
fn apply_prim(ctx: &mut EvalCtx, op: &str, args: &[Value]) -> *mut Value {
    let result = match op {
        "add" => {
            if args.len() < 2 {
                return ctx.set_error(LangError::Engine("add: expected 2 args".to_string()));
            }
            match as_num(&args[0]).and_then(|a| as_num(&args[1]).map(|b| a + b)) {
                Ok(n) => Ok(num_value(n)),
                Err(e) => Err(e),
            }
        }
        "sub" => {
            if args.len() < 2 {
                return ctx.set_error(LangError::Engine("sub: expected 2 args".to_string()));
            }
            match as_num(&args[0]).and_then(|a| as_num(&args[1]).map(|b| a - b)) {
                Ok(n) => Ok(num_value(n)),
                Err(e) => Err(e),
            }
        }
        "mul" => {
            if args.len() < 2 {
                return ctx.set_error(LangError::Engine("mul: expected 2 args".to_string()));
            }
            match as_num(&args[0]).and_then(|a| as_num(&args[1]).map(|b| a * b)) {
                Ok(n) => Ok(num_value(n)),
                Err(e) => Err(e),
            }
        }
        "div" => {
            if args.len() < 2 {
                return ctx.set_error(LangError::Engine("div: expected 2 args".to_string()));
            }
            let b_num = as_num(&args[1]);
            // Match tree-walk: if b.to_f64() == 0.0, return error.
            if let Ok(bn) = &b_num {
                if bn.to_f64() == 0.0 {
                    return ctx.set_error(LangError::Engine("division by zero".to_string()));
                }
            }
            match as_num(&args[0]).and_then(|a| b_num.map(|b| a / b)) {
                Ok(n) => Ok(num_value(n)),
                Err(e) => Err(e),
            }
        }
        "mod" => {
            if args.len() < 2 {
                return ctx.set_error(LangError::Engine("mod: expected 2 args".to_string()));
            }
            let b_num = as_num(&args[1]);
            if let Ok(bn) = &b_num {
                if bn.to_f64() == 0.0 {
                    return ctx.set_error(LangError::Engine("modulo by zero".to_string()));
                }
            }
            match as_num(&args[0]).and_then(|a| b_num.map(|b| a % b)) {
                Ok(n) => Ok(num_value(n)),
                Err(e) => Err(e),
            }
        }
        "eq" => Ok(Value::Bool(args[0] == args[1])),
        "lt" => {
            if args.len() < 2 {
                return ctx.set_error(LangError::Engine("lt: expected 2 args".to_string()));
            }
            match as_num(&args[0]).and_then(|a| as_num(&args[1]).map(|b| a < b)) {
                Ok(b) => Ok(Value::Bool(b)),
                Err(e) => Err(e),
            }
        }
        "gt" => {
            if args.len() < 2 {
                return ctx.set_error(LangError::Engine("gt: expected 2 args".to_string()));
            }
            match as_num(&args[0]).and_then(|a| as_num(&args[1]).map(|b| a > b)) {
                Ok(b) => Ok(Value::Bool(b)),
                Err(e) => Err(e),
            }
        }
        "pow" => {
            if args.len() < 2 {
                return ctx.set_error(LangError::Engine("pow: expected 2 args".to_string()));
            }
            match as_num(&args[0]).and_then(|a| {
                as_num(&args[1]).map(|b| num_value(Num::Float(a.to_f64().powf(b.to_f64()))))
            }) {
                Ok(v) => Ok(v),
                Err(e) => Err(e),
            }
        }
        "gte" => {
            if args.len() < 2 {
                return ctx.set_error(LangError::Engine("gte: expected 2 args".to_string()));
            }
            match as_num(&args[0]).and_then(|a| as_num(&args[1]).map(|b| a >= b)) {
                Ok(b) => Ok(Value::Bool(b)),
                Err(e) => Err(e),
            }
        }
        "lte" => {
            if args.len() < 2 {
                return ctx.set_error(LangError::Engine("lte: expected 2 args".to_string()));
            }
            match as_num(&args[0]).and_then(|a| as_num(&args[1]).map(|b| a <= b)) {
                Ok(b) => Ok(Value::Bool(b)),
                Err(e) => Err(e),
            }
        }
        "neq" => Ok(Value::Bool(args[0] != args[1])),
        "compare" => {
            if args.len() < 2 {
                return ctx.set_error(LangError::Engine("compare: expected 2 args".to_string()));
            }
            match as_num(&args[0]).and_then(|a| {
                as_num(&args[1]).map(|b| {
                    let af = a.to_f64();
                    let bf = b.to_f64();
                    if af < bf {
                        num_value(Num::Int(-1))
                    } else if af > bf {
                        num_value(Num::Int(1))
                    } else {
                        num_value(Num::Int(0))
                    }
                })
            }) {
                Ok(v) => Ok(v),
                Err(e) => Err(e),
            }
        }
        "typeof" => {
            let type_name = match &args[0] {
                Value::Null => "null",
                Value::Bool(_) => "bool",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };
            Ok(Value::from(type_name))
        }
        "not" => Ok(Value::Bool(!is_truthy(&args[0]))),
        "and" => Ok(Value::Bool(is_truthy(&args[0]) && is_truthy(&args[1]))),
        "or" => Ok(Value::Bool(is_truthy(&args[0]) || is_truthy(&args[1]))),
        "neg" => {
            if args.is_empty() {
                return ctx.set_error(LangError::Engine("neg: expected 1 arg".to_string()));
            }
            match as_num(&args[0]).map(|a| match a {
                Num::Int(i) => num_value(Num::Int(-i)),
                Num::Float(f) => num_value(Num::Float(-f)),
            }) {
                Ok(v) => Ok(v),
                Err(e) => Err(e),
            }
        }
        "closure" => {
            // Create a closure from the current node's payload.
            // Supports two payload shapes:
            //   1. Rich: {"params": ["x", ...], "body": idx}
            //   2. Pratt-parsed: {"children": [param_idx, body_idx]}
            let node = match ctx.current_node() {
                Ok(n) => n,
                Err(e) => return ctx.set_error(e),
            };
            let payload = &node.payload;
            if let (Some(p), Some(b)) = (
                payload.get("params").and_then(|v| v.as_array()),
                payload.get("body").and_then(|v| v.as_u64()),
            ) {
                let env = ctx.env.capture();
                Ok(serde_json::json!({
                    "__arrow__": true,
                    "params": p,
                    "body": b,
                    "env": env,
                }))
            } else if let Some(children) =
                payload.get("children").and_then(|c| c.as_array())
            {
                let param_idx = match children
                    .first()
                    .and_then(|v| v.as_u64())
                {
                    Some(idx) => idx as usize,
                    None => return ctx.set_error(LangError::Engine(
                        "closure: missing param child".to_string(),
                    )),
                };
                let body_idx = match children
                    .get(1)
                    .and_then(|v| v.as_u64())
                {
                    Some(idx) => idx as usize,
                    None => return ctx.set_error(LangError::Engine(
                        "closure: missing body child".to_string(),
                    )),
                };
                let param_node = match ctx.ast_at(AstRef::new(param_idx)) {
                    Ok(n) => n,
                    Err(e) => return ctx.set_error(e),
                };
                let mut param_names: Vec<serde_json::Value> = Vec::new();
                if param_node.kind == "paren" || param_node.kind == "array" {
                    let param_children = match param_node
                        .payload
                        .get("children")
                        .and_then(|c| c.as_array())
                    {
                        Some(c) => c,
                        None => return ctx.set_error(LangError::Engine(
                            "closure: params missing children".to_string(),
                        )),
                    };
                    for pc in param_children {
                        let pidx = match pc.as_u64() {
                            Some(idx) => idx as usize,
                            None => return ctx.set_error(LangError::Engine(
                                "closure: param index must be a number".to_string(),
                            )),
                        };
                        let pnode = match ctx.ast_at(AstRef::new(pidx)) {
                            Ok(n) => n,
                            Err(e) => return ctx.set_error(e),
                        };
                        let pname = match pnode.payload.as_str() {
                            Some(s) => s,
                            None => return ctx.set_error(LangError::Engine(
                                "closure: param must be a var with string payload".to_string(),
                            )),
                        };
                        param_names.push(serde_json::Value::String(pname.to_string()));
                    }
                } else {
                    let pname = match param_node.payload.as_str() {
                        Some(s) => s,
                        None => return ctx.set_error(LangError::Engine(
                            "closure: param node must be a var with string payload".to_string(),
                        )),
                    };
                    param_names.push(serde_json::Value::String(pname.to_string()));
                }
                let env = ctx.env.capture();
                Ok(serde_json::json!({
                    "__arrow__": true,
                    "params": param_names,
                    "body": body_idx,
                    "env": env,
                }))
            } else {
                return ctx.set_error(LangError::Engine(
                    "closure prim: missing params/body in payload".to_string(),
                ));
            }
        }
        "array" => {
            let node = match ctx.current_node() {
                Ok(n) => n,
                Err(e) => return ctx.set_error(e),
            };
            match node.payload.get("elements") {
                Some(elements) => Ok(elements.clone()),
                None => {
                    return ctx.set_error(LangError::Engine(
                        "array: missing elements in payload".to_string(),
                    ));
                }
            }
        }
        "object" => {
            let node = match ctx.current_node() {
                Ok(n) => n,
                Err(e) => return ctx.set_error(e),
            };
            let entries = match node.payload.get("entries").and_then(|v| v.as_array()) {
                Some(arr) => arr,
                None => {
                    return ctx.set_error(LangError::Engine(
                        "object: missing entries in payload".to_string(),
                    ));
                }
            };
            let mut obj = serde_json::Map::new();
            for entry in entries {
                let key = match entry.get("key").and_then(|v| v.as_str()) {
                    Some(k) => k,
                    None => {
                        return ctx
                            .set_error(LangError::Engine("object: entry missing key".to_string()));
                    }
                };
                let value = match entry.get("value") {
                    Some(v) => v,
                    None => {
                        return ctx.set_error(LangError::Engine(
                            "object: entry missing value".to_string(),
                        ));
                    }
                };
                if handl_language::is_reserved_value_key(key) {
                    return ctx.set_error(LangError::Engine(format!(
                        "object: key '{}' is reserved",
                        key
                    )));
                }
                obj.insert(key.to_string(), value.clone());
            }
            Ok(Value::Object(obj))
        }
        "index" => {
            let node = match ctx.current_node() {
                Ok(n) => n,
                Err(e) => return ctx.set_error(e),
            };
            let target = match node.payload.get("target") {
                Some(t) => t,
                None => {
                    return ctx.set_error(LangError::Engine(
                        "index: missing target in payload".to_string(),
                    ));
                }
            };
            let index = match node.payload.get("index") {
                Some(i) => i,
                None => {
                    return ctx.set_error(LangError::Engine(
                        "index: missing index in payload".to_string(),
                    ));
                }
            };
            match (target, index) {
                (Value::Array(arr), Value::Number(n)) => {
                    let i = match n.as_u64() {
                        Some(i) => i as usize,
                        None => {
                            return ctx.set_error(LangError::Engine(
                                "index: index must be a non-negative integer".to_string(),
                            ));
                        }
                    };
                    match arr.get(i) {
                        Some(v) => Ok(v.clone()),
                        None => {
                            return ctx.set_error(LangError::Engine(format!(
                                "index: index {} out of bounds",
                                i
                            )));
                        }
                    }
                }
                (Value::Object(obj), Value::String(key)) => match obj.get(key) {
                    Some(v) => Ok(v.clone()),
                    None => {
                        return ctx.set_error(LangError::Engine(format!(
                            "index: key '{}' not found",
                            key
                        )));
                    }
                },
                _ => {
                    return ctx.set_error(LangError::Engine(
                        "index: target must be array/object and index must be number/string"
                            .to_string(),
                    ));
                }
            }
        }
        "assign" => {
            let node = match ctx.current_node() {
                Ok(n) => n,
                Err(e) => return ctx.set_error(e),
            };
            // ADR-0053: `assign { children: [var, value] }` — evaluate
            // child 1 (value) and return it. Fall back to legacy
            // `payload.value` for hand-built ASTs.
            if let Some(children) = node.payload.get("children").and_then(|c| c.as_array()) {
                if children.len() >= 2 {
                    let value_idx = match children[1].as_u64() {
                        Some(i) => i as usize,
                        None => {
                            return ctx.set_error(LangError::Engine(
                                "assign: value index must be a number".to_string(),
                            ));
                        }
                    };
                    eval_child_node(ctx, value_idx)
                } else {
                    Err(LangError::Engine(
                        "assign: expected at least 2 children".to_string(),
                    ))
                }
            } else {
                match node.payload.get("value") {
                    Some(value) => Ok(value.clone()),
                    None => {
                        return ctx.set_error(LangError::Engine(
                            "assign: missing value in payload".to_string(),
                        ));
                    }
                }
            }
        }
        "match" => return prim_match(ctx),
        "handle" => return prim_handle(ctx),
        "json" => {
            // ADR-0053: json node payload is the JSON template string.
            let node = match ctx.current_node() {
                Ok(n) => n,
                Err(e) => return ctx.set_error(e),
            };
            match node.payload.as_str() {
                Some(expr) => {
                    serde_json::from_str(expr)
                        .map_err(|e| LangError::Engine(format!("json: parse error: {}", e)))
                }
                None => Err(LangError::Engine("json: payload must be a string".to_string())),
            }
        }
        _ => Err(LangError::Engine(format!("unknown prim: {}", op))),
    };

    match result {
        Ok(v) => unsafe { box_value(v) },
        Err(e) => ctx.set_error(e),
    }
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
            .ok_or_else(|| LangError::Engine("run: no compiled result (empty program)".into()))?;
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
            return Err(LangError::Engine(format!(
                "compile: unsupported scheme version: {} (engine supports v2)",
                lang.scheme_version
            )));
        }
        let result = compile_all_nodes_jit(lang, ast)?;
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
/// Delegates to the v1 [`helper_push_literal`] (identical behavior).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_push_literal(
    ctx: *mut EvalCtx,
    lit_idx: usize,
) -> *mut Value {
    unsafe { helper_push_literal(ctx, lit_idx) }
}

/// Load a variable by name index from the scope stack.
///
/// Unlike the v1 [`helper_load_var`], an unbound variable returns the name as
/// a string value (matching the v2 tree-walk convention), not an error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_load_var(
    ctx: *mut EvalCtx,
    name_idx: usize,
) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        if name_idx >= ctx.names.len() {
            return ctx.set_error(LangError::Engine(format!(
                "name index {} out of bounds",
                name_idx
            )));
        }
        let name = &ctx.names[name_idx];
        match ctx.env.lookup(name) {
            Some(v) => box_value(v.clone()),
            None => box_value(Value::from(name.as_str())),
        }
    }
}

/// Load the payload variable: reads `node.payload` as a string (the variable
/// name) and looks it up in the scope.
///
/// Unlike the v1 [`helper_load_payload_var`], an unbound variable returns the
/// name as a string value (matching the v2 tree-walk convention).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_load_payload_var(ctx: *mut EvalCtx) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        match ctx.current_node() {
            Ok(node) => match node.payload.as_str() {
                Some(var_name) => match ctx.env.lookup(var_name) {
                    Some(v) => box_value(v.clone()),
                    None => box_value(Value::from(var_name)),
                },
                None => ctx.set_error(LangError::Engine(
                    "expected string payload for $payload var".to_string(),
                )),
            },
            Err(e) => ctx.set_error(e),
        }
    }
}

/// Load the current node's payload directly (no-scheme fallback).
///
/// Delegates to the v1 [`helper_load_payload`] (identical behavior).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_load_payload(ctx: *mut EvalCtx) -> *mut Value {
    unsafe { helper_load_payload(ctx) }
}

/// Evaluate the nth child of the current node, passing the incoming value.
///
/// Reads `node.payload["children"][n]` to get the child's `AstRef`, then
/// calls the child's pre-compiled v2 native function with `incoming` as the
/// second parameter. This is the v2 counterpart of [`helper_eval_child`]: the
/// child's v2 function receives the incoming value flowing through the arrow.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_eval_child(
    ctx: *mut EvalCtx,
    child_n: usize,
    incoming: *mut Value,
) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        let node = match ctx.current_node() {
            Ok(n) => n,
            Err(e) => return ctx.set_error(e),
        };
        let children = match node.payload.get("children").and_then(|c| c.as_array()) {
            Some(arr) => arr,
            None => {
                return ctx.set_error(LangError::Engine(
                    "expected children array in payload".to_string(),
                ));
            }
        };
        let arena_idx = match children.get(child_n).and_then(|v| v.as_u64()) {
            Some(idx) => idx as usize,
            None => {
                return ctx.set_error(LangError::Engine(format!("child {} not found", child_n)));
            }
        };
        let func = match ctx.compiled.get(&arena_idx).copied() {
            Some(f) => f,
            None => {
                return ctx.set_error(LangError::Engine(format!(
                    "no compiled v2 function for node {}",
                    arena_idx
                )));
            }
        };
        let saved_ref = ctx.current_ref;
        ctx.current_ref = arena_idx;
        let result = func(
            ctx as *mut EvalCtx as *mut u8,
            incoming as *mut u8,
        ) as *mut Value;
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
    val: *mut Value,
) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        if name_idx >= ctx.names.len() {
            if !val.is_null() {
                let _ = take_value(val);
            }
            return ctx.set_error(LangError::Engine(format!(
                "name index {} out of bounds",
                name_idx
            )));
        }
        let name = ctx.names[name_idx].clone();
        if val.is_null() {
            return ctx.set_error(LangError::Engine(
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
    val: *mut Value,
) -> *mut Value {
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
                return ctx.set_error(LangError::Engine(
                    "expected name in payload for $payload extend_env".to_string(),
                ));
            }
        };
        if val.is_null() {
            return ctx.set_error(LangError::Engine(
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
/// Delegates to the v1 [`helper_pop_scope`] (identical behavior).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_pop_scope(ctx: *mut EvalCtx) {
    unsafe { helper_pop_scope(ctx) }
}

/// Reclaim a boxed [`Value`] pointer, freeing the allocation.
///
/// Delegates to the v1 [`helper_drop_value`] (identical behavior).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_drop_value(val: *mut Value) {
    unsafe { helper_drop_value(val) }
}

/// Apply a closure value (or host function) to argument values.
///
/// The callee and all argument value pointers are consumed (reclaimed). This
/// mirrors the v1 [`helper_apply_closure`] but calls the child's v2 compiled
/// function (which takes an incoming value parameter). For closure bodies, the
/// incoming value is a boxed `Null` (closure bodies are evaluated as fresh
/// computations). For host-function string callees, dispatches to the module
/// registry.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_apply_closure(
    ctx: *mut EvalCtx,
    callee: *mut Value,
    args: *const *mut Value,
    n: usize,
) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        if callee.is_null() {
            return ctx.set_error(LangError::Engine("closure callee is null".to_string()));
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
    closure_val: Value,
    arg_vals: Vec<Value>,
) -> *mut Value {
    // SAFETY: this is the shared core factored out of the `unsafe extern "C"`
    // helper `helper_v2_apply_closure`. It calls the unsafe `box_value`
    // helper. Callers must uphold the same safety contract as that helper:
    // `ctx` is a valid `&mut EvalCtx`.
    unsafe {
        // ADR-0060: unpack single array arg for variable-arity call.
        // When the closure receives a single array argument, unpack it into
        // individual values before binding params. This supports the
        // data-driven call dispatch where args are packed into an array.
        let arg_vals = if arg_vals.len() == 1 {
            if let Some(arr) = arg_vals[0].as_array() {
                arr.clone()
            } else {
                arg_vals
            }
        } else {
            arg_vals
        };

        // Closure application.
        let obj = match closure_val.as_object() {
            Some(o) => o,
            None => return ctx.set_error(LangError::Engine("callee is not an arrow-value".to_string())),
        };
        if !obj
            .get("__arrow__")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return ctx.set_error(LangError::Engine("callee is not an arrow-value".to_string()));
        }

        let params: Vec<String> = match obj.get("params").and_then(|v| v.as_array()) {
            Some(arr) => {
                let mut params = Vec::with_capacity(arr.len());
                for v in arr {
                    match v.as_str() {
                        Some(s) => params.push(s.to_string()),
                        None => {
                            return ctx.set_error(LangError::Engine(
                                "arrow-value param is not a string".to_string(),
                            ));
                        }
                    }
                }
                params
            }
            None => return ctx.set_error(LangError::Engine("arrow-value missing params".to_string())),
        };

        let body_idx = match obj.get("body").and_then(|v| v.as_u64()) {
            Some(idx) => idx as usize,
            None => return ctx.set_error(LangError::Engine("arrow-value missing body".to_string())),
        };

        let captured_env: HashMap<String, Value> = obj
            .get("env")
            .and_then(|v| v.as_object())
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        if params.len() != arg_vals.len() {
            return ctx.set_error(LangError::Engine(format!(
                "arrow-value expects {} args, got {}",
                params.len(),
                arg_vals.len()
            )));
        }

        let func = match ctx.compiled.get(&body_idx).copied() {
            Some(f) => f,
            None => {
                return ctx.set_error(LangError::Engine(format!(
                    "no compiled v2 function for closure body {}",
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

        let saved_ref = ctx.current_ref;
        ctx.current_ref = body_idx;

        // Closure bodies receive a boxed Null as incoming (fresh computation).
        let null_incoming = box_value(Value::Null);
        let result = func(
            ctx as *mut EvalCtx as *mut u8,
            null_incoming as *mut u8,
        ) as *mut Value;
        // Reclaim the null incoming if it wasn't consumed.
        // The body function returns a boxed result; the null incoming leaks
        // if not consumed — acceptable per the v1 safety model.

        ctx.current_ref = saved_ref;
        ctx.env = saved_env;

        result
    }
}

/// `bind` prim: arrow-calculus env extension (ADR-0052). Mirrors the v2
/// tree-walk's `prim_bind`. The `bind` node's payload has `name` and
/// `children: [value_idx, body_idx]`. Evaluates the value child, pushes a
/// scope with `name = value`, evaluates the body child in the extended env,
/// then pops the scope.
fn prim_v2_bind(ctx: &mut EvalCtx) -> *mut Value {
    // Collect payload fields first, dropping the immutable `current_node`
    // borrow before the mutable `eval_child_node` calls below.
    let (name, value_idx, body_idx): (String, usize, usize) = match ctx.current_node() {
        Ok(node) => {
            let name = match node.payload.get("name").and_then(|v| v.as_str()) {
                Some(n) => n.to_string(),
                None => {
                    return ctx.set_error(LangError::Engine(
                        "bind: missing name".to_string(),
                    ));
                }
            };
            let children = match node.payload.get("children").and_then(|c| c.as_array()) {
                Some(c) => c,
                None => {
                    return ctx.set_error(LangError::Engine(
                        "bind: expected children array in payload".to_string(),
                    ));
                }
            };
            let value_idx = match children.first().and_then(|v| v.as_u64()) {
                Some(i) => i as usize,
                None => {
                    return ctx.set_error(LangError::Engine(
                        "bind: missing value child".to_string(),
                    ));
                }
            };
            let body_idx = match children.get(1).and_then(|v| v.as_u64()) {
                Some(i) => i as usize,
                None => {
                    return ctx.set_error(LangError::Engine(
                        "bind: missing body child".to_string(),
                    ));
                }
            };
            (name, value_idx, body_idx)
        }
        Err(e) => return ctx.set_error(e),
    };
    // Evaluate the value in the current env.
    let value = match eval_child_node(ctx, value_idx) {
        Ok(v) => v,
        Err(e) => return ctx.set_error(e),
    };
    // Extend env with name = value, evaluate body, pop.
    ctx.env.push_scope();
    ctx.env.define(&name, value);
    let result = eval_child_node(ctx, body_idx);
    ctx.env.pop_scope();
    match result {
        Ok(v) => unsafe { box_value(v) },
        Err(e) => ctx.set_error(e),
    }
}

/// `loop_step` derived step op (ADR-0052 §"Derived forms"): the ergonomic
/// `loop` surface lowering. `args = [body_proc]` is the body closure
/// (evaluated from the carried sub-scheme arg); `incoming` is the loop pair
/// `(in, state)` from the enclosing `Loop` combinator.
///
/// The body closure is applied to the current state (`in` on the first step,
/// `state` thereafter) via [`apply_closure_value`]. It must return an object:
/// - `{ out: v }`    → `either_left(v)`  (terminate with `v`)
/// - `{ state: v }`  → `either_right(v)` (continue with new state `v`)
///
/// Param destructuring: if the body closure has >1 params and the current state
/// is an object, bind each param to the corresponding field by name. Mirrors
/// the v2 tree-walk's `eval_derived_step` `loop_step` arm exactly.
fn prim_v2_loop_step(
    ctx: &mut EvalCtx,
    args: &[Value],
    incoming: Option<Value>,
) -> *mut Value {
    let inc = incoming.unwrap_or(Value::Null);
    let in_val = inc
        .get("__pair__")
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.first())
        .cloned()
        .unwrap_or(Value::Null);
    let state = inc
        .get("__pair__")
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.get(1))
        .cloned()
        .unwrap_or(Value::Null);
    let body_proc = match args.first() {
        Some(b) => b.clone(),
        None => {
            return ctx.set_error(LangError::Engine(
                "loop_step: missing body closure".to_string(),
            ));
        }
    };
    let current = if state.is_null() { in_val } else { state };
    let call_args: Vec<Value> = if let Some(obj_state) = current.as_object() {
        if let Some(closure_obj) = body_proc.as_object()
            && let Some(params) = closure_obj.get("params").and_then(|v| v.as_array())
            && params.len() > 1
        {
            params
                .iter()
                .map(|p| {
                    let name = p.as_str().unwrap_or("_");
                    obj_state.get(name).cloned().unwrap_or(Value::Null)
                })
                .collect()
        } else {
            vec![current]
        }
    } else {
        vec![current]
    };
    let result_ptr = apply_closure_value(ctx, body_proc, call_args);
    if result_ptr.is_null() {
        // Error already set on ctx by apply_closure_value.
        return std::ptr::null_mut();
    }
    let result = unsafe { take_value(result_ptr) };
    let obj = match result.as_object() {
        Some(o) => o,
        None => {
            return ctx.set_error(LangError::Engine(
                "loop_step: body must return { out: v } or { state: v }".to_string(),
            ));
        }
    };
    if let Some(out) = obj.get("out") {
        unsafe { box_value(serde_json::json!({"__either__": "left", "value": out})) }
    } else if let Some(new_state) = obj.get("state") {
        unsafe { box_value(serde_json::json!({"__either__": "right", "value": new_state})) }
    } else {
        ctx.set_error(LangError::Engine(
            "loop_step: body must return { out: v } or { state: v }".to_string(),
        ))
    }
}

/// Call a foreign (host) function (by name index).
///
/// Delegates to the v1 [`helper_call_foreign`] logic (identical behavior —
/// foreign dispatch does not use the incoming value).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_call_foreign(
    ctx: *mut EvalCtx,
    name_idx: usize,
    args: *const *mut Value,
    n: usize,
) -> *mut Value {
    unsafe { helper_call_foreign(ctx, name_idx, args, n) }
}

/// Construct a pair value `{"__pair__": [a, b]}` from two value pointers.
///
/// Both input pointers are consumed (reclaimed).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_pair(
    _ctx: *mut EvalCtx,
    a: *mut Value,
    b: *mut Value,
) -> *mut Value {
    unsafe {
        let a_val = if a.is_null() { Value::Null } else { take_value(a) };
        let b_val = if b.is_null() { Value::Null } else { take_value(b) };
        box_value(serde_json::json!({"__pair__": [a_val, b_val]}))
    }
}

/// Extract the first component of a pair value.
///
/// The input pointer is consumed (reclaimed). Returns a boxed clone of the
/// first component. If the input is not a pair, sets an error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_unpair_fst(
    ctx: *mut EvalCtx,
    val: *mut Value,
) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        if val.is_null() {
            return ctx.set_error(LangError::Engine("unpair_fst: null input".to_string()));
        }
        let v = take_value(val);
        match v.get("__pair__").and_then(|p| p.as_array()).and_then(|arr| arr.first()) {
            Some(fst) => box_value(fst.clone()),
            None => ctx.set_error(LangError::Engine("unpair_fst: input is not a pair".to_string())),
        }
    }
}

/// Extract the second component of a pair value.
///
/// The input pointer is consumed (reclaimed). Returns a boxed clone of the
/// second component. If the input is not a pair, sets an error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_unpair_snd(
    ctx: *mut EvalCtx,
    val: *mut Value,
) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        if val.is_null() {
            return ctx.set_error(LangError::Engine("unpair_snd: null input".to_string()));
        }
        let v = take_value(val);
        match v.get("__pair__").and_then(|p| p.as_array()).and_then(|arr| arr.get(1)) {
            Some(snd) => box_value(snd.clone()),
            None => ctx.set_error(LangError::Engine("unpair_snd: input is not a pair".to_string())),
        }
    }
}

/// Construct a `left` Either value `{"__either__": "left", "value": v}`.
///
/// The input pointer is consumed (reclaimed).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_either_left(
    _ctx: *mut EvalCtx,
    val: *mut Value,
) -> *mut Value {
    unsafe {
        let v = if val.is_null() { Value::Null } else { take_value(val) };
        box_value(serde_json::json!({"__either__": "left", "value": v}))
    }
}

/// Construct a `right` Either value `{"__either__": "right", "value": v}`.
///
/// The input pointer is consumed (reclaimed).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_either_right(
    _ctx: *mut EvalCtx,
    val: *mut Value,
) -> *mut Value {
    unsafe {
        let v = if val.is_null() { Value::Null } else { take_value(val) };
        box_value(serde_json::json!({"__either__": "right", "value": v}))
    }
}

/// Check whether a value is a `left` Either.
///
/// Returns 1 if the value is `{"__either__": "left", ...}`, 0 otherwise. The
/// input pointer is NOT consumed (it is read-only).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn helper_v2_either_is_left(
    _ctx: *mut EvalCtx,
    val: *mut Value,
) -> u8 {
    unsafe {
        if val.is_null() {
            return 0;
        }
        let v = &*val;
        if v.get("__either__").and_then(|s| s.as_str()) == Some("left") {
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
    val: *mut Value,
) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        if val.is_null() {
            return ctx.set_error(LangError::Engine("either_value: null input".to_string()));
        }
        let v = take_value(val);
        match v.get("value") {
            Some(inner) => box_value(inner.clone()),
            None => ctx.set_error(LangError::Engine(
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
    pair: *mut Value,
    op_idx: usize,
) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        if op_idx >= ctx.names.len() {
            return ctx.set_error(LangError::Engine(format!(
                "derived_step_arg_incoming: op index {} out of bounds",
                op_idx
            )));
        }
        let op = ctx.names[op_idx].clone();
        if pair.is_null() {
            return box_value(Value::Null);
        }
        let p = (*pair).clone();
        let in_val = p
            .get("__pair__")
            .and_then(|arr| arr.as_array())
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null);
        let state = p
            .get("__pair__")
            .and_then(|arr| arr.as_array())
            .and_then(|a| a.get(1))
            .cloned()
            .unwrap_or(Value::Null);
        let arg_incoming = if op == "for_step" {
            if state.is_null() {
                // First step: in_val = pair(count, initial). acc = snd(in_val).
                in_val
                    .get("__pair__")
                    .and_then(|arr| arr.as_array())
                    .and_then(|a| a.get(1))
                    .cloned()
                    .unwrap_or(Value::Null)
            } else {
                // Later: state = pair(counter, acc). acc = snd(state).
                state
                    .get("__pair__")
                    .and_then(|arr| arr.as_array())
                    .and_then(|a| a.get(1))
                    .cloned()
                    .unwrap_or(Value::Null)
            }
        } else {
            // while_step / fix_step: current = in (first step) or state.
            if state.is_null() { in_val } else { state }
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
    args: *const *mut Value,
    n: usize,
    incoming: *mut Value,
) -> *mut Value {
    unsafe {
        let ctx = &mut *ctx;
        if op_idx >= ctx.names.len() {
            for i in 0..n {
                let p = *args.add(i);
                if !p.is_null() {
                    let _ = take_value(p);
                }
            }
            return ctx.set_error(LangError::Engine(format!(
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
pub unsafe extern "C" fn helper_v2_loop_check(
    ctx: *mut EvalCtx,
    result: *mut Value,
) -> u8 {
    unsafe {
        let ctx = &mut *ctx;
        if result.is_null() {
            ctx.set_error(LangError::Engine("loop_check: null result".to_string()));
            return 2; // arbitrary — error is set
        }
        let v = &*result;
        if v.get("__either__").and_then(|s| s.as_str()) == Some("left") {
            0
        } else if v.get("__either__").and_then(|s| s.as_str()) == Some("right") {
            1
        } else if v.get("__pair__").is_some() {
            2
        } else {
            ctx.set_error(LangError::Engine(
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
/// shape). Returns `(params_as_values, body_arena_index)`.
fn resolve_closure_params_body(
    ctx: &mut EvalCtx,
) -> Result<(Vec<Value>, usize), LangError> {
    let node = ctx.current_node()?;
    let payload = &node.payload;
    if let (Some(p), Some(b)) = (
        payload.get("params").and_then(|v| v.as_array()),
        payload.get("body").and_then(|v| v.as_u64()),
    ) {
        return Ok((p.clone(), b as usize));
    }
    let children = payload
        .get("children")
        .and_then(|c| c.as_array())
        .ok_or_else(|| LangError::Engine("closure: missing params/body in payload".to_string()))?;
    let param_idx = children
        .first()
        .and_then(|v| v.as_u64())
        .ok_or_else(|| LangError::Engine("closure: missing param child".to_string()))?
        as usize;
    let body_idx = children
        .get(1)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| LangError::Engine("closure: missing body child".to_string()))?
        as usize;
    let ast = unsafe { &*ctx.ast };
    let param_node = ast.at(AstRef::new(param_idx))?;
    let param_names: Vec<String> = if param_node.kind == "paren"
        || param_node.kind == "array"
    {
        let param_children = param_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                LangError::Engine("closure: params missing children".to_string())
            })?;
        let mut names = Vec::with_capacity(param_children.len());
        for pc in param_children {
            let pidx = pc
                .as_u64()
                .ok_or_else(|| {
                    LangError::Engine("closure: param index must be a number".to_string())
                })?
                as usize;
            let pnode = ast.at(AstRef::new(pidx))?;
            let pname = pnode.payload.as_str().ok_or_else(|| {
                LangError::Engine(
                    "closure: param must be a var with string payload".to_string(),
                )
            })?;
            names.push(pname.to_string());
        }
        names
    } else {
        let pname = param_node.payload.as_str().ok_or_else(|| {
            LangError::Engine(
                "closure: param node must be a var with string payload".to_string(),
            )
        })?;
        vec![pname.to_string()]
    };
    Ok((
        param_names.into_iter().map(Value::from).collect(),
        body_idx,
    ))
}

/// v2 `array` prim: construct an array value from the current node's payload.
///
/// Two payload shapes (mirrors v1 `array` prim):
/// - Rich: `{"elements": [v1, v2, ...]}` — literal values, returned as-is.
/// - Pratt-parsed: `{"children": [elem_idx, ...]}` — each element is an arena
///   index evaluated via [`eval_child_node`].
fn prim_v2_array(ctx: &mut EvalCtx) -> *mut Value {
    let node = match ctx.current_node() {
        Ok(n) => n,
        Err(e) => return ctx.set_error(e),
    };
    if let Some(elements) = node.payload.get("elements") {
        return unsafe { box_value(elements.clone()) };
    }
    let children = match node.payload.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => {
            return ctx.set_error(LangError::Engine(
                "array: missing elements/children in payload".to_string(),
            ))
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
            return ctx.set_error(LangError::Engine(
                "array: element index must be a number".to_string(),
            ))
        }
    };
    let mut result = Vec::with_capacity(indices.len());
    for idx in indices {
        match eval_child_node(ctx, idx) {
            Ok(v) => result.push(v),
            Err(e) => return ctx.set_error(e),
        }
    }
    unsafe { box_value(Value::Array(result)) }
}

/// v2 `object` prim: construct an object value from the current node's payload.
///
/// Two payload shapes (mirrors v1 `object` prim):
/// - Rich: `{"entries": [{"key": "k", "value": v}, ...]}`.
/// - Pratt-parsed: `{"children": [entry_idx, ...]}` where each entry node has
///   `{"children": [key_idx, value_idx]}`.
fn prim_v2_object(ctx: &mut EvalCtx) -> *mut Value {
    let node = match ctx.current_node() {
        Ok(n) => n,
        Err(e) => return ctx.set_error(e),
    };
    if let Some(entries) = node.payload.get("entries").and_then(|v| v.as_array()) {
        let mut obj = serde_json::Map::new();
        for entry in entries {
            let key = match entry.get("key").and_then(|v| v.as_str()) {
                Some(k) => k.to_string(),
                None => {
                    return ctx.set_error(LangError::Engine(
                        "object: entry missing key".to_string(),
                    ))
                }
            };
            let value = match entry.get("value") {
                Some(v) => v.clone(),
                None => {
                    return ctx.set_error(LangError::Engine(
                        "object: entry missing value".to_string(),
                    ))
                }
            };
            if handl_language::is_reserved_value_key(&key) {
                return ctx.set_error(LangError::Engine(format!(
                    "object: key '{}' is reserved",
                    key
                )));
            }
            obj.insert(key, value);
        }
        return unsafe { box_value(Value::Object(obj)) };
    }
    let children = match node.payload.get("children").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => {
            return ctx.set_error(LangError::Engine(
                "object: missing entries/children in payload".to_string(),
            ))
        }
    };
    // Collect entry indices, then for each entry collect (key_idx, val_idx),
    // releasing borrows before evaluating children.
    let entry_indices: Vec<usize> = match children
        .iter()
        .map(|c| c.as_u64().map(|i| i as usize))
        .collect::<Option<Vec<_>>>()
    {
        Some(v) => v,
        None => {
            return ctx.set_error(LangError::Engine(
                "object: entry index must be a number".to_string(),
            ))
        }
    };
    let ast = unsafe { &*ctx.ast };
    // Collect (key_idx, val_idx, direct_key) for each entry. Releasing
    // borrows before evaluating children below.
    let mut entries: Vec<(usize, usize, Option<String>)> =
        Vec::with_capacity(entry_indices.len());
    for entry_idx in &entry_indices {
        let entry_node = match ast.at(AstRef::new(*entry_idx)) {
            Ok(n) => n,
            Err(e) => return ctx.set_error(e),
        };
        let entry_children = match entry_node.payload.get("children").and_then(|c| c.as_array())
        {
            Some(c) => c,
            None => {
                return ctx.set_error(LangError::Engine(
                    "object: entry node missing children".to_string(),
                ))
            }
        };
        let key_idx = match entry_children.first().and_then(|v| v.as_u64()) {
            Some(i) => i as usize,
            None => {
                return ctx.set_error(LangError::Engine(
                    "object: entry missing key child".to_string(),
                ))
            }
        };
        let val_idx = match entry_children.get(1).and_then(|v| v.as_u64()) {
            Some(i) => i as usize,
            None => {
                return ctx.set_error(LangError::Engine(
                    "object: entry missing value child".to_string(),
                ))
            }
        };
        // Object key: if the key node is a `var` (bare identifier) or
        // `string`, use its payload string directly — don't evaluate it.
        // Otherwise (computed key), evaluate the key expression and use
        // its string representation. Bracket computed keys `[expr]:` are
        // desugared to `paren { expr }` before eval (ADR-0058), so they
        // fall into the `None` (evaluate) branch. Mirrors the v2
        // tree-walk's object prim.
        let key_node = match ast.at(AstRef::new(key_idx)) {
            Ok(n) => n,
            Err(e) => return ctx.set_error(e),
        };
        let direct_key = if key_node.kind == "var" || key_node.kind == "string" {
            key_node.payload.as_str().map(String::from)
        } else {
            None
        };
        entries.push((key_idx, val_idx, direct_key));
    }
    let mut obj = serde_json::Map::new();
    for (key_idx, val_idx, direct_key) in entries {
        let key = match direct_key {
            Some(k) => k,
            None => {
                let key_val = match eval_child_node(ctx, key_idx) {
                    Ok(v) => v,
                    Err(e) => return ctx.set_error(e),
                };
                match key_val.as_str() {
                    Some(k) => k.to_string(),
                    None => key_val.to_string(),
                }
            }
        };
        let value = match eval_child_node(ctx, val_idx) {
            Ok(v) => v,
            Err(e) => return ctx.set_error(e),
        };
        if handl_language::is_reserved_value_key(&key) {
            return ctx.set_error(LangError::Engine(format!(
                "object: key '{}' is reserved",
                key
            )));
        }
        obj.insert(key, value);
    }
    unsafe { box_value(Value::Object(obj)) }
}

/// v2 `index` prim: index into an array or object using pre-evaluated
/// `target` and `index` values from the current node's payload.
///
/// - `(Array, Number)` → `arr[i]`.
/// - `(Object, String)` → `obj[key]`.
fn prim_v2_index(ctx: &mut EvalCtx) -> *mut Value {
    let node = match ctx.current_node() {
        Ok(n) => n,
        Err(e) => return ctx.set_error(e),
    };
    let target = match node.payload.get("target") {
        Some(t) => t,
        None => {
            return ctx.set_error(LangError::Engine(
                "index: missing target in payload".to_string(),
            ))
        }
    };
    let index = match node.payload.get("index") {
        Some(i) => i,
        None => {
            return ctx.set_error(LangError::Engine(
                "index: missing index in payload".to_string(),
            ))
        }
    };
    let result = match (target, index) {
        (Value::Array(arr), Value::Number(n)) => {
            let i = match n.as_u64() {
                Some(i) => i as usize,
                None => {
                    return ctx.set_error(LangError::Engine(
                        "index: index must be a non-negative integer".to_string(),
                    ))
                }
            };
            match arr.get(i).cloned() {
                Some(v) => Ok(v),
                None => Err(LangError::Engine(format!(
                    "index: index {} out of bounds",
                    i
                ))),
            }
        }
        (Value::Object(obj), Value::String(key)) => match obj.get(key).cloned() {
            Some(v) => Ok(v),
            None => Err(LangError::Engine(format!(
                "index: key '{}' not found",
                key
            ))),
        },
        _ => Err(LangError::Engine(
            "index: target must be array/object and index must be number/string".to_string(),
        )),
    };
    match result {
        Ok(v) => unsafe { box_value(v) },
        Err(e) => ctx.set_error(e),
    }
}

/// v2 `assign` prim: evaluate and return the assigned value.
///
/// ADR-0053: `assign { children: [var, value] }` — evaluate child 1 (the
/// value expression) and return it. Falls back to legacy `payload.value`
/// for hand-built ASTs.
fn prim_v2_assign(ctx: &mut EvalCtx) -> *mut Value {
    let node = match ctx.current_node() {
        Ok(n) => n,
        Err(e) => return ctx.set_error(e),
    };
    if let Some(children) = node.payload.get("children").and_then(|c| c.as_array()) {
        if children.len() >= 2 {
            let value_idx = match children[1].as_u64() {
                Some(idx) => idx as usize,
                None => {
                    return ctx.set_error(LangError::Engine(
                        "assign: value index must be a number".to_string(),
                    ));
                }
            };
            let result = eval_child_node(ctx, value_idx);
            return match result {
                Ok(v) => unsafe { box_value(v) },
                Err(e) => ctx.set_error(e),
            };
        }
    }
    match node.payload.get("value") {
        Some(v) => unsafe { box_value(v.clone()) },
        None => ctx.set_error(LangError::Engine(
            "assign: missing value in payload".to_string(),
        )),
    }
}

/// Apply a v2 native op (shared logic). Mirrors the v2 tree-walk's
/// `apply_native` exactly.
fn apply_native_v2(
    ctx: &mut EvalCtx,
    op: &str,
    args: &[Value],
    incoming: Option<Value>,
) -> *mut Value {
    let result = match op {
        "id" => Ok(incoming
            .or_else(|| args.first().cloned())
            .unwrap_or(Value::Null)),
        "tag" => {
            let v = incoming
                .or_else(|| args.first().cloned())
                .unwrap_or(Value::Null);
            if is_truthy(&v) {
                Ok(serde_json::json!({"__either__": "left", "value": v}))
            } else {
                Ok(serde_json::json!({"__either__": "right", "value": v}))
            }
        }
        "closure" => {
            match resolve_closure_params_body(ctx) {
                Ok((params, body)) => {
                    let captured = ctx.env.capture();
                    Ok(serde_json::json!({
                        "__arrow__": true,
                        "params": params,
                        "body": body,
                        "env": captured,
                    }))
                }
                Err(e) => Err(e),
            }
        }
        "bind" => return prim_v2_bind(ctx),
        // Ergonomic `loop` surface lowering (ADR-0052). args = [body_proc]
        // — the body closure, evaluated from the carried sub-scheme
        // arg. `incoming` is the loop pair `(in, state)` passed by the
        // enclosing `Loop` combinator. The body closure is applied to the
        // current state (in on the first step, state thereafter); it returns
        // an object `{ out: v }` (terminate) or `{ state: v }` (continue).
        // Param destructuring mirrors the v2 tree-walk's `loop_step` arm.
        "loop_step" => return prim_v2_loop_step(ctx, args, incoming),
        // `while_step` derived step op (ADR-0052). args = [cond_result,
        // body_result] — re-evaluated per iteration by the compiler with
        // the current value as incoming. `incoming` is the loop pair.
        "while_step" => {
            let inc = incoming.unwrap_or(Value::Null);
            let in_val = inc
                .get("__pair__")
                .and_then(|p| p.as_array())
                .and_then(|a| a.first())
                .cloned()
                .unwrap_or(Value::Null);
            let state = inc
                .get("__pair__")
                .and_then(|p| p.as_array())
                .and_then(|a| a.get(1))
                .cloned()
                .unwrap_or(Value::Null);
            let current = if state.is_null() { in_val } else { state };
            let cond_val = args.first().cloned().unwrap_or(Value::Null);
            if is_truthy(&cond_val) {
                let body_val = args.get(1).cloned().unwrap_or(Value::Null);
                Ok(serde_json::json!({"__either__": "right", "value": body_val}))
            } else {
                Ok(serde_json::json!({"__either__": "left", "value": current}))
            }
        }
        // `for_step` derived step op. args = [body_result]. State is
        // pair(counter, acc). First step: initialize from in = pair(count,
        // initial). While counter > 0, continue with pair(counter-1,
        // body(acc)); else terminate with acc.
        "for_step" => {
            let inc = incoming.unwrap_or(Value::Null);
            let in_val = inc
                .get("__pair__")
                .and_then(|p| p.as_array())
                .and_then(|a| a.first())
                .cloned()
                .unwrap_or(Value::Null);
            let state = inc
                .get("__pair__")
                .and_then(|p| p.as_array())
                .and_then(|a| a.get(1))
                .cloned()
                .unwrap_or(Value::Null);
            let (counter, acc) = if state.is_null() {
                let count = in_val
                    .get("__pair__")
                    .and_then(|p| p.as_array())
                    .and_then(|a| a.first())
                    .cloned()
                    .unwrap_or(Value::Null);
                let initial = in_val
                    .get("__pair__")
                    .and_then(|p| p.as_array())
                    .and_then(|a| a.get(1))
                    .cloned()
                    .unwrap_or(Value::Null);
                (count, initial)
            } else {
                let c = state
                    .get("__pair__")
                    .and_then(|p| p.as_array())
                    .and_then(|a| a.first())
                    .cloned()
                    .unwrap_or(Value::Null);
                let a = state
                    .get("__pair__")
                    .and_then(|p| p.as_array())
                    .and_then(|a| a.get(1))
                    .cloned()
                    .unwrap_or(Value::Null);
                (c, a)
            };
            let count_i = counter.as_i64();
            let count_f = counter.as_f64();
            let still_going = match (count_i, count_f) {
                (Some(i), _) => i > 0,
                (None, Some(f)) => f > 0.0,
                (None, None) => false,
            };
            if still_going {
                let body_val = args.first().cloned().unwrap_or(Value::Null);
                let new_counter = match count_i {
                    Some(i) => Value::from(i - 1),
                    None => Value::from(count_f.unwrap_or(0.0) - 1.0),
                };
                let new_pair = serde_json::json!({"__pair__": [new_counter, body_val]});
                Ok(serde_json::json!({"__either__": "right", "value": new_pair}))
            } else {
                Ok(serde_json::json!({"__either__": "left", "value": acc}))
            }
        }
        // `fix_step` derived step op. args = [body_result]. If body result
        // equals current, terminate (fixpoint); else continue.
        "fix_step" => {
            let inc = incoming.unwrap_or(Value::Null);
            let in_val = inc
                .get("__pair__")
                .and_then(|p| p.as_array())
                .and_then(|a| a.first())
                .cloned()
                .unwrap_or(Value::Null);
            let state = inc
                .get("__pair__")
                .and_then(|p| p.as_array())
                .and_then(|a| a.get(1))
                .cloned()
                .unwrap_or(Value::Null);
            let current = if state.is_null() { in_val } else { state };
            let new_val = args.first().cloned().unwrap_or(Value::Null);
            if new_val == current {
                Ok(serde_json::json!({"__either__": "left", "value": new_val}))
            } else {
                Ok(serde_json::json!({"__either__": "right", "value": new_val}))
            }
        }
        "array" => {
            // Two payload shapes (mirrors v1 `array` prim):
            // - Rich: `{"elements": [v1, v2, ...]}` — literal values.
            // - Pratt-parsed: `{"children": [elem_idx, ...]}` — each
            //   element is an arena index to evaluate.
            return prim_v2_array(ctx);
        }
        "object" => {
            // Two payload shapes (mirrors v1 `object` prim):
            // - Rich: `{"entries": [{"key": "k", "value": v}, ...]}`.
            // - Pratt-parsed: `{"children": [entry_idx, ...]}` where each
            //   entry node has `{"children": [key_idx, value_idx]}`.
            return prim_v2_object(ctx);
        }
        "index" => {
            return prim_v2_index(ctx);
        }
        "assign" => {
            return prim_v2_assign(ctx);
        }
        "json" => {
            // ADR-0060: json node payload is the JSON template string. When
            // the json node has children (desugared from json{...}(args)),
            // child 0 is the template node and children 1..N are args.
            let node = match ctx.current_node() {
                Ok(n) => n,
                Err(e) => return ctx.set_error(e),
            };
            // Clone children to release the borrow on ctx before we
            // mutate ctx.current_ref in the loop below.
            let children_opt = node.payload.get("children").and_then(|c| c.as_array()).cloned();
            let payload_str = node.payload.as_str().map(|s| s.to_string());
            // Check if the json node has children (desugared from call).
            if let Some(children) = children_opt {
                if children.len() >= 2 {
                    let template_idx = match children[0].as_u64() {
                        Some(v) => v as usize,
                        None => return ctx.set_error(LangError::Engine("json: child index must be a number".to_string())),
                    };
                    let template_node = match ctx.ast_at(AstRef::new(template_idx)) {
                        Ok(n) => n,
                        Err(e) => return ctx.set_error(e),
                    };
                    let template = match template_node.payload.as_str() {
                        Some(s) => s.to_string(),
                        None => return ctx.set_error(LangError::Engine("json: template payload must be a string".to_string())),
                    };
                    let mut s = template;
                    let saved_ref = ctx.current_ref;
                    for (i, entry) in children[1..].iter().enumerate() {
                        let idx = match entry.as_u64() {
                            Some(v) => v as usize,
                            None => return ctx.set_error(LangError::Engine("json: child index must be a number".to_string())),
                        };
                        let func = match ctx.compiled.get(&idx).copied() {
                            Some(f) => f,
                            None => return ctx.set_error(LangError::Engine(format!("json: no compiled function for node {}", idx))),
                        };
                        ctx.current_ref = idx;
                        let ctx_ptr = ctx as *mut EvalCtx as *mut u8;
                        let result_ptr = unsafe { func(ctx_ptr, std::ptr::null_mut()) as *mut Value };
                        let val = unsafe { (*result_ptr).clone() };
                        let replacement = match serde_json::to_string(&val) {
                            Ok(r) => r,
                            Err(e) => return ctx.set_error(LangError::Engine(format!("json: {}", e))),
                        };
                        s = s.replace(&format!("${}", i), &replacement);
                    }
                    ctx.current_ref = saved_ref;
                    return match serde_json::from_str(&s) {
                        Ok(v) => unsafe { box_value(v) },
                        Err(e) => ctx.set_error(LangError::Engine(format!("json: parse error: {}", e))),
                    };
                }
            }
            match payload_str {
                Some(expr) => match serde_json::from_str(&expr) {
                    Ok(v) => Ok(v),
                    Err(e) => Err(LangError::Engine(format!("json: parse error: {}", e))),
                },
                None => Err(LangError::Engine("json: payload must be a string".to_string())),
            }
        }
        "add" => bin_num_v2(args, |a, b| a + b),
        "sub" => bin_num_v2(args, |a, b| a - b),
        "mul" => bin_num_v2(args, |a, b| a * b),
        "div" => {
            let a = as_num(&args[0]);
            let b = as_num(&args[1]);
            match (a, b) {
                (Ok(a), Ok(b)) => {
                    if b.to_f64() == 0.0 {
                        Err(LangError::Engine("division by zero".to_string()))
                    } else {
                        Ok(num_value(a / b))
                    }
                }
                (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
        "mod" => {
            let a = as_num(&args[0]);
            let b = as_num(&args[1]);
            match (a, b) {
                (Ok(a), Ok(b)) => {
                    if b.to_f64() == 0.0 {
                        Err(LangError::Engine("modulo by zero".to_string()))
                    } else {
                        Ok(num_value(match (a, b) {
                            (Num::Int(a), Num::Int(b)) => Num::Int(a % b),
                            (a, b) => Num::Float(a.to_f64() % b.to_f64()),
                        }))
                    }
                }
                (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
        "neg" => {
            match as_num(&args[0]).map(|a| match a {
                Num::Int(i) => num_value(Num::Int(-i)),
                Num::Float(f) => num_value(Num::Float(-f)),
            }) {
                Ok(v) => Ok(v),
                Err(e) => Err(e),
            }
        }
        "eq" => Ok(Value::Bool(args[0] == args[1])),
        "neq" => Ok(Value::Bool(args[0] != args[1])),
        "lt" => cmp_num_v2(args, |a, b| a < b),
        "gt" => cmp_num_v2(args, |a, b| a > b),
        "lte" => cmp_num_v2(args, |a, b| a <= b),
        "gte" => cmp_num_v2(args, |a, b| a >= b),
        "pow" => {
            let a = match as_num(&args[0]) {
                Ok(v) => v,
                Err(e) => return ctx.set_error(e),
            };
            let b = match as_num(&args[1]) {
                Ok(v) => v,
                Err(e) => return ctx.set_error(e),
            };
            Ok(num_value(Num::Float(a.to_f64().powf(b.to_f64()))))
        }
        "compare" => {
            let a = match as_num(&args[0]) {
                Ok(v) => v,
                Err(e) => return ctx.set_error(e),
            };
            let b = match as_num(&args[1]) {
                Ok(v) => v,
                Err(e) => return ctx.set_error(e),
            };
            match a.to_f64().partial_cmp(&b.to_f64()) {
                Some(cmp) => Ok(num_value(Num::Int(match cmp {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                }))),
                None => Err(LangError::Engine(
                    "compare: incomparable values".to_string(),
                )),
            }
        }
        // ADR-0060: route_on_type — encoded as "route_on_type:<tag>" in the
        // op name by the cranelift compiler. Takes the value from incoming
        // (or args[0]) and returns either_left/either_right.
        op if op.starts_with("route_on_type:") => {
            let tag = &op["route_on_type:".len()..];
            let val = incoming
                .or_else(|| args.first().cloned())
                .unwrap_or(Value::Null);
            let actual_tag = match &val {
                Value::Null => "null",
                Value::Bool(_) => "bool",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };
            if actual_tag == tag {
                Ok(serde_json::json!({"__either__": "left", "value": val}))
            } else {
                Ok(serde_json::json!({"__either__": "right", "value": val}))
            }
        }
        // ADR-0060: call_host — takes a string callee (args[0]) and an args
        // array (args[1]), looks up the callee in the module registry.
        "call_host" => {
            let callee = match args.first().and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return ctx.set_error(LangError::Engine(
                    "call_host: callee must be a string".to_string(),
                )),
            };
            let modules_ptr = match ctx.modules {
                Some(p) => p,
                None => return ctx.set_error(LangError::Engine(
                    "call_host: no module registry".to_string(),
                )),
            };
            let modules: &dyn handl_platform::ModuleLookup = unsafe { &*modules_ptr };
            let func = match modules.lookup(callee) {
                Some(f) => f,
                None => return ctx.set_error(LangError::Engine(format!(
                    "call_host: function '{}' not found",
                    callee
                ))),
            };
            let call_args: Vec<Value> = args
                .get(1)
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().cloned().collect())
                .unwrap_or_default();
            let mut pctx = handl_platform::EvalContext::new();
            func.call(&call_args, &mut pctx)
                .map_err(|e| LangError::Engine(e.to_string()))
        }
        // ADR-0060: proj_rest — evaluate all children after index 0 and
        // return them as a JSON array.
        "proj_rest" => {
            let children = match ctx.current_node() {
                Ok(node) => match node.payload.get("children").and_then(|c| c.as_array()) {
                    Some(c) => c.clone(),
                    None => return ctx.set_error(LangError::Engine(
                        "proj_rest: expected children array in payload".to_string(),
                    )),
                },
                Err(e) => return ctx.set_error(e),
            };
            let mut vals = Vec::new();
            let saved_ref = ctx.current_ref;
            for entry in children.iter().skip(1) {
                let idx = match entry.as_u64() {
                    Some(i) => i as usize,
                    None => return ctx.set_error(LangError::Engine(
                        "proj_rest: child index must be a number".to_string(),
                    )),
                };
                let func = match ctx.compiled.get(&idx).copied() {
                    Some(f) => f,
                    None => return ctx.set_error(LangError::Engine(format!(
                        "proj_rest: no compiled function for node {}",
                        idx
                    ))),
                };
                ctx.current_ref = idx;
                let ctx_ptr = ctx as *mut EvalCtx as *mut u8;
                let result_ptr = unsafe { func(ctx_ptr, std::ptr::null_mut()) as *mut Value };
                let val = unsafe { (*result_ptr).clone() };
                vals.push(val);
            }
            ctx.current_ref = saved_ref;
            Ok(Value::Array(vals))
        }
        "typeof" => {
            let arg = args.first().or(incoming.as_ref()).unwrap_or(&Value::Null);
            let type_name = match arg {
                Value::Null => "null",
                Value::Bool(_) => "bool",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };
            Ok(Value::from(type_name))
        }
        "not" => Ok(Value::Bool(!is_truthy(&args[0]))),
        "and" => Ok(Value::Bool(is_truthy(&args[0]) && is_truthy(&args[1]))),
        "or" => Ok(Value::Bool(is_truthy(&args[0]) || is_truthy(&args[1]))),
        "is_pos" => {
            let v = incoming.unwrap_or(Value::Null);
            match as_num(&v).map(|n| Value::Bool(n > Num::Int(0))) {
                Ok(v) => Ok(v),
                Err(e) => Err(e),
            }
        }
        "dec" => {
            let v = incoming.unwrap_or(Value::Null);
            match as_num(&v).map(|n| num_value(n - Num::Int(1))) {
                Ok(v) => Ok(v),
                Err(e) => Err(e),
            }
        }
        "inc" => {
            let v = incoming.unwrap_or(Value::Null);
            match as_num(&v).map(|n| num_value(n + Num::Int(1))) {
                Ok(v) => Ok(v),
                Err(e) => Err(e),
            }
        }
        "half_floor" => {
            let v = incoming.unwrap_or(Value::Null);
            match as_num(&v).map(|n| match n {
                Num::Int(i) => num_value(Num::Int(i / 2)),
                Num::Float(f) => num_value(Num::Float((f / 2.0).floor())),
            }) {
                Ok(v) => Ok(v),
                Err(e) => Err(e),
            }
        }
        "inc_fst" => {
            let inc = args.first().cloned().or(incoming).unwrap_or(Value::Null);
            let pair_arr = inc.get("__pair__").and_then(|p| p.as_array());
            let a = pair_arr.and_then(|arr| arr.first()).cloned();
            let b = pair_arr.and_then(|arr| arr.get(1)).cloned();
            match (a, b) {
                (Some(a), Some(b)) => match as_num(&a).map(|a_n| {
                    let new_a = num_value(a_n + Num::Int(1));
                    serde_json::json!({"__pair__": [new_a, b]})
                }) {
                    Ok(v) => Ok(v),
                    Err(e) => Err(e),
                },
                _ => Err(LangError::Engine("inc_fst: input is not a pair".to_string())),
            }
        }
        "countdown_step" => {
            let inc = args.first().cloned().or(incoming).unwrap_or(Value::Null);
            let outer_arr = inc.get("__pair__").and_then(|p| p.as_array());
            let a = outer_arr.and_then(|arr| arr.first()).cloned();
            let b = outer_arr.and_then(|arr| arr.get(1)).cloned().unwrap_or(Value::Null);
            let inner_arr = a.as_ref().and_then(|v| v.get("__pair__")).and_then(|p| p.as_array());
            let result_val = inner_arr.and_then(|arr| arr.first()).cloned();
            let initial = inner_arr.and_then(|arr| arr.get(1)).cloned().unwrap_or(Value::Null);
            match result_val {
                Some(result_val) => {
                    let state = if b.is_null() { initial } else { b };
                    match as_num(&state).map(|n| {
                        if n > Num::Int(0) {
                            serde_json::json!({"__either__": "right", "value": num_value(n - Num::Int(1))})
                        } else {
                            serde_json::json!({"__either__": "left", "value": result_val})
                        }
                    }) {
                        Ok(v) => Ok(v),
                        Err(e) => Err(e),
                    }
                }
                None => Err(LangError::Engine(
                    "countdown_step: in must be pair(result, initial)".to_string(),
                )),
            }
        }
        "infinite_step" => {
            let inc = args.first().cloned().or(incoming).unwrap_or(Value::Null);
            let b = inc
                .get("__pair__")
                .and_then(|p| p.as_array())
                .and_then(|arr| arr.get(1))
                .cloned()
                .unwrap_or(Value::Null);
            Ok(serde_json::json!({"__either__": "right", "value": b}))
        }
        // ADR-0053: the data-driven `if → match` desugar rewrites `if`
        // nodes into `match` nodes before eval, so the v2 compiled path
        // must dispatch `match`/`handle` to the existing prims (which
        // read the current node from `ctx` and walk the arm children).
        "match" => return prim_match(ctx),
        "handle" => return prim_handle(ctx),
        _ => Err(LangError::Engine(format!("unknown v2 native op: {}", op))),
    };

    match result {
        Ok(v) => unsafe { box_value(v) },
        Err(e) => ctx.set_error(e),
    }
}

/// Binary numeric op helper (v2). Preserves int-int arithmetic.
fn bin_num_v2<F>(args: &[Value], f: F) -> Result<Value, LangError>
where
    F: FnOnce(Num, Num) -> Num,
{
    let a = as_num(&args[0])?;
    let b = as_num(&args[1])?;
    Ok(num_value(f(a, b)))
}

/// Numeric comparison helper (v2).
fn cmp_num_v2<F>(args: &[Value], f: F) -> Result<Value, LangError>
where
    F: FnOnce(Num, Num) -> bool,
{
    let a = as_num(&args[0])?;
    let b = as_num(&args[1])?;
    Ok(Value::Bool(f(a, b)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstNode, EvalSchemeDef, Scheme};
    use handl_platform::{EvalContext, Function, Module, ModuleLookup};
    use handl_plugin_engine_eval::EvalRequest;
    use handl_plugin_module::ModuleRegistry;
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
                Scheme::arr_native_lit("const", vec![], json!(99)),
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
                Scheme::arr_native_lit("const", vec![], Value::Null),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("x".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                        Scheme::arr_native_lit("extend_env", vec![], Value::String("x".into())),
                    ),
                    Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                ),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                        Scheme::arr_native_lit("extend_env", vec![], Value::String("$payload".into())),
                    ),
                    Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                ),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("unknown".into()))))
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
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                        Scheme::arr_native_lit("extend_env", vec![], Value::String("$payload".into())),
                    ),
                    Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                ),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                        Scheme::arr_native_lit("extend_env", vec![], Value::String("$payload".into())),
                    ),
                    Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                ),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.bind",
                "bind",
                Scheme::arr_native("bind", vec![]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(5)),               // 0
            AstNode::new("lit").with_payload(json!(1)),               // 1
            AstNode::new("var").with_payload(json!("x")),             // 2
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
                Scheme::seq(
                    Scheme::seq(Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native("tag", vec![])),
                    Scheme::choice(Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(2u64))),
                ),
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
                Scheme::seq(
                    Scheme::seq(Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native("tag", vec![])),
                    Scheme::choice(Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(2u64))),
                ),
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
                Scheme::seq(
                    Scheme::seq(Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native("tag", vec![])),
                    Scheme::choice(Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(2u64))),
                ),
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
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
                Scheme::arr_native("sub", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
                Scheme::arr_native("mul", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
                Scheme::arr_native("div", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
                Scheme::arr_native("div", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(0)),
            AstNode::new("binary.div").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn engine_prim_eq() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.eq",
                "binary.eq",
                Scheme::arr_native("eq", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
                Scheme::arr_native("lt", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.gt",
                "binary.gt",
                Scheme::arr_native("gt", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
                Scheme::arr_native("not", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64))]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.and",
                "binary.and",
                Scheme::arr_native("and", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.or",
                "binary.or",
                Scheme::arr_native("or", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
                Scheme::arr_native("unknown_op", vec![]),
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
                Scheme::arr_foreign("double", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64))]),
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
                Scheme::arr_foreign("double", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64))]),
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
                Scheme::arr_native("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                Scheme::app(Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                        Scheme::arr_native_lit("extend_env", vec![], Value::String("$payload".into())),
                    ),
                    Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                ),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                Scheme::arr_native("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                Scheme::app(Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                        Scheme::arr_native_lit("extend_env", vec![], Value::String("$payload".into())),
                    ),
                    Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                ),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
            .merge(EvalSchemeDef::new("e.c", "c", Scheme::arr_native_lit("const", vec![], json!(42))))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("c")]);
        let result = compile_all_nodes_jit(&lang, &ast);
        assert!(result.is_ok());
    }

    #[test]
    fn compile_var_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.v", "v", Scheme::arr_native_lit("proj_env", vec![], Value::String("x".into()))))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("v")]);
        let result = compile_all_nodes_jit(&lang, &ast);
        assert!(result.is_ok());
    }

    #[test]
    fn compile_var_payload_succeeds() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.v", "v", Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into()))))
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
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                        Scheme::arr_native_lit("extend_env", vec![], Value::String("$payload".into())),
                    ),
                    Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                ),
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
                Scheme::arr_native("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                Scheme::app(Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
                Scheme::seq(
                    Scheme::seq(Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native("tag", vec![])),
                    Scheme::choice(Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(2u64))),
                ),
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
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
                Scheme::arr_foreign("double", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64))]),
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
    fn compile_version_mismatch_errors() {
        let mut lang = Language::new();
        lang.scheme_version = SchemeVersion::new("v3");
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(42))]);
        match compile_all_nodes_jit(&lang, &ast) {
            Ok(_) => panic!("expected version mismatch error, got Ok"),
            Err(e) => {
                let msg = format!("{}", e);
                assert!(msg.contains("unsupported scheme version"));
            }
        }
    }

    // --- Vau (fexpr) tests (ADR-0041 §6, ADR-0045) ---

    #[test]
    #[ignore = "vau (fexpr) has no v2 equivalent — removed in ADR-0052 cutover"]
    fn engine_vau_does_not_force_child() {
        // Scheme::Vau has no v2 equivalent; this test is ignored.
        /*
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.div",
                "binary.div",
                Scheme::arr_native("div", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.vau",
                "vau",
                Scheme::vau(vec!["x".into()], Scheme::arr_native_lit("const", vec![], json!(42))),
            ))
            .unwrap();
        // 0: lit(1), 1: lit(0), 2: binary.div {children:[0,1]} — would error
        // 3: vau {children:[2]} — binds x to the unevaluated div, returns 42
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(0)),
            AstNode::new("binary.div").with_payload(json!({"children": [0, 1]})),
            AstNode::new("vau").with_payload(json!({"children": [2]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
        */
    }

    // --- match / handle prim tests (mirror tree-walk semantics) ---

    /// Build a language with the eval schemes needed for match/handle tests.
    fn match_lang() -> Language {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.match",
                "match",
                Scheme::arr_native("match", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.handle",
                "handle",
                Scheme::arr_native("handle", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
            ))
            .unwrap();
        lang
    }

    #[test]
    #[ignore = "match/handle prims not in v2 native op set (ADR-0052 cutover)"]
    fn engine_match_wildcard() {
        // match 5 { _: 1 } → 1
        let engine = CraneliftEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("var").with_payload(json!("_")),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})),
            AstNode::new("block").with_payload(json!({"children": [3]})),
            AstNode::new("match").with_payload(json!({"children": [0, 4]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(1));
    }

    #[test]
    #[ignore = "match/handle prims not in v2 native op set (ADR-0052 cutover)"]
    fn engine_match_binder() {
        // match 5 { x: x } → 5
        let engine = CraneliftEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})),
            AstNode::new("block").with_payload(json!({"children": [3]})),
            AstNode::new("match").with_payload(json!({"children": [0, 4]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(5));
    }

    #[test]
    #[ignore = "match/handle prims not in v2 native op set (ADR-0052 cutover)"]
    fn engine_match_literal() {
        // match 3 { 1: "a", 3: "b" } → "b"
        let engine = CraneliftEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!("a")),
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})),
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("lit").with_payload(json!("b")),
            AstNode::new("arm").with_payload(json!({"children": [4, 5]})),
            AstNode::new("block").with_payload(json!({"children": [3, 6]})),
            AstNode::new("match").with_payload(json!({"children": [0, 7]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!("b"));
    }

    #[test]
    #[ignore = "match/handle prims not in v2 native op set (ADR-0052 cutover)"]
    fn engine_match_no_arm_errors() {
        // match 9 { 1: "a" } → error
        let engine = CraneliftEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(9)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!("a")),
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})),
            AstNode::new("block").with_payload(json!({"children": [3]})),
            AstNode::new("match").with_payload(json!({"children": [0, 4]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    #[ignore = "match/handle prims not in v2 native op set (ADR-0052 cutover)"]
    fn engine_match_array_destructuring() {
        // match [1,2] { [a, b]: a + b } → 3
        let engine = CraneliftEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!([1, 2])),
            AstNode::new("var").with_payload(json!("a")),
            AstNode::new("var").with_payload(json!("b")),
            AstNode::new("array").with_payload(json!({"children": [1, 2]})),
            AstNode::new("var").with_payload(json!("a")),
            AstNode::new("var").with_payload(json!("b")),
            AstNode::new("binary.add").with_payload(json!({"children": [4, 5]})),
            AstNode::new("arm").with_payload(json!({"children": [3, 6]})),
            AstNode::new("block").with_payload(json!({"children": [7]})),
            AstNode::new("match").with_payload(json!({"children": [0, 8]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(3));
    }

    #[test]
    #[ignore = "match/handle prims not in v2 native op set (ADR-0052 cutover)"]
    fn engine_match_array_rest() {
        // match [1,2,3,4] { [a, b, ...rest]: rest } → [3,4]
        let engine = CraneliftEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!([1, 2, 3, 4])),
            AstNode::new("var").with_payload(json!("a")),
            AstNode::new("var").with_payload(json!("b")),
            AstNode::new("var").with_payload(json!("rest")),
            AstNode::new("rest").with_payload(json!({"children": [3]})),
            AstNode::new("array").with_payload(json!({"children": [1, 2, 4]})),
            AstNode::new("var").with_payload(json!("rest")),
            AstNode::new("arm").with_payload(json!({"children": [5, 6]})),
            AstNode::new("block").with_payload(json!({"children": [7]})),
            AstNode::new("match").with_payload(json!({"children": [0, 8]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!([3, 4]));
    }

    #[test]
    #[ignore = "match/handle prims not in v2 native op set (ADR-0052 cutover)"]
    fn engine_match_object_destructuring() {
        // match {x: 1, y: 2} { {x: a, y: b}: a + b } → 3
        let engine = CraneliftEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!({"x": 1, "y": 2})),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("var").with_payload(json!("a")),
            AstNode::new("entry").with_payload(json!({"children": [1, 2]})),
            AstNode::new("var").with_payload(json!("y")),
            AstNode::new("var").with_payload(json!("b")),
            AstNode::new("entry").with_payload(json!({"children": [4, 5]})),
            AstNode::new("object").with_payload(json!({"children": [3, 6]})),
            AstNode::new("var").with_payload(json!("a")),
            AstNode::new("var").with_payload(json!("b")),
            AstNode::new("binary.add").with_payload(json!({"children": [8, 9]})),
            AstNode::new("arm").with_payload(json!({"children": [7, 10]})),
            AstNode::new("block").with_payload(json!({"children": [11]})),
            AstNode::new("match").with_payload(json!({"children": [0, 12]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(3));
    }

    #[test]
    #[ignore = "match/handle prims not in v2 native op set (ADR-0052 cutover)"]
    fn engine_match_object_rest() {
        // match {x: 1, y: 2, z: 3} { {x: a, ...rest}: rest } → {y: 2, z: 3}
        let engine = CraneliftEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!({"x": 1, "y": 2, "z": 3})),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("var").with_payload(json!("a")),
            AstNode::new("entry").with_payload(json!({"children": [1, 2]})),
            AstNode::new("var").with_payload(json!("rest")),
            AstNode::new("rest").with_payload(json!({"children": [4]})),
            AstNode::new("object").with_payload(json!({"children": [3, 5]})),
            AstNode::new("var").with_payload(json!("rest")),
            AstNode::new("arm").with_payload(json!({"children": [6, 7]})),
            AstNode::new("block").with_payload(json!({"children": [8]})),
            AstNode::new("match").with_payload(json!({"children": [0, 9]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(
            engine.eval(&lang, &req).unwrap(),
            json!({"y": 2, "z": 3})
        );
    }

    #[test]
    #[ignore = "match/handle prims not in v2 native op set (ADR-0052 cutover)"]
    fn engine_handle_dispatches() {
        // handle 5 { x: x + 1 } → 6
        let engine = CraneliftEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("binary.add").with_payload(json!({"children": [2, 3]})),
            AstNode::new("arm").with_payload(json!({"children": [1, 4]})),
            AstNode::new("block").with_payload(json!({"children": [5]})),
            AstNode::new("handle").with_payload(json!({"children": [0, 6]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(6));
    }

    #[test]
    #[ignore = "match/handle prims not in v2 native op set (ADR-0052 cutover)"]
    fn engine_handle_no_clause_returns_body() {
        // handle 5 { 1: "a" } → 5 (no clause matches, body returned as-is)
        let engine = CraneliftEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!("a")),
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})),
            AstNode::new("block").with_payload(json!({"children": [3]})),
            AstNode::new("handle").with_payload(json!({"children": [0, 4]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(5));
    }

    // ===================================================================
    // V2 engine tests (CraneliftEngineV2)
    // ===================================================================

    // 1. arr const

    #[test]
    fn v2_arr_const_returns_literal() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.c", "c", Scheme::arr_native_lit("const", vec![], json!(42))))
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
            .merge(EvalSchemeDef::new("e.c", "c", Scheme::arr_native_lit("const", vec![], Value::Null)))
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("x".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("const", vec![], json!(7)),
                        Scheme::arr_native_lit("extend_env", vec![], Value::String("x".into())),
                    ),
                    Scheme::arr_native_lit("proj_env", vec![], Value::String("x".into())),
                ),
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
            .merge(EvalSchemeDef::new("e.v", "v", Scheme::arr_native_lit("proj_env", vec![], Value::String("unknown".into()))))
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("x".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("const", vec![], json!(5)),
                        Scheme::arr_native_lit("extend_env", vec![], Value::String("x".into())),
                    ),
                    Scheme::arr_native_lit("proj_env", vec![], Value::String("x".into())),
                ),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.id",
                "id",
                Scheme::arr_native("id", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64))]),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("x".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("const", vec![], json!(5)),
                        Scheme::arr_native_lit("extend_env", vec![], Value::String("x".into())),
                    ),
                    Scheme::arr_native_lit("proj_env", vec![], Value::String("x".into())),
                ),
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
                Scheme::seq(
                    Scheme::seq(Scheme::arr_native_lit("const", vec![], json!(true)), Scheme::arr_native("tag", vec![])),
                    Scheme::choice(Scheme::arr_native_lit("const", vec![], json!(1)), Scheme::arr_native_lit("const", vec![], json!(2))),
                ),
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
                Scheme::seq(
                    Scheme::seq(Scheme::arr_native_lit("const", vec![], json!(false)), Scheme::arr_native("tag", vec![])),
                    Scheme::choice(Scheme::arr_native_lit("const", vec![], json!(1)), Scheme::arr_native_lit("const", vec![], json!(2))),
                ),
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
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
                Scheme::arr_native("eq", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
    #[ignore = "v1 kernel removed in ADR-0052; engine only supports v2"]
    fn v2_engine_accepts_v1_lang() {
        // SchemeVersion::v1() no longer exists (ADR-0052 big-bang cutover).
        /*
        let mut lang = Language::new();
        lang.scheme_version = SchemeVersion::v1();
        lang.eval
            .merge(EvalSchemeDef::new("e.c", "c", Scheme::arr_native_lit("const", vec![], json!(42))))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("c")]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
        */
    }

    #[test]
    fn v2_engine_accepts_v2_lang() {
        let mut lang = Language::new();
        lang.scheme_version = SchemeVersion::v2();
        lang.eval
            .merge(EvalSchemeDef::new("e.c", "c", Scheme::arr_native_lit("const", vec![], json!(42))))
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
        assert!(matches!(err, LangError::Engine(_)));
    }

    // 10. Foreign dispatch

    #[test]
    fn v2_foreign_arr_dispatches_to_module() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.double",
                "double",
                Scheme::arr_foreign("double", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64))]),
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
                Scheme::arr_native("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into()))))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                Scheme::app(Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into()))))
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
                    Scheme::arr_native(
                        op,
                        vec![
                            Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                            Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                        ],
                    ),
                ))
                .unwrap();
        }
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                        Scheme::arr_native("tag", vec![]),
                    ),
                    Scheme::choice(
                        Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64)),
                        Scheme::arr_native_lit("proj_child", vec![], Value::from(2u64)),
                    ),
                ),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new("e.object", "object", Scheme::arr_native("object", vec![])))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new("e.function", "function", Scheme::arr_native("closure", vec![])))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.loop",
                "loop",
                Scheme::seq(
                    Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)),
                    Scheme::loop_feedback(Scheme::arr_native(
                        "loop_step",
                        vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))],
                    )),
                ),
            ))
            .unwrap();
        lang
    }

    /// `loop 42 (x ~> { out: x })` — single-step terminate, scalar
    /// state, one param. Expected: 42.
    #[test]
    fn v2_loop_step_single_step_terminate() {
        let lang = loop_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!("out")),                       // 0
            AstNode::new("var").with_payload(json!("x")),                         // 1
            AstNode::new("entry").with_payload(json!({"children": [0, 1]})),      // 2
            AstNode::new("object").with_payload(json!({"children": [2]})),        // 3
            AstNode::new("function").with_payload(json!({"params": ["x"], "body": 3})), // 4
            AstNode::new("lit").with_payload(json!(42)),                          // 5
            AstNode::new("loop").with_payload(json!({"children": [5, 4]})),       // 6
        ]);
        let engine = CraneliftEngineV2::new();
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }

    /// `loop { n: 3, acc: 1 }([n, acc] ~> if n == 0 then { out: acc }
    /// else { state: { n: n - 1, acc: acc * n } })` — multi-step with param
    /// destructuring, `if`, arithmetic, nested object construction.
    /// Computes 3! = 6.
    #[test]
    fn v2_loop_step_factorial_destructured() {
        let lang = loop_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!("n")),                        // 0
            AstNode::new("lit").with_payload(json!("acc")),                      // 1
            AstNode::new("lit").with_payload(json!("out")),                      // 2
            AstNode::new("lit").with_payload(json!("state")),                    // 3
            AstNode::new("lit").with_payload(json!(0)),                          // 4
            AstNode::new("lit").with_payload(json!(1)),                          // 5
            AstNode::new("var").with_payload(json!("n")),                        // 6
            AstNode::new("var").with_payload(json!("acc")),                      // 7
            AstNode::new("binary.eq").with_payload(json!({"children": [6, 4]})), // 8
            AstNode::new("binary.sub").with_payload(json!({"children": [6, 5]})),// 9
            AstNode::new("binary.mul").with_payload(json!({"children": [7, 6]})),// 10
            AstNode::new("entry").with_payload(json!({"children": [0, 9]})),     // 11
            AstNode::new("entry").with_payload(json!({"children": [1, 10]})),    // 12
            AstNode::new("object").with_payload(json!({"children": [11, 12]})),  // 13
            AstNode::new("entry").with_payload(json!({"children": [2, 7]})),     // 14
            AstNode::new("object").with_payload(json!({"children": [14]})),      // 15
            AstNode::new("entry").with_payload(json!({"children": [3, 13]})),    // 16
            AstNode::new("object").with_payload(json!({"children": [16]})),      // 17
            AstNode::new("if").with_payload(json!({"children": [8, 15, 17]})),   // 18
            AstNode::new("function").with_payload(json!({"params": ["n", "acc"], "body": 18})), // 19
            AstNode::new("lit").with_payload(json!({"n": 3, "acc": 1})),         // 20
            AstNode::new("loop").with_payload(json!({"children": [20, 19]})),    // 21
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
                Scheme::seq(
                    Scheme::arr_native_lit("const", vec![], json!(5)),
                    Scheme::loop_feedback(Scheme::arr_native(
                        "while_step",
                        vec![
                            Scheme::arr_native("is_pos", vec![]),
                            Scheme::arr_native("dec", vec![]),
                        ],
                    )),
                ),
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
                Scheme::seq(
                    Scheme::arr_native_lit("const", vec![], json!(0)),
                    Scheme::loop_feedback(Scheme::arr_native(
                        "while_step",
                        vec![
                            Scheme::arr_native("is_pos", vec![]),
                            Scheme::arr_native("dec", vec![]),
                        ],
                    )),
                ),
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
                Scheme::seq(
                    Scheme::arr_native_lit("const", vec![], pair(json!(5), json!(0))),
                    Scheme::loop_feedback(Scheme::arr_native(
                        "for_step",
                        vec![Scheme::arr_native("inc", vec![])],
                    )),
                ),
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
                Scheme::seq(
                    Scheme::arr_native_lit("const", vec![], pair(json!(0), json!(99))),
                    Scheme::loop_feedback(Scheme::arr_native(
                        "for_step",
                        vec![Scheme::arr_native("inc", vec![])],
                    )),
                ),
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
                Scheme::seq(
                    Scheme::arr_native_lit("const", vec![], json!(100)),
                    Scheme::loop_feedback(Scheme::arr_native(
                        "fix_step",
                        vec![Scheme::arr_native("half_floor", vec![])],
                    )),
                ),
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
                Scheme::seq(
                    Scheme::arr_native_lit("const", vec![], json!(0)),
                    Scheme::loop_feedback(Scheme::arr_native(
                        "fix_step",
                        vec![Scheme::arr_native("half_floor", vec![])],
                    )),
                ),
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
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let compile_result = compile_all_nodes_jit(&lang, &ast).unwrap();
        let engine = CraneliftEngineV2::new();
        let result = engine
            .run_compiled(&lang, &ast, &compile_result)
            .unwrap();
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
                Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
        let result = compile_all_nodes(&lang, &ast, &mode).unwrap();
        assert!(tmp.exists());
        assert!(std::fs::metadata(&tmp).map(|m| m.len() > 0).unwrap_or(false));
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
                    .merge(EvalSchemeDef::new("e.c", "c", Scheme::arr_native_lit("const", vec![], json!(42))))
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
                        Scheme::arr_native("add", vec![Scheme::arr_native_lit("proj_child", vec![], Value::from(0u64)), Scheme::arr_native_lit("proj_child", vec![], Value::from(1u64))]),
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
                        Scheme::seq(
                            Scheme::seq(Scheme::arr_native_lit("const", vec![], json!(true)), Scheme::arr_native("tag", vec![])),
                            Scheme::choice(Scheme::arr_native_lit("const", vec![], json!(1)), Scheme::arr_native_lit("const", vec![], json!(2))),
                        ),
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
                        Scheme::seq(
                            Scheme::seq(Scheme::arr_native_lit("const", vec![], json!(false)), Scheme::arr_native("tag", vec![])),
                            Scheme::choice(Scheme::arr_native_lit("const", vec![], json!(1)), Scheme::arr_native_lit("const", vec![], json!(2))),
                        ),
                    ))
                    .unwrap();
                ("branch_false", lang, make_ast(vec![AstNode::new("if")]))
            },
            // bind
            {
                let mut lang = Language::new();
                lang.eval
                    .merge(EvalSchemeDef::new("e.var", "var", Scheme::arr_native_lit("proj_env", vec![], Value::String("x".into()))))
                    .unwrap();
                lang.eval
                    .merge(EvalSchemeDef::new(
                        "e.let",
                        "let",
                        Scheme::seq(
                            Scheme::seq(
                                Scheme::arr_native_lit("const", vec![], json!(10)),
                                Scheme::arr_native_lit("extend_env", vec![], Value::String("x".into())),
                            ),
                            Scheme::arr_native_lit("proj_env", vec![], Value::String("x".into())),
                        ),
                    ))
                    .unwrap();
                ("bind", lang, make_ast(vec![AstNode::new("let")]))
            },
        ];

        let tw = handl_plugin_engine_eval_tree_walk::TreeWalkEngine::new();
        let bc = handl_plugin_engine_eval_bytecode::BytecodeEngineV2::new();
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
}
