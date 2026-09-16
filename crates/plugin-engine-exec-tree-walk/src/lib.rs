//! # handl-plugin-engine-exec-tree-walk
//!
//! The **reference interpreter** for the eval operation (ADR-0044 §3, §4,
//! ADR-0052). Interprets the arrow-calculus [`handl_language::Term`]
//! kernel directly via tree-walking: it looks up
//! `lang.eval[node.kind]` → [`handl_language::EvalSchemeDef`] →
//! [`Term`], and interprets the scheme. It is entirely data-driven.
//!
//! ## Value shapes
//!
//! The arrow-calculus combinators operate on a few tagged JSON value
//! shapes:
//!
//! - **Pair** (`first`): `{"__pair__": [a, b]}`. `first inner` runs
//!   `inner` on `a` and carries `b` through, producing
//!   `{"__pair__": [inner(a), b]}`.
//! - **Either** (`Choice` / `tag`): `{"__either__": "left"|"right",
//!   "value": v}`. `Choice { l, r }` routes a `left` Either to `l` and a
//!   `right` Either to `r`. The `tag` native op converts a truthy value
//!   into a `left` Either and a falsy value into a `right` Either (the
//!   `Branch` lowering: `c >>> arr tag >>> Choice(t, e)`).
//! - **Arrow-value** (`App`): `{"__arrow__": true, "params": [...],
//!   "body": <idx>, "env": {...}}`. `App` evaluates the callee; if it is
//!   a string naming a registered host function, dispatches to it;
//!   otherwise applies the closure (swap-in captured env, bind params,
//!   eval body node, restore).
//!
//! ## `$payload` convention
//!
//! When a `proj_env` or `extend_env` literal name is the string
//! `"$payload"`, the engine reads the actual name from the current node's
//! payload:
//! - `proj_env "$payload"` → reads `node.payload` as a string (the
//!   variable name).
//! - `extend_env "$payload"` → reads `node.payload["name"]` as a string
//!   (the bind name).
//!
//! ## No-scheme fallback
//!
//! If a node kind has no eval scheme in the `Language`, the engine returns
//! `node.payload` as the value directly. This handles literal nodes (e.g.
//! a `"lit"` node whose payload is `42`). `foreign.<name>` kinds dispatch
//! to the host function `<name>` with evaluated children.
//!
//! ## Loop semantics
//!
//! [`Term::Loop`] implements **causal/strict** feedback (ADR-0052). The
//! handler seeds `State` with [`Value::Null`], constructs the pair
//! `(In, State)`, and evaluates `body` with that pair as `incoming`. The
//! body's result shape selects one of two modes:
//!
//! - **Single-step.** The body returns a **pair** `(Out, NewState)`;
//!   `Loop` returns `Out`.
//! - **Multi-step (Either-based).** The body returns an **Either**:
//!   `left(out)` → done; `right(new_state)` → re-run with
//!   `pair(original_input, new_state)`. A max-iteration guard (100_000)
//!   prevents infinite loops.

#![warn(missing_docs)]

/// Backward-compatibility re-exports for consumers that still reference
/// the `v2` module path and `V2Interpreter` type name (ADR-0052 cutover).
pub mod v2 {
    pub use crate::{AssertFailure, AssertHandler, ScopeStack, TreeWalkEngine as V2Interpreter};
}

/// The structured payload returned when an `assert` native op fails.
///
/// Host test runners can register a handler via
/// [`TreeWalkEngine::with_assert_handler`] to turn this into a
/// `FailureDetail` for the reporter (ADR-0089, ADR-0092).
#[derive(Debug, Clone)]
pub struct AssertFailure {
    /// The assertion label. For keyword `assert` this is `"assert"`.
    pub assertion: String,
    /// The expected value rendered as JSON, when meaningful.
    pub expected: Option<String>,
    /// The actual value rendered as JSON, when meaningful.
    pub actual: Option<String>,
    /// An optional line-oriented diff between expected and actual.
    pub diff: Option<String>,
    /// A short human-readable message explaining the failure.
    pub message: String,
}

/// Callback that converts a failed keyword assertion into a host-provided
/// failure value.
///
/// The engine calls `on_failure` from the `assert` native op when the
/// assertion does not hold. The returned [`Value`] becomes the result of
/// the `assert` expression; a typical handler returns a `result.err(...)`
/// tagged value. A host runner can side-channel the [`AssertFailure`] into
/// its reporter state.
pub trait AssertHandler: std::fmt::Debug {
    /// Return the value that a failed keyword assertion should evaluate to.
    fn on_failure(&self, failure: AssertFailure) -> Value;
}

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use rustc_hash::FxHashMap;

use handl_language::{
    ArrSource, Ast, AstNode, AstRef, LangError, Language, Term, Value, tag_value,
};
use handl_platform::{ModuleLookup, ValueCallable};
use handl_plugin_engine_exec::pattern_match::{self, PatternMatchCtx};
use handl_plugin_engine_exec::{EvalEngine, EvalRequest, ImportResolver};
use handl_value::{
    TaggedValue, cell2_comp_v, cell2_id, cell2_src, cell2_tgt, cell2_whisker, record_proj,
    tagged_to_value,
};
use plugin_engine::{Model, ModelCtx, RewriteResult, TermData, is_hott_tag};

// Thread-local storage for the current AST and Language, so the
// ValueCallable impl can access them when host functions (e.g. fn.map)
// call arrow-values via ctx.try_call_value.
thread_local! {
    static CURRENT_AST: RefCell<Option<Rc<Ast>>> = RefCell::new(None);
    static CURRENT_LANG: RefCell<Option<Rc<Language>>> = RefCell::new(None);
    static CURRENT_MODULES: RefCell<Option<Rc<dyn ModuleLookup>>> = RefCell::new(None);
    // Host-provided globals (e.g. `self`), shared via `Rc` so the
    // `TreeWalkCallable` path (host functions calling arrow-values) can
    // re-attach them to a fresh `ScopeStack` without a deep clone.
    static CURRENT_GLOBALS: RefCell<Option<Rc<FxHashMap<Rc<str>, TaggedValue>>>> = RefCell::new(None);
    // Import resolver for `import` native ops inside arrows that are called
    // across runtime boundaries (e.g. a `.hnl` module arrow invoked by the
    // test runner needs the module loader's resolver to re-resolve its home
    // AST's imports).
    static CURRENT_IMPORT_RESOLVER: RefCell<Option<Rc<dyn ImportResolver>>> = RefCell::new(None);
    // Registry of "home" eval frames — the `Rc`s a minted arrow needs for
    // its `body` index to remain meaningful after the arrow has crossed a
    // runtime boundary (a `.hnl` module function receiving or returning a
    // closure). Keyed by the `Rc<Ast>` pointer, which stays alive as long
    // as the entry exists (the entry holds an `Rc::clone` of `ast`), so a
    // pointer value can never be recycled while its frame is registered.
    static HOME_FRAMES: RefCell<HashMap<u64, EvalFrame>> = RefCell::new(HashMap::new());
    // Memo of deserialized `body_def` arenas (ADR-0106 item 2, the "Cost of
    // step 2" TODO). Applying a self-describing arrow otherwise re-runs
    // `tagged_to_value` + `as_ast` on *every* call, which allocates per call
    // in hot/recursive loops.
    //
    // Keyed by the `Rc<TaggedValue>` pointer of the definition — the same
    // trick (and the same safety argument) as `HOME_FRAMES`: each entry holds
    // an `Rc::clone` of the definition, so the address cannot be recycled
    // while the entry is registered, and a re-used address can never hit a
    // stale entry. Structurally identical but distinct arrows have distinct
    // addresses and therefore distinct entries.
    //
    // Growth is bounded by `ARROW_ARENA_CACHE_CAP`: the table is cleared when
    // it reaches the cap, which costs at most one re-deserialization per live
    // arrow afterwards.
    static ARROW_ARENA_CACHE: RefCell<HashMap<usize, (Rc<TaggedValue>, Rc<Ast>)>> =
        RefCell::new(HashMap::new());
}

/// Maximum number of deserialized arrow-body arenas memoized per thread.
/// Small by design: applications cycle through a handful of hot arrows, and
/// the table is cleared (not evicted one-by-one) when it fills.
const ARROW_ARENA_CACHE_CAP: usize = 64;

// ---------------------------------------------------------------------------
// EvalFrame — arrow-value provenance across runtime boundaries
// ---------------------------------------------------------------------------

/// A snapshot of the eval state an arrow value needs for its `body` index
/// to be meaningful: the AST the index refers to, the `Language` whose
/// schemes interpret it, the module chain for host dispatch inside the
/// body, and the host-provided globals (e.g. `self`).
///
/// Arrows minted inside an [`EvalEngine::eval`] call carry `home: None` and
/// are applied against the *ambient* AST (threaded through `apply_app`'s
/// `ast` parameter). An arrow that crosses a `serde_json` boundary into
/// another runtime — e.g. a `HandlFunction` receiving an arrow argument or
/// returning a closure — is stamped with `"__home__": <id>` so that
/// `apply_app` / `call_value` can route application back here.
#[derive(Clone)]
pub struct EvalFrame {
    /// The AST the arrow's `body` index refers to.
    pub ast: Rc<Ast>,
    /// The `Language` whose eval schemes interpret that AST.
    pub lang: Rc<Language>,
    /// The module chain for host-function dispatch inside the body.
    pub modules: Option<Rc<dyn ModuleLookup>>,
    /// The host-provided globals visible to the body.
    pub globals: Option<Rc<FxHashMap<Rc<str>, TaggedValue>>>,
    /// The import resolver for `import` native ops inside the body.
    pub import_resolver: Option<Rc<dyn ImportResolver>>,
}

impl EvalFrame {
    /// Install this frame in the eval thread-locals, run `f`, then restore
    /// the previous values — the same save/restore discipline as
    /// [`EvalEngine::eval`], so nested `ctx.try_call_value` /
    /// `TreeWalkCallable` calls inside the arrow body see the arrow's home
    /// frame rather than the ambient one.
    pub fn enter<T>(&self, f: impl FnOnce() -> T) -> T {
        let prev_ast = CURRENT_AST
            .with(|c| std::mem::replace(&mut *c.borrow_mut(), Some(Rc::clone(&self.ast))));
        let prev_lang = CURRENT_LANG
            .with(|c| std::mem::replace(&mut *c.borrow_mut(), Some(Rc::clone(&self.lang))));
        let prev_modules =
            CURRENT_MODULES.with(|c| std::mem::replace(&mut *c.borrow_mut(), self.modules.clone()));
        let prev_globals =
            CURRENT_GLOBALS.with(|c| std::mem::replace(&mut *c.borrow_mut(), self.globals.clone()));
        let prev_import_resolver = CURRENT_IMPORT_RESOLVER
            .with(|c| std::mem::replace(&mut *c.borrow_mut(), self.import_resolver.clone()));

        let out = f();
        CURRENT_AST.with(|c| *c.borrow_mut() = prev_ast);
        CURRENT_LANG.with(|c| *c.borrow_mut() = prev_lang);
        CURRENT_MODULES.with(|c| *c.borrow_mut() = prev_modules);
        CURRENT_GLOBALS.with(|c| *c.borrow_mut() = prev_globals);
        CURRENT_IMPORT_RESOLVER.with(|c| *c.borrow_mut() = prev_import_resolver);
        out
    }
}

/// Look up a registered home [`EvalFrame`] by id.
pub fn home_frame(id: u64) -> Option<EvalFrame> {
    HOME_FRAMES.with(|m| m.borrow().get(&id).cloned())
}

/// Register `frame` under `id` if not already present and return `id`.
fn register_frame_at(id: u64, frame: EvalFrame) -> u64 {
    HOME_FRAMES.with(|m| {
        let mut map = m.borrow_mut();
        map.entry(id).or_insert(frame);
    });
    id
}

/// The standalone arena carried by a `body_def`, deserialized at most once
/// per definition value (ADR-0106 item 2, "Cost of step 2").
///
/// `def` is the arrow's `body_def`: a `{"__ast__": <Ast>}` value produced by
/// `Ast::subtree` + `ast_value`. Applying a self-describing arrow needs that
/// arena; deserializing it on every application allocates per call, so this
/// memoizes it per thread:
///
/// - **Key**: the `Rc<TaggedValue>` pointer of `def`. Safe because the cache
///   entry stores an `Rc::clone` of `def`, so the allocation cannot be freed
///   (and the address reused for a different value) while the entry lives —
///   the same argument as `HOME_FRAMES`. Two structurally identical but
///   distinct definitions have different addresses and different entries.
/// - **Bound**: the table is cleared once it reaches
///   [`ARROW_ARENA_CACHE_CAP`] entries, so memory stays bounded; the cost of
///   clearing is at most one re-deserialization per live arrow.
///
/// Returns `None` when the definition is not a well-formed `__ast__` value or
/// the arena is empty — the caller then falls back to `body`/`home`.
fn cached_def_arena(def: &Rc<TaggedValue>) -> Option<Rc<Ast>> {
    let key = Rc::as_ptr(def) as usize;
    ARROW_ARENA_CACHE.with(|cache| {
        if let Some((_, ast)) = cache.borrow().get(&key) {
            return Some(Rc::clone(ast));
        }
        // Miss: deserialize once, then memoize (with a keep-alive `Rc` of the
        // definition so the key cannot dangle).
        let ast = handl_language::as_ast(&tagged_to_value(def)).filter(|a| !a.is_empty())?;
        let ast = Rc::new(ast);
        let mut map = cache.borrow_mut();
        if map.len() >= ARROW_ARENA_CACHE_CAP {
            map.clear();
        }
        map.insert(key, (Rc::clone(def), Rc::clone(&ast)));
        Some(ast)
    })
}

/// Capture the ambient eval frame — the `CURRENT_AST` / `CURRENT_LANG` /
/// `CURRENT_MODULES` / `CURRENT_GLOBALS` thread-locals of the eval currently
/// running on this thread — and return its home-frame id for `__home__`
/// stamping.
///
/// Returns `None` when no tree-walk eval is running (e.g. a host function
/// invoked directly, outside any `eval`). The id is the `Rc<Ast>` pointer:
/// the registered entry holds an `Rc` clone of the AST, so the pointer can
/// never be recycled while the frame is registered, and repeated captures
/// within one eval return the same id.
///
/// This is the seam that lets a nested `Runtime` (e.g. a `.hnl` module
/// function's bridge eval) invoke arrow values minted by the *outer* eval:
/// the host stamps incoming arrow arguments with this id, and
/// `apply_app`/`call_value` route application back to this frame.
pub fn current_home_frame() -> Option<u64> {
    let ast = CURRENT_AST.with(|c| c.borrow().clone())?;
    let id = Rc::as_ptr(&ast) as usize as u64;
    let lang = CURRENT_LANG.with(|c| c.borrow().clone())?;
    let modules = CURRENT_MODULES.with(|c| c.borrow().clone());
    let globals = CURRENT_GLOBALS.with(|c| c.borrow().clone());
    let import_resolver = CURRENT_IMPORT_RESOLVER.with(|c| c.borrow().clone());
    Some(register_frame_at(
        id,
        EvalFrame {
            ast,
            lang,
            modules,
            globals,
            import_resolver,
        },
    ))
}

/// Register a home [`EvalFrame`] for an eval performed outside the
/// [`EvalEngine::eval`] thread-local window — e.g. the per-call bridge eval
/// a `.hnl` module function runs — so arrows *returned* from that eval can
/// be invoked later against this AST.
///
/// The id is derived from `Rc::as_ptr(&ast)`; the caller must pass an `Rc`
/// it owns (e.g. the parsed AST kept alive for the frame's lifetime). The
/// registered entry keeps the `Rc`s alive for the life of the thread.
///
/// `globals` are the extra root-scope bindings the eval ran with (e.g.
/// per-call argument bindings); `None` means the frame has no host globals
/// beyond what the module chain provides.
pub fn register_home_frame(
    ast: Rc<Ast>,
    lang: Rc<Language>,
    modules: Option<Rc<dyn ModuleLookup>>,
    globals: Option<HashMap<String, Value>>,
    import_resolver: Option<Rc<dyn ImportResolver>>,
) -> u64 {
    let globals = globals.map(|g| {
        Rc::new(
            g.into_iter()
                .map(|(k, v)| (Rc::from(k.as_str()), TaggedValue::from(v)))
                .collect::<FxHashMap<Rc<str>, TaggedValue>>(),
        )
    });
    let id = Rc::as_ptr(&ast) as usize as u64;
    register_frame_at(
        id,
        EvalFrame {
            ast,
            lang,
            modules,
            globals,
            import_resolver,
        },
    )
}

// ---------------------------------------------------------------------------
// ScopeStack
// ---------------------------------------------------------------------------

/// A scope stack: `Vec<FxHashMap<Rc<str>, TaggedValue>>` searched innermost-first.
///
/// Used by the tree-walk interpreter for `extend_env` binding and closure
/// capture/swap-in. The `scopes` field is `pub(crate)` so the engine can
/// perform the closure-env swap-in.
///
/// Host-provided globals (e.g. `self`) are kept in a separate
/// `globals` field (a cheaply-cloned `Rc<FxHashMap>`) rather than in
/// `scopes[0]`. This prevents the (potentially massive) globals from
/// being captured into every closure's env on creation and re-cloned on
/// every closure call. `lookup()` falls back to `globals` after
/// searching the scope stack, and `capture()` only captures user-created
/// scopes — never globals.
pub struct ScopeStack {
    /// The stack of scopes, innermost last. `pub(crate)` so the engine
    /// can perform the closure-env swap-in.
    pub(crate) scopes: Vec<FxHashMap<Rc<str>, TaggedValue>>,
    /// The shared captured environment for a closure call, held in a
    /// cheaply-cloned `Rc` so it is **never deep-cloned** on closure
    /// invocation. Only set by `apply_app`/`call_value` when applying an
    /// arrow-value; `None` for ordinary (non-closure) scope stacks.
    /// `lookup()` consults it after the local scopes but before globals.
    pub(crate) captured: Option<Rc<FxHashMap<Rc<str>, TaggedValue>>>,
    /// Host-provided globals (e.g. `self`), held in a cheaply-cloned
    /// `Rc` so they are shared — never deep-cloned — across closure
    /// invocations. Accessible via `lookup()` as a fallback but excluded
    /// from `capture()`.
    pub(crate) globals: Rc<FxHashMap<Rc<str>, TaggedValue>>,
    /// Set when `assign` updates a binding in an *enclosing* scope (not the
    /// innermost one) with a different value. Read and cleared by
    /// `rec` fixpoint detection (`prim_eval_rec`) — such writes are the
    /// ADR-0074 feedback wires: the `while`/`do-while`/`for` desugars
    /// update `__continue`/`__xs` (and user bindings like `x = x + 1`)
    /// from inside `if`/`match` branch blocks, and a pass that produces
    /// any such write must not count as converged.
    pub(crate) enclosing_updated: bool,
    /// Runtime effect/coeffect row accumulated during this evaluation.
    pub(crate) effects: Vec<String>,
}

#[allow(clippy::new_without_default)]
impl ScopeStack {
    /// Create a new scope stack with a single empty root scope and no
    /// globals.
    pub fn new() -> Self {
        Self {
            scopes: vec![FxHashMap::default()],
            captured: None,
            globals: Rc::new(FxHashMap::default()),
            enclosing_updated: false,
            effects: Vec::new(),
        }
    }

    /// Set the host-provided globals map (an `Rc` clone, cheap). Globals
    /// are accessible via `lookup()` but excluded from `capture()`.
    pub fn set_globals(&mut self, globals: Rc<FxHashMap<Rc<str>, TaggedValue>>) {
        self.globals = globals;
    }

    /// Look up a variable, searching innermost scope first, then the
    /// shared captured env (if any), then falling back to the
    /// host-provided globals.
    pub fn lookup(&self, name: &str) -> Option<&TaggedValue> {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.get(name))
            .or_else(|| {
                self.captured
                    .as_ref()
                    .and_then(|captured| captured.get(name))
            })
            .or_else(|| self.globals.get(name))
    }

    /// Define a variable in the current (innermost) scope.
    pub fn define(&mut self, name: &str, value: TaggedValue) {
        // INFALLIBLE: the scope stack is initialized with one frame in
        // `new()` and `pop_scope` only pops when len > 1, so it is never
        // empty during evaluation.
        let last = self.scopes.last_mut().expect("scope stack is never empty");
        last.insert(Rc::from(name), value);
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
    /// rebindings such as `x = x + 1` be observed after the loop. Only
    /// the local scope stack is searched; `captured` and `globals` are
    /// read-only snapshots, so a name visible only through them is
    /// shadowed by a fresh local definition (matching `define`).
    pub fn assign(&mut self, name: &str, value: TaggedValue) {
        let last = self.scopes.len() - 1;
        for (i, scope) in self.scopes.iter_mut().enumerate().rev() {
            if let Some(slot) = scope.get_mut(name) {
                if !handl_value::eq(slot, &value) {
                    *slot = value;
                    // Only cross-scope writes are fixpoint-relevant: an
                    // update to a binding defined earlier in the *same*
                    // innermost scope is ordinary sequencing and must not
                    // keep a `rec` pass alive.
                    if i < last {
                        self.enclosing_updated = true;
                    }
                }
                return;
            }
        }
        self.define(name, value);
    }

    /// Push a new empty scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(FxHashMap::default());
    }

    /// Pop the innermost scope.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Capture all visible user-created variables into a single flat map,
    /// returned as a cheaply-cloned `Rc`. Globals (held in the `globals`
    /// field) are intentionally excluded: they are accessible via
    /// `lookup()` at call time and must not be deep-cloned into every
    /// closure's captured env. The captured env is shared (via `Rc`) at
    /// call time — see `apply_app` — so this is the one-time snapshot
    /// cost paid at closure creation, not on every call.
    ///
    /// Includes the parent closure's captured env (if any) so that nested
    /// closures retain access to bindings from all enclosing closure
    /// scopes, not just the current scope stack. Current scope bindings
    /// take priority over parent captured bindings (shadowing).
    pub fn capture(&self) -> Rc<FxHashMap<Rc<str>, TaggedValue>> {
        let mut captured = FxHashMap::default();
        // Include parent closure's captured env first (lower priority).
        if let Some(parent_captured) = &self.captured {
            for (k, v) in parent_captured.iter() {
                captured.insert(Rc::clone(k), v.clone());
            }
        }
        // Then include current scopes (higher priority, overwrites parent).
        for scope in &self.scopes {
            for (k, v) in scope {
                captured.insert(Rc::clone(k), v.clone());
            }
        }
        Rc::new(captured)
    }
}

// ---------------------------------------------------------------------------
// PatternMatchCtx adapter (ADR-0061)
// ---------------------------------------------------------------------------

/// Adapter that implements [`PatternMatchCtx`] for the tree-walk engine.
///
/// Holds borrowed references to the engine, language, and scope stack so
/// that the shared `pattern_match::try_pattern` can bind variables and
/// evaluate literal pattern nodes without duplicating pattern logic.
struct TreeWalkPatternCtx<'a> {
    /// The tree-walk engine (for `eval_node` on literal patterns).
    engine: &'a TreeWalkEngine,
    /// The language definition (for scheme lookup during evaluation).
    lang: &'a Language,
    /// The scope stack (for binder and rest bindings).
    env: &'a mut ScopeStack,
}

impl<'a> PatternMatchCtx for TreeWalkPatternCtx<'a> {
    fn define_binding(&mut self, name: &str, value: Value) {
        self.env.define(name, TaggedValue::from(value));
    }

    fn eval_pattern_node(&mut self, ast: &Ast, node_ref: AstRef) -> Result<Value, LangError> {
        self.engine.eval_node(ast, node_ref, self.lang, self.env)
    }
}

// ---------------------------------------------------------------------------
// TreeWalkEngine
// ---------------------------------------------------------------------------

/// The tree-walk eval engine (ADR-0052 arrow-calculus reference handler).
///
/// A tree-walk handler that interprets [`Term`] directly. Construct
/// with [`TreeWalkEngine::new`] and optionally attach a module registry
/// via [`TreeWalkEngine::with_modules`] or inject globals via
/// [`TreeWalkEngine::with_global`].
#[derive(Clone)]
pub struct TreeWalkEngine {
    /// The module registry for `Foreign` dispatch (may be `None`).
    modules: Option<Rc<dyn ModuleLookup>>,
    /// Global variables injected by the host, pre-populated into the root
    /// scope on every eval call.
    globals: HashMap<String, Value>,
    /// The import resolver for `import "path"` statements (Phase 9.5).
    /// When `Some`, the `import` native op delegates to this resolver to
    /// load the module and bind its top-level bindings into the current
    /// scope. `None` → `import` errors gracefully.
    import_resolver: Option<Rc<dyn ImportResolver>>,
    /// Cache for handler-eliminated schemes, keyed by node kind (ADR-0076).
    /// Avoids re-running `eliminate_and_check` on every eval of the same
    /// node kind.
    eliminated_cache: RefCell<HashMap<String, Term>>,
    /// Optional handler for the `assert` native op to raise structured
    /// failure details into the host runner.
    assert_handler: Option<Rc<dyn AssertHandler>>,
    /// Runtime handler stack for `Term::Handle` / `Term::Op` dynamic
    /// dispatch. `Term::Handle` pushes its evaluated `impl_` value here
    /// for the duration of its body; `Term::Op` dispatch searches the
    /// stack innermost-last for a `{"__tag__": "Handler", "impls": {...}}`
    /// value providing the op name before falling back to native
    /// dispatch.
    pub(crate) handler_stack: RefCell<Vec<TaggedValue>>,
}

impl TreeWalkEngine {
    /// Create a new tree-walk engine with no module registry.
    pub fn new() -> Self {
        Self {
            modules: None,
            globals: HashMap::new(),
            import_resolver: None,
            eliminated_cache: RefCell::new(HashMap::new()),
            assert_handler: None,
            handler_stack: RefCell::new(Vec::new()),
        }
    }

    /// Attach a module registry (builder style).
    pub fn with_modules(mut self, modules: Rc<dyn ModuleLookup>) -> Self {
        self.modules = Some(modules);
        self
    }

    /// Register a handler for `assert` failures (builder style).
    ///
    /// When the `assert` native op fails, it calls `handler.on_failure`
    /// and returns the resulting [`Value`] as the `assert` expression's
    /// result. If no handler is registered, the op falls back to a generic
    /// `result.err(...)` tagged value instead of raising.
    pub fn with_assert_handler(mut self, handler: Rc<dyn AssertHandler>) -> Self {
        self.assert_handler = Some(handler);
        self
    }

    /// Inject a global variable visible to all eval calls (builder style).
    /// Used by the host to inject `self` (the current language value) and
    /// other ambient values (ADR-0041 Phase 3).
    pub fn with_global(mut self, name: impl Into<String>, value: Value) -> Self {
        self.globals.insert(name.into(), value);
        self.eliminated_cache = RefCell::new(HashMap::new());
        self
    }

    /// Attach an import resolver for `import "path"` statements (builder
    /// style, Phase 9.5). The `import` native op delegates to this resolver
    /// to load the referenced module and bind its top-level bindings into
    /// the current scope.
    pub fn with_import_resolver(mut self, resolver: Rc<dyn ImportResolver>) -> Self {
        self.import_resolver = Some(resolver);
        self
    }

