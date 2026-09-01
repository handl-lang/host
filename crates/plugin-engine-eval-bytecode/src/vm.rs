//! Bytecode VM internals: scope stack, numeric helpers, and the execution
//! frame.
//!
//! [`VmFrame`] executes a compiled [`Vec<Instr>`](crate::Instr) program
//! against an operand stack and a scope stack. [`Instr::EvalChild`] and
//! [`Instr::Call`] recurse into [`VmFrame::exec_node`], which compiles the
//! node's scheme (via a per-node cache) and executes.
//!
//! ## Value shapes
//!
//! The v2 VM operates on the same tagged JSON value shapes as the v2
//! tree-walk handler:
//!
//! - **Pair** (`first`): `{"__pair__": [a, b]}`.
//! - **Either** (`Choice` / `tag`): `{"__either__": "left"|"right", "value": v}`.
//! - **Arrow-value** (`App`): `{"__arrow__": true, "params": [...], "body": <idx>, "env": {...}}`.
//!
//! ## Loop execution model
//!
//! [`Instr::LoopCheck`] implements causal/strict feedback. The compiler
//! emits `Push(Null), Pair` to seed the initial pair `(incoming, Null)`, then
//! the body code, then `LoopCheck(loop_back)`. Each iteration: the body
//! consumes the pair from the stack and leaves its result. `LoopCheck` pops
//! the result and inspects its shape:
//! - **Either left** -> push the carried value, fall through (done).
//! - **Either right** -> the carried value is the new state; reconstruct
//!   `pair(original_input, new_state)`, push it, jump to `loop_back`.
//! - **Pair** (not an Either) -> single-step mode: push `fst`, fall through.
//!
//! A max-iteration counter (100_000) prevents infinite loops. The
//! `original_input` is saved by a look-ahead in the `Push` handler: when
//! `Push(Null)` is followed by `Pair` and no loop is active, the stack top
//! (the incoming value) is saved as `loop_input` before `Pair` consumes it.
//!
//! ## Native ops and `incoming`
//!
//! In the tree-walk, `incoming` is passed explicitly to each `eval_inner`
//! call. In the VM, the incoming value flows through the operand stack. For
//! 0-arg native ops that use `incoming` (`id`, `tag`, `is_pos`, `dec`,
//! `inc`, `half_floor`, `inc_fst`, `countdown_step`, `infinite_step`), the
//! incoming value IS the top of the stack -- the op pops it and pushes its
//! result. For ops with args (`add`, `sub`, etc.), the args are on the stack
//! and `incoming` is not used. For `closure` (0-arg, no incoming), nothing is
//! popped.
//!
//! ## Derived step ops
//!
//! All four derived step ops from ADR-0052 are supported:
//! - `loop_step` (the ergonomic `loop` surface lowering): the `NativeOp`
//!   handler pops the loop pair from the operand stack (below the
//!   inline-compiled args) and passes it as `incoming` to
//!   [`VmFrame::apply_native_v2`], which applies the body proc via
//!   [`VmFrame::apply_app`].
//! - `while_step`/`for_step`/`fix_step`: the `DerivedStepOp` instruction
//!   re-evaluates the arg sub-programs per iteration with the current value
//!   (extracted from the loop pair) as incoming, then dispatches to
//!   [`VmFrame::apply_native_v2`]. See the [`instr`](crate::instr) module
//!   docs for details.

use std::collections::HashMap;
use std::rc::Rc;

use handl_language::{Ast, AstNode, AstRef, LangError, Language, Value};

use crate::compile::compile;
use crate::instr::Instr;

// ---------------------------------------------------------------------------
// Scope stack — identical semantics to the tree-walk interpreter
// ---------------------------------------------------------------------------

/// A scope stack: `Vec<HashMap<String, Value>>` searched innermost-first.
///
/// Mirrors the tree-walk's `ScopeStack` exactly: `extend_env` pushes a child
/// scope; lookup searches innermost-first; closure calls swap in the captured
/// environment, bind params, evaluate the body, then restore the original
/// scope stack.
pub(crate) struct ScopeStack {
    /// The stack of scopes, innermost last. `pub(crate)` so the frame can
    /// perform the closure-env swap-in.
    pub(crate) scopes: Vec<HashMap<String, Value>>,
}

impl ScopeStack {
    pub(crate) fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    /// Look up a variable, searching innermost scope first.
    pub(crate) fn lookup(&self, name: &str) -> Option<&Value> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    /// Define a variable in the current (innermost) scope.
    pub(crate) fn define(&mut self, name: &str, value: Value) {
        let last = self.scopes.last_mut().expect("scope stack is never empty");
        last.insert(name.to_string(), value);
    }

    /// Push a new empty scope.
    pub(crate) fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pop the innermost scope (never pops the last scope).
    pub(crate) fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Capture all visible variables into a single flat map.
    ///
    /// Used by the `closure` primitive to snapshot the current environment.
    /// Inner scopes shadow outer ones (later entries overwrite earlier).
    pub(crate) fn capture(&self) -> HashMap<String, Value> {
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
// Numeric helpers — identical semantics to the tree-walk interpreter
// ---------------------------------------------------------------------------

/// A number that can be either integer or float.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub(crate) enum Num {
    /// Integer value.
    Int(i64),
    /// Float value.
    Float(f64),
}

impl Num {
    /// Convert to `f64`.
    pub(crate) fn to_f64(self) -> f64 {
        match self {
            Num::Int(i) => i as f64,
            Num::Float(f) => f,
        }
    }
}

/// Extract a number from a [`Value`].
pub(crate) fn as_num(v: &Value) -> Result<Num, LangError> {
    if let Some(i) = v.as_i64() {
        return Ok(Num::Int(i));
    }
    if let Some(f) = v.as_f64() {
        return Ok(Num::Float(f));
    }
    Err(LangError::Engine(format!("expected number, got {}", v)))
}

/// Convert a [`Num`] back to a [`Value`].
pub(crate) fn num_value(n: Num) -> Value {
    match n {
        Num::Int(i) => Value::from(i),
        Num::Float(f) => Value::from(f),
    }
}

/// Whether a [`Value`] is truthy.
///
/// `false`, `null`, `0`, and `""` are falsy; everything else is truthy.
pub(crate) fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Pair / Either value-shape helpers (mirror the v2 tree-walk)
// ---------------------------------------------------------------------------

/// Construct a pair value `{"__pair__": [a, b]}`.
fn pair(a: Value, b: Value) -> Value {
    serde_json::json!({"__pair__": [a, b]})
}

/// Extract the first component of a pair value, or `None` if not a pair.
fn pair_fst(v: &Value) -> Option<&Value> {
    v.get("__pair__")
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.first())
}

/// Extract the second component of a pair value, or `None` if not a pair.
fn pair_snd(v: &Value) -> Option<&Value> {
    v.get("__pair__")
        .and_then(|p| p.as_array())
        .and_then(|arr| arr.get(1))
}

/// Construct a `left` Either value `{"__either__": "left", "value": v}`.
fn either_left(v: Value) -> Value {
    serde_json::json!({"__either__": "left", "value": v})
}

/// Construct a `right` Either value `{"__either__": "right", "value": v}`.
fn either_right(v: Value) -> Value {
    serde_json::json!({"__either__": "right", "value": v})
}

/// Whether an Either value is a `left`.
fn is_either_left(v: &Value) -> bool {
    v.get("__either__").and_then(|s| s.as_str()) == Some("left")
}

/// Whether an Either value is a `right`.
fn is_either_right(v: &Value) -> bool {
    v.get("__either__").and_then(|s| s.as_str()) == Some("right")
}

/// Extract the carried value from an Either.
fn either_value(v: &Value) -> Option<&Value> {
    v.get("value")
}

// ---------------------------------------------------------------------------
// VmFrame -- the v2 execution context
// ---------------------------------------------------------------------------

/// The execution frame for the v2 bytecode VM.
///
/// Owns the scope stack and a per-node v2 compilation cache. The `ast` and
/// `lang` references are immutable; `modules` is the optional module registry
/// for `Foreign` dispatch.
pub(crate) struct VmFrame<'a, 'h> {
    /// The AST being evaluated.
    ast: &'a Ast,
    /// The language definition (for scheme lookup during compilation).
    lang: &'a Language,
    /// The module registry (for `Foreign` dispatch).
    modules: Option<&'h Rc<dyn handl_platform::ModuleLookup>>,
    /// Per-node v2 compilation cache: `AstRef index -> compiled instructions`.
    cache: HashMap<usize, Vec<Instr>>,
    /// The scope stack.
    scopes: ScopeStack,
}

impl<'a, 'h> VmFrame<'a, 'h> {
    /// Create a new v2 frame with a fresh cache and scope stack.
    pub(crate) fn new(
        ast: &'a Ast,
        lang: &'a Language,
        modules: Option<&'h Rc<dyn handl_platform::ModuleLookup>>,
    ) -> Self {
        Self {
            ast,
            lang,
            modules,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        }
    }

