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
//! - **Control flow is native**: [`Scheme::Branch`] compiles to native
//!   `brif`/`jump` instructions. [`Scheme::Bind`] scope push/pop is native.
//!   [`Scheme::App`] closure dispatch is a native call to a helper.
//! - **Value operations call Rust helpers**: [`Scheme::Prim`],
//!   [`Scheme::Foreign`], [`Scheme::Child`], variable load/store, and payload
//!   access are native calls into Rust functions that operate on [`Value`]
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
//! baked into the IR. At execution time, [`Scheme::Child`] dispatches to the
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
//!   `{"__closure__": true, "params": [...], "body": <arena_idx>, "env": {...}}`,
//!   created by the `"closure"` primitive.
//! - Primitives: `add, sub, mul, div, mod, eq, lt, gt, not, and, or, neg,
//!   closure` — identical semantics to the tree-walk.
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
/// The engine checks `lang.scheme_version` against `SchemeVersion::v1()` and
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
    /// The standalone executable does **not** support `Scheme::Foreign`
    /// dispatch (there is no host functions registry at runtime). Corpus
    /// cases that use foreign functions will fail at evaluation time; this
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
    /// for [`Scheme::Foreign`] dispatch, matching [`eval`].
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
        // Scheme version check — reject versions the engine doesn't support.
        if !SchemeVersion::v1().supports(&lang.scheme_version) {
            return Err(LangError::Engine(format!(
                "unsupported scheme version: {} (engine supports v1)",
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

    let result_ptr: *mut Value =
        unsafe { root_fn(&mut ctx as *mut EvalCtx as *mut u8) as *mut Value };

    // Check for deferred error.
    if let Some(err) = ctx.error.take() {
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
    compiled: HashMap<usize, compile::CompiledFn>,
    /// Literal constant pool: values referenced by `Scheme::Lit`.
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
        let result = func(ctx as *mut EvalCtx as *mut u8) as *mut Value;
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
            None => return ctx.set_error(LangError::Engine("callee is not a closure".to_string())),
        };
        if !obj
            .get("__closure__")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return ctx.set_error(LangError::Engine("callee is not a closure".to_string()));
        }

        let params: Vec<String> = match obj.get("params").and_then(|v| v.as_array()) {
            Some(arr) => {
                let mut params = Vec::with_capacity(arr.len());
                for v in arr {
                    match v.as_str() {
                        Some(s) => params.push(s.to_string()),
                        None => {
                            return ctx.set_error(LangError::Engine(
                                "closure param is not a string".to_string(),
                            ));
                        }
                    }
                }
                params
            }
            None => return ctx.set_error(LangError::Engine("closure missing params".to_string())),
        };

        let body_idx = match obj.get("body").and_then(|v| v.as_u64()) {
            Some(idx) => idx as usize,
            None => return ctx.set_error(LangError::Engine("closure missing body".to_string())),
        };

        let captured_env: HashMap<String, Value> = obj
            .get("env")
            .and_then(|v| v.as_object())
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        if params.len() != arg_vals.len() {
            return ctx.set_error(LangError::Engine(format!(
                "closure expects {} args, got {}",
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

        let result = func(ctx as *mut EvalCtx as *mut u8) as *mut Value;

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
/// `current_ref`, calls the function, and reclaims the boxed result. Returns
/// the evaluated [`Value`] or sets `ctx.error` and returns `Err`.
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
    let result_ptr = unsafe { func(ctx as *mut EvalCtx as *mut u8) as *mut Value };
    ctx.current_ref = saved_ref;
    if result_ptr.is_null() {
        return Err(ctx
            .error
            .take()
            .unwrap_or_else(|| LangError::Engine("eval_child_node: null result".to_string())));
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
        let matched = match try_pattern(ctx, AstRef::new(pattern_idx), &scrut_val) {
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
        let matched = match try_pattern(ctx, AstRef::new(pattern_idx), &body_val) {
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

/// Interpret a pattern AST node structurally against a scrutinee value.
///
/// - `var` node named `_` or `_*` → wildcard (always matches, binds nothing).
/// - `var` node with another name → binder (binds name → scrut, always
///   matches).
/// - `array` node → array destructuring with optional `...rest`.
/// - `object` node → object destructuring with optional `...rest`.
/// - Any other node → literal: evaluate the pattern as a value and compare by
///   equality.
///
/// Returns `true` if the pattern matches. Bindings are inserted into
/// `ctx.env` (the caller is responsible for push/pop scope).
fn try_pattern(ctx: &mut EvalCtx, pattern_ref: AstRef, scrut: &Value) -> Result<bool, LangError> {
    let ast = unsafe { &*ctx.ast };
    let pattern_node = ast.at(pattern_ref)?;
    if pattern_node.kind == "var" {
        if let Some(name) = pattern_node.payload.as_str() {
            if name == "_" || name.starts_with('_') {
                return Ok(true);
            }
            ctx.env.define(name, scrut.clone());
            return Ok(true);
        }
    }
    if pattern_node.kind == "array" {
        return try_array_pattern(ctx, pattern_ref, scrut);
    }
    if pattern_node.kind == "object" {
        return try_object_pattern(ctx, pattern_ref, scrut);
    }
    let pat_val = eval_child_node(ctx, pattern_ref.index())?;
    Ok(pat_val == *scrut)
}

/// Extract the binder name from a `rest` pattern node (`...name`).
///
/// Returns `Some(name)` if the rest node binds a name, or `None` if it's a
/// wildcard rest (`..._` or `..._*`). Returns `Err` if the rest node is
/// malformed.
fn rest_binder_name(ctx: &mut EvalCtx, rest_ref: AstRef) -> Result<Option<String>, LangError> {
    let ast = unsafe { &*ctx.ast };
    let rest_node = ast.at(rest_ref)?;
    let children = rest_node
        .payload
        .get("children")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LangError::Engine("rest pattern: missing children".to_string()))?;
    let var_idx = children
        .first()
        .and_then(|v| v.as_u64())
        .ok_or_else(|| LangError::Engine("rest pattern: missing binder".to_string()))?;
    let var_node = ast.at(AstRef::new(var_idx as usize))?;
    if var_node.kind == "var" {
        if let Some(name) = var_node.payload.as_str() {
            if name == "_" || name.starts_with('_') {
                return Ok(None);
            }
            return Ok(Some(name.to_string()));
        }
    }
    Err(LangError::Engine(
        "rest pattern: binder is not a var".to_string(),
    ))
}

/// Array destructuring: `[a, b, ...rest]`.
///
/// Positional sub-patterns are matched left-to-right. A `...rest` element
/// captures the remaining scrutinee elements as an array bound to `rest`.
/// Without a rest, the array length must match exactly.
fn try_array_pattern(
    ctx: &mut EvalCtx,
    pattern_ref: AstRef,
    scrut: &Value,
) -> Result<bool, LangError> {
    let ast = unsafe { &*ctx.ast };
    let pattern_node = ast.at(pattern_ref)?;
    let children = pattern_node
        .payload
        .get("children")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LangError::Engine("array pattern: missing children".to_string()))?;
    let scrut_arr = match scrut.as_array() {
        Some(a) => a,
        None => return Ok(false),
    };
    let mut rest_idx: Option<usize> = None;
    let mut positional: Vec<usize> = Vec::new();
    for child in children {
        let idx = child
            .as_u64()
            .ok_or_else(|| LangError::Engine("array pattern: bad child index".to_string()))?
            as usize;
        let child_node = ast.at(AstRef::new(idx))?;
        if child_node.kind == "rest" {
            if rest_idx.is_some() {
                return Err(LangError::Engine(
                    "array pattern: multiple rest patterns".to_string(),
                ));
            }
            rest_idx = Some(idx);
        } else {
            positional.push(idx);
        }
    }
    let n = positional.len();
    if rest_idx.is_none() && scrut_arr.len() != n {
        return Ok(false);
    }
    if scrut_arr.len() < n {
        return Ok(false);
    }
    for (i, &pat_idx) in positional.iter().enumerate() {
        if !try_pattern(ctx, AstRef::new(pat_idx), &scrut_arr[i])? {
            return Ok(false);
        }
    }
    if let Some(ridx) = rest_idx {
        if let Some(name) = rest_binder_name(ctx, AstRef::new(ridx))? {
            let rest_vals: Vec<Value> = scrut_arr[n..].to_vec();
            ctx.env.define(&name, Value::Array(rest_vals));
        }
    }
    Ok(true)
}

/// Object destructuring: `{ x: a, y: b, ...rest }`.
///
/// Each entry's key is a field name (from a `var` or evaluated to a string);
/// the entry's value is a sub-pattern matched against the corresponding field.
/// A `...rest` element captures the unmatched keys as an object.
fn try_object_pattern(
    ctx: &mut EvalCtx,
    pattern_ref: AstRef,
    scrut: &Value,
) -> Result<bool, LangError> {
    let ast = unsafe { &*ctx.ast };
    let pattern_node = ast.at(pattern_ref)?;
    let children = pattern_node
        .payload
        .get("children")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LangError::Engine("object pattern: missing children".to_string()))?;
    let scrut_obj = match scrut.as_object() {
        Some(o) => o,
        None => return Ok(false),
    };
    let mut rest_idx: Option<usize> = None;
    let mut entries: Vec<(String, usize)> = Vec::new();
    for child in children {
        let idx = child
            .as_u64()
            .ok_or_else(|| LangError::Engine("object pattern: bad child index".to_string()))?
            as usize;
        let child_node = ast.at(AstRef::new(idx))?;
        if child_node.kind == "rest" {
            if rest_idx.is_some() {
                return Err(LangError::Engine(
                    "object pattern: multiple rest patterns".to_string(),
                ));
            }
            rest_idx = Some(idx);
        } else if child_node.kind == "entry" {
            let entry_children = child_node
                .payload
                .get("children")
                .and_then(|v| v.as_array())
                .ok_or_else(|| LangError::Engine("object pattern: malformed entry".to_string()))?;
            let key_idx = entry_children[0]
                .as_u64()
                .ok_or_else(|| {
                    LangError::Engine("object pattern: bad entry key index".to_string())
                })? as usize;
            let val_idx = entry_children[1]
                .as_u64()
                .ok_or_else(|| {
                    LangError::Engine("object pattern: bad entry val index".to_string())
                })? as usize;
            let key_node = ast.at(AstRef::new(key_idx))?;
            let key = if key_node.kind == "var" {
                key_node.payload.as_str().unwrap_or("").to_string()
            } else {
                let key_val = eval_child_node(ctx, key_idx)?;
                key_val.as_str().unwrap_or("").to_string()
            };
            entries.push((key, val_idx));
        }
    }
    let mut matched_keys: Vec<String> = Vec::new();
    for (key, val_idx) in &entries {
        match scrut_obj.get(key) {
            Some(scrut_val) => {
                matched_keys.push(key.clone());
                if !try_pattern(ctx, AstRef::new(*val_idx), scrut_val)? {
                    return Ok(false);
                }
            }
            None => return Ok(false),
        }
    }
    if let Some(ridx) = rest_idx {
        if let Some(name) = rest_binder_name(ctx, AstRef::new(ridx))? {
            let rest_obj: serde_json::Map<String, Value> = scrut_obj
                .iter()
                .filter(|(k, _)| !matched_keys.contains(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            ctx.env.define(&name, Value::Object(rest_obj));
        }
    }
    Ok(true)
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
            let node = match ctx.current_node() {
                Ok(n) => n,
                Err(e) => return ctx.set_error(e),
            };
            let payload = &node.payload;
            let params = match payload.get("params").and_then(|v| v.as_array()) {
                Some(arr) => arr,
                None => {
                    return ctx.set_error(LangError::Engine(
                        "closure prim: missing params in payload".to_string(),
                    ));
                }
            };
            let body = match payload.get("body").and_then(|v| v.as_u64()) {
                Some(b) => b,
                None => {
                    return ctx.set_error(LangError::Engine(
                        "closure prim: missing body in payload".to_string(),
                    ));
                }
            };
            let env = ctx.env.capture();
            let closure = serde_json::json!({
                "__closure__": true,
                "params": params,
                "body": body,
                "env": env,
            });
            Ok(closure)
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
            match node.payload.get("value") {
                Some(value) => Ok(value.clone()),
                None => {
                    return ctx.set_error(LangError::Engine(
                        "assign: missing value in payload".to_string(),
                    ));
                }
            }
        }
        "match" => return prim_match(ctx),
        "handle" => return prim_handle(ctx),
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
        if !handl_language::SchemeVersion::v1().supports(&lang.scheme_version) {
            return Err(LangError::Engine(format!(
                "compile: unsupported scheme version: {} (engine supports v1)",
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
// Tests
// ---------------------------------------------------------------------------

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
                Scheme::lit(json!(99)),
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
                Scheme::lit(Value::Null),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("x")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::Bind {
                    name: "x".into(),
                    value: Box::new(Scheme::Child(AstRef::new(0))),
                    body: Box::new(Scheme::Child(AstRef::new(1))),
                },
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::Bind {
                    name: "$payload".into(),
                    value: Box::new(Scheme::Child(AstRef::new(0))),
                    body: Box::new(Scheme::Child(AstRef::new(1))),
                },
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
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("unknown")))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("var")]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert_eq!(
            err,
            LangError::UnknownVariable {
                name: "unknown".into()
            }
        );
    }

    #[test]
    fn engine_child_recurses() {
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::Bind {
                    name: "$payload".into(),
                    value: Box::new(Scheme::Child(AstRef::new(0))),
                    body: Box::new(Scheme::Child(AstRef::new(1))),
                },
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::Bind {
                    name: "$payload".into(),
                    value: Box::new(Scheme::Child(AstRef::new(0))),
                    body: Box::new(Scheme::Child(AstRef::new(1))),
                },
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
    fn engine_branch_true() {
        let engine = CraneliftEngine::new();
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
                Scheme::Branch {
                    cond: Box::new(Scheme::child(0)),
                    then_: Box::new(Scheme::child(1)),
                    else_: Box::new(Scheme::child(2)),
                },
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
                Scheme::Branch {
                    cond: Box::new(Scheme::child(0)),
                    then_: Box::new(Scheme::child(1)),
                    else_: Box::new(Scheme::child(2)),
                },
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
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::prim("sub", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::prim("mul", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::prim("div", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::prim("div", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::prim("eq", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::prim("lt", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.gt",
                "binary.gt",
                Scheme::prim("gt", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::prim("not", vec![Scheme::child(0)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.and",
                "binary.and",
                Scheme::prim("and", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.or",
                "binary.or",
                Scheme::prim("or", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::prim("unknown_op", vec![]),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("weird")]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unknown prim"));
    }

    #[test]
    fn engine_foreign_dispatches_to_registered_host() {
        let engine = CraneliftEngine::with_modules(conformance_registry());
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
                Scheme::foreign("double", vec![Scheme::child(0)]),
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
                Scheme::prim("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::Bind {
                    name: "$payload".into(),
                    value: Box::new(Scheme::Child(AstRef::new(0))),
                    body: Box::new(Scheme::Child(AstRef::new(1))),
                },
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
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
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                Scheme::Bind {
                    name: "$payload".into(),
                    value: Box::new(Scheme::Child(AstRef::new(0))),
                    body: Box::new(Scheme::Child(AstRef::new(1))),
                },
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
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
        lang.scheme_version = SchemeVersion::new("v2");
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

    // --- Vau (fexpr) tests (ADR-0041 §6, ADR-0045) ---

    #[test]
    fn engine_vau_does_not_force_child() {
        // A vau node binds its child (unevaluated) but the body returns a
        // constant. The child is a div-by-zero node that would error if
        // evaluated, proving the child is never forced.
        let engine = CraneliftEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.div",
                "binary.div",
                Scheme::prim("div", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.vau",
                "vau",
                Scheme::vau(vec!["x".into()], Scheme::lit(json!(42))),
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
    }

    // --- match / handle prim tests (mirror tree-walk semantics) ---

    /// Build a language with the eval schemes needed for match/handle tests.
    fn match_lang() -> Language {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.match",
                "match",
                Scheme::prim("match", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.handle",
                "handle",
                Scheme::prim("handle", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        lang
    }

    #[test]
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
}