    /// Add a grade name (or a list of grade names) to the accumulated
    /// runtime effect/coeffect row.
    fn collect_grades(&self, value: &TaggedValue, env: &mut ScopeStack) {
        match value {
            TaggedValue::Str(s) => env.effects.push(s.to_string()),
            TaggedValue::Arr(a) => {
                for v in a.iter() {
                    if let Some(s) = v.as_str() {
                        env.effects.push(s.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    /// Collect the grade(s) declared by an `Arrow` record, if any.
    fn collect_arrow_grades(&self, value: &TaggedValue, env: &mut ScopeStack) {
        if let TaggedValue::Obj(map) = value {
            if map.get("__tag__").and_then(|t| t.as_str()) == Some("Arrow") {
                if let Some(grade) = map.get("grade") {
                    self.collect_grades(grade, env);
                }
                if let Some(grades) = map.get("grades") {
                    self.collect_grades(grades, env);
                }
            }
        }
    }

    /// Interpret a given [`Term`] in the context of `current_node`.
    ///
    /// This is the core handler entry point. `incoming` is the value fed
    /// by an enclosing `Seq`/`First`/`Choice`/`Loop`; top-level calls
    /// pass `None`.
    ///
    /// # Scope discipline
    ///
    /// `extend_env` pushes a scope and returns the value flowing through.
    /// The scope's lifetime follows the arrow pipeline: it is popped by
    /// the enclosing [`Term::Seq`] whose `fst` (transitively) contained
    /// the `extend_env`, after that `Seq`'s `snd` completes. This
    /// realizes the `Bind` lowering
    /// `value >>> extend_env "x" >>> body` =
    /// `Seq(Seq(value, extend_env), body)`: the scope pushed by
    /// `extend_env` (the inner `Seq`'s `snd`) is reported up through the
    /// inner `Seq` and popped by the outer `Seq` after `body`. A bare
    /// `extend_env` with no enclosing `Seq` is popped here.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_scheme(
        &self,
        scheme: &Term,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<Value>,
    ) -> Result<Value, LangError> {
        let incoming_tagged = incoming.map(TaggedValue::from);
        let val = self.eval_scheme_tagged(
            scheme,
            ast,
            node_ref,
            lang,
            env,
            current_node,
            incoming_tagged,
        )?;
        Ok(tagged_to_value(&val))
    }

    /// Internal tagged counterpart of [`eval_scheme`](Self::eval_scheme).
    ///
    /// Same as `eval_scheme` but works entirely in [`TaggedValue`] — no
    /// Value↔TaggedValue conversion. Takes `Option<TaggedValue>` as
    /// incoming and returns a `TaggedValue`. Handles scope popping
    /// internally (a bare `extend_env` with no enclosing `Seq` is balanced
    /// here, same as `eval_scheme`).
    #[allow(clippy::too_many_arguments)]
    fn eval_scheme_tagged(
        &self,
        scheme: &Term,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<TaggedValue>,
    ) -> Result<TaggedValue, LangError> {
        // Surface schemes may still use the derived forms. Expand them to the
        // core combinators once before interpretation.
        let scheme = scheme.desugar();
        let (val, did_push) =
            self.eval_inner(&scheme, ast, node_ref, lang, env, current_node, incoming)?;
        // A bare extend_env (no enclosing Seq to pop it) is balanced here.
        if did_push {
            env.pop_scope();
        }
        Ok(val)
    }

    /// [`TermData`]-driven counterpart of
    /// [`eval_scheme_tagged`](Self::eval_scheme_tagged).
    ///
    /// Evaluates a sub-scheme given as a [`TermData`] by delegating to
    /// [`eval_inner_td`](Self::eval_inner_td) and balancing any scope it
    /// pushed. The sub-scheme is already desugared (the top-level
    /// [`eval_scheme_tagged`] call desugars the whole scheme before
    /// conversion), so no desugar step is needed here.
    #[allow(clippy::too_many_arguments)]
    fn eval_scheme_tagged_td(
        &self,
        td: &TermData,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<TaggedValue>,
    ) -> Result<TaggedValue, LangError> {
        let (val, did_push) =
            self.eval_inner_td(td, ast, node_ref, lang, env, current_node, incoming)?;
        if did_push {
            env.pop_scope();
        }
        Ok(val)
    }

    /// Internal recursive interpreter. Returns the value and whether a
    /// scope was pushed that the *caller* is responsible for popping.
    ///
    /// Only `extend_env` pushes a scope. `Seq` pops the scope its `fst`
    /// reported and propagates whatever `snd` reported (so an
    /// `extend_env` in the inner `Seq`'s `snd` position — the `Bind`
    /// lowering shape — is popped by the *enclosing* `Seq` after its
    /// `snd` completes). The other combinators (`First`/`Choice`/`Loop`/
    /// `App`) pop any scope their sub-evaluation pushed immediately,
    /// since they have no "binding persists down the pipeline" semantics.
    #[allow(clippy::too_many_arguments)]
    fn eval_inner(
        &self,
        scheme: &Term,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<TaggedValue>,
    ) -> Result<(TaggedValue, bool), LangError> {
        let td = TermData::from_term(scheme);
        self.eval_inner_td(&td, ast, node_ref, lang, env, current_node, incoming)
    }

    /// The [`TermData`]-driven counterpart of [`eval_inner`](Self::eval_inner).
    ///
    /// Dispatches on [`TermData::tag`] and reads variant fields via the
    /// `get`/`get_str`/`get_term`/`get_term_list` accessors (ADR-0103 Phase 6).
    /// The `&Term` entry point ([`eval_inner`](Self::eval_inner)) converts the
    /// scheme to [`TermData`] once and delegates here.
    #[allow(clippy::too_many_arguments)]
    fn eval_inner_td(
        &self,
        td: &TermData,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<TaggedValue>,
    ) -> Result<(TaggedValue, bool), LangError> {
        match td.tag() {
            "Arr" => {
                let value = td
                    .get("value")
                    .cloned()
                    .expect("Arr term missing 'value' field");
                let tv = TaggedValue::from(value);
                self.collect_arrow_grades(&tv, env);
                Ok((tv, false))
            }
            "Op" => {
                let op = td.get_str("name").expect("Op term missing 'name' field");
                let args = td.get_term_list("args");
                let source = td
                    .arr_source()
                    .expect("Op term missing or malformed 'source' field");
                // `lit` is stored as `null` when `None`; treat Null as absent.
                let lit = td.get("lit").filter(|v| !v.is_null());
                let (val, pushed) = self.eval_arr(
                    op,
                    &args,
                    &source,
                    lit,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                self.collect_arrow_grades(&val, env);
                Ok((val, pushed))
            }
            "Seq" => {
                let fst = td.get_term("fst").expect("Seq term missing 'fst' field");
                let snd = td.get_term("snd").expect("Seq term missing 'snd' field");
                let (fst_val, fst_pushed) = self.eval_inner_td(
                    &fst,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming.clone(),
                )?;
                let (snd_val, snd_pushed) = self.eval_inner_td(
                    &snd,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    Some(fst_val),
                )?;
                // Pop the scope `fst` reported (e.g. extend_env as fst).
                if fst_pushed {
                    env.pop_scope();
                }
                // Propagate `snd_pushed`: an extend_env in snd position
                // (the Bind lowering shape) is popped by the *enclosing*
                // Seq after its own snd completes.
                Ok((snd_val, snd_pushed))
            }
            "First" => {
                let inner = td
                    .get_term("inner")
                    .expect("First term missing 'inner' field");
                let inc = incoming.ok_or_else(|| {
                    LangError::engine_node(
                        "First: missing incoming value".to_string(),
                        current_node,
                    )
                })?;
                let a = pair_fst(&inc)
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "First: incoming is not a pair".to_string(),
                            current_node,
                        )
                    })?
                    .clone();
                let b = pair_snd(&inc)
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "First: incoming is not a pair".to_string(),
                            current_node,
                        )
                    })?
                    .clone();
                let (inner_result, pushed) =
                    self.eval_inner_td(&inner, ast, node_ref, lang, env, current_node, Some(a))?;
                if pushed {
                    env.pop_scope();
                }
                Ok((pair(inner_result, b), false))
            }
            "Choice" => {
                let l = td.get_term("l").expect("Choice term missing 'l' field");
                let r = td.get_term("r").expect("Choice term missing 'r' field");
                let inc = incoming.ok_or_else(|| {
                    LangError::engine_node(
                        "Choice: missing incoming value".to_string(),
                        current_node,
                    )
                })?;
                let (val, pushed) = if is_either_left(&inc) {
                    let v = either_value(&inc)
                        .ok_or_else(|| {
                            LangError::engine_node(
                                "Choice: left Either missing value".to_string(),
                                current_node,
                            )
                        })?
                        .clone();
                    self.eval_inner_td(&l, ast, node_ref, lang, env, current_node, Some(v))?
                } else if is_either_right(&inc) {
                    let v = either_value(&inc)
                        .ok_or_else(|| {
                            LangError::engine_node(
                                "Choice: right Either missing value".to_string(),
                                current_node,
                            )
                        })?
                        .clone();
                    self.eval_inner_td(&r, ast, node_ref, lang, env, current_node, Some(v))?
                } else {
                    return Err(LangError::engine_node(
                        "Choice: incoming is not an Either".to_string(),
                        current_node,
                    ));
                };
                if pushed {
                    env.pop_scope();
                }
                Ok((val, false))
            }
            "Loop" => {
                let body = td.get_term("body").expect("Loop term missing 'body' field");
                // Causal/strict feedback (ADR-0052 §"Feedback semantics").
                // Two modes, selected by the body's result shape (see the
                // module-level "Loop semantics" doc):
                //
                // 1. Single-step: body returns a pair (Out, NewState);
                //    Loop returns Out.
                // 2. Multi-step (Either-based): body returns an Either;
                //    left(out) terminates, right(new_state) re-runs with
                //    the new state. A max-iteration guard bounds the loop.
                //
                // Optimization: when the body is a `loop_step` Arr, the
                // body_proc sub-scheme evaluates to a closure value that
                // captures the definition-time env. That env doesn't change
                // between iterations (only the loop state changes, passed
                // as an argument to `apply_app`). Hoist the closure
                // evaluation outside the loop so it runs once instead of
                // once per iteration.
                if body.tag() == "Op" {
                    let op = body.get_str("name").unwrap_or("");
                    let source = body.arr_source().unwrap_or(ArrSource::Native);
                    if source == ArrSource::Native && op == "loop_step" {
                        let args = body.get_term_list("args");
                        let body_proc = args.first().ok_or_else(|| {
                            LangError::engine_node(
                                "loop_step: missing body proc".to_string(),
                                current_node,
                            )
                        })?;
                        // Evaluate the closure once, before the loop.
                        let closure_val = self.eval_scheme_tagged_td(
                            body_proc,
                            ast,
                            node_ref,
                            lang,
                            env,
                            current_node,
                            None,
                        )?;
                        let inc = incoming.unwrap_or(TaggedValue::Null);
                        let mut state = TaggedValue::Null;
                        const MAX_ITERS: usize = 100_000;
                        for _ in 0..MAX_ITERS {
                            let current = if state.is_null() {
                                inc.clone()
                            } else {
                                state.clone()
                            };
                            let step_args = build_loop_step_args(&closure_val, &current);
                            let result =
                                self.apply_app(&closure_val, &step_args, ast, lang, env)?;
                            let either = interpret_loop_step_result(&result);
                            if is_either_left(&either) {
                                let out = either_value(&either).cloned().ok_or_else(|| {
                                    LangError::engine_node(
                                        "Loop: left Either missing value".to_string(),
                                        current_node,
                                    )
                                })?;
                                return Ok((out, false));
                            }
                            if is_either_right(&either) {
                                state = either_value(&either).cloned().ok_or_else(|| {
                                    LangError::engine_node(
                                        "Loop: right Either missing value".to_string(),
                                        current_node,
                                    )
                                })?;
                                continue;
                            }
                            // interpret_loop_step_result always returns
                            // an Either, so this is unreachable.
                            return Err(LangError::engine_node(
                                "Loop: loop_step produced non-Either".to_string(),
                                current_node,
                            ));
                        }
                        return Err(LangError::engine_node(
                            "Loop: exceeded max iterations".to_string(),
                            current_node,
                        ));
                    }
                }
                let inc = incoming.unwrap_or(TaggedValue::Null);
                let mut state = TaggedValue::Null;
                const MAX_ITERS: usize = 100_000;
                for _ in 0..MAX_ITERS {
                    let input = pair(inc.clone(), state.clone());
                    let (result, pushed) = self.eval_inner_td(
                        &body,
                        ast,
                        node_ref,
                        lang,
                        env,
                        current_node,
                        Some(input),
                    )?;
                    if pushed {
                        env.pop_scope();
                    }
                    if is_either_left(&result) {
                        let out = either_value(&result).cloned().ok_or_else(|| {
                            LangError::engine_node(
                                "Loop: left Either missing value".to_string(),
                                current_node,
                            )
                        })?;
                        return Ok((out, false));
                    }
                    if is_either_right(&result) {
                        state = either_value(&result).cloned().ok_or_else(|| {
                            LangError::engine_node(
                                "Loop: right Either missing value".to_string(),
                                current_node,
                            )
                        })?;
                        continue;
                    }
                    // Not an Either → single-step mode: expect a pair
                    // (Out, State) and return Out.
                    let out = pair_fst(&result).ok_or_else(|| {
                        LangError::engine_node(
                            "Loop: body did not return a pair or Either".to_string(),
                            current_node,
                        )
                    })?;
                    return Ok((out.clone(), false));
                }
                Err(LangError::engine_node(
                    "Loop: exceeded max iterations".to_string(),
                    current_node,
                ))
            }
            "App" => {
                let callee = td
                    .get_term("callee")
                    .expect("App term missing 'callee' field");
                let args = td.get_term_list("args");
                let (callee_val, cpushed) = self.eval_inner_td(
                    &callee,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming.clone(),
                )?;
                if cpushed {
                    env.pop_scope();
                }
                let mut arg_vals = Vec::with_capacity(args.len());
                for arg in &args {
                    let (v, pushed) = self.eval_inner_td(
                        arg,
                        ast,
                        node_ref,
                        lang,
                        env,
                        current_node,
                        incoming.clone(),
                    )?;
                    if pushed {
                        env.pop_scope();
                    }
                    arg_vals.push(v);
                }
                for v in &arg_vals {
                    self.collect_arrow_grades(v, env);
                }
                self.collect_arrow_grades(&callee_val, env);
                // ADR-0060: unpack single array arg for variable-arity call.
                // When the App receives a single argument that is an array,
                // unpack it into individual arguments. This supports the
                // eval.call scheme where proj_rest packs all call args into
                // one array value.
                //
                // ADR-0074: also unpack a single `Pair` (product `***`)
                // argument into individual arguments. This supports the
                // `value >>> function` pipe model where multi-arg callees
                // receive a product: `a *** b >>> f` → `f(a, b)`.
                //
                // Array-pattern arrows receive the array as a single argument,
                // so they must not be unpacked here; `apply_app` destructures
                // the pattern internally.
                let is_array_pattern = matches!(
                    &callee_val,
                    TaggedValue::Arrow {
                        array_pattern: true,
                        ..
                    }
                );
                let actual_args: Vec<TaggedValue> = if is_array_pattern {
                    arg_vals
                } else if arg_vals.len() == 1 {
                    if arg_vals[0].is_array() {
                        arg_vals[0]
                            .as_array()
                            .ok_or_else(|| {
                                LangError::engine_node(
                                    "App: expected array argument".to_string(),
                                    current_node,
                                )
                            })?
                            .iter()
                            .cloned()
                            .collect()
                    } else if matches!(arg_vals[0], TaggedValue::Pair(_)) {
                        flatten_pair(&arg_vals[0])
                    } else {
                        arg_vals
                    }
                } else {
                    arg_vals
                };
                // Dispatch: if the callee is a string, it's a host function
                // name — look it up in the module registry and call it
                // (same path as `call` nodes). Otherwise, it's an
                // arrow-value — apply it as a closure.
                let val = if let Some(name) = callee_val.as_str() {
                    self.call_host_fn(name, &actual_args, current_node)?
                } else {
                    self.apply_app(&callee_val, &actual_args, ast, lang, env)?
                };
                Ok((val, false))
            }
            // `handle body with target = impl_` (ADR-0076): install the
            // handler dynamically — evaluate `impl_` to a handler value,
            // push it onto the engine's handler stack, run `body`, then
            // pop. `target` is retained for diagnostics. While the
            // handler is on the stack, `Term::Op` dispatch finds it via
            // `dispatch_op` (grade-based dynamic dispatch).
            //
            // When `impl_` does not evaluate to a value (it is a pure
            // arrow scheme rather than a handler-producing term), fall
            // back to compile-time handler elimination: rewrite every
            // bare arr(target) in the body to the impl graph via
            // `eliminate_and_check`, which also surfaces unhandled
            // capabilities as a compile-time error. The elimination
            // result is cached per node kind.
            "Handle" => {
                let body = td
                    .get_term("body")
                    .expect("Handle term missing 'body' field");
                let target = td.get_str("target").unwrap_or("").to_string();
                let impl_ = td
                    .get_term("impl_")
                    .expect("Handle term missing 'impl_' field");
                match self.eval_scheme_tagged_td(
                    &impl_,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming.clone(),
                ) {
                    Ok(handler_val) => {
                        let _ = target; // retained for diagnostics
                        self.handler_stack.borrow_mut().push(handler_val);
                        let result = self.eval_inner_td(
                            &body,
                            ast,
                            node_ref,
                            lang,
                            env,
                            current_node,
                            incoming,
                        );
                        self.handler_stack.borrow_mut().pop();
                        result
                    }
                    Err(_) => {
                        let eliminated = {
                            let mut cache = self.eliminated_cache.borrow_mut();
                            if let Some(e) = cache.get(current_node.kind.as_str()) {
                                e.clone()
                            } else {
                                // `eliminate_and_check` operates on the `Term`
                                // enum, so convert back for this one case.
                                let term = td
                                    .to_term()
                                    .map_err(|e| LangError::engine_node(e, current_node))?;
                                let e = term.eliminate_handlers();
                                cache.insert(current_node.kind.to_string(), e.clone());
                                e
                            }
                        };
                        self.eval_inner(
                            &eliminated,
                            ast,
                            node_ref,
                            lang,
                            env,
                            current_node,
                            incoming,
                        )
                    }
                }
            }
            // Bounded iteration (ADR-0098): evaluate `fuel` to a
            // non-negative integer budget and `initial` to the starting
            // state, then while budget remains run `predicate` on the
            // state — `false` breaks early, `true` applies `step` to
            // advance the state. Returns the final state.
            "Iter" => {
                let fuel = td.get_term("fuel").expect("Iter term missing 'fuel' field");
                let predicate = td
                    .get_term("predicate")
                    .expect("Iter term missing 'predicate' field");
                let step = td.get_term("step").expect("Iter term missing 'step' field");
                let initial = td
                    .get_term("initial")
                    .expect("Iter term missing 'initial' field");
                let fuel_val = self.eval_scheme_tagged_td(
                    &fuel,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming.clone(),
                )?;
                let max: u64 = match fuel_val {
                    TaggedValue::Int(i) if i >= 0 => i as u64,
                    TaggedValue::Float(f) if f.is_finite() && f >= 0.0 => f as u64,
                    _ => {
                        return Err(LangError::engine_node(
                            "Iter: fuel did not evaluate to a non-negative number".to_string(),
                            current_node,
                        ));
                    }
                };
                let mut state = self.eval_scheme_tagged_td(
                    &initial,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                for _ in 0..max {
                    let cont = self.eval_scheme_tagged_td(
                        &predicate,
                        ast,
                        node_ref,
                        lang,
                        env,
                        current_node,
                        Some(state.clone()),
                    )?;
                    match cont.as_bool() {
                        Some(true) => {
                            state = self.eval_scheme_tagged_td(
                                &step,
                                ast,
                                node_ref,
                                lang,
                                env,
                                current_node,
                                Some(state),
                            )?;
                        }
                        Some(false) => break,
                        _ => {
                            return Err(LangError::engine_node(
                                "Iter: predicate did not evaluate to a boolean".to_string(),
                                current_node,
                            ));
                        }
                    }
                }
                Ok((state, false))
            }
            // `Child { role }` (ADR-0081): resolve the named child role to a
            // child position via the language view, project that child, and
            // evaluate it with a fresh incoming context. Missing roles and
            // missing children are silently `Null`, matching the optional-child
            // behavior of `proj_child_or`.
            "Child" => {
                let role = td.get_str("role").unwrap_or("");
                let idx = match lang.child_role_index(&current_node.kind, role) {
                    Ok(Some(idx)) => idx,
                    _ => {
                        return Ok((TaggedValue::Null, false));
                    }
                };
                let children = current_node
                    .payload
                    .get("children")
                    .and_then(|v| v.as_array());
                let Some(child_arena) = children
                    .and_then(|arr| arr.get(idx))
                    .and_then(|v| v.as_u64())
                else {
                    return Ok((TaggedValue::Null, false));
                };
                let child_ref = AstRef::new(child_arena as usize);
                let val = self.eval_node_inner(ast, child_ref, lang, env)?;
                Ok((val, false))
            }
            "Rewrite" => Err(LangError::NotImplemented),
            "Path" => Ok((incoming.unwrap_or(TaggedValue::from(Value::Null)), false)),
            "Equiv" => {
                let a = td.get_term("a").expect("Equiv term missing 'a' field");
                let b = td.get_term("b").expect("Equiv term missing 'b' field");
                let inc = incoming.unwrap_or(TaggedValue::from(Value::Null));
                let (av, _) = self.eval_inner_td(
                    &a,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    Some(inc.clone()),
                )?;
                let (bv, _) =
                    self.eval_inner_td(&b, ast, node_ref, lang, env, current_node, Some(inc))?;
                let val = Value::Bool(tagged_to_value(&av) == tagged_to_value(&bv));
                Ok((TaggedValue::from(val), false))
            }
            "Const" => {
                let value = td
                    .get("value")
                    .cloned()
                    .expect("Const term missing 'value' field");
                Ok((TaggedValue::from(value), false))
            }
            "Src" => {
                let cell = td.get_term("cell").expect("Src term missing 'cell' field");
                let v = self.eval_scheme_tagged_td(
                    &cell,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                Ok((cell2_src(&v), false))
            }
            "Tgt" => {
                let cell = td.get_term("cell").expect("Tgt term missing 'cell' field");
                let v = self.eval_scheme_tagged_td(
                    &cell,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                Ok((cell2_tgt(&v), false))
            }
            "RecordProj" => {
                let record = td
                    .get_term("record")
                    .expect("RecordProj term missing 'record' field");
                let field = td.get_str("field").unwrap_or("");
                let record_val = self.eval_scheme_tagged_td(
                    &record,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                Ok((record_proj(&record_val, field), false))
            }
            "RecordWith" => {
                let record = td
                    .get_term("record")
                    .expect("RecordWith term missing 'record' field");
                let field = td.get_str("field").unwrap_or("");
                let value = td
                    .get_term("value")
                    .expect("RecordWith term missing 'value' field");
                let record_val = self.eval_scheme_tagged_td(
                    &record,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming.clone(),
                )?;
                let value_val = self.eval_scheme_tagged_td(
                    &value,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                let mut map = record_val.as_object().cloned().unwrap_or_default();
                map.insert(Rc::from(field), value_val);
                Ok((TaggedValue::Obj(Rc::new(map)), false))
            }
            "Tag" => {
                let tag = td.get_str("tag").unwrap_or("");
                let value = td
                    .get_term("value")
                    .expect("Tag term missing 'value' field");
                let val = self.eval_scheme_tagged_td(
                    &value,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                let tagged = tag_value(tag, &tagged_to_value(&val))
                    .map(TaggedValue::from)
                    .unwrap_or(TaggedValue::Null);
                Ok((tagged, false))
            }
            "CellId" => {
                let arrow = td
                    .get_term("arrow")
                    .expect("CellId term missing 'arrow' field");
                let v = self.eval_scheme_tagged_td(
                    &arrow,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                Ok((cell2_id(&v), false))
            }
            "CellCompV" => {
                let first = td
                    .get_term("first")
                    .expect("CellCompV term missing 'first' field");
                let second = td
                    .get_term("second")
                    .expect("CellCompV term missing 'second' field");
                let fst = self.eval_scheme_tagged_td(
                    &first,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming.clone(),
                )?;
                let snd = self.eval_scheme_tagged_td(
                    &second,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                Ok((cell2_comp_v(&fst, &snd), false))
            }
            "Whisker" => {
                let arrow = td
                    .get_term("arrow")
                    .expect("Whisker term missing 'arrow' field");
                let cell = td
                    .get_term("cell")
                    .expect("Whisker term missing 'cell' field");
                let side_str = match td.get_str("side").unwrap_or("") {
                    "Left" => "Left",
                    "Right" => "Right",
                    other => {
                        return Err(LangError::engine_node(
                            format!("Whisker: unknown side '{other}'"),
                            current_node,
                        ));
                    }
                };
                let arrow_val = self.eval_scheme_tagged_td(
                    &arrow,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming.clone(),
                )?;
                let cell_val = self.eval_scheme_tagged_td(
                    &cell,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                Ok((cell2_whisker(side_str, &arrow_val, &cell_val), false))
            }
            "Refine" => {
                let predicate = td
                    .get_term("predicate")
                    .expect("Refine term missing 'predicate' field");
                let inc = incoming.unwrap_or(TaggedValue::Null);
                let pred = self.eval_scheme_tagged_td(
                    &predicate,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    Some(inc.clone()),
                )?;
                if is_truthy(&pred) {
                    Ok((inc, false))
                } else {
                    Err(LangError::engine_node(
                        "Refine: predicate not satisfied".to_string(),
                        current_node,
                    ))
                }
            }
            "Cobind" => {
                let grade = td
                    .get_term("grade")
                    .expect("Cobind term missing 'grade' field");
                let body = td
                    .get_term("body")
                    .expect("Cobind term missing 'body' field");
                let grade_val = self.eval_scheme_tagged_td(
                    &grade,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming.clone(),
                )?;
                self.collect_grades(&grade_val, env);
                let v = self.eval_scheme_tagged_td(
                    &body,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                Ok((v, false))
            }
            "Univalence" => {
                let equiv = td
                    .get_term("equiv")
                    .expect("Univalence term missing 'equiv' field");
                let equiv_val = self.eval_scheme_tagged_td(
                    &equiv,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                if let Some(tag) = equiv_val.get("__tag__").and_then(|v| v.as_str()) {
                    if tag == "Equiv" {
                        let a = equiv_val.get("a").cloned().unwrap_or(TaggedValue::Null);
                        let b = equiv_val.get("b").cloned().unwrap_or(TaggedValue::Null);
                        let path = equiv_val.get("path").cloned().unwrap_or(TaggedValue::Null);
                        let mut map = FxHashMap::default();
                        map.insert(Rc::from("__tag__"), TaggedValue::Str(Rc::from("Path")));
                        map.insert(Rc::from("a"), a);
                        map.insert(Rc::from("b"), b);
                        map.insert(Rc::from("path"), path);
                        return Ok((TaggedValue::Obj(Rc::new(map)), false));
                    }
                }
                Ok((equiv_val, false))
            }
            "Glue" => {
                let family = td
                    .get_term("family")
                    .expect("Glue term missing 'family' field");
                let path = td.get_term("path").expect("Glue term missing 'path' field");
                let _ = self.eval_scheme_tagged_td(
                    &path,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming.clone(),
                )?;
                let family_val = self.eval_scheme_tagged_td(
                    &family,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                Ok((family_val, false))
            }
            "Fibrant" => {
                let base = td
                    .get_term("base")
                    .expect("Fibrant term missing 'base' field");
                let family = td
                    .get_term("family")
                    .expect("Fibrant term missing 'family' field");
                let _ = self.eval_scheme_tagged_td(
                    &family,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming.clone(),
                )?;
                let base_val = self.eval_scheme_tagged_td(
                    &base,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )?;
                Ok((base_val, false))
            }
            "JElim" => {
                let path = td
                    .get_term("path")
                    .expect("JElim term missing 'path' field");
                let motive = td
                    .get_term("motive")
                    .expect("JElim term missing 'motive' field");
                let base = td
                    .get_term("base")
                    .expect("JElim term missing 'base' field");
                let path_val = self.eval_scheme_tagged_td(
                    &path,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming.clone(),
                )?;
                let base_val = self.eval_scheme_tagged_td(
                    &base,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming.clone(),
                )?;
                if let Some(tag) = path_val.get("__tag__").and_then(|v| v.as_str()) {
                    if tag == "Path" || tag == "Rewrite" {
                        let motive_val = self.eval_scheme_tagged_td(
                            &motive,
                            ast,
                            node_ref,
                            lang,
                            env,
                            current_node,
                            Some(base_val),
                        )?;
                        return Ok((motive_val, false));
                    }
                }
                Ok((base_val, false))
            }
            _ => Err(LangError::engine_node(
                format!("eval_inner_td: unknown tag '{}'", td.tag()),
                current_node,
            )),
        }
    }

    /// Evaluate an `Arr` combinator. Returns the value and whether this
    /// Arr pushed a scope (`extend_env` pushes; the caller — `Seq` or
    /// `eval_scheme` — is responsible for popping it).
    #[allow(clippy::too_many_arguments)]
    fn eval_arr(
        &self,
        op: &str,
        args: &[TermData],
        source: &ArrSource,
        lit: Option<&Value>,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<TaggedValue>,
    ) -> Result<(TaggedValue, bool), LangError> {
        // `$payload` sentinel: the op name is read from the current
        // node's payload — a bare string payload, or the `name`/`op`
        // field of an object payload (e.g. `arr op` captures the
        // identifier text via `payload_tokens` into `payload.name`).
        // Resolved once here so both the derived-step interception and
        // `apply_native` dispatch see the real op name.
        let from_payload = op == "$payload";
        let resolved_op;
        let op = if from_payload {
            resolved_op = current_node
                .payload
                .as_str()
                .map(str::to_string)
                .or_else(|| {
                    current_node
                        .payload
                        .get("name")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .or_else(|| {
                    current_node
                        .payload
                        .get("op")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .ok_or_else(|| {
                    LangError::engine_node(
                        "expected string payload for $payload op".to_string(),
                        current_node,
                    )
                })?;
            resolved_op.as_str()
        } else {
            op
        };
        // Derived-pattern composite ops (while_step/for_step/fix_step)
        // carry sub-arrows as args that must be evaluated with the
        // *current value* extracted from the incoming pair, not with the
        // pair itself. The generic arg evaluation below would feed the
        // pair to the sub-schemes, which is wrong for these ops.
        // Intercept before it. These are handler-interpreted derived
        // operations (ADR-0052 §"Derived forms"): the description is
        // primitive-only (`Loop` + `Arr`); the interpretation routes the
        // value through the carried cond/body sub-arrows. This is NOT a
        // new kernel primitive — `while_step` is an `Arr` op (one of the
        // six primitives); its interpretation is the derived pattern.
        if matches!(source, ArrSource::Native)
            && matches!(op, "while_step" | "for_step" | "fix_step" | "loop_step")
        {
            return self.eval_derived_step(
                op,
                args,
                ast,
                node_ref,
                lang,
                env,
                current_node,
                incoming,
            );
        }
        // Evaluate sub-scheme args (each gets the same incoming context).
        let mut arg_vals = Vec::with_capacity(args.len());
        for arg in args {
            let (v, pushed) = self.eval_inner_td(
                arg,
                ast,
                node_ref,
                lang,
                env,
                current_node,
                incoming.clone(),
            )?;
            if pushed {
                env.pop_scope();
            }
            arg_vals.push(v);
        }
        // Grade-based dynamic dispatch: for `Native` and `Foreign` ops,
        // search the handler stack (and the incoming value's own `impls`)
        // for a `Handler` implementation of the op before falling back to
        // the static `source` dispatch below.
        let dispatch_name = match source {
            ArrSource::Native => Some(op.to_string()),
            ArrSource::Foreign(name) => Some(name.clone()),
            _ => None,
        };
        if let Some(name) = dispatch_name {
            if let Some(res) = self.dispatch_op(
                &name,
                incoming.as_ref(),
                ast,
                node_ref,
                lang,
                env,
                current_node,
            )? {
                return Ok(res);
            }
        }
        match source {
            ArrSource::Native => {
                // `arr op` (`op: "$payload"`): the scheme declares no
                // args — the incoming value is the lifted op's operand.
                // A `Pair` operand is flattened into positional args so
                // `arr add` applied to `(a, b)` yields `add a b`. Unary
                // ops that read `incoming` (e.g. `inc`) still see it
                // through the `incoming` parameter.
                if from_payload && arg_vals.is_empty() {
                    match &incoming {
                        Some(v) => {
                            let op_args = flatten_pair(v);
                            return self.apply_native(
                                op,
                                &op_args,
                                lit,
                                ast,
                                node_ref,
                                lang,
                                env,
                                current_node,
                                incoming,
                            );
                        }
                        None => {
                            return Err(LangError::engine_node(
                                format!("arr: op '{}' has no input value", op),
                                current_node,
                            ));
                        }
                    }
                }
                self.apply_native(
                    op,
                    &arg_vals,
                    lit,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    incoming,
                )
            }
            ArrSource::Foreign(name) => {
                let modules = self.modules.as_ref().ok_or_else(|| {
                    LangError::engine_node(format!("unregistered foreign: {}", name), current_node)
                })?;
                let func = modules.lookup(name).ok_or_else(|| {
                    LangError::engine_node(format!("unregistered foreign: {}", name), current_node)
                })?;
                // Convert TaggedValue args to Value for the host function
                // boundary (host functions still use serde_json::Value).
                let value_args: Vec<Value> = arg_vals.iter().map(tagged_to_value).collect();
                let mut pctx = handl_platform::EvalContext::new();
                pctx.set_value_callable(Rc::new(TreeWalkCallable));
                let val = func
                    .call(&value_args, &mut pctx)
                    .map_err(|e| LangError::engine_node(e.to_string(), current_node))?;
                Ok((TaggedValue::from(val), false))
            }
            ArrSource::DynamicForeign => self.apply_native(
                op,
                &arg_vals,
                lit,
                ast,
                node_ref,
                lang,
                env,
                current_node,
                incoming,
            ),
            ArrSource::Judgment => Err(LangError::engine_node(
                "judgment ops are not supported by the tree-walk engine".to_string(),
                current_node,
            )),
        }
    }

    /// Grade-based `Op` dispatch: search the runtime handler stack
    /// (innermost-last) for a handler value whose `impls` map provides an
    /// implementation for `op`, then — when no stack handler applies —
    /// the incoming value's own `impls` field (a tagged value
    /// `{"__tag__": <sort>, "grades": [...], "impls": {...}}` may embed
    /// per-grade op implementations).
    ///
    /// A `Handler` value is a tagged object
    /// `{"__tag__": "Handler", "impls": {"<op>": <Term>, ...}}` where each
    /// `impls` entry is a serialized [`Term`] run with the op's `incoming`
    /// value. Returns `Ok(None)` when no handler provides `op`, so the
    /// caller can fall back to native `op` dispatch.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn dispatch_op(
        &self,
        op: &str,
        incoming: Option<&TaggedValue>,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
    ) -> Result<Option<(TaggedValue, bool)>, LangError> {
        let impl_json = {
            let stack = self.handler_stack.borrow();
            stack.iter().rev().find_map(|h| handler_impl(h, op))
        }
        .or_else(|| incoming.and_then(|v| handler_impl(v, op)));
        let Some(impl_json) = impl_json else {
            return Ok(None);
        };
        let impl_term: Term = serde_json::from_value(impl_json).map_err(|e| {
            LangError::engine_node(
                format!("handler impl for op '{op}' is not a Term: {e}"),
                current_node,
            )
        })?;
        let (val, pushed) = self.eval_inner(
            &impl_term,
            ast,
            node_ref,
            lang,
            env,
            current_node,
            incoming.cloned(),
        )?;
        Ok(Some((val, pushed)))
    }

    /// Evaluate a derived-pattern composite step op
    /// (`while_step`/`for_step`/`fix_step`). These are handler-interpreted
    /// derived operations (ADR-0052 §"Derived forms"): the `Term`
    /// description is built from primitives (`Loop` + `Arr`), and the
    /// handler interprets the composite op by recursively evaluating the
    /// carried sub-arrows (`cond`/`body`) with the *current value*
    /// extracted from the incoming pair — not the pair itself. Each
    /// returns an `Either` consumed by the multi-step `Loop` handler:
    /// `left` terminates, `right` feeds the new state back.
    #[allow(clippy::too_many_arguments)]
    fn eval_derived_step(
        &self,
        op: &str,
        raw_args: &[TermData],
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<TaggedValue>,
    ) -> Result<(TaggedValue, bool), LangError> {
        let inc = incoming.unwrap_or(TaggedValue::Null);
        let in_val = pair_fst(&inc).cloned().unwrap_or(TaggedValue::Null);
        let state = pair_snd(&inc).cloned().unwrap_or(TaggedValue::Null);
        match op {
            "while_step" => {
                // args = [cond, body]. The current value is `in` on the
                // first step (state is Null) and `state` thereafter. Run
                // `cond` on it; if truthy, run `body` and continue with
                // its result; else terminate with the current value.
                let cond = raw_args.first().ok_or_else(|| {
                    LangError::engine_node(
                        "while_step: missing cond sub-arrow".to_string(),
                        current_node,
                    )
                })?;
                let body = raw_args.get(1).ok_or_else(|| {
                    LangError::engine_node(
                        "while_step: missing body sub-arrow".to_string(),
                        current_node,
                    )
                })?;
                let current = if state.is_null() { in_val } else { state };
                let cond_val = self.eval_scheme_tagged_td(
                    cond,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    Some(current.clone()),
                )?;
                if is_truthy(&cond_val) {
                    let new_val = self.eval_scheme_tagged_td(
                        body,
                        ast,
                        node_ref,
                        lang,
                        env,
                        current_node,
                        Some(current),
                    )?;
                    Ok((either_right(new_val), false))
                } else {
                    Ok((either_left(current), false))
                }
            }
            "for_step" => {
                // args = [body]. State is pair(counter, acc). On the first
                // step (state Null), initialize from `in = pair(count,
                // initial)`. While counter > 0, run `body` on `acc` and
                // continue with pair(counter-1, new_acc); else terminate
                // with `acc`.
                let body = raw_args.first().ok_or_else(|| {
                    LangError::engine_node(
                        "for_step: missing body sub-arrow".to_string(),
                        current_node,
                    )
                })?;
                let (counter, acc) = if state.is_null() {
                    let count = pair_fst(&in_val).cloned().ok_or_else(|| {
                        LangError::engine_node(
                            "for_step: input must be pair(count, initial) when state is Null"
                                .to_string(),
                            current_node,
                        )
                    })?;
                    let initial = pair_snd(&in_val).cloned().unwrap_or(TaggedValue::Null);
                    (count, initial)
                } else {
                    let c = pair_fst(&state).cloned().ok_or_else(|| {
                        LangError::engine_node(
                            "for_step: state must be pair(counter, acc)".to_string(),
                            current_node,
                        )
                    })?;
                    let a = pair_snd(&state).cloned().unwrap_or(TaggedValue::Null);
                    (c, a)
                };
                let count_n = as_num(&counter).map_err(|e| e.with_node(current_node))?;
                if count_n > Num::Int(0) {
                    let new_acc = self.eval_scheme_tagged_td(
                        body,
                        ast,
                        node_ref,
                        lang,
                        env,
                        current_node,
                        Some(acc),
                    )?;
                    let new_counter = num_value(count_n - Num::Int(1));
                    Ok((either_right(pair(new_counter, new_acc)), false))
                } else {
                    Ok((either_left(acc), false))
                }
            }
            "fix_step" => {
                // args = [body]. The current value is `in` on the first
                // step (state Null) and `state` thereafter. Run `body`; if
                // the result equals the current value, the fixpoint is
                // reached — terminate; else continue with the result.
                let body = raw_args.first().ok_or_else(|| {
                    LangError::engine_node(
                        "fix_step: missing body sub-arrow".to_string(),
                        current_node,
                    )
                })?;
                let current = if state.is_null() { in_val } else { state };
                let new_val = self.eval_scheme_tagged_td(
                    body,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    Some(current.clone()),
                )?;
                if handl_value::eq(&new_val, &current) {
                    Ok((either_left(new_val), false))
                } else {
                    Ok((either_right(new_val), false))
                }
            }
            "loop_step" => {
                // Ergonomic loop surface (ADR-0052). args = [body_proc].
                // The current state is `in` on the first step (state is
                // Null) and `state` thereafter. The body proc is applied to
                // the current state; it returns:
                //   { state: v }  → continue with new state v
                //   { out: v }    → terminate with v (backward compat)
                //   bare value    → terminate with the value (v0.1 `return v`)
                //
                // Param destructuring: if the body proc has multiple params
                // and the current state is an object, bind each field to a
                // param by name (e.g. `[n, acc] ~> ...` with state
                // `{ n: 5, acc: 1 }` binds n=5, acc=1).
                //
                // NOTE: When `loop_step` is the direct body of a `Loop`
                // combinator, the closure evaluation is hoisted outside
                // the loop by the `Term::Loop` arm in `eval_inner` (see
                // the optimization there). This path is only reached when
                // `loop_step` appears in a non-hoisted position.
                let body_proc = raw_args.first().ok_or_else(|| {
                    LangError::engine_node("loop_step: missing body proc".to_string(), current_node)
                })?;
                let current = if state.is_null() { in_val } else { state };
                let closure_val = self.eval_scheme_tagged_td(
                    body_proc,
                    ast,
                    node_ref,
                    lang,
                    env,
                    current_node,
                    None,
                )?;
                let args = build_loop_step_args(&closure_val, &current);
                let result = self.apply_app(&closure_val, &args, ast, lang, env)?;
                Ok((interpret_loop_step_result(&result), false))
            }
            _ => Err(LangError::engine_node(
                format!("unknown derived step op: {}", op),
                current_node,
            )),
        }
    }

    /// Apply a native `arr` op. Returns the value and whether a scope was
    /// pushed (`extend_env` pushes).
    #[allow(clippy::too_many_arguments)]
    fn apply_native(
        &self,
        op: &str,
        args: &[TaggedValue],
        lit: Option<&Value>,
        ast: &Ast,
        _node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<TaggedValue>,
    ) -> Result<(TaggedValue, bool), LangError> {
        match op {
            "const" => Ok((
                TaggedValue::from(lit.cloned().unwrap_or(Value::Null)),
                false,
            )),
            "desugar_error" => {
                // Raise a LangError carrying the reason from the node's
                // payload. This op is the eval scheme for the
                // `desugar-error` node kind, which the desugar pass emits
                // when a declaration fails a compile-time check (e.g.
                // strict positivity violation in `data`). The op name and
                // node kind are data-driven (declared in eval.yaml /
                // ast.yaml), not hardcoded kind checks.
                //
                // The error message is read from `current_node.payload`
                // (`kind`/`reason` fields) for `desugar-error` nodes, or
                // falls back to the scheme's `lit` payload for nodes like
                // `data` that carry their own payload shape (`name`/
                // `children`) but still use the `desugar_error` op.
                let (reason, kind) = if let Some(lit_obj) = lit.and_then(|v| v.as_object()) {
                    let reason = current_node
                        .payload
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .or_else(|| lit_obj.get("reason").and_then(|v| v.as_str()))
                        .unwrap_or("unknown desugar error");
                    let kind = current_node
                        .payload
                        .get("kind")
                        .and_then(|v| v.as_str())
                        .or_else(|| lit_obj.get("kind").and_then(|v| v.as_str()))
                        .unwrap_or("desugar-error");
                    (reason, kind)
                } else {
                    let reason = current_node
                        .payload
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown desugar error");
                    let kind = current_node
                        .payload
                        .get("kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("desugar-error");
                    (reason, kind)
                };
                Err(LangError::engine_node(
                    format!("{}: {}", kind, reason),
                    current_node,
                ))
            }
            "id" => {
                // Identity: return the incoming value, or the single arg.
                Ok((
                    incoming
                        .or_else(|| args.first().cloned())
                        .unwrap_or(TaggedValue::Null),
                    false,
                ))
            }
            "proj_env" => {
                let name = lit.and_then(|v| v.as_str()).ok_or_else(|| {
                    LangError::engine_node(
                        "proj_env: missing name literal".to_string(),
                        current_node,
                    )
                })?;
                let var_name = if name == "$payload" {
                    current_node
                        .payload
                        .as_str()
                        .ok_or_else(|| {
                            LangError::engine_node(
                                "expected string payload for $payload proj_env".to_string(),
                                current_node,
                            )
                        })?
                        .to_string()
                } else {
                    name.to_string()
                };
                // Unbound → name as a string (allows identifiers like
                // `eval` to flow into `App` host-function dispatch).
                let val = env
                    .lookup(&var_name)
                    .cloned()
                    .unwrap_or_else(|| TaggedValue::Str(Rc::from(var_name)));
                Ok((val, false))
            }
            "proj_child" => {
                let idx = lit.and_then(|v| v.as_u64()).ok_or_else(|| {
                    LangError::engine_node(
                        "proj_child: missing index literal".to_string(),
                        current_node,
                    )
                })?;
                let children = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "proj_child: expected children array in payload".to_string(),
                            current_node,
                        )
                    })?;
                let arena_idx = children
                    .get(idx as usize)
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        LangError::engine_node(
                            format!("proj_child: child {} not found", idx),
                            current_node,
                        )
                    })?;
                let target_ref = AstRef::new(arena_idx as usize);
                let val = self.eval_node_inner(ast, target_ref, lang, env)?;
                Ok((val, false))
            }
            "proj_child_or" => {
                // Like `proj_child`, but tolerates an absent child (e.g. the
                // missing `else` of a else-less `if`): when the child index
                // is out of range the result is `Null` instead of an error.
                let idx = lit.and_then(|v| v.as_u64()).ok_or_else(|| {
                    LangError::engine_node(
                        "proj_child_or: missing index literal".to_string(),
                        current_node,
                    )
                })?;
                let arena_idx = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .and_then(|children| children.get(idx as usize))
                    .and_then(|v| v.as_u64());
                match arena_idx {
                    Some(i) => {
                        let val = self.eval_node_inner(ast, AstRef::new(i as usize), lang, env)?;
                        Ok((val, false))
                    }
                    None => Ok((TaggedValue::Null, false)),
                }
            }
            "extend_env" => {
                let name = lit.and_then(|v| v.as_str()).ok_or_else(|| {
                    LangError::engine_node("extend_env: missing name literal", current_node)
                })?;
                let bind_name = if name == "$payload" {
                    current_node
                        .payload
                        .get("name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            LangError::engine_node(
                                "expected name in payload for $payload extend_env",
                                current_node,
                            )
                        })?
                        .to_string()
                } else {
                    name.to_string()
                };
                // Push a scope, bind the incoming value, return it so it
                // flows through. The matching pop is the caller's
                // responsibility (Seq or eval_scheme).
                env.push_scope();
                let val = incoming.unwrap_or(TaggedValue::Null);
                env.define(&bind_name, val.clone());
                Ok((val, true))
            }
            "tag" => {
                // Branch lowering: truthy → left Either, falsy → right.
                let v =
                    incoming.unwrap_or_else(|| args.first().cloned().unwrap_or(TaggedValue::Null));
                if is_truthy(&v) {
                    Ok((either_left(v), false))
                } else {
                    Ok((either_right(v), false))
                }
            }
            "closure" => {
                // Read params/body from the current node's payload (rich
                // or Pratt-parsed shapes).
                let payload = &current_node.payload;
                let (params, rest, body, array_pattern) = if let (Some(p), Some(b)) = (
                    payload.get("params").and_then(|v| v.as_array()),
                    payload.get("body").and_then(|v| v.as_u64()),
                ) {
                    let p_names: Vec<String> = p
                        .iter()
                        .map(|v| {
                            v.as_str()
                                .ok_or_else(|| {
                                    LangError::engine_node(
                                        "closure: param must be a string".to_string(),
                                        current_node,
                                    )
                                })
                                .map(String::from)
                        })
                        .collect::<Result<_, _>>()?;
                    let rest_name = payload
                        .get("rest")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    (p_names, rest_name, b as usize, false)
                } else if let Some(children) = payload.get("children").and_then(|c| c.as_array()) {
                    let param_idx = children.first().and_then(|v| v.as_u64()).ok_or_else(|| {
                        LangError::engine_node(
                            "closure: missing param child".to_string(),
                            current_node,
                        )
                    })? as usize;
                    let body_idx = children.get(1).and_then(|v| v.as_u64()).ok_or_else(|| {
                        LangError::engine_node(
                            "closure: missing body child".to_string(),
                            current_node,
                        )
                    })? as usize;
                    let param_node = ast.at(AstRef::new(param_idx))?;
                    // `param_names` resolves the fixed params and an
                    // optional rest param (`...name`) data-driven via
                    // `pattern_role`/variadic-role metadata (ADR-0081).
                    let (param_names, rest_name) = lang
                        .param_names(ast, param_node)
                        .map_err(|e| LangError::engine_node(e.to_string(), current_node))?;
                    let array_pattern = lang.is_array_pattern(param_node);
                    (param_names, rest_name, body_idx, array_pattern)
                } else {
                    return Err(LangError::engine_node(
                        "closure: missing params/body in payload".to_string(),
                        current_node,
                    ));
                };
                let captured = env.capture();
                // ADR-0106 item 2 (step 1): carry the body as explicit,
                // serializable structure so the arrow is self-describing.
                // `Ast::subtree` extracts a standalone arena for the body
                // (children remapped) and `ast_value` wraps it as
                // `{"__ast__": <Ast>}` — the same encoding used for vau-bound
                // unevaluated expressions. Additive only: the apply path
                // still uses `body`/`home`, and extraction failure degrades
                // to `None` rather than failing the closure creation.
                let body_def = ast
                    .subtree(AstRef::new(body))
                    .ok()
                    .map(|sub| Rc::new(TaggedValue::from(handl_language::ast_value(&sub))));
                let closure = TaggedValue::Arrow {
                    params: Rc::from(params),
                    rest: rest.map(Rc::from),
                    body,
                    env: captured,
                    home: None,
                    array_pattern,
                    body_def,
                };
                Ok((closure, false))
            }
            "array" => {
                // Two payload shapes:
                // - Rich: `{"elements": [v1, v2, ...]}` — literal values.
                // - Pratt-parsed: `{"children": [elem_idx, ...]}` — each
                //   element is an arena index to evaluate.
                if let Some(elements) = current_node.payload.get("elements") {
                    Ok((TaggedValue::from(elements.clone()), false))
                } else if let Some(children) = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                {
                    let mut result = Vec::with_capacity(children.len());
                    for entry in children {
                        let idx = entry.as_u64().ok_or_else(|| {
                            LangError::engine_node(
                                "array: element index must be a number".to_string(),
                                current_node,
                            )
                        })? as usize;
                        let child_ref = AstRef::new(idx);
                        // Spread: if the child is a `rest` node (from
                        // `...expr` syntax), flatten its value into the
                        // array if it's an array.
                        if ast.get(child_ref).map(|n| n.kind.as_str()) == Some("rest") {
                            let val = self.eval_node_inner(ast, child_ref, lang, env)?;
                            if let Some(arr) = val.as_array() {
                                result.extend(arr.iter().cloned());
                            } else {
                                result.push(val);
                            }
                        } else {
                            result.push(self.eval_node_inner(ast, child_ref, lang, env)?);
                        }
                    }
                    Ok((TaggedValue::Arr(Rc::from(result)), false))
                } else {
                    Err(LangError::engine_node(
                        "array: missing elements/children in payload".to_string(),
                        current_node,
                    ))
                }
            }
            "object" => {
                // Two payload shapes:
                // - Rich: `{"entries": [{"key": "k", "value": v}, ...]}`.
                // - Pratt-parsed: `{"children": [entry_idx, ...]}` where
                //   each entry node has `{"children": [key_idx,
                //   value_idx]}`.
                if let Some(entries) = current_node
                    .payload
                    .get("entries")
                    .and_then(|v| v.as_array())
                {
                    let mut obj = FxHashMap::default();
                    for entry in entries {
                        let key = entry.get("key").and_then(|v| v.as_str()).ok_or_else(|| {
                            LangError::engine_node(
                                "object: entry missing key".to_string(),
                                current_node,
                            )
                        })?;
                        let value = entry.get("value").ok_or_else(|| {
                            LangError::engine_node(
                                "object: entry missing value".to_string(),
                                current_node,
                            )
                        })?;
                        if handl_language::is_reserved_value_key(key) {
                            return Err(LangError::engine_node(
                                format!("object: key '{}' is reserved", key),
                                current_node,
                            ));
                        }
                        obj.insert(Rc::from(key), TaggedValue::from(value.clone()));
                    }
                    Ok((TaggedValue::Obj(Rc::new(obj)), false))
                } else if let Some(children) = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                {
                    let start = lang
                        .child_role_index(&current_node.kind, "entries")?
                        .unwrap_or(0);
                    // ADR-0074 v0.1: `{ expr }` (bare-expression block) vs
                    // `{ key: val }` (object). When ALL children from the
                    // `entries` role are non-`entry` nodes (bare expressions),
                    // treat as a block: evaluate each and return the last value.
                    // When any child is an `entry` node, treat as an object.
                    let all_bare = children.iter().skip(start).all(|c| {
                        c.as_u64()
                            .and_then(|idx| ast.at(AstRef::new(idx as usize)).ok())
                            .map(|n| n.kind != "entry")
                            .unwrap_or(true)
                    });
                    if all_bare && !children.is_empty() {
                        // Block semantics: evaluate all, return last.
                        let mut last = TaggedValue::Null;
                        for entry in children.iter().skip(start) {
                            let idx = entry.as_u64().ok_or_else(|| {
                                LangError::engine_node(
                                    "object: entry index must be a number".to_string(),
                                    current_node,
                                )
                            })? as usize;
                            last = self.eval_node_inner(ast, AstRef::new(idx), lang, env)?;
                        }
                        return Ok((last, false));
                    }
                    let mut obj = FxHashMap::default();
                    for entry in children.iter().skip(start) {
                        let entry_idx = entry.as_u64().ok_or_else(|| {
                            LangError::engine_node(
                                "object: entry index must be a number".to_string(),
                                current_node,
                            )
                        })? as usize;
                        let entry_node = ast.at(AstRef::new(entry_idx))?;
                        // Spread entry: if the child is not an `entry` node,
                        // evaluate it and merge its fields into the object.
                        // This implements `{ ...s, field: val }` syntax.
                        if entry_node.kind != "entry" {
                            let spread_val =
                                self.eval_node_inner(ast, AstRef::new(entry_idx), lang, env)?;
                            if let Some(spread_obj) = spread_val.as_object() {
                                for (k, v) in spread_obj {
                                    obj.insert(Rc::clone(k), v.clone());
                                }
                            }
                            continue;
                        }
                        let entry_children = entry_node
                            .payload
                            .get("children")
                            .and_then(|c| c.as_array())
                            .ok_or_else(|| {
                                LangError::engine_node(
                                    "object: entry node missing children".to_string(),
                                    current_node,
                                )
                            })?;
                        let key_role = lang.child_role_index(&entry_node.kind, "key")?.unwrap_or(0);
                        let val_role = lang
                            .child_role_index(&entry_node.kind, "value")?
                            .unwrap_or(1);
                        let key_idx = entry_children
                            .get(key_role)
                            .and_then(|v| v.as_u64())
                            .ok_or_else(|| {
                                LangError::engine_node(
                                    "object: entry missing key child".to_string(),
                                    current_node,
                                )
                            })? as usize;
                        let val_idx = entry_children
                            .get(val_role)
                            .and_then(|v| v.as_u64())
                            .ok_or_else(|| {
                                LangError::engine_node(
                                    "object: entry missing value child".to_string(),
                                    current_node,
                                )
                            })? as usize;
                        let key_node = ast.at(AstRef::new(key_idx))?;
                        // Static vs computed key is declared data-driven:
                        // the `entry` shape's `literal_children` list for
                        // the `key` role (ADR-0081) names the child kinds
                        // whose payload is the key text — don't evaluate
                        // them. This makes `{ n: 5 }` use key "n" and
                        // `{ "s": 5 }` use key "s". Otherwise (computed
                        // key), evaluate the key expression and use its
                        // string representation. Bracket computed keys
                        // `[expr]:` are desugared to `paren { expr }`
                        // before eval (ADR-0058), so they fall into the
                        // `else` branch. Role-less shapes fall back to the
                        // legacy string-payload heuristic.
                        let is_literal_key = lang
                            .literal_child(&entry_node.kind, "key", &key_node.kind)
                            .unwrap_or_else(|| key_node.payload.is_string());
                        let key = if is_literal_key {
                            let key_text = key_node.payload.as_str().ok_or_else(|| {
                                LangError::engine_node(
                                    "object: literal key missing string payload".to_string(),
                                    current_node,
                                )
                            })?;
                            key_text.to_string()
                        } else {
                            let key_val =
                                self.eval_node_inner(ast, AstRef::new(key_idx), lang, env)?;
                            if let Some(s) = key_val.as_str() {
                                s.to_string()
                            } else {
                                tagged_to_value(&key_val).to_string()
                            }
                        };
                        let value = self.eval_node_inner(ast, AstRef::new(val_idx), lang, env)?;
                        if handl_language::is_reserved_value_key(&key) {
                            return Err(LangError::engine_node(
                                format!("object: key '{}' is reserved", key),
                                current_node,
                            ));
                        }
                        obj.insert(Rc::from(key.as_str()), value);
                    }
                    Ok((TaggedValue::Obj(Rc::new(obj)), false))
                } else {
                    Err(LangError::engine_node(
                        "object: missing entries/children in payload".to_string(),
                        current_node,
                    ))
                }
            }
            // `index` — `target[index]` subscript. Operates on the two
            // evaluated children (the `index` node kind carries children,
            // not payload): array+int → element, object+string → value.
            // Unlike `field_access`, a missing object key is an error.
            "index" => {
                let target = args.first().cloned().unwrap_or(TaggedValue::Null);
                let index = args.get(1).cloned().unwrap_or(TaggedValue::Null);
                match (target, index) {
                    (TaggedValue::Arr(arr), index) => {
                        let i = index
                            .as_f64()
                            .filter(|n| *n >= 0.0 && n.fract() == 0.0)
                            .map(|n| n as usize)
                            .ok_or_else(|| {
                                LangError::engine_node(
                                    "index: index must be a non-negative integer".to_string(),
                                    current_node,
                                )
                            })?;
                        arr.get(i).cloned().map(|v| (v, false)).ok_or_else(|| {
                            LangError::engine_node(
                                format!("index: index {} out of bounds", i),
                                current_node,
                            )
                        })
                    }
                    (TaggedValue::Obj(map), TaggedValue::Str(key)) => map
                        .get(key.as_ref())
                        .cloned()
                        .map(|v| (v, false))
                        .ok_or_else(|| {
                            LangError::engine_node(
                                format!("index: key '{}' not found", key),
                                current_node,
                            )
                        }),
                    (target, index) => Err(LangError::engine_node(
                        format!(
                            "index: target must be array/object and index must be number/string; got {} and {}",
                            type_tag(&target),
                            type_tag(&index),
                        ),
                        current_node,
                    )),
                }
            }
            "assign" => {
                // ADR-0053: `assign { children: [var, value] }` or
                // `assign { children: [var, value, body] }` —
                // evaluate value, optionally bind var in a new scope and
                // evaluate body, or just return the value.
                // Falls back to legacy `payload.value` for hand-built ASTs.
                if let Some(children) = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                {
                    if children.len() >= 2 {
                        let value_idx = children[1].as_u64().ok_or_else(|| {
                            LangError::engine_node(
                                "assign: value index must be a number".to_string(),
                                current_node,
                            )
                        })? as usize;
                        if children.len() == 3 {
                            // `x = y in body` — bind and evaluate body.
                            let var_idx = children[0].as_u64().ok_or_else(|| {
                                LangError::engine_node(
                                    "assign: var index must be a number".to_string(),
                                    current_node,
                                )
                            })? as usize;
                            let body_idx = children[2].as_u64().ok_or_else(|| {
                                LangError::engine_node(
                                    "assign: body index must be a number".to_string(),
                                    current_node,
                                )
                            })? as usize;
                            let var_name = {
                                let var_node = ast.at(AstRef::new(var_idx))?;
                                var_node.payload.as_str().ok_or_else(|| {
                                    LangError::engine_node(
                                        "assign: LHS must be an identifier".to_string(),
                                        current_node,
                                    )
                                })?
                            };
                            let value =
                                self.eval_node_inner(ast, AstRef::new(value_idx), lang, env)?;
                            env.push_scope();
                            env.define(var_name, value);
                            let result =
                                self.eval_node_inner(ast, AstRef::new(body_idx), lang, env);
                            env.pop_scope();
                            result.map(|v| (v, false))
                        } else {
                            // `x = y` — just evaluate and return the value.
                            self.eval_node_inner(ast, AstRef::new(value_idx), lang, env)
                                .map(|v| (v, false))
                        }
                    } else {
                        Err(LangError::engine_node(
                            "assign: expected at least 2 children".to_string(),
                            current_node,
                        ))
                    }
                } else {
                    // Legacy shape: `assign { target, value }` (hand-built
                    // ASTs). Just return the value.
                    let value = current_node.payload.get("value").ok_or_else(|| {
                        LangError::engine_node(
                            "assign: missing value in payload".to_string(),
                            current_node,
                        )
                    })?;
                    Ok((TaggedValue::from(value.clone()), false))
                }
            }
            "match" => self.prim_match(ast, lang, env, current_node),
            "handle" => self.prim_handle(ast, lang, env, current_node),
            "json" => self.prim_json(ast, lang, env, current_node),
            "bind" => self.prim_bind(ast, lang, env, current_node),
            // Phase 9.5: import/export. The eval schemes for `stmt.import`
            // and `stmt.export` are `Arr { op: import/export, source: Native }`,
            // so they dispatch here. The engine stays generic: it delegates
            // module loading to the configured `ImportResolver` trait object
            // (no hardcoded kind checks — the op names come from eval.yaml).
            "import" => self.prim_import(args, current_node, env),
            "export" => {
                // Bootstrap pass-through: evaluate the child expression (already
                // computed as `args[0]` via `proj_child(0)`) and return it.
                // Export marking is a no-op for now.
                Ok((args.first().cloned().unwrap_or(TaggedValue::Null), false))
            }
            // Arithmetic / comparison / logic. The numeric helpers (`bin_add`,
            // `bin_num`, `cmp_num`, `as_num`) produce span-less errors because
            // they only see `&[TaggedValue]`; attach the current node's span
            // here so type-mismatch diagnostics point at the offending source.
            "add" => bin_add(args).map_err(|e| e.with_node(current_node)),
            "sub" => bin_num(args, |a, b| a - b).map_err(|e| e.with_node(current_node)),
            "mul" => bin_num(args, |a, b| a * b).map_err(|e| e.with_node(current_node)),
            "div" => {
                let a = as_num(&args[0]).map_err(|e| e.with_node(current_node))?;
                let b = as_num(&args[1]).map_err(|e| e.with_node(current_node))?;
                if b.to_f64() == 0.0 {
                    return Err(LangError::engine_node(
                        "division by zero".to_string(),
                        current_node,
                    ));
                }
                Ok((num_value(a / b), false))
            }
            "mod" => {
                let a = as_num(&args[0]).map_err(|e| e.with_node(current_node))?;
                let b = as_num(&args[1]).map_err(|e| e.with_node(current_node))?;
                if b.to_f64() == 0.0 {
                    return Err(LangError::engine_node(
                        "modulo by zero".to_string(),
                        current_node,
                    ));
                }
                let val = match (a, b) {
                    (Num::Int(a), Num::Int(b)) => num_value(Num::Int(a % b)),
                    (a, b) => num_value(Num::Float(a.to_f64() % b.to_f64())),
                };
                Ok((val, false))
            }
            "neg" => {
                let a = as_num(&args[0]).map_err(|e| e.with_node(current_node))?;
                let val = match a {
                    Num::Int(i) => num_value(Num::Int(-i)),
                    Num::Float(f) => num_value(Num::Float(-f)),
                };
                Ok((val, false))
            }
            "eq" => Ok((
                TaggedValue::Bool(handl_value::eq(&args[0], &args[1])),
                false,
            )),
            "neq" => Ok((
                TaggedValue::Bool(!handl_value::eq(&args[0], &args[1])),
                false,
            )),
            "lt" => cmp_num(args, |a, b| a < b).map_err(|e| e.with_node(current_node)),
            "gt" => cmp_num(args, |a, b| a > b).map_err(|e| e.with_node(current_node)),
            "lte" => cmp_num(args, |a, b| a <= b).map_err(|e| e.with_node(current_node)),
            "gte" => cmp_num(args, |a, b| a >= b).map_err(|e| e.with_node(current_node)),
            "pow" => {
                let a = as_num(&args[0]).map_err(|e| e.with_node(current_node))?;
                let b = as_num(&args[1]).map_err(|e| e.with_node(current_node))?;
                Ok((num_value(Num::Float(a.to_f64().powf(b.to_f64()))), false))
            }
            "compare" => {
                let a = as_num(&args[0]).map_err(|e| e.with_node(current_node))?;
                let b = as_num(&args[1]).map_err(|e| e.with_node(current_node))?;
                let cmp = a.partial_cmp(&b).ok_or_else(|| {
                    LangError::engine_node("compare: incomparable values".to_string(), current_node)
                })?;
                Ok((
                    num_value(Num::Int(match cmp {
                        std::cmp::Ordering::Less => -1,
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Greater => 1,
                    })),
                    false,
                ))
            }
            // ADR-0060: route_on_type — takes a value (from args or incoming)
            // and a literal type tag. Returns either_left(value) if the
            // type matches, either_right(value) otherwise.
            "route_on_type" => {
                let tag = lit.and_then(|v| v.as_str()).ok_or_else(|| {
                    LangError::engine_node(
                        "route_on_type: missing type tag literal".to_string(),
                        current_node,
                    )
                })?;
                let val = args
                    .first()
                    .cloned()
                    .or(incoming)
                    .unwrap_or(TaggedValue::Null);
                if type_tag(&val) == tag {
                    Ok((either_left(val), false))
                } else {
                    Ok((either_right(val), false))
                }
            }
            // ADR-0060: call_host — the callee string arrives via
            // `incoming` (from route_on_type → Choice), and `args[0]` is
            // the args array (from proj_rest). Looks up the callee in the
            // module registry and invokes the host function with the array
            // elements as individual arguments. If the callee is a declared
            // graded `op`, dispatch it through the handler stack instead.
            "call_host" => {
                let callee = incoming.as_ref().and_then(|v| v.as_str()).ok_or_else(|| {
                    LangError::engine_node(
                        "call_host: callee must be a string".to_string(),
                        current_node,
                    )
                })?;

                // Grade-based op call: `add(1, 2)` where `add` is an `op`.
                if let Some(op_decl) = lang.op_decl(callee) {
                    let op_incoming = args.first().cloned().unwrap_or(TaggedValue::Null);
                    let op_td = TermData::op(callee.to_string(), vec![], &ArrSource::Native, None);
                    let (mut result, pushed) = self.eval_inner_td(
                        &op_td,
                        ast,
                        _node_ref,
                        lang,
                        env,
                        current_node,
                        Some(op_incoming),
                    )?;
                    if pushed {
                        env.pop_scope();
                    }
                    if result.get("__tag__").is_none() {
                        if let Some(output) = op_decl.get("output").and_then(Value::as_str) {
                            if let Some(tagged) = tag_value(output, &tagged_to_value(&result)) {
                                result = TaggedValue::from(tagged);
                            }
                        }
                    }
                    return Ok((result, false));
                }

                let modules = self.modules.as_ref().ok_or_else(|| {
                    LangError::engine_node(
                        "call_host: no module registry".to_string(),
                        current_node,
                    )
                })?;
                let func = modules.lookup(callee).ok_or_else(|| {
                    LangError::engine_node(
                        format!("call_host: function '{}' not found", callee),
                        current_node,
                    )
                })?;
                let call_args: Vec<Value> = args
                    .first()
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().map(tagged_to_value).collect())
                    .unwrap_or_default();
                let mut pctx = handl_platform::EvalContext::new();
                pctx.set_value_callable(Rc::new(TreeWalkCallable));
                let val = func
                    .call(&call_args, &mut pctx)
                    .map_err(|e| LangError::engine_node(e.to_string(), current_node))?;
                Ok((TaggedValue::from(val), false))
            }
            // ADR-0060: proj_rest — evaluate all children after index 0 and
            // return them as a JSON array. Used by eval.call to collect the
            // variable-arity argument list into a single array value.
            "proj_rest" => {
                let children = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "proj_rest: expected children array in payload".to_string(),
                            current_node,
                        )
                    })?;
                let mut vals = Vec::new();
                for entry in children.iter().skip(1) {
                    let idx = entry.as_u64().ok_or_else(|| {
                        LangError::engine_node(
                            "proj_rest: child index must be a number".to_string(),
                            current_node,
                        )
                    })? as usize;
                    vals.push(self.eval_node_inner(ast, AstRef::new(idx), lang, env)?);
                }
                Ok((TaggedValue::Arr(Rc::from(vals)), false))
            }
            // ADR-0079: `eval_let` removed (let keyword deleted, graph blocks
            // replace it). The `eval_block` handler now handles assign children.
            // eval_block — evaluate all children of a block node in
            // sequence, returning the last child's value. Used by
            // `do { expr; expr; ... }` surface syntax (ADR-0054).
            "eval_block" => {
                let children = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "eval_block: expected children array in payload".to_string(),
                            current_node,
                        )
                    })?;
                if children.is_empty() {
                    return Ok((TaggedValue::Null, false));
                }
                env.push_scope();
                let mut result = TaggedValue::Null;
                let mut eval_err: Option<LangError> = None;
                'block: for entry in children.iter() {
                    let idx = entry.as_u64().ok_or_else(|| {
                        LangError::engine_node(
                            "eval_block: child index must be a number".to_string(),
                            current_node,
                        )
                    })? as usize;
                    let child_node = ast.at(AstRef::new(idx))?;
                    if is_assign_node(lang, &child_node) {
                        if let Some(ac) = child_node
                            .payload
                            .get("children")
                            .and_then(|c| c.as_array())
                        {
                            if ac.len() == 2 {
                                // Binding target/value by child role
                                // (ADR-0081), falling back to the positional
                                // children for role-less shapes.
                                let var_idx = role_child_arena(lang, &child_node, "target")
                                    .or_else(|| ac[0].as_u64().map(|i| i as usize))
                                    .ok_or_else(|| {
                                        LangError::engine_node(
                                            "eval_block: assign var index must be a number"
                                                .to_string(),
                                            current_node,
                                        )
                                    })?;
                                let value_idx = role_child_arena(lang, &child_node, "value")
                                    .or_else(|| ac[1].as_u64().map(|i| i as usize))
                                    .ok_or_else(|| {
                                        LangError::engine_node(
                                            "eval_block: assign value index must be a number"
                                                .to_string(),
                                            current_node,
                                        )
                                    })?;
                                let var_name = binding_target_name(
                                    lang,
                                    ast,
                                    var_idx,
                                    "eval_block",
                                    current_node,
                                )?;
                                match self.eval_node_inner(ast, AstRef::new(value_idx), lang, env) {
                                    Ok(value) => {
                                        // `x = y` updates the nearest
                                        // enclosing binding when `x` is
                                        // already bound (ADR-0074 feedback
                                        // wires); otherwise it defines a
                                        // block-local binding.
                                        env.assign(&var_name, value.clone());
                                        result = value;
                                        continue 'block;
                                    }
                                    Err(e) => {
                                        eval_err = Some(e);
                                        break 'block;
                                    }
                                }
                            }
                        }
                    }
                    match self.eval_node_inner(ast, AstRef::new(idx), lang, env) {
                        Ok(v) => result = v,
                        Err(e) => {
                            eval_err = Some(e);
                            break 'block;
                        }
                    }
                }
                env.pop_scope();
                if let Some(e) = eval_err {
                    return Err(e);
                }
                Ok((result, false))
            }
            // eval_rec — recursive computation region (ADR-0074). The `rec`
            // node's children are the flattened block bindings (grammar
            // `grammar.rec.prefix` with `flatten: true`). Each child is an
            // expression; `assign` children (`a = expr`) define bindings
            // that may reference each other cyclically. The engine iterates
            // to a fixpoint: on each pass, every binding's value expression
            // is re-evaluated using the current values of other bindings.
            // When no binding's value changes between passes, the fixpoint
            // is reached. A fuel limit (10000 iterations) prevents infinite
            // loops. `return` inside a `rec` region exits the region early
            // with the return value. The last child's value is the result
            // of the region (unless a `return` exits early).
            //
            // Bindings are NOT pre-initialized. On the first pass, a binding
            // that references another not-yet-computed binding will either
            // find it in an enclosing scope (if it exists there) or get an
            // error (e.g. arithmetic on an unbound variable). Errors during
            // binding evaluation are caught and the binding is skipped for
            // that pass — it will be retried on the next pass once its
            // dependencies have values. This allows mutually-dependent
            // bindings to converge regardless of their textual order.
            "eval_rec" => self.prim_eval_rec(ast, lang, env, current_node),
            "typeof" => {
                let arg = args
                    .first()
                    .or(incoming.as_ref())
                    .unwrap_or(&TaggedValue::Null);
                Ok((TaggedValue::Str(Rc::from(type_tag(arg))), false))
            }
            "not" => Ok((TaggedValue::Bool(!is_truthy(&args[0])), false)),
            "and" => Ok((
                TaggedValue::Bool(is_truthy(&args[0]) && is_truthy(&args[1])),
                false,
            )),
            "or" => Ok((
                TaggedValue::Bool(is_truthy(&args[0]) || is_truthy(&args[1])),
                false,
            )),
            // Utility unary numeric ops (used by derived-pattern tests in
            // plugin-lib-stdlib's arrow_iter tests). These take the incoming value as
            // their single operand.
            "is_pos" => {
                let v = incoming.unwrap_or(TaggedValue::Null);
                let n = as_num(&v).map_err(|e| e.with_node(current_node))?;
                Ok((TaggedValue::Bool(n > Num::Int(0)), false))
            }
            "dec" => {
                let v = incoming.unwrap_or(TaggedValue::Null);
                let n = as_num(&v).map_err(|e| e.with_node(current_node))?;
                Ok((num_value(n - Num::Int(1)), false))
            }
            "inc" => {
                let v = incoming.unwrap_or(TaggedValue::Null);
                let n = as_num(&v).map_err(|e| e.with_node(current_node))?;
                Ok((num_value(n + Num::Int(1)), false))
            }
            "half_floor" => {
                let v = incoming.unwrap_or(TaggedValue::Null);
                let n = as_num(&v).map_err(|e| e.with_node(current_node))?;
                let result = match n {
                    Num::Int(i) => Num::Int(i / 2),
                    Num::Float(f) => Num::Float((f / 2.0).floor()),
                };
                Ok((num_value(result), false))
            }
            // Test helper: increment the first component of a pair,
            // carrying the second through. Used by the Loop
            // causal-single-step test.
            "inc_fst" => {
                let inc = args
                    .first()
                    .cloned()
                    .or(incoming)
                    .unwrap_or(TaggedValue::Null);
                let a = pair_fst(&inc)
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "inc_fst: input is not a pair".to_string(),
                            current_node,
                        )
                    })?
                    .clone();
                let b = pair_snd(&inc)
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "inc_fst: input is not a pair".to_string(),
                            current_node,
                        )
                    })?
                    .clone();
                let a_n = as_num(&a).map_err(|e| e.with_node(current_node))?;
                let new_a = num_value(a_n + Num::Int(1));
                Ok((pair(new_a, b), false))
            }
            // Test helper: multi-step Loop countdown. Incoming is
            // pair(in, state) where in = pair(result, initial_count). On
            // the first step (state Null) the state is initialized to
            // initial_count. Then: state > 0 -> either_right(state - 1)
            // (continue); state == 0 -> either_left(result) (done).
            "countdown_step" => {
                let inc = args
                    .first()
                    .cloned()
                    .or(incoming)
                    .unwrap_or(TaggedValue::Null);
                let a = pair_fst(&inc).cloned().ok_or_else(|| {
                    LangError::engine_node(
                        "countdown_step: input is not a pair".to_string(),
                        current_node,
                    )
                })?;
                let b = pair_snd(&inc).cloned().unwrap_or(TaggedValue::Null);
                let result_val = pair_fst(&a).cloned().ok_or_else(|| {
                    LangError::engine_node(
                        "countdown_step: in must be pair(result, initial)".to_string(),
                        current_node,
                    )
                })?;
                let initial = pair_snd(&a).cloned().unwrap_or(TaggedValue::Null);
                let state = if b.is_null() { initial } else { b };
                let n = as_num(&state).map_err(|e| e.with_node(current_node))?;
                if n > Num::Int(0) {
                    Ok((either_right(num_value(n - Num::Int(1))), false))
                } else {
                    Ok((either_left(result_val), false))
                }
            }
            // Test helper: a body that never terminates — always returns
            // either_right(state), feeding the state back unchanged. Used
            // to exercise the Loop max-iteration guard.
            "infinite_step" => {
                let inc = args
                    .first()
                    .cloned()
                    .or(incoming)
                    .unwrap_or(TaggedValue::Null);
                let b = pair_snd(&inc).cloned().unwrap_or(TaggedValue::Null);
                Ok((either_right(b), false))
            }
            // v0.1: `product` — categorical product `***` (ADR-0074).
            // Receives the two evaluated child results [f_result, g_result]
            // and pairs them.
            "product" => {
                let left = args.first().cloned().unwrap_or(TaggedValue::Null);
                let right = args.get(1).cloned().unwrap_or(TaggedValue::Null);
                Ok((pair(left, right), false))
            }
            // Arrow combinators (surface syntax: `first f`, `second f`,
            // `left f`, `right f`, `f +++ g`, `f app x`). The sub-arrows
            // arrive as evaluated args (`TaggedValue::Arrow`); the
            // incoming `Pair`/`Either` is destructured and the sub-arrow
            // applied via `apply_app`.
            "first" => {
                let f = args.first().cloned().unwrap_or(TaggedValue::Null);
                let input = incoming
                    .or_else(|| args.get(1).cloned())
                    .unwrap_or(TaggedValue::Null);
                let a = pair_fst(&input).cloned().ok_or_else(|| {
                    LangError::engine_node("first: input is not a pair".to_string(), current_node)
                })?;
                let b = pair_snd(&input).cloned().unwrap_or(TaggedValue::Null);
                let fa = self.apply_app(&f, &[a], ast, lang, env)?;
                Ok((pair(fa, b), false))
            }
            "second" => {
                let f = args.first().cloned().unwrap_or(TaggedValue::Null);
                let input = incoming
                    .or_else(|| args.get(1).cloned())
                    .unwrap_or(TaggedValue::Null);
                let a = pair_fst(&input).cloned().unwrap_or(TaggedValue::Null);
                let b = pair_snd(&input).cloned().ok_or_else(|| {
                    LangError::engine_node("second: input is not a pair".to_string(), current_node)
                })?;
                let fb = self.apply_app(&f, &[b], ast, lang, env)?;
                Ok((pair(a, fb), false))
            }
            "left" => {
                let f = args.first().cloned().unwrap_or(TaggedValue::Null);
                let input = incoming
                    .or_else(|| args.get(1).cloned())
                    .unwrap_or(TaggedValue::Null);
                if is_either_left(&input) {
                    let v = either_value(&input).cloned().unwrap_or(TaggedValue::Null);
                    let fv = self.apply_app(&f, &[v], ast, lang, env)?;
                    Ok((either_left(fv), false))
                } else if is_either_right(&input) {
                    Ok((input, false))
                } else {
                    Err(LangError::engine_node(
                        "left: input is not an either".to_string(),
                        current_node,
                    ))
                }
            }
            "right" => {
                let f = args.first().cloned().unwrap_or(TaggedValue::Null);
                let input = incoming
                    .or_else(|| args.get(1).cloned())
                    .unwrap_or(TaggedValue::Null);
                if is_either_right(&input) {
                    let v = either_value(&input).cloned().unwrap_or(TaggedValue::Null);
                    let fv = self.apply_app(&f, &[v], ast, lang, env)?;
                    Ok((either_right(fv), false))
                } else if is_either_left(&input) {
                    Ok((input, false))
                } else {
                    Err(LangError::engine_node(
                        "right: input is not an either".to_string(),
                        current_node,
                    ))
                }
            }
            // `f +++ g` — categorical sum. `Left a` → `Left (f a)`;
            // `Right b` → `Right (g b)`.
            "sum" => {
                let f = args.first().cloned().unwrap_or(TaggedValue::Null);
                let g = args.get(1).cloned().unwrap_or(TaggedValue::Null);
                let input = incoming
                    .or_else(|| args.get(2).cloned())
                    .unwrap_or(TaggedValue::Null);
                if is_either_left(&input) {
                    let v = either_value(&input).cloned().unwrap_or(TaggedValue::Null);
                    let fv = self.apply_app(&f, &[v], ast, lang, env)?;
                    Ok((either_left(fv), false))
                } else if is_either_right(&input) {
                    let v = either_value(&input).cloned().unwrap_or(TaggedValue::Null);
                    let gv = self.apply_app(&g, &[v], ast, lang, env)?;
                    Ok((either_right(gv), false))
                } else {
                    Err(LangError::engine_node(
                        "sum: input is not an either".to_string(),
                        current_node,
                    ))
                }
            }
            // `f app x` — arrow application: apply the arrow value
            // (arg 0) to the argument (arg 1, or the incoming value).
            "app" => {
                let f = args.first().cloned().unwrap_or(TaggedValue::Null);
                let x = args
                    .get(1)
                    .cloned()
                    .or(incoming)
                    .unwrap_or(TaggedValue::Null);
                let v = self.apply_app(&f, &[x], ast, lang, env)?;
                Ok((v, false))
            }
            // v0.1: `nullish` — nullish coalescing `??` (ADR-0074 A.3).
            // If left is some(v), return v; if left is none or null, return
            // right; otherwise return right.
            "nullish" => {
                let left = args.first().cloned().unwrap_or(TaggedValue::Null);
                let right = args.get(1).cloned().unwrap_or(TaggedValue::Null);
                if left.is_null() {
                    Ok((right, false))
                } else if left.get("_tag").and_then(|t| t.as_str()) == Some("Some") {
                    let inner = left.get("value").cloned().unwrap_or(TaggedValue::Null);
                    Ok((inner, false))
                } else if left.get("_tag").and_then(|t| t.as_str()) == Some("None") {
                    Ok((right, false))
                } else {
                    Ok((right, false))
                }
            }
            // `field_access` — dynamic field access `target[field]`,
            // native on `TaggedValue`. Keeping this in-engine matters: the
            // `field-access` scheme is `Native`, so `a.b`/`a[i]` on a
            // `TaggedValue::Obj`/`Arr` never crosses the `serde_json` host
            // boundary. A record of closures (e.g. the module records
            // `HandlModule` produces) would otherwise force each closure's
            // captured env through `tagged_to_value`, whose recursive env
            // serialization is exponential in the number of prior bindings
            // and overflows on module-sized scopes.
            //
            // Semantics mirror `FieldAccessHostFn` (fp.rs): object+string
            // → lookup, array+int → index, string+string → `a.b` concat
            // (module-path construction: an unbound `mod` identifier
            // evaluates to the string "mod", so `mod.f` yields "mod.f"
            // which `apply_app` dispatches to the module registry).
            "field_access" => {
                let target = args.first().cloned().unwrap_or(TaggedValue::Null);
                let field = current_node
                    .payload
                    .get("field")
                    .and_then(|v| v.as_str())
                    .map(|s| TaggedValue::Str(Rc::from(s)))
                    .or_else(|| args.get(1).cloned())
                    .unwrap_or(TaggedValue::Null);
                match (target, field) {
                    (TaggedValue::Obj(map), TaggedValue::Str(key)) => {
                        let v = map.get(key.as_ref()).cloned().unwrap_or(TaggedValue::Null);
                        Ok((v, false))
                    }
                    (TaggedValue::Arr(arr), field) => {
                        let i = field
                            .as_f64()
                            .filter(|n| *n >= 0.0 && n.fract() == 0.0)
                            .map(|n| n as usize)
                            .ok_or_else(|| {
                                LangError::engine_node(
                                    "field_access: array index must be a non-negative integer"
                                        .to_string(),
                                    current_node,
                                )
                            })?;
                        arr.get(i).cloned().map(|v| (v, false)).ok_or_else(|| {
                            LangError::engine_node(
                                format!(
                                    "field_access: index {i} out of bounds (len {})",
                                    arr.len()
                                ),
                                current_node,
                            )
                        })
                    }
                    (TaggedValue::Pair(p), TaggedValue::Str(key)) => match key.as_ref() {
                        "first" => Ok((p.0.clone(), false)),
                        "second" => Ok((p.1.clone(), false)),
                        "_tag" => Ok((TaggedValue::Str(Rc::from("Pair")), false)),
                        _ => Ok((TaggedValue::Null, false)),
                    },
                    (TaggedValue::EitherLeft(v), TaggedValue::Str(key)) => match key.as_ref() {
                        "_tag" => Ok((TaggedValue::Str(Rc::from("left")), false)),
                        "tag" => Ok((TaggedValue::Str(Rc::from("left")), false)),
                        "value" => Ok((v.as_ref().clone(), false)),
                        _ => Ok((TaggedValue::Null, false)),
                    },
                    (TaggedValue::EitherRight(v), TaggedValue::Str(key)) => match key.as_ref() {
                        "_tag" => Ok((TaggedValue::Str(Rc::from("right")), false)),
                        "tag" => Ok((TaggedValue::Str(Rc::from("right")), false)),
                        "value" => Ok((v.as_ref().clone(), false)),
                        _ => Ok((TaggedValue::Null, false)),
                    },
                    (
                        TaggedValue::Arrow {
                            params,
                            rest,
                            body,
                            env,
                            home,
                            array_pattern,
                            ..
                        },
                        TaggedValue::Str(key),
                    ) => match key.as_ref() {
                        "__arrow__" => Ok((TaggedValue::Bool(true), false)),
                        "params" => {
                            let arr: Vec<TaggedValue> = params
                                .iter()
                                .map(|p| TaggedValue::Str(Rc::from(p.as_str())))
                                .collect();
                            Ok((TaggedValue::Arr(Rc::from(arr)), false))
                        }
                        "rest" => match rest {
                            Some(r) => Ok((TaggedValue::Str(r), false)),
                            None => Ok((TaggedValue::Null, false)),
                        },
                        "body" => Ok((TaggedValue::Int(body as i64), false)),
                        "env" => Ok((TaggedValue::Obj(Rc::clone(&env)), false)),
                        "home" => match home {
                            Some(id) => Ok((TaggedValue::Int(id as i64), false)),
                            None => Ok((TaggedValue::Null, false)),
                        },
                        "array_pattern" => Ok((TaggedValue::Bool(array_pattern), false)),
                        _ => Ok((TaggedValue::Null, false)),
                    },
                    (TaggedValue::Str(s), TaggedValue::Str(key)) => {
                        let joined: Rc<str> = Rc::from(format!("{}.{}", s.as_ref(), key.as_ref()));
                        Ok((TaggedValue::Str(joined), false))
                    }
                    (target, field) => Err(LangError::engine_node(
                        format!(
                            "field_access: target must be object/array, field must be string/number; got {} and {}",
                            type_tag(&target),
                            type_tag(&field),
                        ),
                        current_node,
                    )),
                }
            }
            // `arrow_field` — extract `inputs`/`grade`/`output` from an arrow value.
            // Handles both tagged `Arrow` records produced by `graded_arrow` and
            // `TaggedValue::Arrow` closures produced by ungraded `~>`.
            "arrow_field" => {
                if args.len() != 2 {
                    return Err(LangError::engine_node(
                        format!(
                            "arrow_field: expected 2 arguments (arrow, field_name), got {}",
                            args.len()
                        ),
                        current_node,
                    ));
                }
                let arrow = args.first().cloned().unwrap_or(TaggedValue::Null);
                let field = args.get(1).and_then(|v| v.as_str()).ok_or_else(|| {
                    LangError::engine_node(
                        "arrow_field: field_name must be a string".to_string(),
                        current_node,
                    )
                })?;
                match arrow {
                    TaggedValue::Obj(map)
                        if map.get("__tag__").and_then(|t| t.as_str()) == Some("Arrow") =>
                    {
                        let v = match field {
                            "inputs" => map.get("inputs").cloned().unwrap_or(TaggedValue::Null),
                            "grade" => map.get("grade").cloned().unwrap_or(TaggedValue::Null),
                            "output" => map.get("output").cloned().unwrap_or(TaggedValue::Null),
                            _ => {
                                return Err(LangError::engine_node(
                                    format!("arrow_field: unknown Arrow field '{}'", field),
                                    current_node,
                                ));
                            }
                        };
                        Ok((v, false))
                    }
                    TaggedValue::Arrow {
                        params,
                        body,
                        env: captured,
                        home,
                        ..
                    } => match field {
                        "inputs" => {
                            let arr: Vec<TaggedValue> = params
                                .iter()
                                .map(|p| TaggedValue::Str(Rc::from(p.as_str())))
                                .collect();
                            Ok((TaggedValue::Arr(Rc::from(arr)), false))
                        }
                        "grade" => Ok((TaggedValue::Null, false)),
                        "output" => {
                            if home.is_some() {
                                return Err(LangError::engine_node(
                                    "arrow_field: cannot project output from a foreign-frame arrow"
                                        .to_string(),
                                    current_node,
                                ));
                            }
                            let mut closure_env = ScopeStack::new();
                            closure_env.captured = Some(Rc::clone(&captured));
                            closure_env.globals = Rc::clone(&env.globals);
                            let body_val = self.eval_node_inner(
                                ast,
                                AstRef::new(body),
                                lang,
                                &mut closure_env,
                            )?;
                            Ok((body_val, false))
                        }
                        _ => Err(LangError::engine_node(
                            format!("arrow_field: unknown arrow field '{}'", field),
                            current_node,
                        )),
                    },
                    _ => Err(LangError::engine_node(
                        "arrow_field: not an arrow value".to_string(),
                        current_node,
                    )),
                }
            }
            // v0.1: `opt_chain` — optional chaining `?.` (ADR-0074 A.4).
            // args = [input, field_name]. If input is some(v), return
            // some(v[field_name]); if input is none or null, return none.
            "opt_chain" => {
                let input = args.first().cloned().unwrap_or(TaggedValue::Null);
                let field_name = args.get(1).and_then(|v| v.as_str()).ok_or_else(|| {
                    LangError::engine_node(
                        "opt_chain: field name must be a string".to_string(),
                        current_node,
                    )
                })?;
                if input.is_null() {
                    // null → none
                    Ok((handl_value::tval!({"_tag": "None"}), false))
                } else if input.get("_tag").and_then(|t| t.as_str()) == Some("Some") {
                    // some(v) → some(v.field)
                    let inner = input.get("value").cloned().unwrap_or(TaggedValue::Null);
                    let field_val = inner.get(field_name).cloned().unwrap_or(TaggedValue::Null);
                    Ok((
                        handl_value::tval!({"_tag": "Some", "value": field_val}),
                        false,
                    ))
                } else if input.get("_tag").and_then(|t| t.as_str()) == Some("None") {
                    // none → none
                    Ok((handl_value::tval!({"_tag": "None"}), false))
                } else {
                    // Non-option value: treat as some(value) and extract field
                    let field_val = input.get(field_name).cloned().unwrap_or(TaggedValue::Null);
                    Ok((
                        handl_value::tval!({"_tag": "Some", "value": field_val}),
                        false,
                    ))
                }
            }
            "assert" => {
                // ADR-0089 / ADR-0092 keyword assertion. The `assert`
                // node's single `expr` child is evaluated. If the child is
                // an assignment/equality-shaped node (its AST shape declares
                // both `target` and `value` roles), both sides are evaluated
                // and compared; otherwise the child is evaluated as a truthy
                // test. Child positions are resolved by named role
                // (engine-genericity rule, ADR-0081).
                //
                // On success the op returns `result.ok(true)`; on failure it
                // returns `result.err({message, expected, actual, diff})`
                // (ADR-0077, ADR-0089).
                let expr_idx = role_child_arena(lang, current_node, "expr").ok_or_else(|| {
                    LangError::engine_node("assert: missing expr child".to_string(), current_node)
                })?;
                let expr_node = ast.at(AstRef::new(expr_idx)).map_err(|e| {
                    LangError::engine_node(
                        format!("assert: cannot resolve expr child: {}", e),
                        current_node,
                    )
                })?;
                let handler = self.assert_handler.clone();
                let ok_value = || -> TaggedValue {
                    TaggedValue::from(
                        tag_value("ok", &Value::Bool(true))
                            .expect("ok tag value must not contain __tag__"),
                    )
                };
                let err_value = |failure: AssertFailure| -> TaggedValue {
                    let value = if let Some(ref handler) = handler {
                        handler.on_failure(failure)
                    } else {
                        let payload = serde_json::json!({
                            "message": failure.message,
                            "assertion": failure.assertion,
                            "expected": failure.expected,
                            "actual": failure.actual,
                            "diff": failure.diff,
                        });
                        tag_value("err", &serde_json::json!({"value": payload}))
                            .expect("err tag value must not contain __tag__")
                    };
                    TaggedValue::from(value)
                };
                if let (Some(target_idx), Some(value_idx)) = (
                    role_child_arena(lang, expr_node, "target"),
                    role_child_arena(lang, expr_node, "value"),
                ) {
                    let target_val =
                        self.eval_node_inner(ast, AstRef::new(target_idx), lang, env)?;
                    let value_val = self.eval_node_inner(ast, AstRef::new(value_idx), lang, env)?;
                    if target_val == value_val {
                        Ok((ok_value(), false))
                    } else {
                        let expected = tagged_to_value(&value_val);
                        let actual = tagged_to_value(&target_val);
                        let expected_json = serde_json::to_string(&expected).ok();
                        let actual_json = serde_json::to_string(&actual).ok();
                        let diff = if let (Some(e), Some(a)) = (&expected_json, &actual_json) {
                            Some(format!("expected:\\n  {}\\nactual:\\n  {}", e, a))
                        } else {
                            None
                        };
                        let failure = AssertFailure {
                            assertion: "assert".to_string(),
                            expected: expected_json,
                            actual: actual_json,
                            diff: diff,
                            message: format!(
                                "assertion failed: {:?} != {:?}",
                                target_val, value_val
                            ),
                        };
                        Ok((err_value(failure), false))
                    }
                } else {
                    let val = self.eval_node_inner(ast, AstRef::new(expr_idx), lang, env)?;
                    if is_truthy(&val) {
                        Ok((ok_value(), false))
                    } else {
                        let actual = tagged_to_value(&val);
                        let actual_json = serde_json::to_string(&actual).ok();
                        let failure = AssertFailure {
                            assertion: "assert".to_string(),
                            expected: Some("truthy value".to_string()),
                            actual: actual_json,
                            diff: None,
                            message: format!("assertion failed: {:?} is falsy", val),
                        };
                        Ok((err_value(failure), false))
                    }
                }
            }
            "runtime_assert" => {
                // ADR-0068 graduated guarantees: runtime backstop for
                // predicate annotations the typechecker could not normalize
                // statically (`Verdict::Blocked`). The `runtime-assert` node
                // has two children — `predicate` (a closure that takes the
                // expr's value and returns bool) and `expr` (the expression
                // to check). The op evaluates the predicate, evaluates the
                // expr, applies the predicate to the expr value, and either
                // returns the value (pass-through) or raises a `LangError`.
                //
                // Child positions are resolved by named role
                // (`lang.child_role_index`), falling back to positional
                // [0, 1] when the shape declares no roles (engine-genericity
                // rule, ADR-0081).
                let children = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "runtime_assert: expected children array in payload".to_string(),
                            current_node,
                        )
                    })?;
                let pred_idx = role_index_of(lang, "runtime-assert", "predicate").unwrap_or(0);
                let expr_idx = role_index_of(lang, "runtime-assert", "expr").unwrap_or(1);
                let pred_arena =
                    children
                        .get(pred_idx)
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| {
                            LangError::engine_node(
                                format!("runtime_assert: predicate child {} not found", pred_idx),
                                current_node,
                            )
                        })? as usize;
                let expr_arena =
                    children
                        .get(expr_idx)
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| {
                            LangError::engine_node(
                                format!("runtime_assert: expr child {} not found", expr_idx),
                                current_node,
                            )
                        })? as usize;
                // 1. Evaluate the predicate child → closure.
                let pred_val = self.eval_node_inner(ast, AstRef::new(pred_arena), lang, env)?;
                // 2. Evaluate the expr child → value.
                let expr_val = self.eval_node_inner(ast, AstRef::new(expr_arena), lang, env)?;
                // 3. If the predicate is not a closure (e.g. a plain type
                //    tag from a non-predicate annotation that was wrapped
                //    by the over-eager lowering pass), pass through without
                //    checking — the typechecker already accepted it
                //    statically.
                if !matches!(pred_val, TaggedValue::Arrow { .. }) {
                    return Ok((expr_val, false));
                }
                // 4. Apply the closure to the value → bool.
                let check_result =
                    self.apply_app(&pred_val, &[expr_val.clone()], ast, lang, env)?;
                // 5. If true → return the value (pass-through).
                //    If false → raise LangError.
                if is_truthy(&check_result) {
                    Ok((expr_val, false))
                } else {
                    Err(LangError::engine_node(
                        "runtime_assert: assertion failed — predicate returned false".to_string(),
                        current_node,
                    ))
                }
            }
            // ADR-0074 follow-up #3: `>>>` (`flow`) — function application
            // with a single input argument. Unlike `App` (used by `call`
            // to unpack a `proj_rest` array of call args), `flow` passes
            // the left operand through as one argument, only destructuring
            // a categorical `Pair` (`***`) into multiple positional args.
            // This lets list-argument combinators like `comb.converge`
            // receive the list as their single `args` parameter.
            "flow" => {
                if args.len() != 2 {
                    return Err(LangError::engine_node(
                        format!("flow: expected 2 args, got {}", args.len()),
                        current_node,
                    ));
                }
                let input = &args[0];
                let callee = &args[1];
                let actual_args: Vec<TaggedValue> = if matches!(input, TaggedValue::Pair(_)) {
                    flatten_pair(input)
                } else {
                    vec![input.clone()]
                };
                let val = if let Some(name) = callee.as_str() {
                    self.call_host_fn(name, &actual_args, current_node)?
                } else {
                    self.apply_app(callee, &actual_args, ast, lang, env)?
                };
                Ok((val, false))
            }
            "record" => {
                if args.len() % 2 != 0 {
                    return Err(LangError::engine_node(
                        format!(
                            "record: expected even number of arguments, got {}",
                            args.len()
                        ),
                        current_node,
                    ));
                }
                let mut map = serde_json::Map::new();
                for i in (0..args.len()).step_by(2) {
                    let key = args[i].as_str().ok_or_else(|| {
                        LangError::engine_node(
                            format!("record: key at index {} must be a string", i),
                            current_node,
                        )
                    })?;
                    map.insert(key.to_string(), tagged_to_value(&args[i + 1]));
                }
                Ok((TaggedValue::from(Value::Object(map)), false))
            }
            "paren" => {
                let children = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::engine_node(
                            "paren: expected children array in payload".to_string(),
                            current_node,
                        )
                    })?;
                if children.is_empty() {
                    Ok((TaggedValue::Null, false))
                } else if children.len() == 1 {
                    let idx = children[0].as_u64().ok_or_else(|| {
                        LangError::engine_node(
                            "paren: child index must be a number".to_string(),
                            current_node,
                        )
                    })? as usize;
                    let val = self.eval_node_inner(ast, AstRef::new(idx), lang, env)?;
                    Ok((val, false))
                } else {
                    let mut vals = Vec::with_capacity(children.len());
                    for c in children {
                        let idx = c.as_u64().ok_or_else(|| {
                            LangError::engine_node(
                                "paren: child index must be a number".to_string(),
                                current_node,
                            )
                        })? as usize;
                        vals.push(self.eval_node_inner(ast, AstRef::new(idx), lang, env)?);
                    }
                    Ok((TaggedValue::Arr(Rc::from(vals)), false))
                }
            }
            "tag.construct" => {
                if args.len() != 2 {
                    return Err(LangError::engine_node(
                        format!(
                            "tag.construct: expected 2 arguments (tag_name, payload), got {}",
                            args.len()
                        ),
                        current_node,
                    ));
                }
                let tag_name = args[0].as_str().ok_or_else(|| {
                    LangError::engine_node(
                        "tag.construct: tag_name must be a string".to_string(),
                        current_node,
                    )
                })?;
                let mut obj = match &args[1] {
                    TaggedValue::Null => FxHashMap::default(),
                    TaggedValue::Obj(map) => map.as_ref().clone(),
                    _ => {
                        return Err(LangError::engine_node(
                            "tag.construct: payload must be an object or null".to_string(),
                            current_node,
                        ));
                    }
                };
                if obj.contains_key("__tag__") || obj.contains_key("__registry__") {
                    return Err(LangError::engine_node(
                        "tag.construct: payload contains reserved __tag__ or __registry__ key"
                            .to_string(),
                        current_node,
                    ));
                }
                obj.insert(Rc::from("__tag__"), TaggedValue::Str(Rc::from(tag_name)));
                obj.insert(
                    Rc::from("__registry__"),
                    TaggedValue::Str(Rc::from("handl.canonical")),
                );
                Ok((TaggedValue::Obj(Rc::new(obj)), false))
            }
            "number_add" => {
                // Collect the two addends from explicit args, from the
                // second component of an incoming pair, or from an incoming
                // array. This lets a handler `Term::Op { name: "number_add",
                // args: [] }` consume the `(incoming, evaluated_args)` pair
                // that `ValueModel::dispatch_op` passes.
                let operands: Vec<TaggedValue> = if args.is_empty() {
                    if let Some(inc) = incoming.as_ref() {
                        if let Some(snd) = pair_snd(inc) {
                            if let Some(arr) = snd.as_array() {
                                arr.iter().cloned().collect()
                            } else {
                                vec![snd.clone()]
                            }
                        } else if let Some(arr) = inc.as_array() {
                            arr.iter().cloned().collect()
                        } else {
                            vec![inc.clone()]
                        }
                    } else {
                        vec![]
                    }
                } else {
                    args.iter().cloned().collect()
                };

                if operands.len() < 2 {
                    return Err(LangError::engine_node(
                        "number_add: expected at least two Number inputs".to_string(),
                        current_node,
                    ));
                }

                let a = &operands[0];
                let b = &operands[1];

                if a.get("__tag__").and_then(|t| t.as_str()) != Some("Number") {
                    return Err(LangError::engine_node(
                        "number_add: first input is not a Number".to_string(),
                        current_node,
                    ));
                }
                let a_value = a.get("value").ok_or_else(|| {
                    LangError::engine_node(
                        "number_add: first Number has no value".to_string(),
                        current_node,
                    )
                })?;
                let a_num = as_num(a_value).map_err(|e| e.with_node(current_node))?;

                if b.get("__tag__").and_then(|t| t.as_str()) != Some("Number") {
                    return Err(LangError::engine_node(
                        "number_add: second input is not a Number".to_string(),
                        current_node,
                    ));
                }
                let b_value = b.get("value").ok_or_else(|| {
                    LangError::engine_node(
                        "number_add: second Number has no value".to_string(),
                        current_node,
                    )
                })?;
                let b_num = as_num(b_value).map_err(|e| e.with_node(current_node))?;

                let sum = a_num + b_num;
                let first_obj = tagged_to_value(a);
                let mut payload = first_obj.as_object().cloned().unwrap_or_default();
                payload.remove("__tag__");
                payload.insert("value".to_string(), Value::from(num_value(sum)));
                let result = tag_value("Number", &Value::Object(payload)).ok_or_else(|| {
                    LangError::engine_node(
                        "number_add: failed to mint Number result".to_string(),
                        current_node,
                    )
                })?;
                Ok((TaggedValue::from(result), false))
            }
            // Frontier: transport a value along a path. The path is already
            // evaluated for effects; the runtime result is the value itself.
            "transport" => Ok((args.get(1).cloned().unwrap_or(TaggedValue::Null), false)),
            _ => Err(LangError::engine_node(
                format!("unknown v2 native op: {}", op),
                current_node,
            )),
        }
    }

    /// Apply an `App` callee: closure application only (ADR-0060).
    /// Call a host function by name (string callee). This is the
    /// `App`/`>>>` dispatch path for string callees — mirrors the
    /// `call_host` native op used by `call` nodes. Looks up the function
    /// in the module registry and invokes it with the given arguments.
    fn call_host_fn(
        &self,
        name: &str,
        args: &[TaggedValue],
        current_node: &AstNode,
    ) -> Result<TaggedValue, LangError> {
        let modules = self.modules.as_ref().ok_or_else(|| {
            LangError::engine_node("call_host: no module registry".to_string(), current_node)
        })?;
        let func = modules.lookup(name).ok_or_else(|| {
            LangError::engine_node(
                format!("call_host: function '{}' not found", name),
                current_node,
            )
        })?;
        let call_args: Vec<Value> = args.iter().map(tagged_to_value).collect();
        let mut pctx = handl_platform::EvalContext::new();
        pctx.set_value_callable(Rc::new(TreeWalkCallable));
        let val = func
            .call(&call_args, &mut pctx)
            .map_err(|e| LangError::engine_node(e.to_string(), current_node))?;
        Ok(TaggedValue::from(val))
    }

    ///
    /// Host-function dispatch (string callee) has been moved to the
    /// `call_host` native op. This helper only handles arrow-value
    /// (closure) application: extract params/body/env, build a fresh
    /// scope from the captured env, bind params, evaluate the body.
    fn apply_app(
        &self,
        callee: &TaggedValue,
        args: &[TaggedValue],
        ast: &Ast,
        lang: &Language,
        env: &mut ScopeStack,
    ) -> Result<TaggedValue, LangError> {
        // Closure application — match on TaggedValue::Arrow directly.
        let (params, rest_param, body_idx, captured_env, home, array_pattern, body_def) =
            match callee {
                TaggedValue::Arrow {
                    params,
                    rest,
                    body,
                    env,
                    home,
                    array_pattern,
                    body_def,
                } => (
                    params.clone(),
                    rest.clone(),
                    *body,
                    Rc::clone(env),
                    *home,
                    *array_pattern,
                    body_def.clone(),
                ),
                _ => {
                    return Err(LangError::engine(
                        "callee is not an arrow-value".to_string(),
                    ));
                }
            };
        // ADR-0106 item 2 step 2: prefer the arrow's **explicit body
        // definition** when it carries one. `body_def` holds the body as a
        // standalone arena (`{"__ast__": <Ast>}`, minted via
        // `Ast::subtree` + `ast_value`), so an arrow that crossed a JSON
        // round-trip — whose minting frame and AST are gone — is still
        // applicable. The ambient `body` index and the `home` frame remain
        // the fallback for arrows minted without a definition (bytecode,
        // cranelift, hand-built values).
        //
        // The definition is deserialized through `cached_def_arena`, which
        // memoizes the arena per definition value: applying the same
        // self-describing arrow N times deserializes once, not N times.
        let def_arena: Option<Rc<Ast>> = body_def.as_ref().and_then(cached_def_arena);
        let (ast, body_idx, home): (&Ast, usize, Option<u64>) = match &def_arena {
            // The extracted arena is post-order, so the body root is last.
            Some(a) => (a.as_ref(), a.len() - 1, None),
            None => (ast, body_idx, home),
        };
        if let Some(frame_id) = home {
            // The arrow was minted by a *different* eval frame (it crossed
            // a runtime boundary): its `body` index refers to that frame's
            // AST. Evaluate it there — the frame is installed into the
            // thread-locals for the duration so nested host calls
            // (`ctx.try_call_value`) resolve against the arrow's home
            // modules/globals too. The arrow is re-entered with
            // `home: None` so its `body` resolves ambiently within the
            // frame; any surplus args are curried through inside the frame
            // (intermediate arrows index into the frame's AST).
            let frame = home_frame(frame_id).ok_or_else(|| {
                LangError::engine(format!("arrow: unknown home frame {}", frame_id))
            })?;
            // Host dispatch inside the body must see the arrow's *home*
            // module chain, not the ambient engine's.
            let mut engine = self.clone();
            engine.modules = frame.modules.clone();
            engine.import_resolver = frame.import_resolver.clone();
            // ADR-0106 item 2 step 1: this re-wraps an existing body in the
            // same arena, so no new definition is minted here — `body_def`
            // stays `None` and the `home`/`body` fallback applies as before.
            let local_callee = TaggedValue::Arrow {
                params: params.clone(),
                rest: rest_param.clone(),
                body: body_idx,
                env: captured_env.clone(),
                home: None,
                array_pattern,
                body_def: None,
            };
            let mut frame_env = ScopeStack::new();
            if let Some(g) = &frame.globals {
                frame_env.set_globals(Rc::clone(g));
            }
            return frame.enter(|| {
                engine.apply_app(&local_callee, args, &frame.ast, &frame.lang, &mut frame_env)
            });
        }
        // Array-pattern arrows (`[x, y] ~> body`) consume a single array
        // argument and destructure it by position. The array itself is the
        // sole argument; fixed names bind to leading elements and a rest
        // binding receives the remaining tail.
        if array_pattern {
            if args.len() != 1 {
                return Err(LangError::engine(format!(
                    "array-pattern arrow expects 1 array argument, got {}",
                    args.len()
                )));
            }
            let arr = args[0].as_array().ok_or_else(|| {
                LangError::engine("array-pattern arrow expects an array argument".to_string())
            })?;
            let fixed = params.len();
            if arr.len() < fixed {
                return Err(LangError::engine(format!(
                    "array-pattern arrow expects at least {} element(s), got {}",
                    fixed,
                    arr.len()
                )));
            }
            if rest_param.is_none() && arr.len() != fixed {
                return Err(LangError::engine(format!(
                    "array-pattern arrow expects exactly {} element(s), got {}",
                    fixed,
                    arr.len()
                )));
            }
            let (head, tail) = arr.split_at(fixed);
            return self.apply_arrow_body(
                &params,
                rest_param.as_ref(),
                body_idx,
                captured_env,
                head,
                tail,
                ast,
                lang,
                Some(Rc::clone(&env.globals)),
            );
        }
        // Positional arrows: ADR-0074 multi-param arrows may receive a
        // single `Pair` (product `***`) and destructure it into individual
        // args. `(a, b) ~> body` applied to `Pair(x, y)` binds `a=x, b=y`.
        let actual_args: Vec<TaggedValue> =
            if params.len() > 1 && args.len() == 1 && matches!(args[0], TaggedValue::Pair(_)) {
                flatten_pair(&args[0])
            } else {
                args.to_vec()
            };
        if actual_args.len() < params.len() {
            return Err(LangError::engine(format!(
                "arrow-value expects {} args, got {}",
                params.len(),
                args.len()
            )));
        }
        // Curried application: arrows are unary-under-the-hood (`f :: x ~>
        // y ~> body`), so applying more args than the arrow's parameter
        // list binds the first `params.len()` args, evaluates the body,
        // and applies the result to the remainder. `f(1, 5)` on a curried
        // arrow is `(f 1) 5`.
        //
        // Rest params (`[a, ...rest] ~> body`) change this: when the arrow
        // declares a rest param, the surplus args bind to it as an array
        // instead of currying, and the call consumes all args at once.
        if let Some(rest_name) = &rest_param {
            let (head, tail) = actual_args.split_at(params.len());
            return self.apply_arrow_body(
                &params,
                Some(rest_name),
                body_idx,
                captured_env,
                head,
                tail,
                ast,
                lang,
                Some(Rc::clone(&env.globals)),
            );
        }
        let (head, mut rest) = actual_args.split_at(params.len());
        let mut val = self.apply_arrow_body(
            &params,
            None,
            body_idx,
            captured_env,
            head,
            &[],
            ast,
            lang,
            Some(Rc::clone(&env.globals)),
        )?;
        while !rest.is_empty() {
            val = self.apply_app(&val, rest, ast, lang, env)?;
            rest = &[];
        }
        Ok(val)
    }

    /// Apply an arrow body: build a fresh env that **shares** the carried
    /// env via `Rc` (an O(1) refcount bump — no deep clone of the captured
    /// variables on every call), push a single scope for the params, and
    /// evaluate the body. The caller's env is never mutated — no
    /// swap/restore hack.
    ///
    /// `globals` are the host globals the body should fall back to: the
    /// ambient env's globals for same-frame arrows, or the home frame's
    /// globals for boundary-crossed arrows (`None` → none). The carried
    /// env only contains user-created variables (globals are excluded at
    /// capture time), so `lookup()` checks the local param scope first,
    /// then the shared captured env, then globals — variable shadowing and
    /// captured-var lookup both work correctly.
    fn apply_arrow_body(
        &self,
        params: &[String],
        rest_param: Option<&Rc<str>>,
        body_idx: usize,
        captured_env: Rc<FxHashMap<Rc<str>, TaggedValue>>,
        actual_args: &[TaggedValue],
        rest_args: &[TaggedValue],
        ast: &Ast,
        lang: &Language,
        globals: Option<Rc<FxHashMap<Rc<str>, TaggedValue>>>,
    ) -> Result<TaggedValue, LangError> {
        let body_ref = AstRef::new(body_idx);
        let mut arrow_env = ScopeStack::new();
        arrow_env.captured = Some(captured_env);
        if let Some(g) = globals {
            arrow_env.set_globals(g);
        }
        arrow_env.push_scope();
        for (param, arg) in params.iter().zip(actual_args.iter()) {
            arrow_env.define(param, arg.clone());
        }
        // Rest param: bind the surplus args as an array (possibly empty).
        if let Some(name) = rest_param {
            arrow_env.define(
                name.as_ref(),
                TaggedValue::Arr(Rc::from(rest_args.to_vec())),
            );
        }
        self.eval_node_inner(ast, body_ref, lang, &mut arrow_env)
    }

    /// Evaluate a single node by looking up its eval scheme and
    /// interpreting it. This is the child-node evaluation helper used by
    /// `proj_child` and closure bodies.
    ///
    /// Fallback (no scheme): `foreign.<name>` kinds dispatch to the host
    /// function `<name>` with evaluated children; other kinds return the
    /// payload as the value.
    pub fn eval_node(
        &self,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
    ) -> Result<Value, LangError> {
        let val = self.eval_node_inner(ast, node_ref, lang, env)?;
        Ok(tagged_to_value(&val))
    }

    /// Internal node evaluator — the [`TaggedValue`] counterpart of
    /// [`eval_node`](Self::eval_node). Returns a [`TaggedValue`].
    fn eval_node_inner(
        &self,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
    ) -> Result<TaggedValue, LangError> {
        let node = ast.at(node_ref)?;
        let scheme_def = lang.eval.get_by_identity(&node.kind);
        match scheme_def {
            Some(def) => self.eval_scheme_tagged(&def.scheme, ast, node_ref, lang, env, node, None),
            None => {
                // Fallback for `foreign.<name>` node kinds.
                if let Some(foreign_name) = node.kind.strip_prefix("foreign.") {
                    let modules = self.modules.as_ref().ok_or_else(|| {
                        LangError::engine_node(
                            format!("unregistered foreign: {}", foreign_name),
                            node,
                        )
                    })?;
                    let func = modules.lookup(foreign_name).ok_or_else(|| {
                        LangError::engine_node(
                            format!("unregistered foreign: {}", foreign_name),
                            node,
                        )
                    })?;
                    let mut arg_vals = Vec::new();
                    if let Some(children) = node.payload.get("children").and_then(|c| c.as_array())
                    {
                        for child_ref in children {
                            if let Some(idx) = child_ref.as_u64() {
                                let child_ref = AstRef::new(idx as usize);
                                let val = self.eval_node_inner(ast, child_ref, lang, env)?;
                                arg_vals.push(tagged_to_value(&val));
                            }
                        }
                    }
                    let mut pctx = handl_platform::EvalContext::new();
                    pctx.set_value_callable(Rc::new(TreeWalkCallable));
                    let val = func
                        .call(&arg_vals, &mut pctx)
                        .map_err(|e| LangError::engine_node(e.to_string(), node))?;
                    Ok(TaggedValue::from(val))
                } else {
                    Ok(TaggedValue::from(node.payload.clone()))
                }
            }
        }
    }

    /// Convenience: look up the `EvalSchemeDef` for the node and interpret
    /// it. Demonstrates the handler running a language's schemes
    /// end-to-end (ADR-0052).
    pub fn eval_lowered(
        &self,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
    ) -> Result<Value, LangError> {
        self.eval_node(ast, node_ref, lang, env)
    }

    /// Backward-compatible alias for [`eval_scheme`](Self::eval_scheme).
    #[doc(hidden)]
    pub fn eval_scheme_v2(
        &self,
        scheme: &Term,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<Value>,
    ) -> Result<Value, LangError> {
        self.eval_scheme(scheme, ast, node_ref, lang, env, current_node, incoming)
    }

    /// Backward-compatible alias for [`eval_node`](Self::eval_node).
    #[doc(hidden)]
    pub fn eval_node_v2(
        &self,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
    ) -> Result<Value, LangError> {
        self.eval_node(ast, node_ref, lang, env)
    }

    /// `bind` op: arrow-calculus env extension (ADR-0052).
    ///
    /// `x = y in body` → `bind { name: "x", children: [y, body] }`.
    /// Evaluates `y` in the current env, pushes a scope with `x = y`,
    /// evaluates `body` in the extended env, pops the scope. No closure
    /// creation, no env capture — just env extension via `Seq` semantics.
    /// `import` op (Phase 9.5): load a module via the configured
    /// [`ImportResolver`] and bind its top-level bindings into the current
    /// scope.
    ///
    /// The `stmt.import` eval scheme is
    /// `Arr { op: import, args: [proj_child(0)], source: Native }`, so
    /// `args[0]` is the evaluated path expression (a string literal). As a
    /// fallback, the path is also read from `current_node.payload["path"]`
    /// (the `ast.import` shape declares a `path` field).
    ///
    /// The resolver returns a `Value::Object` of `{name: value, ...}`. The
    /// node's `payload.names`/`payload.alias` (set by the `from "path"
    /// import ...` grammar tail) select the binding mode: a named import
    /// projects a single export, `* as alias` binds the whole object, and
    /// `*`/bare `import "path"` splices each export into the current
    /// (innermost) scope via [`ScopeStack::define`]. No new scope is pushed
    /// (`did_push = false`): imported bindings share the importer's scope.
    ///
    /// If no resolver is configured, the op errors gracefully.
    fn prim_import(
        &self,
        args: &[TaggedValue],
        current_node: &AstNode,
        env: &mut ScopeStack,
    ) -> Result<(TaggedValue, bool), LangError> {
        // Resolve the module path: prefer the evaluated child (`args[0]`),
        // fall back to the node's `path` payload field.
        let path: String = args
            .first()
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| {
                current_node
                    .payload
                    .get("path")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .ok_or_else(|| {
                LangError::engine_node(
                    "import: missing module path (expected string argument or 'path' payload)"
                        .to_string(),
                    current_node,
                )
            })?;
        let resolver = self.import_resolver.as_ref().ok_or_else(|| {
            LangError::engine_node(
                "import: no import resolver configured".to_string(),
                current_node,
            )
        })?;
        let bindings = resolver.resolve_import(&path).map_err(|e| {
            // Annotate with the import node for diagnostics if the error
            // is a bare engine error.
            LangError::engine_node(format!("import \"{}\": {}", path, e), current_node)
        })?;
        // Binding semantics (`payload.names` / `payload.alias`, ADR-0096):
        // - `import "path"` / `from "path" import *` (no alias): splice each
        //   top-level export into the current scope.
        // - `from "path" import * as alias`: bind the whole module object to
        //   `alias`.
        // - `from "path" import name [as alias]`: project `name` out of the
        //   module's exports and bind it under `alias` (or `name`).
        // - `from "path" import { a, b as x }`: project each named export and
        //   bind it under the local name (with optional `as` alias).
        let names_val = current_node.payload.get("names");
        let alias = current_node.payload.get("alias").and_then(|v| v.as_str());
        if let Some(arr) = names_val.and_then(|v| v.as_array()) {
            for item in arr {
                let (imported, local) = match item {
                    Value::String(s) => (s.as_str(), s.as_str()),
                    Value::Object(obj) => {
                        let n = obj.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                            LangError::engine_node(
                                "import: missing 'name' in braced import entry".to_string(),
                                current_node,
                            )
                        })?;
                        let a = obj.get("as").and_then(|v| v.as_str()).unwrap_or(n);
                        (n, a)
                    }
                    _ => {
                        return Err(LangError::engine_node(
                            "import: invalid braced import entry".to_string(),
                            current_node,
                        ));
                    }
                };
                if imported == "*" {
                    return Err(LangError::engine_node(
                        "import: cannot use '*' inside a braced import list".to_string(),
                        current_node,
                    ));
                }
                let exported = match &bindings {
                    Value::Object(map) => map.get(imported).cloned(),
                    _ => None,
                };
                let value = exported.ok_or_else(|| {
                    LangError::engine_node(
                        format!("import \"{}\": module has no export `{}`", path, imported),
                        current_node,
                    )
                })?;
                env.define(local, TaggedValue::from(value));
            }
        } else if let Some(imported) = names_val.and_then(|v| v.as_str()).filter(|n| *n != "*") {
            let exported = match &bindings {
                Value::Object(map) => map.get(imported).cloned(),
                _ => None,
            };
            let value = exported.ok_or_else(|| {
                LangError::engine_node(
                    format!("import \"{}\": module has no export `{}`", path, imported),
                    current_node,
                )
            })?;
            env.define(alias.unwrap_or(imported), TaggedValue::from(value));
        } else if let Some(alias) = alias {
            env.define(alias, TaggedValue::from(bindings.clone()));
        } else if let Value::Object(map) = bindings {
            for (name, val) in map {
                env.define(&name, TaggedValue::from(val));
            }
        }
        // `import` is a statement; it produces no value.
        Ok((TaggedValue::Null, false))
    }

    fn prim_bind(
        &self,
        ast: &Ast,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
    ) -> Result<(TaggedValue, bool), LangError> {
        let name = current_node
            .payload
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                LangError::engine_node("bind: missing name".to_string(), current_node)
            })?;
        let children = current_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                LangError::engine_node(
                    "bind: expected children array in payload".to_string(),
                    current_node,
                )
            })?;
        let value_idx = children.first().and_then(|v| v.as_u64()).ok_or_else(|| {
            LangError::engine_node("bind: missing value child".to_string(), current_node)
        })? as usize;
        let body_idx = children.get(1).and_then(|v| v.as_u64()).ok_or_else(|| {
            LangError::engine_node("bind: missing body child".to_string(), current_node)
        })? as usize;
        // Evaluate the value in the current env.
        let value = self.eval_node_inner(ast, AstRef::new(value_idx), lang, env)?;
        // Extend env with name = value, evaluate body, pop.
        env.push_scope();
        env.define(name, value);
        let result = self.eval_node_inner(ast, AstRef::new(body_idx), lang, env);
        env.pop_scope();
        result.map(|v| (v, false))
    }

    /// `match` op: pattern-match a scrutinee against a list of arms.
    ///
    /// The `match` node's payload has `children: [scrut_idx, (motive_idx,) block_idx]`.
    /// An optional motive child is accepted for Phase 6.3 dependent motives;
    /// it is ignored at runtime. The block node (kind `block`) has
    /// `children: [arm_idx, ...]`. Each arm node (kind `arm`) has
    /// `children: [pattern_idx, body_idx]`.
    ///
    /// Pattern matching interprets the pattern AST node structurally:
    /// - A `var` node named `_` or starting with `_` → **wildcard**.
    /// - A `var` node with any other name → **binder** (binds the name to
    ///   the scrutinee value).
    /// - Any other node → **literal**: the pattern is evaluated as a
    ///   value and compared by equality (`==`) to the scrutinee.
    fn prim_match(
        &self,
        ast: &Ast,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
    ) -> Result<(TaggedValue, bool), LangError> {
        let children = current_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                LangError::engine_node(
                    "match: missing children in payload".to_string(),
                    current_node,
                )
            })?;
        let scrut_idx = children.get(0).and_then(|v| v.as_u64()).ok_or_else(|| {
            LangError::engine_node("match: missing scrutinee child".to_string(), current_node)
        })? as usize;
        let has_motive = children.len() >= 3;
        let block_idx = children
            .get(if has_motive { 2 } else { 1 })
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                LangError::engine_node("match: missing block child".to_string(), current_node)
            })? as usize;

        // Evaluate the scrutinee.
        let scrut_val = self.eval_node_inner(ast, AstRef::new(scrut_idx), lang, env)?;
        // Convert to Value for pattern matching (which works on serde_json::Value).
        let scrut_value = tagged_to_value(&scrut_val);

        // Get the block node's children (the arm indices).
        let block_node = ast.at(AstRef::new(block_idx))?;
        let arm_indices = block_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                LangError::engine_node(
                    "match: block node missing children".to_string(),
                    current_node,
                )
            })?;

        // Try each arm.
        for arm_entry in arm_indices {
            let arm_idx = arm_entry.as_u64().ok_or_else(|| {
                LangError::engine_node(
                    "match: arm index must be a number".to_string(),
                    current_node,
                )
            })? as usize;
            let arm_node = ast.at(AstRef::new(arm_idx))?;
            let arm_children = arm_node
                .payload
                .get("children")
                .and_then(|c| c.as_array())
                .ok_or_else(|| {
                    LangError::engine_node(
                        "match: arm node missing children".to_string(),
                        current_node,
                    )
                })?;
            let pattern_idx = arm_children
                .get(0)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    LangError::engine_node(
                        "match: arm missing pattern child".to_string(),
                        current_node,
                    )
                })? as usize;
            let body_idx = arm_children
                .get(1)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    LangError::engine_node(
                        "match: arm missing body child".to_string(),
                        current_node,
                    )
                })? as usize;

            // Interpret the pattern node structurally (ADR-0061: shared
            // pattern matching logic).
            env.push_scope();
            let mut ctx = TreeWalkPatternCtx {
                engine: self,
                lang,
                env,
            };
            let matched =
                pattern_match::try_pattern(ast, AstRef::new(pattern_idx), &scrut_value, &mut ctx)?;
            if matched {
                let result = self.eval_node_inner(ast, AstRef::new(body_idx), lang, env);
                env.pop_scope();
                return result.map(|v| (v, false));
            }
            env.pop_scope();
        }

        Err(LangError::engine_node(
            "match: no arm matched".to_string(),
            current_node,
        ))
    }

    /// `handle` op: evaluate a body, then dispatch the result to the
    /// first matching handler clause.
    ///
    /// Structurally identical to `match` (children = [body_idx,
    /// block_idx], block contains `arm` nodes with [pattern_idx,
    /// handler_idx]). The body is evaluated first; its result is matched
    /// against clause patterns. If no clause matches, the body value is
    /// returned as-is.
    fn prim_handle(
        &self,
        ast: &Ast,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
    ) -> Result<(TaggedValue, bool), LangError> {
        let children = current_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                LangError::engine_node(
                    "handle: missing children in payload".to_string(),
                    current_node,
                )
            })?;
        let body_idx = children.get(0).and_then(|v| v.as_u64()).ok_or_else(|| {
            LangError::engine_node("handle: missing body child".to_string(), current_node)
        })? as usize;
        let block_idx = children.get(1).and_then(|v| v.as_u64()).ok_or_else(|| {
            LangError::engine_node("handle: missing block child".to_string(), current_node)
        })? as usize;

        // Evaluate the body.
        let body_val = self.eval_node_inner(ast, AstRef::new(body_idx), lang, env)?;
        // Convert to Value for pattern matching.
        let body_value = tagged_to_value(&body_val);

        // Get the block node's children (the clause indices).
        let block_node = ast.at(AstRef::new(block_idx))?;
        let clause_indices = block_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                LangError::engine_node(
                    "handle: block node missing children".to_string(),
                    current_node,
                )
            })?;

        // Try each clause.
        for clause_entry in clause_indices {
            let clause_idx = clause_entry.as_u64().ok_or_else(|| {
                LangError::engine_node(
                    "handle: clause index must be a number".to_string(),
                    current_node,
                )
            })? as usize;
            let clause_node = ast.at(AstRef::new(clause_idx))?;
            let clause_children = clause_node
                .payload
                .get("children")
                .and_then(|c| c.as_array())
                .ok_or_else(|| {
                    LangError::engine_node(
                        "handle: clause node missing children".to_string(),
                        current_node,
                    )
                })?;
            let pattern_idx = clause_children
                .get(0)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    LangError::engine_node(
                        "handle: clause missing pattern child".to_string(),
                        current_node,
                    )
                })? as usize;
            let handler_idx = clause_children
                .get(1)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    LangError::engine_node(
                        "handle: clause missing handler child".to_string(),
                        current_node,
                    )
                })? as usize;

            env.push_scope();
            let mut ctx = TreeWalkPatternCtx {
                engine: self,
                lang,
                env,
            };
            let matched =
                pattern_match::try_pattern(ast, AstRef::new(pattern_idx), &body_value, &mut ctx)?;
            if matched {
                let result = self.eval_node_inner(ast, AstRef::new(handler_idx), lang, env);
                env.pop_scope();
                return result.map(|v| (v, false));
            }
            env.pop_scope();
        }

        // No clause matched — return the body value as-is.
        Ok((body_val, false))
    }

    /// `json` op: parse a JSON template string from the node payload.
    ///
    /// The payload is a JSON string (produced by `LiteralKind::StripAffix`
    /// from the `json{...}` token text, ADR-0053). When the `json` node has
    /// children (i.e. `json{...}(args)` desugared by ADR-0060), child 0 is
    /// the original `json` template node and children 1..N are the argument
    /// expressions. This prim evaluates the args and performs $0/$1/…
    /// placeholder substitution into the template string before parsing.
    fn prim_json(
        &self,
        ast: &Ast,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
    ) -> Result<(TaggedValue, bool), LangError> {
        // ADR-0060: if the json node has children (desugared from
        // json{...}(args)), child 0 is the template node, children 1..N
        // are the args. Evaluate args and substitute $0, $1, … into the
        // template string.
        if let Some(children) = current_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
        {
            if children.len() >= 2 {
                // Child 0 is the original json template node.
                let template_idx = children[0].as_u64().ok_or_else(|| {
                    LangError::engine_node(
                        "json: child index must be a number".to_string(),
                        current_node,
                    )
                })? as usize;
                let template_node = ast.at(AstRef::new(template_idx))?;
                let template = template_node.payload.as_str().ok_or_else(|| {
                    LangError::engine_node(
                        "json: template payload must be a string".to_string(),
                        current_node,
                    )
                })?;
                let mut s = template.to_string();
                for (i, arg_entry) in children[1..].iter().enumerate() {
                    let idx = arg_entry.as_u64().ok_or_else(|| {
                        LangError::engine_node(
                            "json: child index must be a number".to_string(),
                            current_node,
                        )
                    })? as usize;
                    let val = self.eval_node_inner(ast, AstRef::new(idx), lang, env)?;
                    let replacement =
                        serde_json::to_string(&tagged_to_value(&val)).map_err(|e| {
                            LangError::engine_node(format!("json: {}", e), current_node)
                        })?;
                    s = s.replace(&format!("${}", i), &replacement);
                }
                return serde_json::from_str::<Value>(&s)
                    .map_err(|e| {
                        LangError::engine_node(format!("json: parse error: {}", e), current_node)
                    })
                    .map(|v| (TaggedValue::from(v), false));
            }
        }
        // No children: parse the payload string as JSON directly.
        let expr = current_node.payload.as_str().ok_or_else(|| {
            LangError::engine_node("json: payload must be a string".to_string(), current_node)
        })?;
        serde_json::from_str::<Value>(expr)
            .map_err(|e| LangError::engine_node(format!("json: parse error: {}", e), current_node))
            .map(|v| (TaggedValue::from(v), false))
    }

    /// `eval_rec` op: recursive computation region (ADR-0074).
    ///
    /// The `rec` node's children are the flattened block bindings. Each
    /// child is an expression:
    /// - `assign` children (`a = expr`) define bindings that may reference
    ///   each other cyclically.
    /// - `return` children exit the region early with the return value.
    /// - Any other expression is evaluated for its side effects / result.
    ///
    /// The engine iterates to a fixpoint: on each pass, every binding's
    /// value expression is re-evaluated using the current values of other
    /// bindings. When no binding's value changes between passes, the
    /// fixpoint is reached. A fuel limit (10000 iterations) prevents
    /// infinite loops. The last child's value is the result of the region
    /// (unless a `return` exits early).
    ///
    /// Bindings are NOT pre-initialized. On the first pass, a binding that
    /// references another not-yet-computed binding will either find it in
    /// an enclosing scope (if it exists there) or encounter an error (e.g.
    /// arithmetic on an unbound variable). Errors during binding evaluation
    /// are caught and the binding is skipped for that pass — it will be
    /// retried on the next pass once its dependencies have values.
    fn prim_eval_rec(
        &self,
        ast: &Ast,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
    ) -> Result<(TaggedValue, bool), LangError> {
        let children = current_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                LangError::engine_node(
                    "eval_rec: expected children array in payload".to_string(),
                    current_node,
                )
            })?;
        if children.is_empty() {
            return Ok((TaggedValue::Null, false));
        }

        // Push a new scope for the rec region. Bindings will be defined
        // here as they are computed. On the first pass, variable lookups
        // for not-yet-computed bindings fall through to the enclosing
        // scope (if the name exists there) or return the name as a string
        // (the default proj_env behavior for unbound variables).
        env.push_scope();

        // Track the previous value of each binding for change detection.
        // Initialize from the enclosing scope so we can detect the first
        // change correctly.
        let mut prev_values: FxHashMap<Rc<str>, TaggedValue> = FxHashMap::default();
        let mut binding_names: Vec<Rc<str>> = Vec::new();
        for entry in children {
            let child_idx = entry.as_u64().ok_or_else(|| {
                LangError::engine_node(
                    "eval_rec: child index must be a number".to_string(),
                    current_node,
                )
            })? as usize;
            let child_node = ast.at(AstRef::new(child_idx))?;
            if is_assign_node(lang, &child_node) {
                if let Some(ac) = child_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                {
                    if ac.len() >= 2 {
                        let var_idx = role_child_arena(lang, &child_node, "target")
                            .or_else(|| ac[0].as_u64().map(|i| i as usize))
                            .ok_or_else(|| {
                                LangError::engine_node(
                                    "eval_rec: assign var index must be a number".to_string(),
                                    current_node,
                                )
                            })?;
                        if let Ok(name) =
                            binding_target_name(lang, ast, var_idx, "eval_rec", current_node)
                        {
                            // Seed from the enclosing scope (before the push).
                            // Since we already pushed a scope, lookup will
                            // search innermost-first, but the inner scope is
                            // empty at this point, so it falls through.
                            let outer_val = env.lookup(&name).cloned().unwrap_or(TaggedValue::Null);
                            let name_rc = Rc::from(name.as_str());
                            prev_values.insert(Rc::clone(&name_rc), outer_val);
                            binding_names.push(name_rc);
                        }
                    }
                }
            }
        }

        let mut result = TaggedValue::Null;
        const MAX_FUEL: usize = 10000;
        let mut converged = false;

        for _iter in 0..MAX_FUEL {
            let mut changed = false;

            for entry in children {
                let child_idx = entry.as_u64().ok_or_else(|| {
                    LangError::engine_node(
                        "eval_rec: child index must be a number".to_string(),
                        current_node,
                    )
                })? as usize;
                let child_node = ast.at(AstRef::new(child_idx))?;

                // Handle `return` — exit the region with the return value.
                if is_return_node(lang, &child_node) {
                    if let Some(rc) = child_node
                        .payload
                        .get("children")
                        .and_then(|c| c.as_array())
                    {
                        if let Some(first) = rc.first() {
                            if let Some(ret_idx) = first.as_u64() {
                                result = self.eval_node_inner(
                                    ast,
                                    AstRef::new(ret_idx as usize),
                                    lang,
                                    env,
                                )?;
                                env.pop_scope();
                                return Ok((result, false));
                            }
                        }
                    }
                    // `return` with no value → exit with Null.
                    env.pop_scope();
                    return Ok((TaggedValue::Null, false));
                }

                // Handle `assign` bindings — evaluate the value expression
                // and update the binding in the current scope. Errors are
                // caught: a binding may reference another not-yet-computed
                // binding (still unbound or Null), which would error. The
                // binding is skipped for this pass and retried next pass.
                if is_assign_node(lang, &child_node) {
                    if let Some(ac) = child_node
                        .payload
                        .get("children")
                        .and_then(|c| c.as_array())
                    {
                        if ac.len() >= 2 {
                            let var_idx = role_child_arena(lang, &child_node, "target")
                                .or_else(|| ac[0].as_u64().map(|i| i as usize))
                                .ok_or_else(|| {
                                    LangError::engine_node(
                                        "eval_rec: assign var index must be a number".to_string(),
                                        current_node,
                                    )
                                })?;
                            let value_idx = role_child_arena(lang, &child_node, "value")
                                .or_else(|| ac[1].as_u64().map(|i| i as usize))
                                .ok_or_else(|| {
                                    LangError::engine_node(
                                        "eval_rec: assign value index must be a number".to_string(),
                                        current_node,
                                    )
                                })?;
                            let var_name =
                                binding_target_name(lang, ast, var_idx, "eval_rec", current_node)?;

                            match self.eval_node_inner(ast, AstRef::new(value_idx), lang, env) {
                                Ok(new_val) => {
                                    env.define(&var_name, new_val.clone());
                                    result = new_val;
                                }
                                Err(_) => {
                                    // Skip — binding couldn't be computed
                                    // this pass (likely references a
                                    // not-yet-computed binding). It will be
                                    // retried on the next pass.
                                }
                            }
                            continue;
                        }
                    }
                }

                // Non-binding expression — evaluate normally.
                result = self.eval_node_inner(ast, AstRef::new(child_idx), lang, env)?;
            }

            // End-of-pass fixpoint check. A binding's value can also change
            // via a nested update — `x = y` inside a block updates the
            // nearest enclosing binding (`ScopeStack::assign`), which is how
            // the `while`/`do-while`/`for` desugars rewrite the rec-scope
            // `__continue`/`__xs` slots from inside the `if`/`match` branch
            // blocks. Comparing only at rec-level assign-eval time misses
            // those writes and converges one pass early (or oscillates), so
            // compare each tracked binding's *current* env value against
            // the value it held at the end of the previous pass — and treat
            // any cross-scope write observed this pass (e.g. a body rebinding
            // an outer variable) as a change.
            if env.enclosing_updated {
                changed = true;
                env.enclosing_updated = false;
            }
            for name in &binding_names {
                let cur = env.lookup(name).cloned().unwrap_or(TaggedValue::Null);
                let prev = prev_values
                    .get(name.as_ref())
                    .cloned()
                    .unwrap_or(TaggedValue::Null);
                prev_values.insert(Rc::clone(name), cur.clone());
                if !rec_value_eq(&cur, &prev) {
                    changed = true;
                }
            }

            if !changed {
                converged = true;
                break;
            }
        }

        if !converged {
            env.pop_scope();
            return Err(LangError::engine_node(
                "rec: exceeded fuel limit (10000 iterations) without reaching fixpoint".to_string(),
                current_node,
            ));
        }

        env.pop_scope();
        Ok((result, false))
    }
}