    /// Define a global variable in the root scope.
    pub(crate) fn define_global(&mut self, name: &str, value: Value) {
        self.scopes.define(name, value);
    }

    /// Compile (if not cached) and execute a single node using v2 bytecode.
    ///
    /// Looks up `lang.eval[node.kind]` to find the node's `Scheme`, compiles
    /// it to v2 instructions (or uses the no-scheme fallback
    /// `[LoadPayload, Return]`), caches the result, and executes. On
    /// subsequent calls for the same [`AstRef`], the cached instructions are
    /// reused.
    pub(crate) fn exec_node(&mut self, node_ref: AstRef) -> Result<Value, LangError> {
        let node = self.ast.at(node_ref)?;

        // Compile-on-demand with cache.
        if !self.cache.contains_key(&node_ref.index()) {
            let scheme_def = self.lang.eval.get_by_identity(&node.kind);
            let instrs = match scheme_def {
                Some(def) => compile(&def.scheme, node)?,
                None => vec![Instr::LoadPayload, Instr::Return],
            };
            self.cache.insert(node_ref.index(), instrs);
        }

        // Clone instructions to avoid borrow conflict with recursive
        // exec_node calls that may insert into the cache.
        let instrs = self
            .cache
            .get(&node_ref.index())
            .expect("just inserted or cached")
            .clone();
        self.exec_instrs(&instrs, node, None)
    }