impl Default for TreeWalkEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeWalkEngine {
    /// Shared eval body for [`EvalEngine::eval`] / [`EvalEngine::eval_shared`]:
    /// install `ast_rc`/`lang_rc`/modules in the thread-locals (saving the
    /// previous values for nested evals), then evaluate the arena root.
    fn eval_ast_rc(&self, lang: &Language, ast_rc: Rc<Ast>) -> Result<Value, LangError> {
        self.eval_ast_rc_with_args(lang, ast_rc, &[])
    }

    /// Like [`eval_ast_rc`](Self::eval_ast_rc), but merges `call_args`
    /// directly into the call frame's `ScopeStack` globals map — **without**
    /// cloning the engine via [`with_global`](Self::with_global).
    ///
    /// The call args are bound into the same `globals` field that
    /// host-provided globals (e.g. `self`) use, so they are:
    ///
    /// - resolvable via [`ScopeStack::lookup`] (globals fallback),
    /// - **excluded from `capture()`** — they are never deep-cloned into
    ///   closures ("no env capture", ADR-0106), and
    /// - not stored on the engine struct, so no `box_clone` is needed.
    ///
    /// This is the tree-walk override of
    /// [`EvalEngine::eval_shared_with_call_args`], used by
    /// `plugin-lib-handl`'s `HandlFunction::call` to bind `__arg_0`..
    /// `__arg_{n-1}` directly on the call frame instead of injecting them as
    /// per-call engine globals.
    fn eval_ast_rc_with_args(
        &self,
        lang: &Language,
        ast_rc: Rc<Ast>,
        call_args: &[(String, Value)],
    ) -> Result<Value, LangError> {
        if ast_rc.is_empty() {
            return Ok(Value::Null);
        }
        // Set thread-locals so host functions can call arrow-values via
        // ctx.try_call_value (see TreeWalkCallable below). Save the previous
        // values so a *nested* eval (a `.hnl` module function calling into
        // another `HandlFunction` — e.g. `dedent` → `str.*` → `str-handl` →
        // `char.*` runs a nested `Runtime::run` → `eval`) restores them on
        // return instead of clobbering the outer runtime's state.
        let lang_rc = Rc::new(lang.clone());
        let modules_rc = self.modules.clone();
        let import_resolver_rc = self.import_resolver.clone();

        let prev_ast =
            CURRENT_AST.with(|c| std::mem::replace(&mut *c.borrow_mut(), Some(Rc::clone(&ast_rc))));
        let prev_lang = CURRENT_LANG
            .with(|c| std::mem::replace(&mut *c.borrow_mut(), Some(Rc::clone(&lang_rc))));
        let prev_modules =
            CURRENT_MODULES.with(|c| std::mem::replace(&mut *c.borrow_mut(), modules_rc));
        let prev_import_resolver = CURRENT_IMPORT_RESOLVER
            .with(|c| std::mem::replace(&mut *c.borrow_mut(), import_resolver_rc));
        let mut env = ScopeStack::new();
        // Build the merged globals map: host-provided globals (e.g. `self`)
        // plus the per-call argument bindings (`__arg_0`, …). Both live in the
        // `globals` field of the `ScopeStack` — accessible via `lookup()` but
        // excluded from `capture()`, so arguments are never deep-cloned into
        // closures (ADR-0106 "no env capture").
        //
        // This replaces the old `with_global`-per-arg path: instead of cloning
        // the engine N times (once per `__arg_N`), we merge all args into a
        // single `Rc` map and install it once — zero engine clones per call.
        let globals_rc: Rc<FxHashMap<Rc<str>, TaggedValue>> = Rc::new({
            let mut map: FxHashMap<Rc<str>, TaggedValue> = self
                .globals
                .iter()
                .map(|(k, v)| (Rc::from(k.as_str()), TaggedValue::from(v.clone())))
                .collect();
            for (name, val) in call_args {
                map.insert(Rc::from(name.as_str()), TaggedValue::from(val.clone()));
            }
            map
        });
        env.set_globals(Rc::clone(&globals_rc));
        let prev_globals = CURRENT_GLOBALS
            .with(|c| std::mem::replace(&mut *c.borrow_mut(), Some(Rc::clone(&globals_rc))));
        // The root is the last node in the arena (Pratt convention).
        let root_ref = AstRef::new(ast_rc.len() - 1);
        let result = self.eval_node(&ast_rc, root_ref, &lang_rc, &mut env);
        // Restore the previous thread-locals (None for a non-nested eval).
        CURRENT_AST.with(|c| *c.borrow_mut() = prev_ast);
        CURRENT_LANG.with(|c| *c.borrow_mut() = prev_lang);
        CURRENT_MODULES.with(|c| *c.borrow_mut() = prev_modules);
        CURRENT_GLOBALS.with(|c| *c.borrow_mut() = prev_globals);
        CURRENT_IMPORT_RESOLVER.with(|c| *c.borrow_mut() = prev_import_resolver);
        result
    }
}

impl EvalEngine for TreeWalkEngine {
    fn eval(&self, lang: &Language, req: &EvalRequest) -> Result<Value, LangError> {
        self.eval_ast_rc(lang, Rc::new(req.ast.clone()))
    }

    /// `Rc`-carrying entry point (e.g. `plugin-lib-handl`'s per-thread
    /// parsed-AST cache): installs the caller's `Rc<Ast>` directly as
    /// `CURRENT_AST`, avoiding the per-call arena deep clone entirely.
    fn eval_shared(&self, lang: &Language, ast: &Rc<Ast>) -> Result<Value, LangError> {
        self.eval_ast_rc(lang, Rc::clone(ast))
    }

    /// Tree-walk override: bind `args` directly into the call frame's
    /// `ScopeStack` globals (alongside host-provided globals like `self`)
    /// without cloning the engine. This is the zero-clone path used by
    /// `HandlFunction::call` — args cross the boundary as values, not via
    /// per-call `with_global` engine clones.
    ///
    /// The args are merged into the same `Rc<FxHashMap>` that the engine's
    /// host globals occupy, so they are resolvable via `lookup()` but
    /// excluded from `capture()` (no env capture, ADR-0106).
    fn eval_shared_with_call_args(
        &self,
        lang: &Language,
        ast: &Rc<Ast>,
        args: &[(String, Value)],
    ) -> Result<Value, LangError> {
        self.eval_ast_rc_with_args(lang, Rc::clone(ast), args)
    }