    /// Execute a program of v2 instructions against the given current node.
    ///
    /// `incoming` is the value fed by an enclosing combinator (top-level
    /// calls pass `None`). The operand stack is fresh; the scope stack
    /// (`self.scopes`) is shared across recursive `exec_node` calls.
    pub(crate) fn exec_instrs(
        &mut self,
        instrs: &[Instr],
        node: &AstNode,
        incoming: Option<Value>,
    ) -> Result<Value, LangError> {
        let mut stack: Vec<Value> = Vec::new();
        // Push the incoming value onto the stack so that 0-arg ops using
        // `op_uses_incoming` (like `id`) can pop it. At the top level
        // (incoming = None), the stack starts empty — matching the
        // tree-walk's `incoming: None` behavior.
        if let Some(v) = &incoming {
            stack.push(v.clone());
        }
        let mut current_incoming = incoming;
        let mut loop_counter: usize = 0;
        const MAX_ITERS: usize = 100_000;
        let mut loop_input: Option<Value> = None;

        let mut ip = 0;
        while ip < instrs.len() {
            match &instrs[ip] {
                Instr::Push(v) => {
                    // Look-ahead: if this is Push(Null) followed by Pair and
                    // no loop is active, save the stack top as the original
                    // loop input. The compiler emits Push(Null), Pair as the
                    // loop prologue; the stack top is the incoming value that
                    // Pair will consume into the initial pair.
                    if loop_input.is_none()
                        && loop_counter == 0
                        && v.is_null()
                        && ip + 1 < instrs.len()
                        && matches!(instrs[ip + 1], Instr::Pair)
                    {
                        loop_input = stack.last().cloned();
                    }
                    stack.push(v.clone());
                }
                Instr::Load(name) => {
                    let val = self
                        .scopes
                        .lookup(name)
                        .cloned()
                        .unwrap_or_else(|| Value::from(name.as_str()));
                    stack.push(val);
                }
                Instr::LoadPayloadVar => {
                    let var_name = node
                        .payload
                        .as_str()
                        .ok_or_else(|| {
                            LangError::Engine("expected string payload for $payload var".to_string())
                        })?
                        .to_string();
                    let val = self
                        .scopes
                        .lookup(&var_name)
                        .cloned()
                        .unwrap_or_else(|| Value::from(var_name.as_str()));
                    stack.push(val);
                }
                Instr::LoadPayload => {
                    stack.push(node.payload.clone());
                }
                Instr::EvalChild(n) => {
                    let children = node
                        .payload
                        .get("children")
                        .and_then(|c| c.as_array())
                        .ok_or_else(|| {
                            LangError::Engine("expected children array in payload".to_string())
                        })?;
                    let arena_idx = children
                        .get(*n)
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| LangError::Engine(format!("child {} not found", n)))?;
                    let child_ref = AstRef::new(arena_idx as usize);
                    let result = self.exec_node(child_ref)?;
                    stack.push(result);
                }
                Instr::Store(name) => {
                    let val = stack
                        .pop()
                        .ok_or_else(|| LangError::Engine("stack underflow on Store".to_string()))?;
                    self.scopes.push_scope();
                    self.scopes.define(name, val);
                }
                Instr::StorePayloadName => {
                    let val = stack
                        .pop()
                        .ok_or_else(|| {
                            LangError::Engine("stack underflow on StorePayloadName".to_string())
                        })?;
                    let bind_name = node
                        .payload
                        .get("name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            LangError::Engine(
                                "expected name in payload for $payload extend_env".to_string(),
                            )
                        })?
                        .to_string();
                    self.scopes.push_scope();
                    self.scopes.define(&bind_name, val);
                }
                Instr::PopScope => {
                    self.scopes.pop_scope();
                }
                Instr::Call(n) => {
                    let mut args = Vec::with_capacity(*n);
                    for _ in 0..*n {
                        args.push(stack.pop().ok_or_else(|| {
                            LangError::Engine("stack underflow on Call".to_string())
                        })?);
                    }
                    args.reverse();
                    let callee = stack
                        .pop()
                        .ok_or_else(|| LangError::Engine("stack underflow on Call".to_string()))?;
                    let result = self.apply_app(&callee, &args)?;
                    stack.push(result);
                }
                Instr::ChoiceJump(left_target, right_target) => {
                    let either = stack
                        .pop()
                        .ok_or_else(|| {
                            LangError::Engine("stack underflow on ChoiceJump".to_string())
                        })?;
                    let val = either_value(&either)
                        .cloned()
                        .ok_or_else(|| {
                            LangError::Engine("Choice: Either missing value".to_string())
                        })?;
                    if is_either_left(&either) {
                        stack.push(val);
                        ip = *left_target;
                        continue;
                    } else if is_either_right(&either) {
                        stack.push(val);
                        ip = *right_target;
                        continue;
                    } else {
                        return Err(LangError::Engine(
                            "Choice: incoming is not an Either".to_string(),
                        ));
                    }
                }
                Instr::Jump(target) => {
                    ip = *target;
                    continue;
                }
                Instr::NativeOp(op, n) => {
                    // For 0-arg ops that use `incoming`: the incoming value
                    // is on top of the stack. Pop it and pass as `incoming`.
                    // For other ops: pop n args, use current_incoming.
                    //
                    // Derived step ops (`loop_step`) are a hybrid: they carry
                    // sub-scheme args (the body proc) compiled inline, but
                    // they also consume the loop pair `(in, state)` that the
                    // enclosing `Loop` left on the stack below those args.
                    // Pop the args first, then pop the pair as `incoming`.
                    let is_derived_step = op_is_derived_step(op);
                    let inc = if is_derived_step {
                        // Filled below after popping args (the pair sits
                        // beneath the args on the stack).
                        None
                    } else if *n == 0 && op_uses_incoming(op) {
                        stack.pop()
                    } else {
                        current_incoming.take()
                    };
                    let mut args = Vec::with_capacity(*n);
                    for _ in 0..*n {
                        args.push(stack.pop().ok_or_else(|| {
                            LangError::Engine("stack underflow on NativeOp".to_string())
                        })?);
                    }
                    args.reverse();
                    let inc = if is_derived_step {
                        Some(stack.pop().ok_or_else(|| {
                            LangError::Engine(
                                "derived step op: missing loop pair on stack".to_string(),
                            )
                        })?)
                    } else {
                        inc
                    };
                    let result = self.apply_native_v2(op, &args, node, inc)?;
                    stack.push(result);
                }
                Instr::ForeignOp(name, n) => {
                    let mut args = Vec::with_capacity(*n);
                    for _ in 0..*n {
                        args.push(stack.pop().ok_or_else(|| {
                            LangError::Engine("stack underflow on ForeignOp".to_string())
                        })?);
                    }
                    args.reverse();
                    let modules = self
                        .modules
                        .ok_or_else(|| LangError::Engine(format!("unregistered foreign: {}", name)))?;
                    let func = modules
                        .lookup(name)
                        .ok_or_else(|| LangError::Engine(format!("unregistered foreign: {}", name)))?;
                    let mut pctx = handl_platform::EvalContext::new();
                    let result = func
                        .call(&args, &mut pctx)
                        .map_err(|e| LangError::Engine(e.to_string()))?;
                    stack.push(result);
                }
                Instr::FirstOp(start, end) => {
                    // Pop the pair from the stack, extract fst/snd, run the
                    // inner sub-program at [start, end) with fst as incoming
                    // on a fresh stack, then re-pair the result with snd.
                    let p = stack
                        .pop()
                        .ok_or_else(|| LangError::Engine("stack underflow on FirstOp".to_string()))?;
                    let a = pair_fst(&p)
                        .ok_or_else(|| LangError::Engine("FirstOp: not a pair".to_string()))?
                        .clone();
                    let b = pair_snd(&p)
                        .ok_or_else(|| LangError::Engine("FirstOp: not a pair".to_string()))?
                        .clone();
                    let sub = &instrs[*start..*end];
                    let result = self.exec_instrs(sub, node, Some(a))?;
                    stack.push(pair(result, b));
                    ip = *end;
                    continue;
                }
                Instr::Pair => {
                    let a = stack
                        .pop()
                        .ok_or_else(|| LangError::Engine("stack underflow on Pair".to_string()))?;
                    let b = stack
                        .pop()
                        .ok_or_else(|| LangError::Engine("stack underflow on Pair".to_string()))?;
                    stack.push(pair(b, a));
                }
                Instr::LoopCheck(loop_back) => {
                    loop_counter += 1;
                    if loop_counter > MAX_ITERS {
                        return Err(LangError::Engine(
                            "Loop: exceeded max iterations".to_string(),
                        ));
                    }
                    let result = stack
                        .pop()
                        .ok_or_else(|| {
                            LangError::Engine("stack underflow on LoopCheck".to_string())
                        })?;
                    if is_either_left(&result) {
                        let out = either_value(&result).cloned().ok_or_else(|| {
                            LangError::Engine("Loop: left Either missing value".to_string())
                        })?;
                        stack.push(out);
                        loop_counter = 0;
                        loop_input = None;
                    } else if is_either_right(&result) {
                        let new_state = either_value(&result).cloned().ok_or_else(|| {
                            LangError::Engine("Loop: right Either missing value".to_string())
                        })?;
                        let orig = loop_input.clone().unwrap_or(Value::Null);
                        stack.push(pair(orig, new_state));
                        ip = *loop_back;
                        continue;
                    } else {
                        let out = pair_fst(&result).ok_or_else(|| {
                            LangError::Engine(
                                "Loop: body did not return a pair or Either".to_string(),
                            )
                        })?;
                        stack.push(out.clone());
                        loop_counter = 0;
                        loop_input = None;
                    }
                }
                Instr::Return => {
                    return stack
                        .pop()
                        .ok_or_else(|| LangError::Engine("stack underflow on Return".to_string()));
                }
                Instr::DerivedStepOp(op, ranges) => {
                    // Pop the loop pair (in, state) from the stack — left
                    // by the enclosing Loop. Extract the current value
                    // (in on first step, state thereafter).
                    let inc = stack.pop().ok_or_else(|| {
                        LangError::Engine(
                            "DerivedStepOp: missing loop pair on stack".to_string(),
                        )
                    })?;
                    let in_val = pair_fst(&inc).cloned().unwrap_or(Value::Null);
                    let state = pair_snd(&inc).cloned().unwrap_or(Value::Null);
                    let current = if state.is_null() { in_val.clone() } else { state.clone() };
                    // Determine the incoming value for the arg sub-programs.
                    // - while_step / fix_step: pass `current` (the evolving
                    //   value) to cond/body.
                    // - for_step: pass `acc` (the second component of the
                    //   pair) to body — not the full pair. The pair is
                    //   (counter, acc); the body operates on `acc`.
                    let arg_incoming = if op == "for_step" {
                        if state.is_null() {
                            // First step: in_val = pair(count, initial).
                            // acc = snd(in_val).
                            pair_snd(&in_val).cloned().unwrap_or(Value::Null)
                        } else {
                            // Later steps: state = pair(counter, acc).
                            pair_snd(&state).cloned().unwrap_or(Value::Null)
                        }
                    } else {
                        current
                    };
                    // Re-evaluate each arg sub-program with `arg_incoming`
                    // as incoming. The sub-programs are at [start, end) in
                    // the current instruction stream.
                    let mut eval_args: Vec<Value> = Vec::with_capacity(ranges.len());
                    for &(start, end) in ranges {
                        let sub = &instrs[start..end];
                        let result = self.exec_instrs(sub, node, Some(arg_incoming.clone()))?;
                        eval_args.push(result);
                    }
                    // Dispatch to apply_native_v2 with the fresh results
                    // and the loop pair as incoming.
                    let result = self.apply_native_v2(
                        op,
                        &eval_args,
                        node,
                        Some(inc),
                    )?;
                    stack.push(result);
                }
            }
            ip += 1;
        }
        stack
            .pop()
            .ok_or_else(|| LangError::Engine("empty operand stack at end of program".to_string()))
    }

    /// Apply a native v2 op. Mirrors the v2 tree-walk's `apply_native` exactly.
    ///
    /// `incoming` is the value flowing through the arrow (used by `id`,
    /// `tag`, `is_pos`, `dec`, `inc`, `half_floor`, `inc_fst`,
    /// `countdown_step`, `infinite_step`).
    fn apply_native_v2(
        &mut self,
        op: &str,
        args: &[Value],
        node: &AstNode,
        incoming: Option<Value>,
    ) -> Result<Value, LangError> {
        match op {
            "id" => Ok(incoming
                .or_else(|| args.first().cloned())
                .unwrap_or(Value::Null)),
            "tag" => {
                let v = incoming
                    .or_else(|| args.first().cloned())
                    .unwrap_or(Value::Null);
                if is_truthy(&v) {
                    Ok(either_left(v))
                } else {
                    Ok(either_right(v))
                }
            }
            "closure" => {
                let payload = &node.payload;
                let (params, body) = if let (Some(p), Some(b)) = (
                    payload.get("params").and_then(|v| v.as_array()),
                    payload.get("body").and_then(|v| v.as_u64()),
                ) {
                    (p.clone(), b as usize)
                } else if let Some(children) =
                    payload.get("children").and_then(|c| c.as_array())
                {
                    let param_idx = children
                        .first()
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| {
                            LangError::Engine("closure: missing param child".to_string())
                        })? as usize;
                    let body_idx = children
                        .get(1)
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| {
                            LangError::Engine("closure: missing body child".to_string())
                        })? as usize;
                    let param_node = self.ast.at(AstRef::new(param_idx))?;
                    let param_names: Vec<String> = if param_node.kind == "paren" {
                        let paren_children = param_node
                            .payload
                            .get("children")
                            .and_then(|c| c.as_array())
                            .ok_or_else(|| {
                                LangError::Engine(
                                    "closure: paren params missing children".to_string(),
                                )
                            })?;
                        let mut names = Vec::with_capacity(paren_children.len());
                        for pc in paren_children {
                            let pidx = pc.as_u64().ok_or_else(|| {
                                LangError::Engine(
                                    "closure: param index must be a number".to_string(),
                                )
                            })? as usize;
                            let pnode = self.ast.at(AstRef::new(pidx))?;
                            let pname = pnode.payload.as_str().ok_or_else(|| {
                                LangError::Engine(
                                    "closure: param must be a var with string payload"
                                        .to_string(),
                                )
                            })?;
                            names.push(pname.to_string());
                        }
                        names
                    } else {
                        let pname = param_node.payload.as_str().ok_or_else(|| {
                            LangError::Engine(
                                "closure: param node must be a var with string payload"
                                    .to_string(),
                            )
                        })?;
                        vec![pname.to_string()]
                    };
                    (
                        param_names.into_iter().map(Value::from).collect(),
                        body_idx,
                    )
                } else {
                    return Err(LangError::Engine(
                        "closure: missing params/body in payload".to_string(),
                    ));
                };
                let captured = self.scopes.capture();
                Ok(serde_json::json!({
                    "__arrow__": true,
                    "params": params,
                    "body": body,
                    "env": captured,
                }))
            }
            "array" => {
                // Two payload shapes (mirrors v1 `array` prim):
                // - Rich: `{"elements": [v1, v2, ...]}` — literal values.
                // - Pratt-parsed: `{"children": [elem_idx, ...]}` — each
                //   element is an arena index to evaluate.
                if let Some(elements) = node.payload.get("elements") {
                    Ok(elements.clone())
                } else if let Some(children) =
                    node.payload.get("children").and_then(|c| c.as_array())
                {
                    let mut result = Vec::with_capacity(children.len());
                    for entry in children {
                        let idx = entry.as_u64().ok_or_else(|| {
                            LangError::Engine(
                                "array: element index must be a number".to_string(),
                            )
                        })? as usize;
                        result.push(self.exec_node(AstRef::new(idx))?);
                    }
                    Ok(Value::Array(result))
                } else {
                    Err(LangError::Engine(
                        "array: missing elements/children in payload".to_string(),
                    ))
                }
            }
            "object" => {
                // Two payload shapes (mirrors v1 `object` prim):
                // - Rich: `{"entries": [{"key": "k", "value": v}, ...]}`.
                // - Pratt-parsed: `{"children": [entry_idx, ...]}` where each
                //   entry node has `{"children": [key_idx, value_idx]}`.
                if let Some(entries) =
                    node.payload.get("entries").and_then(|v| v.as_array())
                {
                    let mut obj = serde_json::Map::new();
                    for entry in entries {
                        let key = entry.get("key").and_then(|v| v.as_str()).ok_or_else(|| {
                            LangError::Engine("object: entry missing key".to_string())
                        })?;
                        let value = entry.get("value").ok_or_else(|| {
                            LangError::Engine("object: entry missing value".to_string())
                        })?;
                        if handl_language::is_reserved_value_key(key) {
                            return Err(LangError::Engine(format!(
                                "object: key '{}' is reserved",
                                key
                            )));
                        }
                        obj.insert(key.to_string(), value.clone());
                    }
                    Ok(Value::Object(obj))
                } else if let Some(children) =
                    node.payload.get("children").and_then(|c| c.as_array())
                {
                    let mut obj = serde_json::Map::new();
                    for entry in children {
                        let entry_idx = entry.as_u64().ok_or_else(|| {
                            LangError::Engine(
                                "object: entry index must be a number".to_string(),
                            )
                        })? as usize;
                        let entry_node = self.ast.at(AstRef::new(entry_idx))?;
                        let entry_children = entry_node
                            .payload
                            .get("children")
                            .and_then(|c| c.as_array())
                            .ok_or_else(|| {
                                LangError::Engine(
                                    "object: entry node missing children".to_string(),
                                )
                            })?;
                        let key_idx = entry_children
                            .get(0)
                            .and_then(|v| v.as_u64())
                            .ok_or_else(|| {
                                LangError::Engine(
                                    "object: entry missing key child".to_string(),
                                )
                            })? as usize;
                        let val_idx = entry_children
                            .get(1)
                            .and_then(|v| v.as_u64())
                            .ok_or_else(|| {
                                LangError::Engine(
                                    "object: entry missing value child".to_string(),
                                )
                            })? as usize;
                        // Object key: if the key node is a `var` (bare
                        // identifier), use its name as the key string
                        // directly — don't evaluate it as a variable. This
                        // makes `{ n: 5 }` use key "n", not the value of n.
                        // Otherwise, evaluate the key expression and use
                        // its string representation. Mirrors the v2
                        // tree-walk's object prim.
                        let key_node = self.ast.at(AstRef::new(key_idx))?;
                        let key = if key_node.kind == "var" {
                            key_node.payload.as_str().ok_or_else(|| {
                                LangError::Engine(
                                    "object: var key missing string payload".to_string(),
                                )
                            })?
                            .to_string()
                        } else if key_node.kind == "string" {
                            key_node.payload.as_str().ok_or_else(|| {
                                LangError::Engine(
                                    "object: string key missing payload".to_string(),
                                )
                            })?
                            .to_string()
                        } else if key_node.kind == "array" {
                            // Computed key: `[expr]: val` evaluates expr and
                            // uses its value as the key (JS-style bracket
                            // computed keys). Only a single-element bracket
                            // is meaningful as a key.
                            let arr_children = key_node
                                .payload
                                .get("children")
                                .and_then(|c| c.as_array());
                            let inner_idx = match arr_children {
                                Some(c) if c.len() == 1 => c[0].as_u64().ok_or_else(|| {
                                    LangError::Engine(
                                        "object: computed key [expr] child index must be a number"
                                            .to_string(),
                                    )
                                })? as usize,
                                _ => {
                                    return Err(LangError::Engine(
                                        "object: computed key [expr] requires exactly one element"
                                            .to_string(),
                                    ))
                                }
                            };
                            let key_val = self.exec_node(AstRef::new(inner_idx))?;
                            if let Some(s) = key_val.as_str() {
                                s.to_string()
                            } else {
                                key_val.to_string()
                            }
                        } else {
                            let key_val = self.exec_node(AstRef::new(key_idx))?;
                            if let Some(s) = key_val.as_str() {
                                s.to_string()
                            } else {
                                key_val.to_string()
                            }
                        };
                        let value = self.exec_node(AstRef::new(val_idx))?;
                        if handl_language::is_reserved_value_key(&key) {
                            return Err(LangError::Engine(format!(
                                "object: key '{}' is reserved",
                                key
                            )));
                        }
                        obj.insert(key, value);
                    }
                    Ok(Value::Object(obj))
                } else {
                    Err(LangError::Engine(
                        "object: missing entries/children in payload".to_string(),
                    ))
                }
            }
            "index" => {
                let target = node.payload.get("target").ok_or_else(|| {
                    LangError::Engine("index: missing target in payload".to_string())
                })?;
                let index = node.payload.get("index").ok_or_else(|| {
                    LangError::Engine("index: missing index in payload".to_string())
                })?;
                match (target, index) {
                    (Value::Array(arr), Value::Number(n)) => {
                        let i = n.as_u64().ok_or_else(|| {
                            LangError::Engine(
                                "index: index must be a non-negative integer".to_string(),
                            )
                        })? as usize;
                        arr.get(i).cloned().ok_or_else(|| {
                            LangError::Engine(format!("index: index {} out of bounds", i))
                        })
                    }
                    (Value::Object(obj), Value::String(key)) => {
                        obj.get(key).cloned().ok_or_else(|| {
                            LangError::Engine(format!("index: key '{}' not found", key))
                        })
                    }
                    _ => Err(LangError::Engine(
                        "index: target must be array/object and index must be number/string"
                            .to_string(),
                    )),
                }
            }
            "assign" => {
                // ADR-0053: `assign { children: [var, value] }` — evaluate
                // child 1 (value) and return it. Fall back to legacy
                // `payload.value` for hand-built ASTs.
                if let Some(children) =
                    node.payload.get("children").and_then(|c| c.as_array())
                {
                    if children.len() >= 2 {
                        let value_idx = children[1].as_u64().ok_or_else(|| {
                            LangError::Engine(
                                "assign: value index must be a number".to_string(),
                            )
                        })? as usize;
                        return self.exec_node(AstRef::new(value_idx));
                    }
                }
                let value = node.payload.get("value").ok_or_else(|| {
                    LangError::Engine("assign: missing value in payload".to_string())
                })?;
                Ok(value.clone())
            }
            "add" => bin_num(args, |a, b| a + b),
            "sub" => bin_num(args, |a, b| a - b),
            "mul" => bin_num(args, |a, b| a * b),
            "div" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                if b.to_f64() == 0.0 {
                    return Err(LangError::Engine("division by zero".to_string()));
                }
                match (a, b) {
                    (Num::Int(a), Num::Int(b)) => Ok(Value::from(a / b)),
                    (a, b) => Ok(Value::from(a.to_f64() / b.to_f64())),
                }
            }
            "mod" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                if b.to_f64() == 0.0 {
                    return Err(LangError::Engine("modulo by zero".to_string()));
                }
                Ok(num_value(match (a, b) {
                    (Num::Int(a), Num::Int(b)) => Num::Int(a % b),
                    (a, b) => Num::Float(a.to_f64() % b.to_f64()),
                }))
            }
            "neg" => {
                let a = as_num(&args[0])?;
                Ok(num_value(match a {
                    Num::Int(i) => Num::Int(-i),
                    Num::Float(f) => Num::Float(-f),
                }))
            }
            "eq" => Ok(Value::Bool(args[0] == args[1])),
            "neq" => Ok(Value::Bool(args[0] != args[1])),
            "lt" => cmp_num(args, |a, b| a < b),
            "gt" => cmp_num(args, |a, b| a > b),
            "lte" => cmp_num(args, |a, b| a <= b),
            "gte" => cmp_num(args, |a, b| a >= b),
            "pow" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok(num_value(Num::Float(a.to_f64().powf(b.to_f64()))))
            }
            "compare" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                let cmp = a.to_f64().partial_cmp(&b.to_f64()).ok_or_else(|| {
                    LangError::Engine("compare: incomparable values".to_string())
                })?;
                Ok(num_value(Num::Int(match cmp {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                })))
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
            "is_pos" => {
                let v = incoming.unwrap_or(Value::Null);
                let n = as_num(&v)?;
                Ok(Value::Bool(n.to_f64() > 0.0))
            }
            "dec" => {
                let v = incoming.unwrap_or(Value::Null);
                let n = as_num(&v)?;
                Ok(num_value(match n {
                    Num::Int(i) => Num::Int(i - 1),
                    Num::Float(f) => Num::Float(f - 1.0),
                }))
            }
            "inc" => {
                let v = incoming.unwrap_or(Value::Null);
                let n = as_num(&v)?;
                Ok(num_value(match n {
                    Num::Int(i) => Num::Int(i + 1),
                    Num::Float(f) => Num::Float(f + 1.0),
                }))
            }
            "half_floor" => {
                let v = incoming.unwrap_or(Value::Null);
                let n = as_num(&v)?;
                Ok(num_value(match n {
                    Num::Int(i) => Num::Int(i / 2),
                    Num::Float(f) => Num::Float((f / 2.0).floor()),
                }))
            }
            "inc_fst" => {
                let inc = args.first().cloned().or(incoming).unwrap_or(Value::Null);
                let a = pair_fst(&inc)
                    .ok_or_else(|| LangError::Engine("inc_fst: input is not a pair".to_string()))?
                    .clone();
                let b = pair_snd(&inc)
                    .ok_or_else(|| LangError::Engine("inc_fst: input is not a pair".to_string()))?
                    .clone();
                let a_n = as_num(&a)?;
                Ok(pair(num_value(match a_n {
                    Num::Int(i) => Num::Int(i + 1),
                    Num::Float(f) => Num::Float(f + 1.0),
                }), b))
            }
            "countdown_step" => {
                let inc = args.first().cloned().or(incoming).unwrap_or(Value::Null);
                let a = pair_fst(&inc)
                    .cloned()
                    .ok_or_else(|| LangError::Engine("countdown_step: input is not a pair".to_string()))?;
                let b = pair_snd(&inc).cloned().unwrap_or(Value::Null);
                let result_val = pair_fst(&a).cloned().ok_or_else(|| {
                    LangError::Engine("countdown_step: in must be pair(result, initial)".to_string())
                })?;
                let initial = pair_snd(&a).cloned().unwrap_or(Value::Null);
                let state = if b.is_null() { initial } else { b };
                let n = as_num(&state)?;
                if n.to_f64() > 0.0 {
                    Ok(either_right(num_value(match n {
                        Num::Int(i) => Num::Int(i - 1),
                        Num::Float(f) => Num::Float(f - 1.0),
                    })))
                } else {
                    Ok(either_left(result_val))
                }
            }
            "infinite_step" => {
                let inc = args.first().cloned().or(incoming).unwrap_or(Value::Null);
                let b = pair_snd(&inc).cloned().unwrap_or(Value::Null);
                Ok(either_right(b))
            }
            // Ergonomic `loop` surface lowering (ADR-0052). args = [body_proc]
            // — the body proc closure, evaluated from the carried sub-scheme
            // arg. `incoming` is the loop pair `(in, state)` popped from the
            // operand stack by the `NativeOp` handler (see `op_is_derived_step`).
            // The body proc is applied to the current state (in on the first
            // step, state thereafter); it returns an object:
            //   { out: v }    → either_left(v)  (terminate)
            //   { state: v }  → either_right(v) (continue)
            // Param destructuring: if the body proc has >1 params and the
            // current state is an object, bind each param to the corresponding
            // field by name. Mirrors the v2 tree-walk's `eval_derived_step`
            // `loop_step` arm exactly.
            "loop_step" => {
                let inc = incoming.unwrap_or(Value::Null);
                let in_val = pair_fst(&inc).cloned().unwrap_or(Value::Null);
                let state = pair_snd(&inc).cloned().unwrap_or(Value::Null);
                let body_proc = args.first().ok_or_else(|| {
                    LangError::Engine("loop_step: missing body proc".to_string())
                })?;
                let current = if state.is_null() { in_val } else { state };
                let call_args: Vec<Value> = if let Some(obj_state) = current.as_object() {
                    if let Some(closure_obj) = body_proc.as_object()
                        && let Some(params) = closure_obj
                            .get("params")
                            .and_then(|v| v.as_array())
                        && params.len() > 1
                    {
                        params
                            .iter()
                            .map(|p| {
                                let name = p.as_str().unwrap_or("_");
                                obj_state
                                    .get(name)
                                    .cloned()
                                    .unwrap_or(Value::Null)
                            })
                            .collect()
                    } else {
                        vec![current]
                    }
                } else {
                    vec![current]
                };
                let result = self.apply_app(body_proc, &call_args)?;
                let obj = result.as_object().ok_or_else(|| {
                    LangError::Engine(
                        "loop_step: body must return { out: v } or { state: v }".to_string(),
                    )
                })?;
                if let Some(out) = obj.get("out") {
                    Ok(either_left(out.clone()))
                } else if let Some(new_state) = obj.get("state") {
                    Ok(either_right(new_state.clone()))
                } else {
                    Err(LangError::Engine(
                        "loop_step: body must return { out: v } or { state: v }".to_string(),
                    ))
                }
            }
            // `while_step` derived step op (ADR-0052 §"Derived forms").
            // args = [cond_result, body_result] — re-evaluated per iteration
            // by the `DerivedStepOp` handler with the current value as
            // incoming. `incoming` is the loop pair (in, state).
            // Current = in (first step) or state (later). If cond is truthy,
            // run body and continue with its result; else terminate.
            "while_step" => {
                let inc = incoming.unwrap_or(Value::Null);
                let in_val = pair_fst(&inc).cloned().unwrap_or(Value::Null);
                let state = pair_snd(&inc).cloned().unwrap_or(Value::Null);
                let current = if state.is_null() { in_val } else { state };
                let cond_val = args.first().cloned().unwrap_or(Value::Null);
                if is_truthy(&cond_val) {
                    let body_val = args.get(1).cloned().unwrap_or(Value::Null);
                    Ok(either_right(body_val))
                } else {
                    Ok(either_left(current))
                }
            }
            // `for_step` derived step op. args = [body_result]. State is
            // pair(counter, acc). First step: initialize from in = pair(count,
            // initial). While counter > 0, run body on acc and continue with
            // pair(counter-1, new_acc); else terminate with acc.
            "for_step" => {
                let inc = incoming.unwrap_or(Value::Null);
                let in_val = pair_fst(&inc).cloned().unwrap_or(Value::Null);
                let state = pair_snd(&inc).cloned().unwrap_or(Value::Null);
                let (counter, acc) = if state.is_null() {
                    let count = pair_fst(&in_val).cloned().unwrap_or(Value::Null);
                    let initial = pair_snd(&in_val).cloned().unwrap_or(Value::Null);
                    (count, initial)
                } else {
                    let c = pair_fst(&state).cloned().unwrap_or(Value::Null);
                    let a = pair_snd(&state).cloned().unwrap_or(Value::Null);
                    (c, a)
                };
                let count_n = as_num(&counter)?;
                if count_n > Num::Int(0) {
                    let body_val = args.first().cloned().unwrap_or(Value::Null);
                    let new_counter = match count_n {
                        Num::Int(i) => num_value(Num::Int(i - 1)),
                        Num::Float(f) => num_value(Num::Float(f - 1.0)),
                    };
                    Ok(either_right(pair(new_counter, body_val)))
                } else {
                    Ok(either_left(acc))
                }
            }
            // `fix_step` derived step op. args = [body_result]. Current = in
            // (first step) or state (later). If body result equals current,
            // terminate (fixpoint); else continue with the result.
            "fix_step" => {
                let inc = incoming.unwrap_or(Value::Null);
                let in_val = pair_fst(&inc).cloned().unwrap_or(Value::Null);
                let state = pair_snd(&inc).cloned().unwrap_or(Value::Null);
                let current = if state.is_null() { in_val } else { state };
                let new_val = args.first().cloned().unwrap_or(Value::Null);
                if new_val == current {
                    Ok(either_left(new_val))
                } else {
                    Ok(either_right(new_val))
                }
            }
            // Variable-arity function application (mirrors the v2 tree-walk
            // `prim_call`). The `call` node's children are
            // `[callee_idx, arg_idx, ...]`; evaluate child 0 as the callee,
            // the rest as the argument list, and dispatch via `apply_app`.
            "call" => self.prim_v2_call(node),
            // `bind` op: arrow-calculus env extension (ADR-0052). Mirrors the
            // v2 tree-walk's `prim_bind`. The `bind` node's payload has
            // `name` and `children: [value_idx, body_idx]`; evaluate value,
            // extend env with name=value, evaluate body, pop scope.
            "bind" => self.prim_v2_bind(node),
            // ADR-0053: json node payload is the JSON template string.
            "json" => {
                let expr = node.payload.as_str().ok_or_else(|| {
                    LangError::Engine("json: payload must be a string".to_string())
                })?;
                serde_json::from_str(expr)
                    .map_err(|e| LangError::Engine(format!("json: parse error: {}", e)))
            }
            _ => Err(LangError::Engine(format!("unknown v2 native op: {}", op))),
        }
    }

    /// `call` native op: variable-arity function application. Mirrors the
    /// v2 tree-walk's `prim_call`. The `call` node's payload has
    /// `children: [callee_idx, arg_idx, ...]`; evaluate child 0 as the
    /// callee, the remaining children as the argument list, and dispatch
    /// via [`apply_app`](Self::apply_app).
    fn prim_v2_call(&mut self, node: &AstNode) -> Result<Value, LangError> {
        let children = node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                LangError::Engine("call: expected children array in payload".to_string())
            })?;
        let callee_idx = children
            .first()
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LangError::Engine("call: missing callee child".to_string()))?
            as usize;

        // ADR-0053: json{...}(args) — when the callee is a `json` node,
        // perform placeholder substitution ($0, $1, …) into the JSON template
        // before parsing.
        if let Some(callee_node) = self.ast.get(AstRef::new(callee_idx)) {
            if callee_node.kind == "json" {
                let template = callee_node.payload.as_str().ok_or_else(|| {
                    LangError::Engine("json: payload must be a string".to_string())
                })?;
                let mut s = template.to_string();
                for (i, entry) in children[1..].iter().enumerate() {
                    let idx = entry.as_u64().ok_or_else(|| {
                        LangError::Engine("call: child index must be a number".to_string())
                    })? as usize;
                    let val = self.exec_node(AstRef::new(idx))?;
                    let replacement = serde_json::to_string(&val)
                        .map_err(|e| LangError::Engine(format!("json: {}", e)))?;
                    s = s.replace(&format!("${}", i), &replacement);
                }
                return serde_json::from_str(&s)
                    .map_err(|e| LangError::Engine(format!("json: parse error: {}", e)));
            }
        }

        let callee = self.exec_node(AstRef::new(callee_idx))?;
        let mut arg_vals = Vec::with_capacity(children.len().saturating_sub(1));
        for entry in &children[1..] {
            let idx = entry.as_u64().ok_or_else(|| {
                LangError::Engine("call: child index must be a number".to_string())
            })? as usize;
            arg_vals.push(self.exec_node(AstRef::new(idx))?);
        }
        self.apply_app(&callee, &arg_vals)
    }

    /// `bind` native op: arrow-calculus env extension (ADR-0052). Mirrors
    /// the v2 tree-walk's `prim_bind`. The `bind` node's payload has
    /// `name` and `children: [value_idx, body_idx]`. Evaluates the value
    /// child, pushes a scope with `name = value`, evaluates the body child
    /// in the extended env, then pops the scope.
    fn prim_v2_bind(&mut self, node: &AstNode) -> Result<Value, LangError> {
        let name = node
            .payload
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LangError::Engine("bind: missing name".to_string()))?;
        let children = node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                LangError::Engine("bind: expected children array in payload".to_string())
            })?;
        let value_idx = children
            .first()
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LangError::Engine("bind: missing value child".to_string()))?
            as usize;
        let body_idx = children
            .get(1)
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LangError::Engine("bind: missing body child".to_string()))?
            as usize;
        // Evaluate the value in the current env.
        let value = self.exec_node(AstRef::new(value_idx))?;
        // Extend env with name = value, evaluate body, pop.
        self.scopes.push_scope();
        self.scopes.define(name, value);
        let result = self.exec_node(AstRef::new(body_idx));
        self.scopes.pop_scope();
        result
    }

    /// Apply an `App` callee: host-function dispatch (string callee) or
    /// closure application. Mirrors the v2 tree-walk's `apply_app` and v1's
    /// `apply_closure`.
    fn apply_app(&mut self, callee: &Value, args: &[Value]) -> Result<Value, LangError> {
        // Host-function dispatch: a string callee naming a registered host fn.
        if let Some(name) = callee.as_str()
            && let Some(modules) = self.modules
            && let Some(func) = modules.lookup(name)
        {
            let mut pctx = handl_platform::EvalContext::new();
            return func
                .call(args, &mut pctx)
                .map_err(|e| LangError::Engine(e.to_string()));
        }
        // Closure application.
        let obj = callee
            .as_object()
            .ok_or_else(|| LangError::Engine("callee is not an arrow-value".to_string()))?;
        if !obj
            .get("__arrow__")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return Err(LangError::Engine("callee is not an arrow-value".to_string()));
        }
        let params: Vec<String> = obj
            .get("params")
            .and_then(|v| v.as_array())
            .ok_or_else(|| LangError::Engine("arrow-value missing params".to_string()))?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(String::from)
                    .ok_or_else(|| LangError::Engine("arrow-value param is not a string".to_string()))
            })
            .collect::<Result<_, _>>()?;
        let body_idx = obj
            .get("body")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LangError::Engine("arrow-value missing body".to_string()))?;
        let body_ref = AstRef::new(body_idx as usize);
        let captured_env: HashMap<String, Value> = obj
            .get("env")
            .and_then(|v| v.as_object())
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        if params.len() != args.len() {
            return Err(LangError::Engine(format!(
                "arrow-value expects {} args, got {}",
                params.len(),
                args.len()
            )));
        }
        let saved = std::mem::replace(&mut self.scopes, ScopeStack::new());
        self.scopes.scopes[0] = captured_env;
        self.scopes.push_scope();
        for (param, arg) in params.iter().zip(args.iter()) {
            self.scopes.define(param, arg.clone());
        }
        let result = self.exec_node(body_ref);
        self.scopes = saved;
        result
    }
}