    fn box_clone(&self) -> Box<dyn EvalEngine> {
        Box::new(self.clone())
    }

    fn with_modules(&self, modules: Rc<dyn ModuleLookup>) -> Box<dyn EvalEngine> {
        Box::new(self.clone().with_modules(modules))
    }

    fn with_global(&self, name: &str, val: Value) -> Box<dyn EvalEngine> {
        Box::new(self.clone().with_global(name, val))
    }

    fn with_import_resolver(&self, resolver: Rc<dyn ImportResolver>) -> Box<dyn EvalEngine> {
        Box::new(self.clone().with_import_resolver(resolver))
    }

    fn seal_module_value(
        &self,
        ast: &Ast,
        lang: &Language,
        value: Value,
    ) -> Result<Value, LangError> {
        if !contains_unhomed_arrow(&value) {
            return Ok(value);
        }
        // Register the module's eval frame so the arrow's `body` index
        // remains meaningful in the importer's scope: `apply_app` /
        // `call_value` route `__home__`-stamped arrows back to this frame
        // (its AST, `Language`, module chain, and globals).

        let frame_id = register_home_frame(
            Rc::new(ast.clone()),
            Rc::new(lang.clone()),
            self.modules.clone(),
            if self.globals.is_empty() {
                None
            } else {
                Some(self.globals.clone())
            },
            self.import_resolver.clone(),
        );
        let mut value = value;
        stamp_home(&mut value, frame_id);
        Ok(value)
    }
}

/// A `plugin_engine::Model` adapter around the tree-walk engine.
///
/// It evaluates bare [`Term`] schemes by mounting them on a minimal
/// dummy AST / Language context and reusing the existing tree-walk
/// evaluation logic.
///
/// # ADR-0103 continuation: arrow-model delegation
///
/// The optional second field (`arrow_model_delegate`) is a Handl-authored
/// `Model` (e.g. [`HandlModelAdapter`](handl_plugin_lib_handl::HandlModelAdapter)
/// wrapping `arrow_model.hnl`). When set via
/// [`ValueModel::with_arrow_model_delegate`], [`Model::run`] delegates
/// HoTT-type terms — those for which [`plugin_engine::is_hott_tag`]
/// returns `true` — to the delegate *before* the native Rust match
/// dispatch. If the delegate returns `NotImplemented`, the engine falls
/// back to its existing match arms, so the delegate is strictly opt-in:
/// a default `ValueModel` (constructed via [`ValueModel::new`]) behaves
/// exactly as before.
pub struct ValueModel(
    /// The underlying tree-walk engine.
    pub TreeWalkEngine,
    /// Optional delegate model for HoTT-type terms (ADR-0103
    /// continuation). `None` for the default bootstrap engine.
    Option<Box<dyn Model<Value = serde_json::Value>>>,
);

impl ValueModel {
    /// Create a `ValueModel` wrapping `engine` with no arrow-model
    /// delegate. Equivalent to the historical `ValueModel(engine)`
    /// constructor; the default engine behaves exactly as before
    /// (HoTT tags use the native Rust match arms).
    pub fn new(engine: TreeWalkEngine) -> Self {
        Self(engine, None)
    }

    /// Configure an arrow-model delegate (a Handl-authored `Model` such
    /// as `HandlModelAdapter`) that handles HoTT-type terms. When set,
    /// [`Model::run`] delegates any term for which
    /// [`plugin_engine::is_hott_tag`] returns `true` to this delegate
    /// before the native match dispatch, falling back on
    /// `NotImplemented`. This is the ADR-0103 continuation seam that
    /// begins stripping HoTT semantics out of the Rust tree-walk engine.
    pub fn with_arrow_model_delegate(
        mut self,
        delegate: impl Model<Value = serde_json::Value> + 'static,
    ) -> Self {
        self.1 = Some(Box::new(delegate));
        self
    }

    /// Evaluate `term` against a minimal dummy AST / Language context via
    /// the tree-walk engine's scheme interpreter (the native fallback for
    /// `Model::run`).
    fn eval_scheme_term(
        &self,
        term: &Term,
        incoming: Value,
    ) -> Result<Value, plugin_engine::LangError> {
        let mut ast = Ast::new();
        ast.push(AstNode::new("model"));
        let node_ref = AstRef::new(0);
        let node = ast
            .at(node_ref)
            .map_err(|_| plugin_engine::LangError::NotImplemented)?;
        let lang = Language::new();
        let mut env = ScopeStack::new();
        self.0
            .eval_scheme(term, &ast, node_ref, &lang, &mut env, node, Some(incoming))
            .map_err(|_| plugin_engine::LangError::NotImplemented)
    }

    /// Grade-based `Op` dispatch: look up the op declaration for the
    /// required grade, verify the incoming value carries that grade, then
    /// search `ctx.handler_stack` (innermost-last) and the language-level
    /// `handler_for_grade` for a matching `Handler` providing `op`. If no
    /// handler matches, returns `Ok(None)` so the caller can fall back to
    /// native dispatch.
    fn dispatch_op(
        &self,
        op: &str,
        args: &[Term],
        incoming: Value,
        ctx: &mut ModelCtx,
    ) -> Result<Option<Value>, plugin_engine::LangError> {
        // a. Look up the op declaration and the required grade, if any.
        let required_grade = ctx
            .lang
            .op_decl(op)
            .and_then(|decl| decl.get("grade").and_then(Value::as_str).map(String::from));

        // b,c. Reject the dispatch when the incoming value does not carry
        // the required grade.
        if let Some(ref grade) = required_grade {
            let grades = incoming
                .get("grades")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                .unwrap_or_default();
            if !grades.contains(&grade.as_str()) {
                return Err(plugin_engine::LangError::NotImplemented);
            }
        }

        // Evaluate the argument sub-terms with `incoming` (the handler
        // `Term` consumes them via `Term::App` / `Term::Child`; the
        // evaluated values are bound into the run via the incoming pair).
        let mut arg_vals = Vec::with_capacity(args.len());
        for arg in args {
            arg_vals.push(self.run(arg, incoming.clone(), ctx)?);
        }

        // d. Search the handler stack from top to bottom for a `Handler`
        //    whose `grades` list contains the required grade and which
        //    provides an implementation for `op`.
        let mut impl_json = None;
        for handler in ctx.handler_stack.iter().rev() {
            if let Some(ref grade) = required_grade {
                let grades = handler
                    .get("grades")
                    .and_then(Value::as_array)
                    .map(|arr| arr.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                    .unwrap_or_default();
                if !grades.contains(&grade.as_str()) {
                    continue;
                }
            }
            if let Some(value) = handler.get("impls").and_then(|m| m.get(op)).cloned() {
                impl_json = Some(value);
                break;
            }
        }

        // e. Fall back to language-level handlers keyed by grade.
        if impl_json.is_none() {
            if let Some(ref grade) = required_grade {
                impl_json = ctx
                    .lang
                    .handler_for_grade(grade)
                    .and_then(|h| h.get("impls").and_then(|m| m.get(op)).cloned());
            }
        }

        // f. No handler found — let the caller fall back to native dispatch.
        let Some(impl_json) = impl_json else {
            return Ok(None);
        };

        let impl_term: Term = serde_json::from_value(impl_json)
            .map_err(|_| plugin_engine::LangError::NotImplemented)?;
        // Feed the handler the incoming value paired with the evaluated
        // args so it can consume them.
        let handler_in = if arg_vals.is_empty() {
            incoming
        } else {
            serde_json::json!({ "__pair__": [incoming, arg_vals] })
        };
        self.run(&impl_term, handler_in, ctx).map(Some)
    }
}

impl Model for ValueModel {
    type Value = serde_json::Value;

    fn run(
        &self,
        term: &Term,
        incoming: Self::Value,
        ctx: &mut ModelCtx,
    ) -> Result<Self::Value, plugin_engine::LangError> {
        // ADR-0103 continuation: delegate HoTT-type terms to the
        // arrow-model delegate (a Handl-authored `.hnl` model) *before*
        // the native Rust match dispatch. This begins stripping HoTT
        // semantics (Path, Equiv, Refine, Rewrite, …) out of the
        // tree-walk engine: when a delegate is configured and the term
        // is a HoTT tag ([`is_hott_tag`]), the delegate's `run` decides
        // the result. On `NotImplemented` we fall through to the
        // existing match arms below, which remain as fallbacks so a
        // default `ValueModel` (no delegate) behaves exactly as before.
        if let Some(delegate) = &self.1 {
            if is_hott_tag(term) {
                match delegate.run(term, incoming.clone(), ctx) {
                    Ok(v) => return Ok(v),
                    // `plugin_engine::LangError` is currently single-variant
                    // (`NotImplemented`); fall through to the native match
                    // arms so the delegate is strictly opt-in.
                    Err(_) => { /* fall through */ }
                }
            }
        }
        // Mirror the model-level handler stack into the engine's runtime
        // stack so `Term::Op` dispatch inside delegated `eval_scheme`
        // calls sees the same handlers.
        *self.0.handler_stack.borrow_mut() = ctx
            .handler_stack
            .iter()
            .cloned()
            .map(TaggedValue::from)
            .collect();
        // ADR-0103 Phase 6: dispatch on the `__tag__` field via TermData
        // instead of matching the `Term` enum. The `&Term` entry point
        // converts once to TermData and dispatches here.
        let td = TermData::from_term(term);
        match td.tag() {
            "Seq" => {
                let fst = td.get_term("fst").expect("Seq term missing 'fst' field");
                let snd = td.get_term("snd").expect("Seq term missing 'snd' field");
                let fst_term = fst
                    .to_term()
                    .map_err(|_| plugin_engine::LangError::NotImplemented)?;
                let snd_term = snd
                    .to_term()
                    .map_err(|_| plugin_engine::LangError::NotImplemented)?;
                let fst_val = self.run(&fst_term, incoming, ctx)?;
                self.run(&snd_term, fst_val, ctx)
            }
            "Rewrite" => {
                let target = td
                    .get_term("target")
                    .expect("Rewrite term missing 'target' field");
                let target_term = target
                    .to_term()
                    .map_err(|_| plugin_engine::LangError::NotImplemented)?;
                match self.run(&target_term, incoming, ctx) {
                    Ok(v) => Ok(v),
                    Err(_) => Err(plugin_engine::LangError::NotImplemented),
                }
            }
            // `Child { role }` (ADR-0081): resolve the named child role to a
            // child position via the language view, project that child out of
            // the current node, then run the child's own eval scheme.
            //
            // The `ModelCtx` only exposes `AstView`/`LanguageView` trait
            // objects, so the recursive `eval_scheme` call — which needs the
            // concrete `Ast`/`Language` — recovers them via `as_any`
            // downcasts. Non-`handl_language` views return `NotImplemented`.
            // TODO: give `plugin_engine::LangError` a payload so real errors
            // can be propagated instead of collapsed to `NotImplemented`.
            "Child" => {
                let role = td.get_str("role").expect("Child term missing 'role' field");
                let ast = ctx
                    .ast
                    .as_any()
                    .downcast_ref::<Ast>()
                    .ok_or(plugin_engine::LangError::NotImplemented)?;
                let lang = ctx
                    .lang
                    .as_any()
                    .downcast_ref::<Language>()
                    .ok_or(plugin_engine::LangError::NotImplemented)?;
                let kind = ctx
                    .ast
                    .node_kind(ctx.node_ref)
                    .ok_or(plugin_engine::LangError::NotImplemented)?;
                let child_idx = ctx
                    .lang
                    .child_role_index(kind, role)
                    .ok_or(plugin_engine::LangError::NotImplemented)?;
                let child_ref = ctx
                    .ast
                    .node_child(ctx.node_ref, child_idx)
                    .ok_or(plugin_engine::LangError::NotImplemented)?;
                let child_kind = ctx
                    .ast
                    .node_kind(child_ref)
                    .ok_or(plugin_engine::LangError::NotImplemented)?;
                let child_node = ast
                    .at(AstRef::new(child_ref))
                    .map_err(|_| plugin_engine::LangError::NotImplemented)?;
                let mut env = ScopeStack::new();
                match ctx.lang.eval_rule(child_kind) {
                    Some(scheme) => self
                        .0
                        .eval_scheme(
                            scheme,
                            ast,
                            AstRef::new(child_ref),
                            lang,
                            &mut env,
                            child_node,
                            Some(incoming),
                        )
                        .map_err(|_| plugin_engine::LangError::NotImplemented),
                    // No eval scheme for the child kind — fall back to the
                    // engine's own node evaluation (which returns the
                    // payload for literal nodes).
                    None => self
                        .0
                        .eval_node(ast, AstRef::new(child_ref), lang, &mut env)
                        .map_err(|_| plugin_engine::LangError::NotImplemented),
                }
            }
            // `handle body with target = impl_`: evaluate `impl_` to a
            // handler value, push it onto `ctx.handler_stack`, run `body`,
            // then pop. `target` is retained for diagnostics.
            "Handle" => {
                let body = td
                    .get_term("body")
                    .expect("Handle term missing 'body' field");
                let impl_ = td
                    .get_term("impl_")
                    .expect("Handle term missing 'impl_' field");
                let body_term = body
                    .to_term()
                    .map_err(|_| plugin_engine::LangError::NotImplemented)?;
                let impl_term = impl_
                    .to_term()
                    .map_err(|_| plugin_engine::LangError::NotImplemented)?;
                let handler = self.run(&impl_term, incoming.clone(), ctx)?;
                ctx.push_handler(handler.clone());
                self.0
                    .handler_stack
                    .borrow_mut()
                    .push(TaggedValue::from(handler));
                let result = self.run(&body_term, incoming, ctx);
                ctx.pop_handler();
                self.0.handler_stack.borrow_mut().pop();
                result
            }
            // `Term::Op`: grade-based dispatch — evaluate each arg with
            // `incoming`, then consult the handler stack (and the incoming
            // value's `impls`) for a handler providing the op before
            // falling back to the engine's native dispatch.
            "Op" => {
                let name = td
                    .get_str("name")
                    .expect("Op term missing 'name' field")
                    .to_string();
                let args_td = td.get_term_list("args");
                let source = td
                    .arr_source()
                    .expect("Op term missing or malformed 'source' field");
                let op_name = match source {
                    ArrSource::Native => name.clone(),
                    ArrSource::Foreign(n) => n,
                    _ => name.clone(),
                };
                // Convert args back to `Term` for `dispatch_op` (which
                // takes `&[Term]`). This is the thin-wrapper round-trip.
                let mut args = Vec::with_capacity(args_td.len());
                for arg_td in &args_td {
                    args.push(
                        arg_td
                            .to_term()
                            .map_err(|_| plugin_engine::LangError::NotImplemented)?,
                    );
                }
                if let Some(v) = self.dispatch_op(&op_name, &args, incoming.clone(), ctx)? {
                    return Ok(v);
                }
                self.eval_scheme_term(term, incoming)
            }
            "Path" => {
                let a = td.get_term("from").expect("Path term missing 'from' field");
                let b = td.get_term("to").expect("Path term missing 'to' field");
                let a_term = a
                    .to_term()
                    .map_err(|_| plugin_engine::LangError::NotImplemented)?;
                let b_term = b
                    .to_term()
                    .map_err(|_| plugin_engine::LangError::NotImplemented)?;
                let a_val = self.run(&a_term, incoming.clone(), ctx)?;
                let b_val = self.run(&b_term, incoming.clone(), ctx)?;
                let w_val = match td.get("witness") {
                    Some(v) if !v.is_null() => {
                        let w_td = TermData(v.clone());
                        let w_term = w_td
                            .to_term()
                            .map_err(|_| plugin_engine::LangError::NotImplemented)?;
                        self.run(&w_term, incoming, ctx)?
                    }
                    _ => Value::Null,
                };
                Ok(serde_json::json!({
                    "__tag__": "Path",
                    "from": a_val,
                    "to": b_val,
                    "witness": w_val
                }))
            }
            "Equiv" => {
                let a = td.get_term("a").expect("Equiv term missing 'a' field");
                let b = td.get_term("b").expect("Equiv term missing 'b' field");
                let a_term = a
                    .to_term()
                    .map_err(|_| plugin_engine::LangError::NotImplemented)?;
                let b_term = b
                    .to_term()
                    .map_err(|_| plugin_engine::LangError::NotImplemented)?;
                let a_val = self.run(&a_term, incoming.clone(), ctx)?;
                let b_val = self.run(&b_term, incoming.clone(), ctx)?;
                let mut eq = a_val == b_val;
                if let Some(p) = td.get_term("path") {
                    if !p.as_value().is_null() {
                        let p_term = p
                            .to_term()
                            .map_err(|_| plugin_engine::LangError::NotImplemented)?;
                        let p_val = self.run(&p_term, incoming, ctx)?;
                        if let Value::Object(obj) = &p_val {
                            if obj.get("__tag__").and_then(Value::as_str) == Some("Path") {
                                let from = obj.get("from");
                                let to = obj.get("to");
                                if from == Some(&a_val) && to == Some(&b_val) {
                                    eq = true;
                                }
                            }
                        }
                    }
                }
                Ok(Value::Bool(eq))
            }
            _ => self.eval_scheme_term(term, incoming),
        }
    }

    fn rewrite(
        &self,
        source: &Term,
        target: &Term,
        witness: &plugin_engine::RewriteWitness,
        incoming: &Self::Value,
        ctx: &mut ModelCtx,
    ) -> Result<RewriteResult<Self::Value>, plugin_engine::LangError> {
        let (source, target, is_rewrite) = match witness {
            plugin_engine::RewriteWitness::Named(name) => {
                if let Some(rule) = ctx.lang.law(name) {
                    let law_source = rule
                        .get("source")
                        .and_then(|v| serde_json::from_value(v.clone()).ok());
                    let law_target = rule
                        .get("target")
                        .and_then(|v| serde_json::from_value(v.clone()).ok());
                    match (law_source, law_target) {
                        (Some(s), Some(t)) => (s, t, false),
                        _ => return Ok(RewriteResult::Unsupported),
                    }
                } else if let Some(rule) = ctx.lang.rewrite_rule(name) {
                    let rewrite_source = rule
                        .get("source")
                        .and_then(|v| serde_json::from_value(v.clone()).ok());
                    let rewrite_target = rule
                        .get("target")
                        .and_then(|v| serde_json::from_value(v.clone()).ok());
                    match (rewrite_source, rewrite_target) {
                        (Some(s), Some(t)) => (s, t, true),
                        _ => return Ok(RewriteResult::Unsupported),
                    }
                } else {
                    (source.clone(), target.clone(), false)
                }
            }
            plugin_engine::RewriteWitness::Compute => (source.clone(), target.clone(), true),
            _ => (source.clone(), target.clone(), false),
        };
        if TermData::from_term(&source).is_tag("Rewrite")
            || TermData::from_term(&target).is_tag("Rewrite")
        {
            return Ok(RewriteResult::Unsupported);
        }
        let a = self.run(&source, incoming.clone(), ctx)?;
        let b = self.run(&target, incoming.clone(), ctx)?;
        if a == b {
            Ok(RewriteResult::Equal(b))
        } else if is_rewrite {
            Ok(RewriteResult::Rewritten(b))
        } else {
            Ok(RewriteResult::NotEqual)
        }
    }
}

// ---------------------------------------------------------------------------
// Arrow-value sealing helpers (Phase 9.5 module import)
// ---------------------------------------------------------------------------
//
// Mirrors of the private helpers in `plugin-lib-handl` (which stamp arrow
// arguments/results crossing `HandlFunction` boundaries); kept local so the
// sealing hook needs no cross-crate plumbing.

/// Whether `value` contains an arrow value (`{"__arrow__": true, ...}`)
/// that carries no `__home__` frame id — i.e. an arrow whose `body` index
/// refers to the ambient eval's AST and must be stamped before it crosses
/// into an importer's scope.
fn contains_unhomed_arrow(value: &Value) -> bool {
    match value {
        Value::Array(a) => a.iter().any(contains_unhomed_arrow),
        Value::Object(o) => {
            if o.get("__arrow__").and_then(|v| v.as_bool()) == Some(true) {
                return o.get("__home__").and_then(|v| v.as_u64()).is_none()
                    || o.values().any(contains_unhomed_arrow);
            }
            o.values().any(contains_unhomed_arrow)
        }
        _ => false,
    }
}

/// Stamp every arrow value inside `value` that lacks a `__home__` frame id
/// with `frame_id` — recursively, including arrows nested inside a
/// closure's captured `env` (their `body` indices refer to the same AST).
/// Arrows that already carry a `__home__` keep it: their provenance is
/// elsewhere.
fn stamp_home(value: &mut Value, frame_id: u64) {
    match value {
        Value::Array(a) => {
            for v in a.iter_mut() {
                stamp_home(v, frame_id);
            }
        }
        Value::Object(o) => {
            if o.get("__arrow__").and_then(|v| v.as_bool()) == Some(true)
                && o.get("__home__").and_then(|v| v.as_u64()).is_none()
            {
                o.insert("__home__".to_string(), Value::from(frame_id));
            }
            for v in o.values_mut() {
                stamp_home(v, frame_id);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// ValueCallable — allows host functions (fn.map, fn.filter, etc.) to call
// arrow-values passed as arguments.
// ---------------------------------------------------------------------------

/// A `ValueCallable` that calls arrow-values using the thread-local AST
/// and Language set by `EvalEngine::eval`.
pub struct TreeWalkCallable;

impl ValueCallable for TreeWalkCallable {
    fn call_value(
        &self,
        callee: &Value,
        args: &[Value],
    ) -> Result<Value, handl_platform::EvalError> {
        let obj = callee.as_object().ok_or_else(|| {
            handl_platform::EvalError::Other("call_value: not an object".to_string())
        })?;
        if !obj
            .get("__arrow__")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return Err(handl_platform::EvalError::Other(
                "call_value: not an arrow-value".to_string(),
            ));
        }
        // Boundary-crossed arrows carry `"__home__"` — the id of the eval
        // frame that minted them. Route the application through that frame
        // (installed in the thread-locals for the duration) so the body's
        // AST index and module chain resolve correctly.
        if let Some(frame_id) = obj.get("__home__").and_then(|v| v.as_u64()) {
            let frame = home_frame(frame_id).ok_or_else(|| {
                handl_platform::EvalError::Other(format!(
                    "call_value: unknown home frame {}",
                    frame_id
                ))
            })?;
            return frame.enter(|| self.call_value_in_frame(callee, args));
        }
        self.call_value_in_frame(callee, args)
    }
}

impl TreeWalkCallable {
    /// Invoke an arrow-value in the *ambient* frame — the thread-locals
    /// currently installed (either the running eval's or a home frame's
    /// after [`EvalFrame::enter`]).
    fn call_value_in_frame(
        &self,
        callee: &Value,
        args: &[Value],
    ) -> Result<Value, handl_platform::EvalError> {
        let obj = callee.as_object().ok_or_else(|| {
            handl_platform::EvalError::Other("call_value: not an object".to_string())
        })?;

        let params: Vec<String> = obj
            .get("params")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                handl_platform::EvalError::Other("arrow-value missing params".to_string())
            })?
            .iter()
            .map(|v| {
                v.as_str().map(String::from).ok_or_else(|| {
                    handl_platform::EvalError::Other("arrow-value param not string".to_string())
                })
            })
            .collect::<Result<_, _>>()?;
        let body_idx = obj.get("body").and_then(|v| v.as_u64()).ok_or_else(|| {
            handl_platform::EvalError::Other("arrow-value missing body".to_string())
        })?;
        let captured_env: FxHashMap<Rc<str>, TaggedValue> = obj
            .get("env")
            .and_then(|v| v.as_object())
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (Rc::from(k.as_str()), TaggedValue::from(v.clone())))
                    .collect()
            })
            .unwrap_or_default();
        // ADR-0074: destructure a single Pair (product) arg for multi-param
        // arrows. Host functions like fn.map may pass a Pair to a multi-param
        // arrow-value.
        let actual_args: Vec<Value> = if params.len() > 1 && args.len() == 1 {
            // Try to flatten if the single arg is a Pair (serialized as
            // {"__pair__": [a, b]}).
            if let Some(pair) = args[0].get("__pair__").and_then(|v| v.as_array()) {
                let mut flat: Vec<Value> = Vec::new();
                fn flatten_pair_json(v: &Value, flat: &mut Vec<Value>) {
                    if let Some(p) = v.get("__pair__").and_then(|p| p.as_array()) {
                        if p.len() == 2 {
                            flatten_pair_json(&p[0], flat);
                            flatten_pair_json(&p[1], flat);
                            return;
                        }
                    }
                    flat.push(v.clone());
                }
                if pair.len() == 2 {
                    flatten_pair_json(&pair[0], &mut flat);
                    flatten_pair_json(&pair[1], &mut flat);
                    flat
                } else {
                    args.to_vec()
                }
            } else {
                args.to_vec()
            }
        } else {
            args.to_vec()
        };
        if actual_args.len() < params.len() {
            return Err(handl_platform::EvalError::Other(format!(
                "arrow-value expects {} args, got {}",
                params.len(),
                actual_args.len()
            )));
        }
        // Get AST and Language from thread-locals.
        let ast = CURRENT_AST.with(|c| c.borrow().clone()).ok_or_else(|| {
            handl_platform::EvalError::Other("call_value: no AST in thread-local".to_string())
        })?;
        let lang = CURRENT_LANG.with(|c| c.borrow().clone()).ok_or_else(|| {
            handl_platform::EvalError::Other("call_value: no Language in thread-local".to_string())
        })?;
        let engine = TreeWalkEngine::new();
        // Set modules if available.
        let engine = if let Some(mods) = CURRENT_MODULES.with(|c| c.borrow().clone()) {
            engine.with_modules(mods)
        } else {
            engine
        };
        // Set import resolver if available (home frame may have crossed
        // a runtime boundary, e.g. the test runner applying a `.hnl` arrow).
        let mut engine = engine;
        engine.import_resolver = CURRENT_IMPORT_RESOLVER.with(|c| c.borrow().clone());

        // Curried application, matching `apply_app`: surplus args apply
        // the body's result (another arrow) to the remainder — `f(1, 5)`
        // on `x ~> y ~> body` is `(f 1) 5`. The recursion goes through
        // `call_value` so intermediate arrows keep routing by `__home__`.
        let (head, mut rest) = actual_args.split_at(params.len());
        let mut val = {
            // Build a fresh env that **shares** the arrow's captured env via
            // `Rc` (an O(1) refcount bump — no deep clone on every call).
            let mut arrow_env = ScopeStack::new();
            arrow_env.captured = Some(Rc::new(captured_env));
            // Re-attach host globals (e.g. `self`) from the thread-local so
            // the closure body can look them up. The captured env only
            // contains user-created variables (globals are excluded at
            // capture time), so this fallback is required.
            if let Some(g) = CURRENT_GLOBALS.with(|c| c.borrow().clone()) {
                arrow_env.set_globals(g);
            }
            arrow_env.push_scope();
            for (param, arg) in params.iter().zip(head.iter()) {
                arrow_env.define(param, TaggedValue::from(arg.clone()));
            }
            // Arrow bodies are carried as self-describing `body_def` AST
            // subtrees so the body is always resolvable regardless of which
            // runtime frame applies the arrow. Use it in preference to the
            // ambient AST `body` index, which may be stale after the arrow
            // crosses a serde boundary.
            let body_res: Result<Value, handl_platform::EvalError> = if let Some(body_def) =
                obj.get("body_def").and_then(|v| handl_language::as_ast(v))
            {
                if body_def.is_empty() {
                    Err(handl_platform::EvalError::Other(
                        "arrow body_def is empty".to_string(),
                    ))
                } else {
                    let body_ref = AstRef::new(body_def.len() - 1);
                    let body_ast = Rc::new(body_def);
                    let frame = EvalFrame {
                        ast: body_ast,
                        lang: Rc::clone(&lang),
                        modules: CURRENT_MODULES.with(|c| c.borrow().clone()),
                        globals: CURRENT_GLOBALS.with(|c| c.borrow().clone()),
                        import_resolver: CURRENT_IMPORT_RESOLVER.with(|c| c.borrow().clone()),
                    };
                    frame.enter(|| {
                        engine
                            .eval_node_inner(&frame.ast, body_ref, &frame.lang, &mut arrow_env)
                            .map(|v| tagged_to_value(&v))
                            .map_err(|e| handl_platform::EvalError::Other(e.to_string()))
                    })
                }
            } else {
                engine
                    .eval_node_inner(&ast, AstRef::new(body_idx as usize), &lang, &mut arrow_env)
                    .map(|v| tagged_to_value(&v))
                    .map_err(|e| handl_platform::EvalError::Other(e.to_string()))
            };
            body_res?
        };
        while !rest.is_empty() {
            val = self.call_value(&val, rest)?;
            rest = &[];
        }
        Ok(val)
    }
}

// ---------------------------------------------------------------------------
// Numeric helpers
// ---------------------------------------------------------------------------

/// A number that tracks int-vs-float.
#[derive(Debug, Clone, Copy)]
enum Num {
    /// Integer value.
    Int(i64),
    /// Float value.
    Float(f64),
}

impl Num {
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

/// Extract a number from a [`TaggedValue`].
fn as_num(v: &TaggedValue) -> Result<Num, LangError> {
    match v {
        TaggedValue::Int(i) => Ok(Num::Int(*i)),
        TaggedValue::Float(f) => Ok(Num::Float(*f)),
        _ => Err(LangError::engine(format!("expected number, got {:?}", v))),
    }
}

/// Convert a [`Num`] back to a [`TaggedValue`].
fn num_value(n: Num) -> TaggedValue {
    match n {
        Num::Int(i) => TaggedValue::Int(i),
        Num::Float(f) => TaggedValue::Float(f),
    }
}

/// Whether a [`TaggedValue`] is truthy.
///
/// `false`, `null`, `0`, and `""` are falsy; everything else is truthy.
fn is_truthy(v: &TaggedValue) -> bool {
    match v {
        TaggedValue::Bool(b) => *b,
        TaggedValue::Null => false,
        TaggedValue::Int(i) => *i != 0,
        TaggedValue::Float(f) => *f != 0.0,
        TaggedValue::Str(s) => !s.is_empty(),
        _ => true,
    }
}

/// Binary numeric op helper.
fn bin_num<F>(args: &[TaggedValue], f: F) -> Result<(TaggedValue, bool), LangError>
where
    F: FnOnce(Num, Num) -> Num,
{
    let a = as_num(&args[0])?;
    let b = as_num(&args[1])?;
    Ok((num_value(f(a, b)), false))
}

/// Addition helper. Supports string concatenation when both operands are
/// strings, and numeric addition when both operands are numbers. Any other
/// combination of types is a type-mismatch error.
fn bin_add(args: &[TaggedValue]) -> Result<(TaggedValue, bool), LangError> {
    let a = &args[0];
    let b = &args[1];
    match (a, b) {
        (TaggedValue::Str(s1), TaggedValue::Str(s2)) => {
            Ok((TaggedValue::Str(Rc::from(format!("{}{}", s1, s2))), false))
        }
        (TaggedValue::Str(_), _) | (_, TaggedValue::Str(_)) => Err(LangError::engine(format!(
            "add: type mismatch ({} + {}): string concatenation requires both operands to be strings",
            tagged_to_value(a),
            tagged_to_value(b)
        ))),
        _ => bin_num(args, |x, y| x + y),
    }
}

/// Numeric comparison helper.
fn cmp_num<F>(args: &[TaggedValue], f: F) -> Result<(TaggedValue, bool), LangError>
where
    F: FnOnce(Num, Num) -> bool,
{
    let a = as_num(&args[0])?;
    let b = as_num(&args[1])?;
    Ok((TaggedValue::Bool(f(a, b)), false))
}

// ---------------------------------------------------------------------------
// Pair / Either value-shape helpers
// ---------------------------------------------------------------------------

/// Return the type tag string for a TaggedValue (ADR-0060).
///
/// Used by `route_on_type` and the `typeof` native op. Returns one of
/// `"null"`, `"bool"`, `"number"`, `"string"`, `"array"`, `"object"`.
fn type_tag(v: &TaggedValue) -> &'static str {
    match v {
        TaggedValue::Null => "null",
        TaggedValue::Bool(_) => "bool",
        TaggedValue::Int(_) | TaggedValue::Float(_) => "number",
        TaggedValue::Str(_) => "string",
        TaggedValue::Arr(_) => "array",
        TaggedValue::Obj(_) => "object",
        TaggedValue::Pair(..) => "object",
        TaggedValue::EitherLeft(_) | TaggedValue::EitherRight(_) => "object",
        TaggedValue::Arrow { .. } => "object",
        // ADR-0083 §2.1: `Bytes` is a scalar type in `value_types.yaml`.
        TaggedValue::Bytes(_) => "bytes",
        // ADR-0083 §2.2: opaque foreign resource handle.
        TaggedValue::Resource(_) => "resource",
        // ADR-0106: nominal identity object (`sym()` / `sym(A)`).
        TaggedValue::Sym(_) => "sym",
    }
}

/// Construct a pair value `(a, b)`.
fn pair(a: TaggedValue, b: TaggedValue) -> TaggedValue {
    handl_value::pair(a, b)
}

/// Extract the first component of a pair value, or `None` if not a pair.
fn pair_fst(v: &TaggedValue) -> Option<&TaggedValue> {
    handl_value::pair_fst(v)
}

/// Extract the second component of a pair value, or `None` if not a pair.
fn pair_snd(v: &TaggedValue) -> Option<&TaggedValue> {
    handl_value::pair_snd(v)
}

/// Recursively flatten a (possibly nested) `Pair` value into a flat list
/// of individual `TaggedValue`s. A non-`Pair` value returns a single-element
/// list. This is the inverse of `***` (product) construction — used by
/// `App`/`>>>` to destructure a product argument into individual params
/// for multi-arity callees.
///
/// `Pair(a, Pair(b, c))` → `[a, b, c]`
/// `Pair(a, b)`          → `[a, b]`
/// `v` (non-pair)        → `[v]`
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

/// Extract the `impls[op]` implementation entry of a handler-like value.
///
/// A `Handler` value is a tagged object
/// `{"__tag__": "Handler", "impls": {"<op>": <Term>, ...}}`; a graded
/// incoming value may also embed an `impls` field. Returns the serialized
/// [`Term`] JSON for `op`, or `None` when the value has no `impls` object
/// or no entry for `op`.
fn handler_impl(v: &TaggedValue, op: &str) -> Option<Value> {
    v.get("impls")?.get(op).map(tagged_to_value)
}

/// Construct a `left` Either value.
fn either_left(v: TaggedValue) -> TaggedValue {
    handl_value::either_left(v)
}

/// Construct a `right` Either value.
fn either_right(v: TaggedValue) -> TaggedValue {
    handl_value::either_right(v)
}

/// Whether an Either value is a `left`.
fn is_either_left(v: &TaggedValue) -> bool {
    handl_value::is_either_left(v)
}

/// Whether an Either value is a `right`.
fn is_either_right(v: &TaggedValue) -> bool {
    handl_value::is_either_right(v)
}

/// Extract the carried value from an Either.
fn either_value(v: &TaggedValue) -> Option<&TaggedValue> {
    handl_value::either_value(v)
}

/// Build the argument list for a `loop_step` body proc application.
///
/// If the closure has multiple params and the current state is an
/// object, bind each param to the corresponding field by name
/// (destructuring). Otherwise, pass the current state as a single
/// argument.
fn build_loop_step_args(closure_val: &TaggedValue, current: &TaggedValue) -> Vec<TaggedValue> {
    if let Some(obj_state) = current.as_object() {
        if let TaggedValue::Arrow {
            params,
            array_pattern,
            ..
        } = closure_val
        {
            if *array_pattern {
                if params.len() == 1 {
                    // Single-param array-pattern arrow: pass the state
                    // directly as the sole array element.
                    return vec![TaggedValue::Arr(Rc::from([current.clone()]))];
                }
                // Multi-param array-pattern: destructure the object state
                // by param name and pack into one array.
                let arr: Vec<TaggedValue> = params
                    .iter()
                    .map(|p| {
                        obj_state
                            .get(p.as_str())
                            .cloned()
                            .unwrap_or(TaggedValue::Null)
                    })
                    .collect();
                return vec![TaggedValue::Arr(Rc::from(arr))];
            }
            if params.len() > 1 {
                // Destructure: bind each param to the
                // corresponding field by name.
                return params
                    .iter()
                    .map(|p| {
                        obj_state
                            .get(p.as_str())
                            .cloned()
                            .unwrap_or(TaggedValue::Null)
                    })
                    .collect();
            }
        }
    }
    vec![current.clone()]
}

/// Interpret the result of a `loop_step` body proc application,
/// producing the `Either` value consumed by the `Loop` combinator.
///
/// - `{ state: v }`  → `right(v)` (continue with new state)
/// - `{ out: v }`    → `left(v)` (terminate, backward compat)
/// - bare object     → `left(result)` (terminate, `return v`)
/// - non-object      → `left(result)` (terminate, `return v`)
fn interpret_loop_step_result(result: &TaggedValue) -> TaggedValue {
    if let Some(obj) = result.as_object() {
        if let Some(new_state) = obj.get("state") {
            return either_right(new_state.clone());
        } else if let Some(out) = obj.get("out") {
            return either_left(out.clone());
        }
        // Object without out/state keys — treat as exit value (return).
        return either_left(result.clone());
    }
    // Non-object (number, string, bool, null, array) — exit with the value.
    either_left(result.clone())
}

// ---------------------------------------------------------------------------
// ADR-0081 data-driven child roles
//
// Engines resolve AST child positions by **named role** from `AstShapeDef`
// (`data/ast.yaml`), never by `kind ==` shape heuristics alone. Each helper
// falls back to the legacy positional behavior only when a node's shape
// declares no roles, so the canonical language and role-declaring languages
// behave identically.
// ---------------------------------------------------------------------------

/// Resolve a child role to its positional index for `node_kind`.
///
/// `None` = the node kind has no shape or declares no such role; callers
/// keep their positional fallback.
fn role_index_of(lang: &Language, node_kind: &str, role: &str) -> Option<usize> {
    lang.child_role_index(node_kind, role).ok().flatten()
}

/// Whether `node_kind`'s shape declares the given child role.
fn role_declared(lang: &Language, node_kind: &str, role: &str) -> bool {
    role_index_of(lang, node_kind, role).is_some()
}

/// Project the child arena index at `role` from `node`'s payload children.
///
/// `None` = no such role declared or no children payload; callers fall back
/// to the positional index.
fn role_child_arena(lang: &Language, node: &AstNode, role: &str) -> Option<usize> {
    let idx = role_index_of(lang, &node.kind, role)?;
    let arena = node
        .payload
        .get("children")?
        .as_array()?
        .get(idx)?
        .as_u64()?;
    Some(arena as usize)
}

/// Whether `node` is a binding (assign-like) node: its shape declares both
/// `target` and `value` roles. Falls back to the legacy kind check for
/// role-less shapes.
fn is_assign_node(lang: &Language, node: &AstNode) -> bool {
    role_declared(lang, &node.kind, "target") && role_declared(lang, &node.kind, "value")
}

/// Resolve the bound name of a binding's `target` child: a bare `var`-like
/// node carries the name in its payload; a wrapper whose shape declares a
/// `subject` role (e.g. a type annotation — the `subject`/`annot` role
/// convention, ADR-0081) delegates to that child. This makes
/// `f: T = v`-shaped statements bind `f` without any kind check.
fn binding_target_name(
    lang: &Language,
    ast: &Ast,
    target_idx: usize,
    err_ctx: &str,
    err_node: &AstNode,
) -> Result<String, LangError> {
    let mut node = ast.at(AstRef::new(target_idx))?;
    // Unwrap `subject`-roled wrappers (annotation-like nodes).
    for _ in 0..8 {
        if let Some(name) = node.payload.as_str() {
            return Ok(name.to_string());
        }
        let subject_idx = match role_child_arena(lang, node, "subject") {
            Some(i) => i,
            None => break,
        };
        node = ast.at(AstRef::new(subject_idx))?;
    }
    node.payload.as_str().map(str::to_string).ok_or_else(|| {
        LangError::engine_node(
            format!("{err_ctx}: binding target must be an identifier"),
            err_node,
        )
    })
}