/// Whether a native op uses the `incoming` value (as opposed to only args).
///
/// Ops that use `incoming` with 0 args pop the incoming value from the stack
/// (it's the value flowing through the arrow). Ops that don't use `incoming`
/// (like `closure`) don't pop anything for a 0-arg call.
fn op_uses_incoming(op: &str) -> bool {
    matches!(
        op,
        "id" | "tag" | "is_pos" | "dec" | "inc" | "half_floor" | "inc_fst"
            | "countdown_step" | "infinite_step"
    )
}

/// Whether a native op is a handler-interpreted derived step op
/// (ADR-0052 §"Derived forms"). These ops carry sub-scheme `Arr.args`
/// compiled inline *and* consume the loop pair `(in, state)` that the
/// enclosing `Loop` combinator left on the operand stack below those args.
/// The `NativeOp` handler pops the args first, then pops the pair as
/// `incoming`, before dispatching to [`VmFrame::apply_native_v2`].
///
/// Only `loop_step` (the ergonomic `loop` surface lowering) is handled here
/// — its body proc arg is a closure evaluated once, then applied per
/// iteration via [`VmFrame::apply_app`]. The remaining derived step ops
/// (`while_step`/`for_step`/`fix_step`) use the [`Instr::DerivedStepOp`]
/// instruction instead, which re-evaluates their cond/body sub-programs
/// per iteration with the current value as incoming.
fn op_is_derived_step(op: &str) -> bool {
    matches!(op, "loop_step")
}