/// Whether `node` is a return node: its shape declares a `value` role but no
/// `target` role (a binding declares both). Falls back to the legacy kind
/// check for role-less shapes.
fn is_return_node(lang: &Language, node: &AstNode) -> bool {
    role_declared(lang, &node.kind, "value") && !role_declared(lang, &node.kind, "target")
}

/// Convergence comparison for `eval_rec` bindings.
///
/// Closures (`TaggedValue::Arrow`) are compared by params and body index
/// only, NOT by their captured env. In a `rec` fixpoint, each pass
/// re-evaluates a closure binding and creates a new closure that captures
/// the current scope (which now includes other rec bindings). The captured
/// env naturally differs each pass even though the closure's code is
/// identical, so a full structural `eq` would never converge. Comparing
/// only params + body lets the fixpoint converge after the env stabilizes.
/// Non-closure values use the standard structural `eq`.
fn rec_value_eq(a: &TaggedValue, b: &TaggedValue) -> bool {
    match (a, b) {
        (
            TaggedValue::Arrow {
                params: p1,
                body: b1,
                ..
            },
            TaggedValue::Arrow {
                params: p2,
                body: b2,
                ..
            },
        ) => p1.len() == p2.len() && p1.iter().zip(p2.iter()).all(|(a, b)| a == b) && b1 == b2,
        _ => handl_value::eq(a, b),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{
        AstNode, AstShapeDef, ChildRoleSpec, EvalSchemeDef, Language, Side, Term,
    };
    use handl_platform::{EvalContext, Function, Module, ModuleLookup};
    use handl_plugin_engine_exec::EvalRequest;
    use handl_plugin_module::ModuleRegistry;
    use serde_json::json;

    // --- v2 scheme constructor helpers ---

    /// `proj_child n` — the v2 equivalent of the old `Term::Child(n)`.
    fn child(n: u64) -> Term {
        Term::arr_native_lit("proj_child", vec![], Value::from(n))
    }

    /// `proj_env name` — the v2 equivalent of the old `Term::Var(name)`.
    fn var(name: &str) -> Term {
        Term::arr_native_lit("proj_env", vec![], name)
    }

    /// `const v` — the v2 equivalent of the old `Term::Lit(v)`.
    fn lit(v: impl Into<Value>) -> Term {
        Term::arr_native_lit("const", vec![], v.into())
    }

    /// `arr_native op args` — the v2 equivalent of the old
    /// `Term::Prim { op, args }`.
    fn prim(op: &str, args: Vec<Term>) -> Term {
        Term::arr_native(op, args)
    }

    /// `arr_foreign name args` — the v2 equivalent of the old
    /// `Term::Foreign { name, args }`.
    fn foreign(name: &str, args: Vec<Term>) -> Term {
        Term::arr_foreign(name, args)
    }

    /// The `Bind` lowering:
    /// `Seq(Seq(value, extend_env name), body)`.
    fn bind(name: &str, value: Term, body: Term) -> Term {
        Term::seq(
            Term::seq(value, Term::arr_native_lit("extend_env", vec![], name)),
            body,
        )
    }

    /// The `Branch` lowering:
    /// `Seq(cond, Seq(arr tag, Choice(then_, else_)))`.
    fn branch(cond: Term, then_: Term, else_: Term) -> Term {
        Term::seq(
            cond,
            Term::seq(Term::arr_native("tag", vec![]), Term::choice(then_, else_)),
        )
    }

    /// `app callee args` — the v2 equivalent of the old
    /// `Term::App { callee, args }`.
    fn app(callee: Term, args: Vec<Term>) -> Term {
        Term::app(callee, args)
    }

    /// Build a simple AST for testing.
    fn make_ast(nodes: Vec<AstNode>) -> Ast {
        let mut ast = Ast::new();
        for n in nodes {
            ast.push(n);
        }
        ast
    }

    /// A minimal node used as the "current node" for scheme-only tests
    /// that do not need child resolution. Its payload is an empty object.
    fn dummy_node() -> AstNode {
        AstNode::new("test").with_payload(json!({}))
    }

    /// Evaluate a standalone scheme with a fresh env and a dummy current
    /// node, passing `None` as incoming.
    fn eval_standalone(
        engine: &TreeWalkEngine,
        scheme: &Term,
        env: &mut ScopeStack,
    ) -> Result<Value, LangError> {
        let ast = make_ast(vec![dummy_node()]);
        let node = ast.at(AstRef::new(0)).unwrap();
        engine.eval_scheme(
            scheme,
            &ast,
            AstRef::new(0),
            &Language::new(),
            env,
            node,
            None,
        )
    }

    /// A module exposing a single `double` function for tests.
    struct DoubleModule;

    impl Module for DoubleModule {
        fn name(&self) -> &str {
            "test"
        }
        fn function(&self, name: &str) -> Option<&dyn Function> {
            if name == "double" {
                Some(&DoubleFn)
            } else {
                None
            }
        }
    }

    /// A host function that doubles its numeric argument.
    struct DoubleFn;

    impl Function for DoubleFn {
        fn call(
            &self,
            args: &[Value],
            _ctx: &mut EvalContext,
        ) -> Result<Value, handl_platform::EvalError> {
            let n = args[0].as_i64().unwrap();
            Ok(Value::from(n * 2))
        }
    }

    /// Build a module registry with the `double` function registered.
    fn double_registry() -> Rc<dyn ModuleLookup> {
        Rc::new(ModuleRegistry::new().with(DoubleModule))
    }

    // --- arr const -------------------------------------------------------

    /// A stub `ImportResolver` returning a fixed bindings object, used to
    /// test the `import` native op in isolation (Phase 9.5).
    struct StubResolver {
        bindings: Value,
    }
    impl ImportResolver for StubResolver {
        fn resolve_import(&self, _path: &str) -> Result<Value, LangError> {
            Ok(self.bindings.clone())
        }
    }

    #[test]
    fn import_op_binds_module_bindings_into_scope() {
        let resolver = Rc::new(StubResolver {
            bindings: json!({ "x": 42, "y": "hi" }),
        });
        let engine = TreeWalkEngine::new().with_import_resolver(resolver);
        let mut env = ScopeStack::new();
        // `import "m.hnl"` — the path is the evaluated arg (a const string).
        let s = prim("import", vec![lit("m.hnl")]);
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        // `import` is a statement; it produces no value.
        assert_eq!(result, Value::Null);
        // The module's bindings are bound into the current scope.
        assert_eq!(tagged_to_value(env.lookup("x").unwrap()), json!(42));
        assert_eq!(tagged_to_value(env.lookup("y").unwrap()), json!("hi"));
    }

    #[test]
    fn import_op_errors_without_resolver() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = prim("import", vec![lit("m.hnl")]);
        let err = eval_standalone(&engine, &s, &mut env).unwrap_err();
        assert!(
            format!("{}", err).contains("no import resolver"),
            "expected no-resolver error, got: {}",
            err
        );
    }

    #[test]
    fn export_op_passes_through_child_value() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        // `export 42` — pass-through of the evaluated child.
        let s = prim("export", vec![lit(json!(42))]);
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn arr_const_returns_literal() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::arr_native_lit("const", vec![], json!(42));
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn arr_const_null_when_no_literal() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::arr_native("const", vec![]);
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, Value::Null);
    }

    // --- arr proj_env ----------------------------------------------------

    #[test]
    fn arr_proj_env_bound_lookup() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        env.define("x", TaggedValue::from(json!(7)));
        let s = Term::arr_native_lit("proj_env", vec![], "x");
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(7));
    }

    #[test]
    fn arr_proj_env_unbound_returns_name() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::arr_native_lit("proj_env", vec![], "unknown");
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, Value::from("unknown"));
    }

    // --- Seq const >>> id ------------------------------------------------

    #[test]
    fn seq_const_then_id() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::seq(
            Term::arr_native_lit("const", vec![], json!(5)),
            Term::arr_native("id", vec![]),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(5));
    }

    // --- Bind shape ------------------------------------------------------

    #[test]
    fn bind_shape_extend_env_then_proj_env() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::seq(
            Term::seq(
                Term::arr_native_lit("const", vec![], json!(5)),
                Term::arr_native_lit("extend_env", vec![], "x"),
            ),
            Term::arr_native_lit("proj_env", vec![], "x"),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(5));
        // Scope must be popped after the Seq: "x" no longer visible.
        assert!(env.lookup("x").is_none());
    }

    // --- First on a pair -------------------------------------------------

    #[test]
    fn first_id_on_pair() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::seq(
            Term::arr_native_lit("const", vec![], pair(json!(3).into(), json!(7).into())),
            Term::first(Term::arr_native("id", vec![])),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(
            TaggedValue::from(result),
            pair(json!(3).into(), json!(7).into())
        );
    }

    #[test]
    fn first_const_on_pair() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::seq(
            Term::arr_native_lit("const", vec![], pair(json!(3).into(), json!(7).into())),
            Term::first(Term::arr_native_lit("const", vec![], json!(99))),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(
            TaggedValue::from(result),
            pair(json!(99).into(), json!(7).into())
        );
    }

    // --- Choice on Either ------------------------------------------------

    #[test]
    fn choice_left() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::seq(
            Term::arr_native_lit("const", vec![], either_left(json!(5).into())),
            Term::choice(
                Term::arr_native_lit("const", vec![], json!(1)),
                Term::arr_native_lit("const", vec![], json!(2)),
            ),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn choice_right() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::seq(
            Term::arr_native_lit("const", vec![], either_right(json!(5).into())),
            Term::choice(
                Term::arr_native_lit("const", vec![], json!(1)),
                Term::arr_native_lit("const", vec![], json!(2)),
            ),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(2));
    }

    // --- tag + Choice (Branch lowering) ----------------------------------

    #[test]
    fn tag_choice_true_yields_then() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::seq(
            Term::seq(
                Term::arr_native_lit("const", vec![], json!(true)),
                Term::arr_native("tag", vec![]),
            ),
            Term::choice(
                Term::arr_native_lit("const", vec![], "yes"),
                Term::arr_native_lit("const", vec![], "no"),
            ),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!("yes"));
    }

    #[test]
    fn tag_choice_false_yields_else() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::seq(
            Term::seq(
                Term::arr_native_lit("const", vec![], json!(false)),
                Term::arr_native("tag", vec![]),
            ),
            Term::choice(
                Term::arr_native_lit("const", vec![], "yes"),
                Term::arr_native_lit("const", vec![], "no"),
            ),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!("no"));
    }

    // --- Loop causal single-step ----------------------------------------

    #[test]
    fn loop_causal_single_step() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        // body: inc_fst maps pair(in, state) -> pair(in+1, state).
        // Loop seeds state=Null, feeds pair(in, Null), returns out=in+1.
        let s = Term::seq(
            Term::arr_native_lit("const", vec![], json!(10)),
            Term::loop_feedback(Term::arr_native("inc_fst", vec![])),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(11));
    }

    // --- Loop multi-step (Either-based) ---------------------------------

    #[test]
    fn loop_multi_step_either_countdown() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        // input = pair(42, 5): result=42, countdown from 5.
        // countdown_step: state>0 -> either_right(state-1) (continue);
        // state==0 -> either_left(42) (done). Runs 5 steps, returns 42.
        let s = Term::seq(
            Term::arr_native_lit("const", vec![], pair(json!(42).into(), json!(5).into())),
            Term::loop_feedback(Term::arr_native("countdown_step", vec![])),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn loop_multi_step_max_iterations_guard() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        // infinite_step always returns either_right(state) — never
        // terminates. The Loop max-iteration guard must fire.
        let s = Term::seq(
            Term::arr_native_lit("const", vec![], json!(1)),
            Term::loop_feedback(Term::arr_native("infinite_step", vec![])),
        );
        let err = eval_standalone(&engine, &s, &mut env).unwrap_err();
        assert!(
            matches!(err, LangError::Engine { ref message, .. } if message.contains("max iterations"))
        );
    }

    #[test]
    fn loop_multi_step_single_step_still_works() {
        // Backward compat: a body returning a pair (not an Either) runs
        // exactly once and returns fst. This is the original single-step
        // behavior, now the fallback when the result is not an Either.
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::seq(
            Term::arr_native_lit("const", vec![], json!(10)),
            Term::loop_feedback(Term::arr_native("inc_fst", vec![])),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(11));
    }

    // --- App with a closure value ---------------------------------------

    #[test]
    fn app_closure_applies_body() {
        let engine = TreeWalkEngine::new();
        // AST:
        // 0: var("x") payload "x"  -- body node; its scheme returns
        //    proj_env "x"
        // 1: test node (current)   -- the node whose scheme is the App
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("$payload")))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("test").with_payload(json!({})),
        ]);
        // Build a closure value pointing at body node 0, param ["x"].
        let closure = json!({
            "__arrow__": true,
            "params": ["x"],
            "body": 0,
            "env": {},
        });
        // App(callee = const <closure>, args = [const 41])
        let s = app(
            Term::arr_native_lit("const", vec![], closure),
            vec![Term::arr_native_lit("const", vec![], json!(41))],
        );
        let mut env = ScopeStack::new();
        let node = ast.at(AstRef::new(1)).unwrap();
        let result = engine
            .eval_scheme(&s, &ast, AstRef::new(1), &lang, &mut env, node, None)
            .unwrap();
        assert_eq!(result, json!(41));
    }

    // --- eval_lowered end-to-end ----------------------------------------

    #[test]
    fn eval_lowered_lit_node() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.lit", "lit", lit(json!(42))))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(42))]);
        let mut env = ScopeStack::new();
        let result = engine
            .eval_lowered(&ast, AstRef::new(0), &lang, &mut env)
            .unwrap();
        assert_eq!(result, json!(42));
    }

    // --- Branch lowering via eval_lowered --------------------------------

    #[test]
    fn lowered_branch_true_takes_then() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        // "if" node: Branch { cond: child(0), then: child(1), else:
        // child(2) } lowered to Seq(cond, Seq(tag, Choice(t, e))).
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                branch(child(0), child(1), child(2)),
            ))
            .unwrap();
        // AST: 0: lit(true), 1: lit(1), 2: lit(2), 3: if{children:[0,1,2]}
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let mut env = ScopeStack::new();
        let result = engine
            .eval_lowered(&ast, AstRef::new(3), &lang, &mut env)
            .unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn lowered_branch_false_takes_else() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                branch(child(0), child(1), child(2)),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(false)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let mut env = ScopeStack::new();
        let result = engine
            .eval_lowered(&ast, AstRef::new(3), &lang, &mut env)
            .unwrap();
        assert_eq!(result, json!(2));
    }

    // --- Arithmetic / comparison parity ---------------------------------

    #[test]
    fn native_add() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::arr_native(
            "add",
            vec![
                Term::arr_native_lit("const", vec![], json!(2)),
                Term::arr_native_lit("const", vec![], json!(3)),
            ],
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn native_eq() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::arr_native(
            "eq",
            vec![
                Term::arr_native_lit("const", vec![], json!(2)),
                Term::arr_native_lit("const", vec![], json!(2)),
            ],
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(true));
    }

    // --- Foreign dispatch ------------------------------------------------

    #[test]
    fn foreign_arr_dispatches_to_module() {
        let engine = TreeWalkEngine::new().with_modules(double_registry());
        let mut env = ScopeStack::new();
        let s = Term::arr_foreign(
            "double",
            vec![Term::arr_native_lit("const", vec![], json!(21))],
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn foreign_arr_no_modules_errors() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::arr_foreign(
            "double",
            vec![Term::arr_native_lit("const", vec![], json!(21))],
        );
        let err = eval_standalone(&engine, &s, &mut env).unwrap_err();
        assert!(
            matches!(err, LangError::Engine { ref message, .. } if message.contains("unregistered foreign"))
        );
    }

    // --- proj_child via eval_lowered -------------------------------------

    #[test]
    fn proj_child_evaluates_child_node() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        // "binary.add": add(child(0), child(1)).
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                prim("add", vec![child(0), child(1)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let mut env = ScopeStack::new();
        let result = engine
            .eval_lowered(&ast, AstRef::new(2), &lang, &mut env)
            .unwrap();
        assert_eq!(result, json!(5));
    }

    // --- Unknown op ------------------------------------------------------

    #[test]
    fn unknown_native_op_errors() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Term::arr_native("bogus", vec![]);
        let err = eval_standalone(&engine, &s, &mut env).unwrap_err();
        assert!(
            matches!(err, LangError::Engine { ref message, .. } if message.contains("unknown v2 native op"))
        );
    }

    // --- Globals injection ----------------------------------------------

    #[test]
    fn with_global_visible_in_env() {
        let engine = TreeWalkEngine::new().with_global("self", json!("my-lang"));
        let mut env = ScopeStack::new();
        // Mirror what `EvalEngine::eval` does: place globals in the
        // `globals` field (not the root scope) so they are visible via
        // `lookup()` but excluded from `capture()`.
        env.set_globals(Rc::new(
            engine
                .globals
                .iter()
                .map(|(k, v)| (Rc::from(k.as_str()), TaggedValue::from(v.clone())))
                .collect(),
        ));
        let s = Term::arr_native_lit("proj_env", vec![], "self");
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!("my-lang"));
    }

    // --- Engine-level tests (via EvalEngine::eval) ----------------------

    #[test]
    fn lit_returns_value() {
        let engine = TreeWalkEngine::new();
        let lang = Language::new();
        // "lit" has no eval scheme → returns payload.
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(42))]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn var_resolves_from_scope() {
        // Create a "var" node with scheme proj_env "$payload", payload =
        // "x". But we need "x" to be in scope. We use a Bind to put it
        // there.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                bind("$payload", child(0), child(1)),
            ))
            .unwrap();
        // Ast:
        // 0: lit(42)
        // 1: var("x")
        // 2: let {name: "x", children: [0, 1]}
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(42)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn var_unknown_variable_errors() {
        // Unbound variables return their name as a string value (so
        // identifiers like `eval` can flow into host-function dispatch
        // via `App`).
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("unknown")))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("var")]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, Value::from("unknown"));
    }

    #[test]
    fn child_recurses_into_child_node() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                prim("add", vec![child(0), child(1)]),
            ))
            .unwrap();
        // Ast:
        // 0: lit(2), 1: lit(3), 2: binary.add {children: [0, 1]}
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn bind_extends_scope() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                bind("$payload", child(0), child(1)),
            ))
            .unwrap();
        // let x = 10 in x → 10
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(10)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(10));
    }

    #[test]
    fn bind_scope_does_not_leak() {
        // After the let, "x" should not be visible.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                bind("$payload", child(0), child(1)),
            ))
            .unwrap();
        // let x = 10 in 20 → 20 (body is lit(20), not var(x))
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(10)),
            AstNode::new("lit").with_payload(json!(20)),
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(20));
    }

    #[test]
    fn branch_true_takes_then() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                branch(child(0), child(1), child(2)),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn branch_false_takes_else() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                branch(child(0), child(1), child(2)),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(false)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(2));
    }

    #[test]
    fn branch_falsy_values() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                branch(child(0), child(1), child(2)),
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
    fn prim_add() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                prim("add", vec![child(0), child(1)]),
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
    fn prim_sub() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.sub",
                "binary.sub",
                prim("sub", vec![child(0), child(1)]),
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
    fn prim_mul() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.mul",
                "binary.mul",
                prim("mul", vec![child(0), child(1)]),
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
    fn prim_div() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.div",
                "binary.div",
                prim("div", vec![child(0), child(1)]),
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
    fn prim_div_by_zero_errors() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.div",
                "binary.div",
                prim("div", vec![child(0), child(1)]),
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
    fn prim_eq() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.eq",
                "binary.eq",
                prim("eq", vec![child(0), child(1)]),
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
    fn prim_lt_gt() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.lt",
                "binary.lt",
                prim("lt", vec![child(0), child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.gt",
                "binary.gt",
                prim("gt", vec![child(0), child(1)]),
            ))
            .unwrap();
        // 3 < 5 → true
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("binary.lt").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(true));

        // 3 > 5 → false
        let ast2 = make_ast(vec![
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("binary.gt").with_payload(json!({"children": [0, 1]})),
        ]);
        let req2 = EvalRequest::new(ast2);
        assert_eq!(engine.eval(&lang, &req2).unwrap(), json!(false));
    }

    #[test]
    fn prim_not_and_or() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.not",
                "unary.not",
                prim("not", vec![child(0)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.and",
                "binary.and",
                prim("and", vec![child(0), child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.or",
                "binary.or",
                prim("or", vec![child(0), child(1)]),
            ))
            .unwrap();

        // not true → false
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("unary.not").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(false));

        // true and false → false
        let ast2 = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("lit").with_payload(json!(false)),
            AstNode::new("binary.and").with_payload(json!({"children": [0, 1]})),
        ]);
        let req2 = EvalRequest::new(ast2);
        assert_eq!(engine.eval(&lang, &req2).unwrap(), json!(false));

        // true or false → true
        let ast3 = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("lit").with_payload(json!(false)),
            AstNode::new("binary.or").with_payload(json!({"children": [0, 1]})),
        ]);
        let req3 = EvalRequest::new(ast3);
        assert_eq!(engine.eval(&lang, &req3).unwrap(), json!(true));
    }

    #[test]
    fn prim_unknown_errors() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.weird",
                "weird",
                prim("unknown_op", vec![]),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("weird")]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unknown v2 native op"));
    }

    #[test]
    fn foreign_dispatches_to_registered_host() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.dbl",
                "dbl",
                foreign("double", vec![child(0)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(21)),
            AstNode::new("dbl").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);

        let engine = engine.with_modules(double_registry());
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn foreign_unregistered_errors() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.dbl",
                "dbl",
                foreign("double", vec![child(0)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(21)),
            AstNode::new("dbl").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);

        // No host functions registered.
        let err = engine.eval(&lang, &req).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unregistered foreign"));
    }

    #[test]
    fn foreign_with_no_host_registry_errors() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.dbl",
                "dbl",
                foreign("double", vec![child(0)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(21)),
            AstNode::new("dbl").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn app_calls_closure() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        // "function" node: creates a closure via arr("closure").
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                prim("closure", vec![]),
            ))
            .unwrap();
        // "var" node: looks up a variable.
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("$payload")))
            .unwrap();
        // "binary.add": adds two children.
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                prim("add", vec![child(0), child(1)]),
            ))
            .unwrap();
        // "call": applies callee to args.
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                app(child(0), vec![child(1)]),
            ))
            .unwrap();

        // Ast for: let f = fn(x) { x + 1 } in f(5)
        // 0: var("x") — used in the body
        // 1: lit(1) — the +1
        // 2: binary.add {children: [0, 1]} — body: x + 1
        // 3: function {params: ["x"], body: 2} — the closure
        // 4: lit(5) — the argument
        // 5: call {children: [3, 4]} — f(5)
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
            AstNode::new("function").with_payload(json!({"params": ["x"], "body": 2})),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("call").with_payload(json!({"children": [3, 4]})),
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(6));
    }

    #[test]
    fn app_non_closure_errors() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.call", "call", app(child(0), vec![])))
            .unwrap();
        // Callee is a lit (not a closure).
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(42)),
            AstNode::new("call").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn empty_ast_returns_null() {
        let engine = TreeWalkEngine::new();
        let lang = Language::new();
        let req = EvalRequest::new(Ast::new());
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn no_scheme_returns_payload() {
        let engine = TreeWalkEngine::new();
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("custom").with_payload(json!("hello"))]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn nested_let_and_add() {
        // let x = 3 in let y = 4 in x + y
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                bind("$payload", child(0), child(1)),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                prim("add", vec![child(0), child(1)]),
            ))
            .unwrap();
        // Ast:
        // 0: lit(3)        — x's value
        // 1: lit(4)        — y's value
        // 2: var("x")      — x reference in x+y
        // 3: var("y")      — y reference in x+y
        // 4: binary.add {children: [2, 3]}  — x + y
        // 5: let {name: "y", children: [1, 4]}  — let y = 4 in (x+y)
        // 6: let {name: "x", children: [0, 5]}  — let x = 3 in (let y = 4 in (x+y))
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
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(7));
    }

    #[test]
    fn trait_dispatch_via_dyn_reference() {
        let engine: Box<dyn EvalEngine> = Box::new(TreeWalkEngine::new());
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(99))]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(99));
    }

    #[test]
    fn default_constructor() {
        let engine = TreeWalkEngine::default();
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(0))]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(0));
    }

    #[test]
    fn child_missing_children_array_errors() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                prim("add", vec![child(0), child(1)]),
            ))
            .unwrap();
        // Node has no "children" in payload.
        let ast = make_ast(vec![AstNode::new("binary.add").with_payload(json!({}))]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn child_index_out_of_range_errors() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                prim("add", vec![child(0), child(1)]),
            ))
            .unwrap();
        // Only one child but scheme expects two.
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("binary.add").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn prim_add_floats() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                prim("add", vec![child(0), child(1)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1.5)),
            AstNode::new("lit").with_payload(json!(2.5)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(4.0));
    }

    #[test]
    fn closure_captures_env() {
        // let x = 10 in let f = fn(y) { x + y } in f(5)
        // The closure should capture x=10 from the enclosing scope.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                bind("$payload", child(0), child(1)),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                prim("add", vec![child(0), child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                prim("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                app(child(0), vec![child(1)]),
            ))
            .unwrap();

        // Ast:
        // 0: lit(10) — x's value
        // 1: var("x") — x reference in body
        // 2: var("y") — y reference in body
        // 3: binary.add {children: [1, 2]} — body: x + y
        // 4: function {params: ["y"], body: 3} — the closure
        // 5: lit(5) — the argument
        // 6: call {children: [4, 5]} — f(5)
        // 7: let {name: "f", children: [4, 6]} — let f = fn(y) in f(5)
        // 8: let {name: "x", children: [0, 7]} — let x = 10 in ...
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
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(15));
    }

    #[test]
    fn app_wrong_arity_errors() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                prim("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                app(child(0), vec![child(1), child(2)]),
            ))
            .unwrap();
        // function expects 1 param, call passes 2 args. Curried
        // application binds `x = 1` and applies the body's result to the
        // surplus arg — the body is a `lit` (not an arrow), so the
        // application still fails with "callee is not an arrow-value".
        let ast = make_ast(vec![
            AstNode::new("function").with_payload(json!({"params": ["x"], "body": 1})),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("call").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    // --- Coverage: edge cases for error paths and type conversions ---

    #[test]
    fn lit_scheme_returns_value() {
        // Direct const evaluation via a node whose eval scheme is const.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.const", "const", lit(json!(99))))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("const")]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(99));
    }

    #[test]
    fn var_payload_not_string_errors() {
        // proj_env "$payload" but payload is not a string.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("$payload")))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("var").with_payload(json!(42))]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn bind_payload_missing_name_errors() {
        // Bind with name="$payload" but payload has no "name" field.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                bind("$payload", child(0), child(1)),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            // Missing "name" in payload.
            AstNode::new("let").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn foreign_host_present_but_fn_not_found_errors() {
        // Host is registered but doesn't have the requested function.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.dbl",
                "dbl",
                foreign("triple", vec![child(0)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(21)),
            AstNode::new("dbl").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);

        let engine = engine.with_modules(double_registry());
        let err = engine.eval(&lang, &req).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unregistered foreign"));
    }

    #[test]
    fn app_callee_is_object_but_not_closure_errors() {
        // Callee is an object without __arrow__ marker.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.call", "call", app(child(0), vec![])))
            .unwrap();
        // Callee is an object {"foo": 1} — not a closure.
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!({"foo": 1})),
            AstNode::new("call").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn closure_prim_missing_params_errors() {
        // "closure" op but payload has no "params" field.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                prim("closure", vec![]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("function").with_payload(json!({"body": 0})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn closure_prim_missing_body_errors() {
        // "closure" op but payload has no "body" field.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                prim("closure", vec![]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("function").with_payload(json!({"params": ["x"]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn prim_sub_floats() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.sub",
                "binary.sub",
                prim("sub", vec![child(0), child(1)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(5.5)),
            AstNode::new("lit").with_payload(json!(1.5)),
            AstNode::new("binary.sub").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(4.0));
    }

    #[test]
    fn prim_mul_floats() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.mul",
                "binary.mul",
                prim("mul", vec![child(0), child(1)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(2.5)),
            AstNode::new("lit").with_payload(json!(3.0)),
            AstNode::new("binary.mul").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(7.5));
    }

    #[test]
    fn prim_div_floats() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.div",
                "binary.div",
                prim("div", vec![child(0), child(1)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(7.5)),
            AstNode::new("lit").with_payload(json!(2.5)),
            AstNode::new("binary.div").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(3.0));
    }

    #[test]
    fn prim_div_int_by_zero_errors() {
        // Int division by zero is caught by the early check.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.div",
                "binary.div",
                prim("div", vec![child(0), child(1)]),
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
    fn num_div_int_by_zero_returns_float_inf() {
        // Directly test Num::Div with int zero to cover that path.
        let result = Num::Int(1) / Num::Int(0);
        assert!(result.to_f64().is_infinite());
    }

    #[test]
    fn num_partial_eq() {
        // Cover PartialEq for Num (used by op "eq").
        assert_eq!(Num::Int(5), Num::Int(5));
        assert_ne!(Num::Int(5), Num::Int(6));
        assert_eq!(Num::Float(5.0), Num::Int(5));
        assert_ne!(Num::Float(5.0), Num::Int(6));
    }

    #[test]
    fn bind_with_literal_name() {
        // Bind with a literal name (not "$payload") — covers the else
        // branch of the bind_name extraction.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("x")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                bind("x", child(0), child(1)),
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
    fn prim_add_non_number_errors() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                prim("add", vec![child(0), child(1)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!("not a number")),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn is_truthy_object_and_array() {
        // Objects and arrays are truthy.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                branch(child(0), child(1), child(2)),
            ))
            .unwrap();
        // Object is truthy → takes then branch.
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!({"a": 1})),
            AstNode::new("lit").with_payload(json!("then")),
            AstNode::new("lit").with_payload(json!("else")),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!("then"));

        // Array is truthy → takes then branch.
        let ast2 = make_ast(vec![
            AstNode::new("lit").with_payload(json!([1, 2])),
            AstNode::new("lit").with_payload(json!("then")),
            AstNode::new("lit").with_payload(json!("else")),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req2 = EvalRequest::new(ast2);
        assert_eq!(engine.eval(&lang, &req2).unwrap(), json!("then"));
    }

    #[test]
    fn is_truthy_nonzero_number() {
        // Non-zero number is truthy.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                branch(child(0), child(1), child(2)),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(42)),
            AstNode::new("lit").with_payload(json!("then")),
            AstNode::new("lit").with_payload(json!("else")),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!("then"));
    }

    #[test]
    fn closure_param_not_string_errors() {
        // Closure has a non-string param. With TaggedValue, non-string
        // params are silently filtered out by the From<Value> conversion,
        // so the closure ends up with fewer params than expected. This
        // causes an arity mismatch error when called with the wrong number
        // of args.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                prim("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                app(child(0), vec![child(1)]),
            ))
            .unwrap();
        // params contains a non-string (number) → filtered out → 0 params.
        // Called with 1 arg → arity mismatch error.
        let ast = make_ast(vec![
            AstNode::new("function").with_payload(json!({"params": [123], "body": 0})),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("call").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn closure_missing_params_array_errors() {
        // With TaggedValue, a malformed __arrow__ object (params not an
        // array) is converted gracefully: params defaults to empty, body
        // defaults to 0. The closure is callable with 0 args. This test
        // verifies the graceful conversion produces a callable closure.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.call", "call", app(child(0), vec![])))
            .unwrap();
        // Callee is a "closure" with params as a string (not array).
        // TaggedValue::from defaults params to [] and body to 0.
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!({
                "__arrow__": true,
                "params": "not_an_array",
                "body": 0
            })),
            AstNode::new("call").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        // The closure has 0 params and is called with 0 args → succeeds.
        // It evaluates body at index 0 (the lit node itself), which returns
        // its payload as a value.
        let result = engine.eval(&lang, &req).unwrap();
        assert!(result.is_object());
    }

    #[test]
    fn closure_missing_body_errors() {
        // With TaggedValue, a malformed __arrow__ object (missing body)
        // is converted gracefully: body defaults to 0. This test verifies
        // the graceful conversion produces a callable closure.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.call", "call", app(child(0), vec![])))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!({
                "__arrow__": true,
                "params": []
            })),
            AstNode::new("call").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        // The closure has 0 params, 0 args, body=0 → succeeds.
        let result = engine.eval(&lang, &req).unwrap();
        assert!(result.is_object());
    }

    #[test]
    fn app_callee_not_object_errors() {
        // Callee is a number, not an object → as_object() returns None.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.call", "call", app(child(0), vec![])))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(42)),
            AstNode::new("call").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    // -----------------------------------------------------------------
    // Pratt-parsed literal tests (children-based payloads)
    // -----------------------------------------------------------------

    /// Build a minimal language for Pratt-parsed literal eval tests:
    /// `value` (lit), `var`, `binary.add`, `array`, `object`, `entry`,
    /// `function` (closure op), `call` (App).
    fn literal_eval_lang() -> Language {
        let mut lang = Language::new();
        // `value` nodes have no scheme — the eval fallback returns
        // `node.payload.clone()`, which is the literal value.
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                prim("add", vec![child(0), child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.array",
                "array",
                prim("array", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.object",
                "object",
                prim("object", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.function",
                "function",
                prim("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                app(child(0), vec![child(1)]),
            ))
            .unwrap();
        // `paren` evaluates to its single child (proj_child 0).
        lang.eval
            .merge(EvalSchemeDef::new("e.paren", "paren", child(0)))
            .unwrap();
        // AST shape roles for collection-like nodes (ADR-0081).
        lang.ast
            .merge(
                AstShapeDef::new("ast.paren", "paren").with_children(vec![ChildRoleSpec {
                    role: "body".into(),
                    variadic: true,
                }]),
            )
            .unwrap();
        lang.ast
            .merge(
                AstShapeDef::new("ast.array", "array").with_children(vec![ChildRoleSpec {
                    role: "elements".into(),
                    variadic: true,
                }]),
            )
            .unwrap();
        lang
    }

    #[test]
    fn pratt_array_eval() {
        // [1, 2, 3] → array{children:[0,1,2]}, each child is a value node.
        let engine = TreeWalkEngine::new();
        let lang = literal_eval_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(1)),
            AstNode::new("value").with_payload(json!(2)),
            AstNode::new("value").with_payload(json!(3)),
            AstNode::new("array").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!([1, 2, 3]));
    }

    #[test]
    fn pratt_empty_array_eval() {
        let engine = TreeWalkEngine::new();
        let lang = literal_eval_lang();
        let ast = make_ast(vec![
            AstNode::new("array").with_payload(json!({"children": []})),
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!([]));
    }

    #[test]
    fn pratt_object_eval() {
        // {a: 1, b: 2} → object{children:[entry{[var(a),value(1)]}, entry{[var(b),value(2)]}]}
        let engine = TreeWalkEngine::new();
        let lang = literal_eval_lang();
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("a")), // 0
            AstNode::new("value").with_payload(json!(1)), // 1
            AstNode::new("entry").with_payload(json!({"children": [0, 1]})), // 2
            AstNode::new("var").with_payload(json!("b")), // 3
            AstNode::new("value").with_payload(json!(2)), // 4
            AstNode::new("entry").with_payload(json!({"children": [3, 4]})), // 5
            AstNode::new("object").with_payload(json!({"children": [2, 5]})), // 6
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!({"a": 1, "b": 2}));
    }

    #[test]
    fn pratt_object_computed_bracket_key_eval() {
        // { [k]: 2, other: 5 } where k = "mykey"
        // After ADR-0058 desugar, `[k]` (an `array` key) is rewritten to
        // `paren { k }` — the engine evaluates the `paren` (which evaluates
        // its inner child) and uses the result as the computed key. This
        // test builds the post-desugar AST directly (the desugar pass
        // itself is tested in handl-runtime::desugar_pass).
        // 0: var("k") (computed key expr)
        // 1: paren{children:[0]} (post-desugar [k] → (k))
        // 2: value(2)
        // 3: entry{children:[1, 2]} ((k): 2)
        // 4: var("other") (literal key name)
        // 5: value(5)
        // 6: entry{children:[4, 5]} (other: 5)
        // 7: object{children:[3, 6]}
        // 8: var("k") (arrow param)
        // 9: object ref (7) — body
        // 10: function{children:[8, 7]}
        // 11: value("mykey")
        // 12: call{children:[10, 11]}
        let engine = TreeWalkEngine::new();
        let lang = literal_eval_lang();
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("k")), // 0
            AstNode::new("paren").with_payload(json!({"children": [0]})), // 1
            AstNode::new("value").with_payload(json!(2)), // 2
            AstNode::new("entry").with_payload(json!({"children": [1, 2]})), // 3
            AstNode::new("var").with_payload(json!("other")), // 4
            AstNode::new("value").with_payload(json!(5)), // 5
            AstNode::new("entry").with_payload(json!({"children": [4, 5]})), // 6
            AstNode::new("object").with_payload(json!({"children": [3, 6]})), // 7
            AstNode::new("var").with_payload(json!("k")), // 8
            AstNode::new("function").with_payload(json!({"children": [8, 7]})), // 9
            AstNode::new("value").with_payload(json!("mykey")), // 10
            AstNode::new("call").with_payload(json!({"children": [9, 10]})), // 11
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(
            engine.eval(&lang, &req).unwrap(),
            json!({"mykey": 2, "other": 5})
        );
    }

    #[test]
    fn pratt_function_paren_params_eval() {
        // (x) -> x + 1, called with 5 → 6
        // 0: var("x"), 1: value(1), 2: binary.add{children:[0,1]}
        // 3: function{children:[0, 2]}  (bare param: children[0]=var, children[1]=body)
        // 4: value(5)
        // 5: call{children:[3, 4]}
        let engine = TreeWalkEngine::new();
        let lang = literal_eval_lang();
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("x")), // 0
            AstNode::new("value").with_payload(json!(1)), // 1
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})), // 2
            AstNode::new("function").with_payload(json!({"children": [0, 2]})), // 3
            AstNode::new("value").with_payload(json!(5)), // 4
            AstNode::new("call").with_payload(json!({"children": [3, 4]})), // 5
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(6));
    }

    #[test]
    fn pratt_function_paren_multi_param_eval() {
        // (x, y) -> x + y, called with (10, 20) → 30
        // The closure op unwraps the paren node to get [x, y] params.
        // But App only passes 1 arg. So we test the closure creation
        // itself (not the call) — verify the closure has 2 params.
        // 0: var("x"), 1: var("y"), 2: paren{children:[0,1]}
        // 3: var("x"), 4: var("y"), 5: binary.add{children:[3,4]}
        // 6: function{children:[2, 5]}
        let engine = TreeWalkEngine::new();
        let lang = literal_eval_lang();
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("x")), // 0
            AstNode::new("var").with_payload(json!("y")), // 1
            AstNode::new("paren").with_payload(json!({"children": [0, 1]})), // 2
            AstNode::new("var").with_payload(json!("x")), // 3
            AstNode::new("var").with_payload(json!("y")), // 4
            AstNode::new("binary.add").with_payload(json!({"children": [3, 4]})), // 5
            AstNode::new("function").with_payload(json!({"children": [2, 5]})), // 6
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        // Result is a closure value with 2 params.
        assert_eq!(result["__arrow__"], json!(true));
        assert_eq!(result["params"], json!(["x", "y"]));
    }

    // -----------------------------------------------------------------
    // match / handle / destructuring conformance tests
    // -----------------------------------------------------------------

    /// Build a minimal language for match/handle eval tests:
    /// `value` (no scheme → payload fallback), `var`, `binary.add`,
    /// `match`, `handle`.
    fn match_lang() -> Language {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                prim("add", vec![child(0), child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.match",
                "match",
                prim("match", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.handle",
                "handle",
                prim("handle", vec![]),
            ))
            .unwrap();
        lang
    }

    #[test]
    fn match_wildcard_pattern() {
        // match 5 { _: 1 } → 1
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(5)), // 0: scrutinee
            AstNode::new("var").with_payload(json!("_")), // 1: wildcard pattern
            AstNode::new("value").with_payload(json!(1)), // 2: body
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})), // 3
            AstNode::new("block").with_payload(json!({"children": [3]})), // 4
            AstNode::new("match").with_payload(json!({"children": [0, 4]})), // 5: root
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(1));
    }

    #[test]
    fn match_binder_pattern() {
        // match 5 { x: x } → 5 (binder binds x to scrutinee)
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(5)), // 0: scrutinee
            AstNode::new("var").with_payload(json!("x")), // 1: binder pattern
            AstNode::new("var").with_payload(json!("x")), // 2: body (returns x)
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})), // 3
            AstNode::new("block").with_payload(json!({"children": [3]})), // 4
            AstNode::new("match").with_payload(json!({"children": [0, 4]})), // 5: root
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(5));
    }

    #[test]
    fn match_literal_pattern() {
        // match 3 { 1: "a", 3: "b" } → "b" (second arm matches by equality)
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(3)), // 0: scrutinee
            AstNode::new("value").with_payload(json!(1)), // 1: pattern (literal 1)
            AstNode::new("value").with_payload(json!("a")), // 2: body
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})), // 3
            AstNode::new("value").with_payload(json!(3)), // 4: pattern (literal 3)
            AstNode::new("value").with_payload(json!("b")), // 5: body
            AstNode::new("arm").with_payload(json!({"children": [4, 5]})), // 6
            AstNode::new("block").with_payload(json!({"children": [3, 6]})), // 7
            AstNode::new("match").with_payload(json!({"children": [0, 7]})), // 8: root
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!("b"));
    }

    #[test]
    fn match_no_arm_errors() {
        // match 9 { 1: "a" } → error (no arm matches)
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(9)), // 0: scrutinee
            AstNode::new("value").with_payload(json!(1)), // 1: pattern (literal 1)
            AstNode::new("value").with_payload(json!("a")), // 2: body
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})), // 3
            AstNode::new("block").with_payload(json!({"children": [3]})), // 4
            AstNode::new("match").with_payload(json!({"children": [0, 4]})), // 5: root
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    #[test]
    fn match_array_destructuring() {
        // match [1, 2] { [a, b]: a + b } → 3
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!([1, 2])), // 0: scrutinee
            AstNode::new("var").with_payload(json!("a")),      // 1: pattern elem a
            AstNode::new("var").with_payload(json!("b")),      // 2: pattern elem b
            AstNode::new("array").with_payload(json!({"children": [1, 2]})), // 3: pattern
            AstNode::new("var").with_payload(json!("a")),      // 4: body a
            AstNode::new("var").with_payload(json!("b")),      // 5: body b
            AstNode::new("binary.add").with_payload(json!({"children": [4, 5]})), // 6: a+b
            AstNode::new("arm").with_payload(json!({"children": [3, 6]})), // 7
            AstNode::new("block").with_payload(json!({"children": [7]})), // 8
            AstNode::new("match").with_payload(json!({"children": [0, 8]})), // 9: root
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(3));
    }

    #[test]
    fn match_array_rest() {
        // match [1, 2, 3, 4] { [a, ...rest]: rest } → [2, 3, 4]
        // (one positional pattern `a` binds 1; rest captures [2,3,4])
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!([1, 2, 3, 4])), // 0: scrutinee
            AstNode::new("var").with_payload(json!("a")),            // 1: pattern elem a
            AstNode::new("var").with_payload(json!("rest")),         // 2: rest binder
            AstNode::new("rest").with_payload(json!({"children": [2]})), // 3: ...rest
            AstNode::new("array").with_payload(json!({"children": [1, 3]})), // 4: pattern [a, ...rest]
            AstNode::new("var").with_payload(json!("rest")), // 5: body (returns rest)
            AstNode::new("arm").with_payload(json!({"children": [4, 5]})), // 6
            AstNode::new("block").with_payload(json!({"children": [6]})), // 7
            AstNode::new("match").with_payload(json!({"children": [0, 7]})), // 8: root
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!([2, 3, 4]));
    }

    #[test]
    fn match_object_destructuring() {
        // match {x: 1, y: 2} { {x: a, y: b}: a + b } → 3
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!({"x": 1, "y": 2})), // 0: scrutinee
            AstNode::new("var").with_payload(json!("x")),                // 1: key x
            AstNode::new("var").with_payload(json!("a")),                // 2: val pattern a
            AstNode::new("entry").with_payload(json!({"children": [1, 2]})), // 3: x: a
            AstNode::new("var").with_payload(json!("y")),                // 4: key y
            AstNode::new("var").with_payload(json!("b")),                // 5: val pattern b
            AstNode::new("entry").with_payload(json!({"children": [4, 5]})), // 6: y: b
            AstNode::new("object").with_payload(json!({"children": [3, 6]})), // 7: pattern
            AstNode::new("var").with_payload(json!("a")),                // 8: body a
            AstNode::new("var").with_payload(json!("b")),                // 9: body b
            AstNode::new("binary.add").with_payload(json!({"children": [8, 9]})), // 10: a+b
            AstNode::new("arm").with_payload(json!({"children": [7, 10]})), // 11
            AstNode::new("block").with_payload(json!({"children": [11]})), // 12
            AstNode::new("match").with_payload(json!({"children": [0, 12]})), // 13: root
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(3));
    }

    #[test]
    fn match_object_rest() {
        // match {x: 1, y: 2, z: 3} { {x: a, ...rest}: rest } → {y: 2, z: 3}
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!({"x": 1, "y": 2, "z": 3})), // 0
            AstNode::new("var").with_payload(json!("x")),                        // 1: key x
            AstNode::new("var").with_payload(json!("a")),                        // 2: val pattern a
            AstNode::new("entry").with_payload(json!({"children": [1, 2]})),     // 3: x: a
            AstNode::new("var").with_payload(json!("rest")),                     // 4: rest binder
            AstNode::new("rest").with_payload(json!({"children": [4]})),         // 5: ...rest
            AstNode::new("object").with_payload(json!({"children": [3, 5]})),    // 6: pattern
            AstNode::new("var").with_payload(json!("rest")), // 7: body (returns rest)
            AstNode::new("arm").with_payload(json!({"children": [6, 7]})), // 8
            AstNode::new("block").with_payload(json!({"children": [8]})), // 9
            AstNode::new("match").with_payload(json!({"children": [0, 9]})), // 10: root
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!({"y": 2, "z": 3}));
    }

    #[test]
    fn handle_dispatches_to_matching_clause() {
        // handle 5 { x: x + 1 } → 6
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(5)), // 0: body
            AstNode::new("var").with_payload(json!("x")), // 1: pattern (binder x)
            AstNode::new("var").with_payload(json!("x")), // 2: handler x
            AstNode::new("value").with_payload(json!(1)), // 3: handler 1
            AstNode::new("binary.add").with_payload(json!({"children": [2, 3]})), // 4: x+1
            AstNode::new("arm").with_payload(json!({"children": [1, 4]})), // 5: clause
            AstNode::new("block").with_payload(json!({"children": [5]})), // 6
            AstNode::new("handle").with_payload(json!({"children": [0, 6]})), // 7: root
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(6));
    }

    #[test]
    fn handle_no_clause_returns_body() {
        // handle 5 { 1: "a" } → 5 (no clause matches, body returned as-is)
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(5)), // 0: body
            AstNode::new("value").with_payload(json!(1)), // 1: pattern (literal 1)
            AstNode::new("value").with_payload(json!("a")), // 2: handler
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})), // 3: clause
            AstNode::new("block").with_payload(json!({"children": [3]})), // 4
            AstNode::new("handle").with_payload(json!({"children": [0, 4]})), // 5: root
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(5));
    }

    // -----------------------------------------------------------------
    // tag pattern matching (ADR-0077)
    // -----------------------------------------------------------------

    #[test]
    fn match_tag_pattern_binds_value() {
        // match {"__tag__": "ok", "value": 42} { ok(v): v, err(e): 0 } → 42
        // The pattern `ok(v)` is a `call` node with callee `var("ok")`
        // and arg `var("v")`.
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!({"__tag__": "ok", "value": 42})), // 0: scrutinee
            AstNode::new("var").with_payload(json!("ok")), // 1: callee (tag name)
            AstNode::new("var").with_payload(json!("v")),  // 2: binder
            AstNode::new("call").with_payload(json!({"children": [1, 2]})), // 3: pattern ok(v)
            AstNode::new("var").with_payload(json!("v")),  // 4: body (returns v)
            AstNode::new("arm").with_payload(json!({"children": [3, 4]})), // 5
            AstNode::new("var").with_payload(json!("err")), // 6: callee (tag name)
            AstNode::new("var").with_payload(json!("e")),  // 7: binder
            AstNode::new("call").with_payload(json!({"children": [6, 7]})), // 8: pattern err(e)
            AstNode::new("value").with_payload(json!(0)),  // 9: body (returns 0)
            AstNode::new("arm").with_payload(json!({"children": [8, 9]})), // 10
            AstNode::new("block").with_payload(json!({"children": [5, 10]})), // 11
            AstNode::new("match").with_payload(json!({"children": [0, 11]})), // 12: root
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(42));
    }

    #[test]
    fn match_tag_pattern_wrong_tag_falls_through() {
        // match {"__tag__": "err", "value": 99} { ok(v): v, err(e): e } → 99
        // The first arm (ok(v)) does not match; the second (err(e)) matches.
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!({"__tag__": "err", "value": 99})), // 0: scrutinee
            AstNode::new("var").with_payload(json!("ok")),                              // 1: callee
            AstNode::new("var").with_payload(json!("v")),                               // 2: binder
            AstNode::new("call").with_payload(json!({"children": [1, 2]})), // 3: pattern ok(v)
            AstNode::new("var").with_payload(json!("v")),                   // 4: body
            AstNode::new("arm").with_payload(json!({"children": [3, 4]})),  // 5
            AstNode::new("var").with_payload(json!("err")),                 // 6: callee
            AstNode::new("var").with_payload(json!("e")),                   // 7: binder
            AstNode::new("call").with_payload(json!({"children": [6, 7]})), // 8: pattern err(e)
            AstNode::new("var").with_payload(json!("e")),                   // 9: body (returns e)
            AstNode::new("arm").with_payload(json!({"children": [8, 9]})),  // 10
            AstNode::new("block").with_payload(json!({"children": [5, 10]})), // 11
            AstNode::new("match").with_payload(json!({"children": [0, 11]})), // 12: root
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(99));
    }

    #[test]
    fn match_tag_pattern_wildcard_binder() {
        // match {"__tag__": "ok", "value": 42} { ok(_): 1 } → 1
        // The wildcard binder `_` matches without binding.
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!({"__tag__": "ok", "value": 42})), // 0: scrutinee
            AstNode::new("var").with_payload(json!("ok")),                             // 1: callee
            AstNode::new("var").with_payload(json!("_")), // 2: wildcard binder
            AstNode::new("call").with_payload(json!({"children": [1, 2]})), // 3: pattern ok(_)
            AstNode::new("value").with_payload(json!(1)), // 4: body
            AstNode::new("arm").with_payload(json!({"children": [3, 4]})), // 5
            AstNode::new("block").with_payload(json!({"children": [5]})), // 6
            AstNode::new("match").with_payload(json!({"children": [0, 6]})), // 7: root
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(1));
    }

    #[test]
    fn match_tag_pattern_non_tagged_value_no_match() {
        // match 42 { ok(v): v } → error (no arm matches)
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(42)), // 0: scrutinee (not tagged)
            AstNode::new("var").with_payload(json!("ok")), // 1: callee
            AstNode::new("var").with_payload(json!("v")),  // 2: binder
            AstNode::new("call").with_payload(json!({"children": [1, 2]})), // 3: pattern ok(v)
            AstNode::new("var").with_payload(json!("v")),  // 4: body
            AstNode::new("arm").with_payload(json!({"children": [3, 4]})), // 5
            AstNode::new("block").with_payload(json!({"children": [5]})), // 6
            AstNode::new("match").with_payload(json!({"children": [0, 6]})), // 7: root
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine { .. }));
    }

    // -----------------------------------------------------------------
    // rec — recursive computation region (ADR-0074)
    // -----------------------------------------------------------------

    /// Build a minimal language for `rec` eval tests: `value` (lit),
    /// `var`, `binary.add`, `binary.sub`, `binary.gt`, `binary.lt`,
    /// `binary.eq`, `if`, `assign`, `return`, `rec`, `let`.
    fn rec_lang() -> Language {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("$payload")))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                prim("add", vec![child(0), child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.sub",
                "binary.sub",
                prim("sub", vec![child(0), child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.gt",
                "binary.gt",
                prim("gt", vec![child(0), child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.lt",
                "binary.lt",
                prim("lt", vec![child(0), child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.eq",
                "binary.eq",
                prim("eq", vec![child(0), child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                branch(child(0), child(1), child(2)),
            ))
            .unwrap();
        // `assign` — the native op evaluates child 1 (the value) and
        // returns it. Inside `rec`, assign nodes are intercepted by the
        // `eval_rec` handler before this scheme is used.
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.assign",
                "assign",
                prim("assign", vec![]),
            ))
            .unwrap();
        // `return` — proj_child 0. Inside `rec`, return nodes are
        // intercepted by the `eval_rec` handler.
        lang.eval
            .merge(EvalSchemeDef::new("e.return", "return", child(0)))
            .unwrap();
        // `rec` — the recursive region native op.
        lang.eval
            .merge(EvalSchemeDef::new("e.rec", "rec", prim("eval_rec", vec![])))
            .unwrap();
        // `let` — bind name to child(0), evaluate child(1).
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                bind("$payload", child(0), child(1)),
            ))
            .unwrap();
        // AST shape roles for assign/return (ADR-0081).
        lang.ast
            .merge(AstShapeDef::new("ast.assign", "assign").with_children(vec![
                ChildRoleSpec {
                    role: "target".into(),
                    variadic: false,
                },
                ChildRoleSpec {
                    role: "value".into(),
                    variadic: false,
                },
            ]))
            .unwrap();
        lang.ast
            .merge(
                AstShapeDef::new("ast.return", "return").with_children(vec![ChildRoleSpec {
                    role: "value".into(),
                    variadic: false,
                }]),
            )
            .unwrap();
        lang
    }

    #[test]
    fn rec_simple_feedback() {
        // rec { a = b + 1; b = 0; a } → 1
        // b is not in any outer scope. On the first pass, `a = b + 1`
        // errors (b is unbound, proj_env returns "b" string, add fails).
        // The error is caught and a is skipped. b = 0 succeeds. On the
        // second pass, a = 0 + 1 = 1. On the third pass, no change →
        // fixpoint. The last child `a` evaluates to 1.
        let engine = TreeWalkEngine::new();
        let lang = rec_lang();
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("b")), // 0: b in b+1
            AstNode::new("value").with_payload(json!(1)), // 1: lit 1
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})), // 2: b+1
            AstNode::new("var").with_payload(json!("a")), // 3: LHS a = ...
            AstNode::new("assign").with_payload(json!({"children": [3, 2]})), // 4: a = b+1
            AstNode::new("value").with_payload(json!(0)), // 5: lit 0
            AstNode::new("var").with_payload(json!("b")), // 6: LHS b = ...
            AstNode::new("assign").with_payload(json!({"children": [6, 5]})), // 7: b = 0
            AstNode::new("var").with_payload(json!("a")), // 8: result expr
            AstNode::new("rec").with_payload(json!({"children": [4, 7, 8]})), // 9: root
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn rec_countdown() {
        // let n = 5 in rec { n = if n > 0 then n - 1 else n; n }
        // n counts down from 5 to 0, then the fixpoint is reached.
        // Result: 0.
        let engine = TreeWalkEngine::new();
        let lang = rec_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(5)), // 0: n initial
            AstNode::new("var").with_payload(json!("n")), // 1: n in n>0
            AstNode::new("value").with_payload(json!(0)), // 2: 0 in n>0
            AstNode::new("binary.gt").with_payload(json!({"children": [1, 2]})), // 3: n>0
            AstNode::new("var").with_payload(json!("n")), // 4: n in n-1
            AstNode::new("value").with_payload(json!(1)), // 5: 1 in n-1
            AstNode::new("binary.sub").with_payload(json!({"children": [4, 5]})), // 6: n-1
            AstNode::new("var").with_payload(json!("n")), // 7: n (else)
            AstNode::new("if").with_payload(json!({"children": [3, 6, 7]})), // 8: if n>0 then n-1 else n
            AstNode::new("var").with_payload(json!("n")),                    // 9: LHS n = ...
            AstNode::new("assign").with_payload(json!({"children": [9, 8]})), // 10: n = if ...
            AstNode::new("var").with_payload(json!("n")),                    // 11: result expr
            AstNode::new("rec").with_payload(json!({"children": [10, 11]})), // 12: rec
            AstNode::new("let").with_payload(json!({"name": "n", "children": [0, 12]})), // 13: root
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(0));
    }

    #[test]
    fn rec_fixpoint_convergence() {
        // let x = 0 in rec { x = if x < 10 then x + 1 else x; x }
        // x increments from 0 to 10, then the fixpoint is reached.
        // Result: 10.
        let engine = TreeWalkEngine::new();
        let lang = rec_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(0)), // 0: x initial
            AstNode::new("var").with_payload(json!("x")), // 1: x in x<10
            AstNode::new("value").with_payload(json!(10)), // 2: 10 in x<10
            AstNode::new("binary.lt").with_payload(json!({"children": [1, 2]})), // 3: x<10
            AstNode::new("var").with_payload(json!("x")), // 4: x in x+1
            AstNode::new("value").with_payload(json!(1)), // 5: 1 in x+1
            AstNode::new("binary.add").with_payload(json!({"children": [4, 5]})), // 6: x+1
            AstNode::new("var").with_payload(json!("x")), // 7: x (else)
            AstNode::new("if").with_payload(json!({"children": [3, 6, 7]})), // 8: if x<10 then x+1 else x
            AstNode::new("var").with_payload(json!("x")),                    // 9: LHS x = ...
            AstNode::new("assign").with_payload(json!({"children": [9, 8]})), // 10: x = if ...
            AstNode::new("var").with_payload(json!("x")),                    // 11: result expr
            AstNode::new("rec").with_payload(json!({"children": [10, 11]})), // 12: rec
            AstNode::new("let").with_payload(json!({"name": "x", "children": [0, 12]})), // 13: root
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(10));
    }

    #[test]
    fn rec_fuel_limit_errors() {
        // let n = 0 in rec { n = n + 1; n }
        // n increments forever — never converges. The fuel limit (10000)
        // must fire and produce a runtime error, not hang.
        let engine = TreeWalkEngine::new();
        let lang = rec_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(0)), // 0: n initial
            AstNode::new("var").with_payload(json!("n")), // 1: n in n+1
            AstNode::new("value").with_payload(json!(1)), // 2: 1 in n+1
            AstNode::new("binary.add").with_payload(json!({"children": [1, 2]})), // 3: n+1
            AstNode::new("var").with_payload(json!("n")), // 4: LHS n = ...
            AstNode::new("assign").with_payload(json!({"children": [4, 3]})), // 5: n = n+1
            AstNode::new("var").with_payload(json!("n")), // 6: result expr
            AstNode::new("rec").with_payload(json!({"children": [5, 6]})), // 7: rec
            AstNode::new("let").with_payload(json!({"name": "n", "children": [0, 7]})), // 8: root
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        let msg = format!("{}", err);
        assert!(
            msg.contains("fuel limit"),
            "expected fuel limit error, got: {}",
            msg
        );
    }

    #[test]
    fn rec_return_exits_early() {
        // rec { x = 42; return x } → 42
        // The return node exits the rec region immediately with x's value.
        let engine = TreeWalkEngine::new();
        let lang = rec_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(42)), // 0: lit 42
            AstNode::new("var").with_payload(json!("x")),  // 1: LHS x = ...
            AstNode::new("assign").with_payload(json!({"children": [1, 0]})), // 2: x = 42
            AstNode::new("var").with_payload(json!("x")),  // 3: return value
            AstNode::new("return").with_payload(json!({"children": [3]})), // 4: return x
            AstNode::new("rec").with_payload(json!({"children": [2, 4]})), // 5: root
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn rec_return_with_bindings() {
        // rec { x = 1; y = x + 1; return y } → 2
        // Multiple bindings are computed, then return exits with y.
        let engine = TreeWalkEngine::new();
        let lang = rec_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(1)), // 0: lit 1
            AstNode::new("var").with_payload(json!("x")), // 1: LHS x = ...
            AstNode::new("assign").with_payload(json!({"children": [1, 0]})), // 2: x = 1
            AstNode::new("var").with_payload(json!("x")), // 3: x in x+1
            AstNode::new("value").with_payload(json!(1)), // 4: 1 in x+1
            AstNode::new("binary.add").with_payload(json!({"children": [3, 4]})), // 5: x+1
            AstNode::new("var").with_payload(json!("y")), // 6: LHS y = ...
            AstNode::new("assign").with_payload(json!({"children": [6, 5]})), // 7: y = x+1
            AstNode::new("var").with_payload(json!("y")), // 8: return value
            AstNode::new("return").with_payload(json!({"children": [8]})), // 9: return y
            AstNode::new("rec").with_payload(json!({"children": [2, 7, 9]})), // 10: root
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(2));
    }

    #[test]
    fn rec_empty_returns_null() {
        // rec { } → Null (no children)
        let engine = TreeWalkEngine::new();
        let lang = rec_lang();
        let ast = make_ast(vec![
            AstNode::new("rec").with_payload(json!({"children": []})), // 0: root
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn rec_single_binding_fixpoint() {
        // rec { x = 5; x } → 5
        // A single constant binding converges immediately.
        let engine = TreeWalkEngine::new();
        let lang = rec_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!(5)), // 0: lit 5
            AstNode::new("var").with_payload(json!("x")), // 1: LHS x = ...
            AstNode::new("assign").with_payload(json!({"children": [1, 0]})), // 2: x = 5
            AstNode::new("var").with_payload(json!("x")), // 3: result expr
            AstNode::new("rec").with_payload(json!({"children": [2, 3]})), // 4: root
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn rec_mutual_dependency() {
        // let b = 0 in let a = 1 in rec { a = b + 1; b = a - 1; a }
        // a depends on b, b depends on a. Starting from outer a=1, b=0:
        // Pass 1: a = 0 + 1 = 1 (no change from outer 1), b = 1 - 1 = 0 (no change from outer 0).
        // Fixpoint reached immediately. Result: a = 1.
        let engine = TreeWalkEngine::new();
        let lang = rec_lang();
        let ast = make_ast(vec![
            // a = b + 1
            AstNode::new("var").with_payload(json!("b")), // 0: b in b+1
            AstNode::new("value").with_payload(json!(1)), // 1: lit 1
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})), // 2: b+1
            AstNode::new("var").with_payload(json!("a")), // 3: LHS a = ...
            AstNode::new("assign").with_payload(json!({"children": [3, 2]})), // 4: a = b+1
            // b = a - 1
            AstNode::new("var").with_payload(json!("a")), // 5: a in a-1
            AstNode::new("value").with_payload(json!(1)), // 6: lit 1
            AstNode::new("binary.sub").with_payload(json!({"children": [5, 6]})), // 7: a-1
            AstNode::new("var").with_payload(json!("b")), // 8: LHS b = ...
            AstNode::new("assign").with_payload(json!({"children": [8, 7]})), // 9: b = a-1
            // result
            AstNode::new("var").with_payload(json!("a")), // 10: result expr
            AstNode::new("rec").with_payload(json!({"children": [4, 9, 10]})), // 11: rec
            // let a = 1 in rec { ... }
            AstNode::new("value").with_payload(json!(1)), // 12: a initial
            AstNode::new("let").with_payload(json!({"name": "a", "children": [12, 11]})), // 13: let a = 1 in rec
            // let b = 0 in (let a = 1 in rec { ... })  [root]
            AstNode::new("value").with_payload(json!(0)), // 14: b initial
            AstNode::new("let").with_payload(json!({"name": "b", "children": [14, 13]})), // 15: root
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(1));
    }

    // --- runtime_assert native op (ADR-0068) -----------------------------

    /// Helper: build a language with the node kinds needed for
    /// runtime-assert tests (function, var, binary.gt, lit, runtime-assert).
    fn runtime_assert_lang() -> Language {
        let mut lang = Language::new();
        // "function" → closure creation.
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                prim("closure", vec![]),
            ))
            .unwrap();
        // "var" → variable lookup.
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", var("$payload")))
            .unwrap();
        // "binary.gt" → greater-than comparison.
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.gt",
                "binary.gt",
                prim("gt", vec![child(0), child(1)]),
            ))
            .unwrap();
        // "lit" has no scheme → returns payload (used for literal values).
        // "runtime-assert" → runtime_assert native op.
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.ra",
                "runtime-assert",
                prim("runtime_assert", vec![]),
            ))
            .unwrap();
        lang
    }

    #[test]
    fn runtime_assert_node_eval_passing_predicate() {
        // Construct: runtime-assert { [predicate, expr] }
        // predicate = (n) -> n > 0, expr = 5
        // The assert should pass and return 5.
        let engine = TreeWalkEngine::new();
        let lang = runtime_assert_lang();
        // 0: var("n") — used in the predicate body
        // 1: lit(0) — the 0 in n > 0
        // 2: binary.gt {children: [0, 1]} — body: n > 0
        // 3: function {params: ["n"], body: 2} — the predicate closure
        // 4: lit(5) — the expr value
        // 5: runtime-assert {children: [3, 4]} — assert(predicate, expr)
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("n")),
            AstNode::new("lit").with_payload(json!(0)),
            AstNode::new("binary.gt").with_payload(json!({"children": [0, 1]})),
            AstNode::new("function").with_payload(json!({"params": ["n"], "body": 2})),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("runtime-assert").with_payload(json!({"children": [3, 4]})),
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn runtime_assert_node_eval_failing_predicate() {
        // Construct: runtime-assert { [predicate, expr] }
        // predicate = (n) -> n > 0, expr = -1
        // The assert should fail with a LangError.
        let engine = TreeWalkEngine::new();
        let lang = runtime_assert_lang();
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("n")),
            AstNode::new("lit").with_payload(json!(0)),
            AstNode::new("binary.gt").with_payload(json!({"children": [0, 1]})),
            AstNode::new("function").with_payload(json!({"params": ["n"], "body": 2})),
            AstNode::new("lit").with_payload(json!(-1)),
            AstNode::new("runtime-assert").with_payload(json!({"children": [3, 4]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(
            matches!(err, LangError::Engine { ref message, .. } if message.contains("assertion failed"))
        );
    }

    #[test]
    fn runtime_assert_node_eval_non_closure_predicate_passes_through() {
        // When the predicate is not a closure (e.g. a plain type tag),
        // the assert should pass through the expr value unchanged.
        let engine = TreeWalkEngine::new();
        let lang = runtime_assert_lang();
        // 0: lit("type.number") — a non-closure predicate
        // 1: lit(42) — the expr value
        // 2: runtime-assert {children: [0, 1]}
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!("type.number")),
            AstNode::new("lit").with_payload(json!(42)),
            AstNode::new("runtime-assert").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(42));
    }

}
// ---------------------------------------------------------------------------
// ADR-0106 item 2 — `body_def` arena memo (`cached_def_arena`)
// ---------------------------------------------------------------------------
//
// These live outside the main test module so they can exercise the memo
// directly. They are the evidence that the per-application
// deserialization cost is actually gone: with the cache removed,
// `cached_def_arena_memoizes_per_definition` fails because every call would
// build a fresh `Rc`.

#[cfg(test)]
mod arrow_arena_cache_tests {
    use super::*;
    use serde_json::json;

    /// Build a well-formed `body_def` for a single-node arena.
    fn def_with_body(kind: &str) -> Rc<TaggedValue> {
        let mut ast = Ast::new();
        ast.push(AstNode::new(kind));
        Rc::new(TaggedValue::from(handl_language::ast_value(&ast)))
    }

    /// The memo returns the *same* arena allocation on the second lookup —
    /// i.e. it does not re-deserialize per application.
    #[test]
    fn cached_def_arena_memoizes_per_definition() {
        let def = def_with_body("lit");
        let first = cached_def_arena(&def).expect("well-formed definition");
        let second = cached_def_arena(&def).expect("well-formed definition");
        assert!(
            Rc::ptr_eq(&first, &second),
            "second lookup must reuse the memoized arena, not deserialize again",
        );
        assert_eq!(second.len(), 1);
        assert_eq!(second.nodes()[0].kind, "lit");
    }

    /// Equal-but-distinct definitions get distinct entries, and neither is
    /// contaminated by the other (the key is the value, not its shape).
    #[test]
    fn cached_def_arena_distinguishes_equal_definitions() {
        let a = def_with_body("lit");
        let b = def_with_body("lit");
        assert_ne!(
            Rc::as_ptr(&a) as usize,
            Rc::as_ptr(&b) as usize,
            "test premise: two separate definitions",
        );
        let ast_a = cached_def_arena(&a).expect("a");
        let ast_b = cached_def_arena(&b).expect("b");
        assert!(
            !Rc::ptr_eq(&ast_a, &ast_b),
            "distinct definitions must not share one cached arena",
        );
        // Both are correct, and each stays stable.
        assert_eq!(ast_a.nodes()[0].kind, "lit");
        assert_eq!(ast_b.nodes()[0].kind, "lit");
        assert!(Rc::ptr_eq(&ast_a, &cached_def_arena(&a).unwrap()));
    }

    /// Malformed definitions are rejected (so the caller falls back), and a
    /// rejection is not memoized as if it were an arena.
    #[test]
    fn cached_def_arena_rejects_malformed_and_empty() {
        let not_ast = Rc::new(TaggedValue::from(json!({ "nope": 1 })));
        assert!(cached_def_arena(&not_ast).is_none());

        let empty_ast = Rc::new(TaggedValue::from(handl_language::ast_value(&Ast::new())));
        assert!(
            cached_def_arena(&empty_ast).is_none(),
            "an empty arena is not usable as a body definition",
        );
    }

    /// The memo is bounded: it never holds more than the cap, and clearing it
    /// costs only a re-deserialization, never correctness.
    #[test]
    fn cached_def_arena_stays_bounded_and_stays_correct() {
        // Keep the definitions alive so their addresses stay distinct.
        let defs: Vec<Rc<TaggedValue>> = (0..ARROW_ARENA_CACHE_CAP + 8)
            .map(|_| def_with_body("lit"))
            .collect();
        for d in &defs {
            assert!(cached_def_arena(d).is_some());
        }
        let size = ARROW_ARENA_CACHE.with(|c| c.borrow().len());
        assert!(
            size <= ARROW_ARENA_CACHE_CAP,
            "memo must stay within its cap, got {size}",
        );
        // Everything still resolves after the clear.
        for d in &defs {
            let ast = cached_def_arena(d).expect("still resolvable after a clear");
            assert_eq!(ast.len(), 1);
        }
    }
}