// ---------------------------------------------------------------------------
// Numeric op helpers (mirror the v2 tree-walk)
// ---------------------------------------------------------------------------

/// Binary numeric op helper. Preserves int-int arithmetic (matching v1).
fn bin_num<F>(args: &[Value], f: F) -> Result<Value, LangError>
where
    F: FnOnce(f64, f64) -> f64,
{
    let a = as_num(&args[0])?;
    let b = as_num(&args[1])?;
    match (a, b) {
        (Num::Int(a), Num::Int(b)) => Ok(Value::from(f(a as f64, b as f64) as i64)),
        (a, b) => Ok(Value::from(f(a.to_f64(), b.to_f64()))),
    }
}

/// Numeric comparison helper.
fn cmp_num<F>(args: &[Value], f: F) -> Result<Value, LangError>
where
    F: FnOnce(f64, f64) -> bool,
{
    let a = as_num(&args[0])?;
    let b = as_num(&args[1])?;
    Ok(Value::Bool(f(a.to_f64(), b.to_f64())))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{Ast, AstNode, EvalSchemeDef, Language, Scheme, SchemeVersion};
    use handl_platform::{EvalContext, Function, Module, ModuleLookup};
    use handl_plugin_module::ModuleRegistry;
    use serde_json::json;
    use std::rc::Rc;

    /// Build a simple AST for testing.
    fn make_ast(nodes: Vec<AstNode>) -> Ast {
        let mut ast = Ast::new();
        for n in nodes {
            ast.push(n);
        }
        ast
    }

    /// A dummy node with an empty-object payload.
    fn dummy_node() -> AstNode {
        AstNode::new("test").with_payload(json!({}))
    }

    /// Evaluate a standalone v2 scheme with a fresh frame and dummy node.
    fn eval_standalone(scheme: &Scheme) -> Result<Value, LangError> {
        let ast = make_ast(vec![dummy_node()]);
        let lang = Language::new();
        let node = ast.at(AstRef::new(0)).unwrap();
        let instrs = compile(scheme, node)?;
        let mut frame = VmFrame::new(&ast, &lang, None);
        frame.exec_instrs(&instrs, node, None)
    }

    // 1. arr const

    #[test]
    fn arr_const_returns_literal() {
        let s = Scheme::arr_native_lit("const", vec![], json!(42));
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn arr_const_null_when_no_literal() {
        let s = Scheme::arr_native("const", vec![]);
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, Value::Null);
    }

    // 2. arr proj_env

    #[test]
    fn arr_proj_env_bound_lookup() {
        let s = Scheme::arr_native_lit("proj_env", vec![], "x");
        let ast = make_ast(vec![dummy_node()]);
        let lang = Language::new();
        let node = ast.at(AstRef::new(0)).unwrap();
        let instrs = compile(&s, node).unwrap();
        let mut frame = VmFrame::new(&ast, &lang, None);
        frame.define_global("x", json!(7));
        let result = frame.exec_instrs(&instrs, node, None).unwrap();
        assert_eq!(result, json!(7));
    }

    #[test]
    fn arr_proj_env_unbound_returns_name() {
        let s = Scheme::arr_native_lit("proj_env", vec![], "unknown");
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, Value::from("unknown"));
    }

    // 3. Seq const >>> id

    #[test]
    fn seq_const_then_id() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], json!(5)),
            Scheme::arr_native("id", vec![]),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(5));
    }

    // 4. Bind shape

    #[test]
    fn bind_shape_extend_env_then_proj_env() {
        let s = Scheme::seq(
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], json!(5)),
                Scheme::arr_native_lit("extend_env", vec![], "x"),
            ),
            Scheme::arr_native_lit("proj_env", vec![], "x"),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(5));
    }

    // 5. First on a pair

    #[test]
    fn first_id_on_pair() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], pair(json!(3), json!(7))),
            Scheme::first(Scheme::arr_native("id", vec![])),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, pair(json!(3), json!(7)));
    }

    #[test]
    fn first_const_on_pair() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], pair(json!(3), json!(7))),
            Scheme::first(Scheme::arr_native_lit("const", vec![], json!(99))),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, pair(json!(99), json!(7)));
    }

    // 6. Choice on Either

    #[test]
    fn choice_left() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], either_left(json!(5))),
            Scheme::choice(
                Scheme::arr_native_lit("const", vec![], json!(1)),
                Scheme::arr_native_lit("const", vec![], json!(2)),
            ),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn choice_right() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], either_right(json!(5))),
            Scheme::choice(
                Scheme::arr_native_lit("const", vec![], json!(1)),
                Scheme::arr_native_lit("const", vec![], json!(2)),
            ),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(2));
    }

    // 7. tag + Choice (Branch lowering)

    #[test]
    fn tag_choice_true_yields_then() {
        let s = Scheme::seq(
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], json!(true)),
                Scheme::arr_native("tag", vec![]),
            ),
            Scheme::choice(
                Scheme::arr_native_lit("const", vec![], "yes"),
                Scheme::arr_native_lit("const", vec![], "no"),
            ),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!("yes"));
    }

    #[test]
    fn tag_choice_false_yields_else() {
        let s = Scheme::seq(
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], json!(false)),
                Scheme::arr_native("tag", vec![]),
            ),
            Scheme::choice(
                Scheme::arr_native_lit("const", vec![], "yes"),
                Scheme::arr_native_lit("const", vec![], "no"),
            ),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!("no"));
    }

    // 8. Loop causal single-step

    #[test]
    fn loop_causal_single_step() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], json!(10)),
            Scheme::loop_feedback(Scheme::arr_native("inc_fst", vec![])),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(11));
    }

    // 8b. Loop multi-step (Either-based)

    #[test]
    fn loop_multi_step_either_countdown() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], pair(json!(42), json!(5))),
            Scheme::loop_feedback(Scheme::arr_native("countdown_step", vec![])),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn loop_multi_step_max_iterations_guard() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], json!(1)),
            Scheme::loop_feedback(Scheme::arr_native("infinite_step", vec![])),
        );
        let err = eval_standalone(&s).unwrap_err();
        assert!(matches!(err, LangError::Engine(ref m) if m.contains("max iterations")));
    }

    // 8c. `loop_step` derived op — the ergonomic `loop` surface lowering
    // (ADR-0052). These tests build the canonical `loop` lowering
    // `Seq(proj_child(0), Loop(loop_step(proj_child(1))))` by hand with a
    // real `proc` body, exercising param destructuring, `if`, arithmetic,
    // and object construction — the same shape `hnl` source parses to.

    /// Wire the eval schemes needed for the `loop` surface: `var`, arithmetic,
    /// `if`, `object`, `function` (closure), `proc`, and `loop`.
    fn loop_lang() -> Language {
        let mut lang = Language::new();
        // var -> proj_env $payload (looks up the var name in the env)
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::arr_native_lit("proj_env", vec![], Value::String("$payload".into())),
            ))
            .unwrap();
        // binary.eq / sub / mul -> native op over the two children
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
                            Scheme::arr_native_lit("proj_child", vec![], json!(0u64)),
                            Scheme::arr_native_lit("proj_child", vec![], json!(1u64)),
                        ],
                    ),
                ))
                .unwrap();
        }
        // if -> Seq(Seq(proj_child(0), tag), Choice(proj_child(1), proj_child(2)))
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("proj_child", vec![], json!(0u64)),
                        Scheme::arr_native("tag", vec![]),
                    ),
                    Scheme::choice(
                        Scheme::arr_native_lit("proj_child", vec![], json!(1u64)),
                        Scheme::arr_native_lit("proj_child", vec![], json!(2u64)),
                    ),
                ),
            ))
            .unwrap();
        // object -> native `object` prim (reads the node's children payload)
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.object",
                "object",
                Scheme::arr_native("object", vec![]),
            ))
            .unwrap();
        // function -> closure prim
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.function",
                "function",
                Scheme::arr_native("closure", vec![]),
            ))
            .unwrap();
        // proc -> proj_child(0)
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.proc",
                "proc",
                Scheme::arr_native_lit("proj_child", vec![], json!(0u64)),
            ))
            .unwrap();
        // loop -> Seq(proj_child(0), Loop(loop_step(proj_child(1))))
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.loop",
                "loop",
                Scheme::seq(
                    Scheme::arr_native_lit("proj_child", vec![], json!(0u64)),
                    Scheme::loop_feedback(Scheme::arr_native(
                        "loop_step",
                        vec![Scheme::arr_native_lit("proj_child", vec![], json!(1u64))],
                    )),
                ),
            ))
            .unwrap();
        lang
    }

    /// `loop 42 (proc (x) -> { out: x })` — single-step terminate,
    /// one param (no destructuring), scalar state. Expected: 42.
    #[test]
    fn loop_step_single_step_terminate() {
        let lang = loop_lang();
        // Arena:
        //   0: lit "out"      1: var "x"
        //   2: entry [0, 1]   3: object {children: [2]}
        //   4: function {params: ["x"], body: 3}
        //   5: proc {children: [4]}
        //   6: lit 42         7: loop {children: [6, 5]}
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!("out")),            // 0
            AstNode::new("var").with_payload(json!("x")),              // 1
            AstNode::new("entry").with_payload(json!({"children": [0, 1]})), // 2
            AstNode::new("object").with_payload(json!({"children": [2]})),   // 3
            AstNode::new("function").with_payload(json!({"params": ["x"], "body": 3})), // 4
            AstNode::new("proc").with_payload(json!({"children": [4]})),    // 5
            AstNode::new("lit").with_payload(json!(42)),                    // 6
            AstNode::new("loop").with_payload(json!({"children": [6, 5]})), // 7
        ]);
        let mut frame = VmFrame::new(&ast, &lang, None);
        let result = frame.exec_node(AstRef::new(7)).unwrap();
        assert_eq!(result, json!(42));
    }

    /// `loop { n: 3, acc: 1 }(proc (n, acc) -> if n == 0 then { out: acc }
    /// else { state: { n: n - 1, acc: acc * n } })` — multi-step with param
    /// destructuring, `if`, arithmetic, nested object construction.
    /// Computes 3! = 6.
    #[test]
    fn loop_step_factorial_destructured() {
        let lang = loop_lang();
        // Arena (see `loop_lang` for the schemes):
        //   0: lit "n"        1: lit "acc"      2: lit "out"    3: lit "state"
        //   4: lit 0          5: lit 1
        //   6: var "n"        7: var "acc"
        //   8: binary.eq   [6, 4]    (n == 0)
        //   9: binary.sub  [6, 5]    (n - 1)
        //  10: binary.mul  [7, 6]    (acc * n)
        //  11: entry [0, 9]          (n: n - 1)
        //  12: entry [1, 10]         (acc: acc * n)
        //  13: object {children: [11, 12]}   (inner state {n, acc})
        //  14: entry [2, 7]          (out: acc)
        //  15: object {children: [14]}       (then {out: acc})
        //  16: entry [3, 13]         (state: inner)
        //  17: object {children: [16]}       (else {state: {...}})
        //  18: if {children: [8, 15, 17]}
        //  19: function {params: ["n","acc"], body: 18}
        //  20: proc {children: [19]}
        //  21: lit {n: 3, acc: 1}
        //  22: loop {children: [21, 20]}
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!("n")),              // 0
            AstNode::new("lit").with_payload(json!("acc")),            // 1
            AstNode::new("lit").with_payload(json!("out")),            // 2
            AstNode::new("lit").with_payload(json!("state")),          // 3
            AstNode::new("lit").with_payload(json!(0)),                // 4
            AstNode::new("lit").with_payload(json!(1)),                // 5
            AstNode::new("var").with_payload(json!("n")),              // 6
            AstNode::new("var").with_payload(json!("acc")),            // 7
            AstNode::new("binary.eq").with_payload(json!({"children": [6, 4]})),   // 8
            AstNode::new("binary.sub").with_payload(json!({"children": [6, 5]})),  // 9
            AstNode::new("binary.mul").with_payload(json!({"children": [7, 6]})),  // 10
            AstNode::new("entry").with_payload(json!({"children": [0, 9]})),       // 11
            AstNode::new("entry").with_payload(json!({"children": [1, 10]})),      // 12
            AstNode::new("object").with_payload(json!({"children": [11, 12]})),    // 13
            AstNode::new("entry").with_payload(json!({"children": [2, 7]})),       // 14
            AstNode::new("object").with_payload(json!({"children": [14]})),        // 15
            AstNode::new("entry").with_payload(json!({"children": [3, 13]})),      // 16
            AstNode::new("object").with_payload(json!({"children": [16]})),        // 17
            AstNode::new("if").with_payload(json!({"children": [8, 15, 17]})),     // 18
            AstNode::new("function").with_payload(json!({"params": ["n", "acc"], "body": 18})), // 19
            AstNode::new("proc").with_payload(json!({"children": [19]})),          // 20
            AstNode::new("lit").with_payload(json!({"n": 3, "acc": 1})),           // 21
            AstNode::new("loop").with_payload(json!({"children": [21, 20]})),      // 22
        ]);
        let mut frame = VmFrame::new(&ast, &lang, None);
        let result = frame.exec_node(AstRef::new(22)).unwrap();
        assert_eq!(result, json!(6));
    }

    // 8d. `while_step` / `for_step` / `fix_step` derived step ops.
    // These use the `DerivedStepOp` instruction which re-evaluates arg
    // sub-programs per iteration with the current value as incoming.

    /// `while` loop: countdown from 5 to 0 using `is_pos`/`dec` ops.
    /// `while_loop(is_pos, dec)` applied to 5.
    #[test]
    fn while_step_countdown() {
        // Scheme: Seq(const 5, Loop(while_step(is_pos, dec)))
        // is_pos: native op, 0 args, uses incoming (checks > 0)
        // dec: native op, 0 args, uses incoming (subtracts 1)
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], json!(5)),
            Scheme::loop_feedback(Scheme::arr_native(
                "while_step",
                vec![
                    Scheme::arr_native("is_pos", vec![]),
                    Scheme::arr_native("dec", vec![]),
                ],
            )),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(0));
    }

    /// `while` loop with zero iterations: cond is immediately false.
    #[test]
    fn while_step_zero_iterations() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], json!(0)),
            Scheme::loop_feedback(Scheme::arr_native(
                "while_step",
                vec![
                    Scheme::arr_native("is_pos", vec![]),
                    Scheme::arr_native("dec", vec![]),
                ],
            )),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(0));
    }

    /// `for` loop: accumulate 5 increments from 0. `for_loop(inc)` applied
    /// to pair(5, 0).
    #[test]
    fn for_step_accumulate() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], pair(json!(5), json!(0))),
            Scheme::loop_feedback(Scheme::arr_native(
                "for_step",
                vec![Scheme::arr_native("inc", vec![])],
            )),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(5));
    }

    /// `for` loop with zero iterations: counter is 0, return initial.
    #[test]
    fn for_step_zero_iterations() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], pair(json!(0), json!(99))),
            Scheme::loop_feedback(Scheme::arr_native(
                "for_step",
                vec![Scheme::arr_native("inc", vec![])],
            )),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(99));
    }

    /// `fix` loop: repeatedly halve 100 until fixpoint at 0.
    /// `fix_loop(half_floor)` applied to 100.
    #[test]
    fn fix_step_halve_to_zero() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], json!(100)),
            Scheme::loop_feedback(Scheme::arr_native(
                "fix_step",
                vec![Scheme::arr_native("half_floor", vec![])],
            )),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(0));
    }

    /// `fix` loop already at fixpoint: 0 → 0.
    #[test]
    fn fix_step_already_at_fixpoint() {
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], json!(0)),
            Scheme::loop_feedback(Scheme::arr_native(
                "fix_step",
                vec![Scheme::arr_native("half_floor", vec![])],
            )),
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(0));
    }

    // 9. App with a closure value

    #[test]
    fn app_closure_applies_body() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::arr_native_lit("proj_env", vec![], "$payload"),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("test").with_payload(json!({})),
        ]);
        let closure = json!({
            "__arrow__": true,
            "params": ["x"],
            "body": 0,
            "env": {},
        });
        let s = Scheme::app(
            Scheme::arr_native_lit("const", vec![], closure),
            vec![Scheme::arr_native_lit("const", vec![], json!(41))],
        );
        let node = ast.at(AstRef::new(1)).unwrap();
        let instrs = compile(&s, node).unwrap();
        let mut frame = VmFrame::new(&ast, &lang, None);
        let result = frame.exec_instrs(&instrs, node, None).unwrap();
        assert_eq!(result, json!(41));
    }

    // 10. Arithmetic parity

    #[test]
    fn native_add() {
        let s = Scheme::arr_native(
            "add",
            vec![
                Scheme::arr_native_lit("const", vec![], json!(2)),
                Scheme::arr_native_lit("const", vec![], json!(3)),
            ],
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn native_eq() {
        let s = Scheme::arr_native(
            "eq",
            vec![
                Scheme::arr_native_lit("const", vec![], json!(2)),
                Scheme::arr_native_lit("const", vec![], json!(2)),
            ],
        );
        let result = eval_standalone(&s).unwrap();
        assert_eq!(result, json!(true));
    }

    // 11. Unknown op

    #[test]
    fn unknown_native_op_errors() {
        let s = Scheme::arr_native("bogus", vec![]);
        let err = eval_standalone(&s).unwrap_err();
        assert!(matches!(err, LangError::Engine(ref m) if m.contains("unknown v2 native op")));
    }

    // 12. Foreign dispatch

    /// A host function that doubles its numeric argument.
    struct DoubleFn;

    impl Function for DoubleFn {
        fn call(
            &self,
            args: &[Value],
            _ctx: &mut EvalContext,
        ) -> Result<Value, handl_platform::EvalError> {
            Ok(Value::from(args[0].as_i64().unwrap() * 2))
        }
    }

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

    fn double_registry() -> Rc<dyn ModuleLookup> {
        Rc::new(ModuleRegistry::new().with(DoubleModule))
    }

    #[test]
    fn foreign_arr_dispatches_to_module() {
        let s = Scheme::arr_foreign(
            "double",
            vec![Scheme::arr_native_lit("const", vec![], json!(21))],
        );
        let ast = make_ast(vec![dummy_node()]);
        let lang = Language::new();
        let node = ast.at(AstRef::new(0)).unwrap();
        let instrs = compile(&s, node).unwrap();
        let reg = double_registry();
        let mut frame = VmFrame::new(&ast, &lang, Some(&reg));
        let result = frame.exec_instrs(&instrs, node, None).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn foreign_arr_no_modules_errors() {
        let s = Scheme::arr_foreign(
            "double",
            vec![Scheme::arr_native_lit("const", vec![], json!(21))],
        );
        let err = eval_standalone(&s).unwrap_err();
        assert!(matches!(err, LangError::Engine(ref m) if m.contains("unregistered foreign")));
    }

    // 13. exec_node (v2 scheme from lang.eval)

    #[test]
    fn exec_node_add() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::arr_native(
                    "add",
                    vec![
                        Scheme::arr_native_lit("proj_child", vec![], json!(0u64)),
                        Scheme::arr_native_lit("proj_child", vec![], json!(1u64)),
                    ],
                ),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("lit").with_payload(json!(3)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
        ]);
        let mut frame = VmFrame::new(&ast, &lang, None);
        let result = frame.exec_node(AstRef::new(2)).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn exec_node_branch_true() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("proj_child", vec![], json!(0u64)),
                        Scheme::arr_native("tag", vec![]),
                    ),
                    Scheme::choice(
                        Scheme::arr_native_lit("proj_child", vec![], json!(1u64)),
                        Scheme::arr_native_lit("proj_child", vec![], json!(2u64)),
                    ),
                ),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(true)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let mut frame = VmFrame::new(&ast, &lang, None);
        let result = frame.exec_node(AstRef::new(3)).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn exec_node_branch_false() {
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.if",
                "if",
                Scheme::seq(
                    Scheme::seq(
                        Scheme::arr_native_lit("proj_child", vec![], json!(0u64)),
                        Scheme::arr_native("tag", vec![]),
                    ),
                    Scheme::choice(
                        Scheme::arr_native_lit("proj_child", vec![], json!(1u64)),
                        Scheme::arr_native_lit("proj_child", vec![], json!(2u64)),
                    ),
                ),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(false)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("if").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let mut frame = VmFrame::new(&ast, &lang, None);
        let result = frame.exec_node(AstRef::new(3)).unwrap();
        assert_eq!(result, json!(2));
    }

    // 14. No-scheme fallback

    #[test]
    fn no_scheme_returns_payload() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("custom").with_payload(json!("hello"))]);
        let mut frame = VmFrame::new(&ast, &lang, None);
        let result = frame.exec_node(AstRef::new(0)).unwrap();
        assert_eq!(result, json!("hello"));
    }

    // 15. Version check

    #[test]
    fn v2_engine_accepts_v2_lang() {
        assert!(SchemeVersion::v2().supports(&SchemeVersion::v2()));
    }
}
