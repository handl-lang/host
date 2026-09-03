//! # handl-plugin-engine-eval-tree-walk
//!
//! The **reference interpreter** for the eval operation (ADR-0044 §3, §4,
//! ADR-0052). Interprets the arrow-calculus [`handl_language::Scheme`]
//! kernel directly via tree-walking: it looks up
//! `lang.eval[node.kind]` → [`handl_language::EvalSchemeDef`] →
//! [`Scheme`], and interprets the scheme. It is entirely data-driven.
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
//! [`Scheme::Loop`] implements **causal/strict** feedback (ADR-0052). The
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
    pub use crate::{ScopeStack, TreeWalkEngine as V2Interpreter};
}

use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

use handl_language::{ArrSource, Ast, AstNode, AstRef, LangError, Language, Scheme, Value};
use handl_platform::{ModuleLookup, ValueCallable};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_eval::pattern_match::{self, PatternMatchCtx};

// Thread-local storage for the current AST and Language, so the
// ValueCallable impl can access them when host functions (e.g. fn.map)
// call arrow-values via ctx.try_call_value.
thread_local! {
    static CURRENT_AST: RefCell<Option<Rc<Ast>>> = RefCell::new(None);
    static CURRENT_LANG: RefCell<Option<Rc<Language>>> = RefCell::new(None);
    static CURRENT_MODULES: RefCell<Option<Rc<dyn ModuleLookup>>> = RefCell::new(None);
}

// ---------------------------------------------------------------------------
// ScopeStack
// ---------------------------------------------------------------------------

/// A scope stack: `Vec<HashMap<String, Value>>` searched innermost-first.
///
/// Used by the tree-walk interpreter for `extend_env` binding and closure
/// capture/swap-in. The `scopes` field is `pub(crate)` so the engine can
/// perform the closure-env swap-in.
pub struct ScopeStack {
    /// The stack of scopes, innermost last. `pub(crate)` so the engine
    /// can perform the closure-env swap-in.
    pub(crate) scopes: Vec<HashMap<String, Value>>,
}

#[allow(clippy::new_without_default)]
impl ScopeStack {
    /// Create a new scope stack with a single empty root scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    /// Look up a variable, searching innermost scope first.
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    /// Define a variable in the current (innermost) scope.
    pub fn define(&mut self, name: &str, value: Value) {
        let last = self.scopes.last_mut().expect("scope stack is never empty");
        last.insert(name.to_string(), value);
    }

    /// Push a new empty scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pop the innermost scope.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Capture all visible variables into a single flat map.
    pub fn capture(&self) -> HashMap<String, Value> {
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
        self.env.define(name, value);
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
/// A tree-walk handler that interprets [`Scheme`] directly. Construct
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
}

impl TreeWalkEngine {
    /// Create a new tree-walk engine with no module registry.
    pub fn new() -> Self {
        Self {
            modules: None,
            globals: HashMap::new(),
        }
    }

    /// Attach a module registry (builder style).
    pub fn with_modules(mut self, modules: Rc<dyn ModuleLookup>) -> Self {
        self.modules = Some(modules);
        self
    }

    /// Inject a global variable visible to all eval calls (builder style).
    /// Used by the host to inject `self` (the current language value) and
    /// other ambient values (ADR-0041 Phase 3).
    pub fn with_global(mut self, name: impl Into<String>, value: Value) -> Self {
        self.globals.insert(name.into(), value);
        self
    }

    /// Interpret a given [`Scheme`] in the context of `current_node`.
    ///
    /// This is the core handler entry point. `incoming` is the value fed
    /// by an enclosing `Seq`/`First`/`Choice`/`Loop`; top-level calls
    /// pass `None`.
    ///
    /// # Scope discipline
    ///
    /// `extend_env` pushes a scope and returns the value flowing through.
    /// The scope's lifetime follows the arrow pipeline: it is popped by
    /// the enclosing [`Scheme::Seq`] whose `fst` (transitively) contained
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
        scheme: &Scheme,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<Value>,
    ) -> Result<Value, LangError> {
        let (val, did_push) = self.eval_inner(
            scheme, ast, node_ref, lang, env, current_node, incoming,
        )?;
        // A bare extend_env (no enclosing Seq to pop it) is balanced here.
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
        scheme: &Scheme,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<Value>,
    ) -> Result<(Value, bool), LangError> {
        match scheme {
            Scheme::Arr {
                op,
                args,
                source,
                lit,
            } => self.eval_arr(
                op, args, source, lit.as_ref(),
                ast, node_ref, lang, env, current_node, incoming,
            ),
            Scheme::Seq { fst, snd } => {
                let (fst_val, fst_pushed) = self.eval_inner(
                    fst, ast, node_ref, lang, env, current_node, incoming,
                )?;
                let (snd_val, snd_pushed) = self.eval_inner(
                    snd, ast, node_ref, lang, env, current_node, Some(fst_val),
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
            Scheme::First { inner } => {
                let inc = incoming.ok_or_else(|| {
                    LangError::Engine("First: missing incoming value".to_string())
                })?;
                let a = pair_fst(&inc)
                    .ok_or_else(|| {
                        LangError::Engine("First: incoming is not a pair".to_string())
                    })?
                    .clone();
                let b = pair_snd(&inc)
                    .ok_or_else(|| {
                        LangError::Engine("First: incoming is not a pair".to_string())
                    })?
                    .clone();
                let (inner_result, pushed) = self.eval_inner(
                    inner, ast, node_ref, lang, env, current_node, Some(a),
                )?;
                if pushed {
                    env.pop_scope();
                }
                Ok((pair(inner_result, b), false))
            }
            Scheme::Choice { l, r } => {
                let inc = incoming.ok_or_else(|| {
                    LangError::Engine("Choice: missing incoming value".to_string())
                })?;
                let (val, pushed) = if is_either_left(&inc) {
                    let v = either_value(&inc)
                        .ok_or_else(|| {
                            LangError::Engine("Choice: left Either missing value".to_string())
                        })?
                        .clone();
                    self.eval_inner(l, ast, node_ref, lang, env, current_node, Some(v))?
                } else if is_either_right(&inc) {
                    let v = either_value(&inc)
                        .ok_or_else(|| {
                            LangError::Engine("Choice: right Either missing value".to_string())
                        })?
                        .clone();
                    self.eval_inner(r, ast, node_ref, lang, env, current_node, Some(v))?
                } else {
                    return Err(LangError::Engine(
                        "Choice: incoming is not an Either".to_string(),
                    ));
                };
                if pushed {
                    env.pop_scope();
                }
                Ok((val, false))
            }
            Scheme::Loop { body } => {
                // Causal/strict feedback (ADR-0052 §"Feedback semantics").
                // Two modes, selected by the body's result shape (see the
                // module-level "Loop semantics" doc):
                //
                // 1. Single-step: body returns a pair (Out, NewState);
                //    Loop returns Out.
                // 2. Multi-step (Either-based): body returns an Either;
                //    left(out) terminates, right(new_state) re-runs with
                //    the new state. A max-iteration guard bounds the loop.
                let inc = incoming.unwrap_or(Value::Null);
                let mut state = Value::Null;
                const MAX_ITERS: usize = 100_000;
                for _ in 0..MAX_ITERS {
                    let input = pair(inc.clone(), state.clone());
                    let (result, pushed) = self.eval_inner(
                        body, ast, node_ref, lang, env, current_node, Some(input),
                    )?;
                    if pushed {
                        env.pop_scope();
                    }
                    if is_either_left(&result) {
                        let out = either_value(&result).cloned().ok_or_else(|| {
                            LangError::Engine("Loop: left Either missing value".to_string())
                        })?;
                        return Ok((out, false));
                    }
                    if is_either_right(&result) {
                        state = either_value(&result).cloned().ok_or_else(|| {
                            LangError::Engine("Loop: right Either missing value".to_string())
                        })?;
                        continue;
                    }
                    // Not an Either → single-step mode: expect a pair
                    // (Out, State) and return Out.
                    let out = pair_fst(&result).ok_or_else(|| {
                        LangError::Engine(
                            "Loop: body did not return a pair or Either".to_string(),
                        )
                    })?;
                    return Ok((out.clone(), false));
                }
                Err(LangError::Engine("Loop: exceeded max iterations".to_string()))
            }
            Scheme::App { callee, args } => {
                let (callee_val, cpushed) = self.eval_inner(
                    callee, ast, node_ref, lang, env, current_node, incoming.clone(),
                )?;
                if cpushed {
                    env.pop_scope();
                }
                let mut arg_vals = Vec::with_capacity(args.len());
                for arg in args {
                    let (v, pushed) = self.eval_inner(
                        arg, ast, node_ref, lang, env, current_node, incoming.clone(),
                    )?;
                    if pushed {
                        env.pop_scope();
                    }
                    arg_vals.push(v);
                }
                // ADR-0060: unpack single array arg for variable-arity call.
                // When the App receives a single argument that is an array,
                // unpack it into individual arguments. This supports the
                // eval.call scheme where proj_rest packs all call args into
                // one array value.
                let actual_args: Vec<Value> = if arg_vals.len() == 1 && arg_vals[0].is_array() {
                    arg_vals[0].as_array().unwrap().iter().cloned().collect()
                } else {
                    arg_vals
                };
                let val = self.apply_app(&callee_val, &actual_args, ast, lang, env)?;
                Ok((val, false))
            }
            // `handle body with target = impl_` (ADR-0076): eliminate the
            // handler at compile time (rewriting every bare arr(target) in
            // the body to the impl graph), then evaluate the resulting
            // handler-free scheme. Uses `eliminate_and_check` to surface
            // unhandled capabilities as a compile-time error.
            Scheme::Handle { .. } => {
                let eliminated = handl_language::eliminate_and_check(scheme)
                    .map_err(|e| LangError::Engine(e.to_string()))?;
                self.eval_inner(
                    &eliminated, ast, node_ref, lang, env, current_node, incoming,
                )
            }
        }
    }

    /// Evaluate an `Arr` combinator. Returns the value and whether this
    /// Arr pushed a scope (`extend_env` pushes; the caller — `Seq` or
    /// `eval_scheme` — is responsible for popping it).
    #[allow(clippy::too_many_arguments)]
    fn eval_arr(
        &self,
        op: &str,
        args: &[Scheme],
        source: &ArrSource,
        lit: Option<&Value>,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<Value>,
    ) -> Result<(Value, bool), LangError> {
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
                op, args, ast, node_ref, lang, env, current_node, incoming,
            );
        }
        // Evaluate sub-scheme args (each gets the same incoming context).
        let mut arg_vals = Vec::with_capacity(args.len());
        for arg in args {
            arg_vals.push(self.eval_scheme(
                arg, ast, node_ref, lang, env, current_node, incoming.clone(),
            )?);
        }
        match source {
            ArrSource::Native => {
                self.apply_native(op, &arg_vals, lit, ast, node_ref, lang, env, current_node, incoming)
            }
            ArrSource::Foreign(name) => {
                let modules = self.modules.as_ref().ok_or_else(|| {
                    LangError::Engine(format!("unregistered foreign: {}", name))
                })?;
                let func = modules.lookup(name).ok_or_else(|| {
                    LangError::Engine(format!("unregistered foreign: {}", name))
                })?;
                let mut pctx = handl_platform::EvalContext::new();
                pctx.set_value_callable(Rc::new(TreeWalkCallable));
                let val = func
                    .call(&arg_vals, &mut pctx)
                    .map_err(|e| LangError::Engine(e.to_string()))?;
                Ok((val, false))
            }
            ArrSource::DynamicForeign => {
                self.apply_native(op, &arg_vals, lit, ast, node_ref, lang, env, current_node, incoming)
            }
            ArrSource::Judgment => Err(LangError::Engine(
                "judgment ops are not supported by the tree-walk engine".to_string(),
            )),
        }
    }

    /// Evaluate a derived-pattern composite step op
    /// (`while_step`/`for_step`/`fix_step`). These are handler-interpreted
    /// derived operations (ADR-0052 §"Derived forms"): the `Scheme`
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
        raw_args: &[Scheme],
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<Value>,
    ) -> Result<(Value, bool), LangError> {
        let inc = incoming.unwrap_or(Value::Null);
        let in_val = pair_fst(&inc).cloned().unwrap_or(Value::Null);
        let state = pair_snd(&inc).cloned().unwrap_or(Value::Null);
        match op {
            "while_step" => {
                // args = [cond, body]. The current value is `in` on the
                // first step (state is Null) and `state` thereafter. Run
                // `cond` on it; if truthy, run `body` and continue with
                // its result; else terminate with the current value.
                let cond = raw_args.first().ok_or_else(|| {
                    LangError::Engine("while_step: missing cond sub-arrow".to_string())
                })?;
                let body = raw_args.get(1).ok_or_else(|| {
                    LangError::Engine("while_step: missing body sub-arrow".to_string())
                })?;
                let current = if state.is_null() { in_val } else { state };
                let cond_val = self.eval_scheme(
                    cond, ast, node_ref, lang, env, current_node, Some(current.clone()),
                )?;
                if is_truthy(&cond_val) {
                    let new_val = self.eval_scheme(
                        body, ast, node_ref, lang, env, current_node, Some(current),
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
                    LangError::Engine("for_step: missing body sub-arrow".to_string())
                })?;
                let (counter, acc) = if state.is_null() {
                    let count = pair_fst(&in_val).cloned().ok_or_else(|| {
                        LangError::Engine(
                            "for_step: input must be pair(count, initial) when state is Null"
                                .to_string(),
                        )
                    })?;
                    let initial = pair_snd(&in_val).cloned().unwrap_or(Value::Null);
                    (count, initial)
                } else {
                    let c = pair_fst(&state).cloned().ok_or_else(|| {
                        LangError::Engine("for_step: state must be pair(counter, acc)".to_string())
                    })?;
                    let a = pair_snd(&state).cloned().unwrap_or(Value::Null);
                    (c, a)
                };
                let count_n = as_num(&counter)?;
                if count_n > Num::Int(0) {
                    let new_acc = self.eval_scheme(
                        body, ast, node_ref, lang, env, current_node, Some(acc),
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
                    LangError::Engine("fix_step: missing body sub-arrow".to_string())
                })?;
                let current = if state.is_null() { in_val } else { state };
                let new_val = self.eval_scheme(
                    body, ast, node_ref, lang, env, current_node, Some(current.clone()),
                )?;
                if new_val == current {
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
                let body_proc = raw_args.first().ok_or_else(|| {
                    LangError::Engine("loop_step: missing body proc".to_string())
                })?;
                let current = if state.is_null() { in_val } else { state };
                let closure_val = self.eval_scheme(
                    body_proc, ast, node_ref, lang, env, current_node, None,
                )?;
                // Check if destructuring applies: closure has multiple
                // params and current state is an object.
                let args: Vec<Value> = if let Some(obj_state) = current.as_object() {
                    if let Some(closure_obj) = closure_val.as_object() {
                        if let Some(params) = closure_obj
                            .get("params")
                            .and_then(|v| v.as_array())
                        {
                            if params.len() > 1 {
                                // Destructure: bind each param to the
                                // corresponding field by name.
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
                        }
                    } else {
                        vec![current]
                    }
                } else {
                    vec![current]
                };
                let result = self.apply_app(
                    &closure_val, &args, ast, lang, env,
                )?;
                // v0.1: `return v` produces a bare value (not wrapped in { out: v }).
                // Recognize bare values as exit signals, and { state: v } as continue.
                // { out: v } is accepted for backward compatibility.
                if let Some(obj) = result.as_object() {
                    if let Some(new_state) = obj.get("state") {
                        // Continue with new state.
                        Ok((either_right(new_state.clone()), false))
                    } else if let Some(out) = obj.get("out") {
                        // Exit with out value (backward compat with { out: v } protocol).
                        Ok((either_left(out.clone()), false))
                    } else {
                        // Object without out/state keys — treat as exit value (return).
                        Ok((either_left(result.clone()), false))
                    }
                } else {
                    // Non-object (number, string, bool, null, array) — exit with the value.
                    Ok((either_left(result.clone()), false))
                }
            }
            _ => Err(LangError::Engine(format!("unknown derived step op: {}", op))),
        }
    }

    /// Apply a native `arr` op. Returns the value and whether a scope was
    /// pushed (`extend_env` pushes).
    #[allow(clippy::too_many_arguments)]
    fn apply_native(
        &self,
        op: &str,
        args: &[Value],
        lit: Option<&Value>,
        ast: &Ast,
        _node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<Value>,
    ) -> Result<(Value, bool), LangError> {
        match op {
            "const" => Ok((lit.cloned().unwrap_or(Value::Null), false)),
            "id" => {
                // Identity: return the incoming value, or the single arg.
                Ok((incoming.or_else(|| args.first().cloned()).unwrap_or(Value::Null), false))
            }
            "proj_env" => {
                let name = lit
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        LangError::Engine("proj_env: missing name literal".to_string())
                    })?;
                let var_name = if name == "$payload" {
                    current_node.payload.as_str().ok_or_else(|| {
                        LangError::Engine(
                            "expected string payload for $payload proj_env".to_string(),
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
                    .unwrap_or_else(|| Value::from(var_name));
                Ok((val, false))
            }
            "proj_child" => {
                let idx = lit
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        LangError::Engine("proj_child: missing index literal".to_string())
                    })?;
                let children = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::Engine("proj_child: expected children array in payload".to_string())
                    })?;
                let arena_idx = children
                    .get(idx as usize)
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        LangError::Engine(format!("proj_child: child {} not found", idx))
                    })?;
                let target_ref = AstRef::new(arena_idx as usize);
                let val = self.eval_node(ast, target_ref, lang, env)?;
                Ok((val, false))
            }
            "extend_env" => {
                let name = lit
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        LangError::Engine("extend_env: missing name literal".to_string())
                    })?;
                let bind_name = if name == "$payload" {
                    current_node
                        .payload
                        .get("name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            LangError::Engine(
                                "expected name in payload for $payload extend_env".to_string(),
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
                let val = incoming.unwrap_or(Value::Null);
                env.define(&bind_name, val.clone());
                Ok((val, true))
            }
            "tag" => {
                // Branch lowering: truthy → left Either, falsy → right.
                let v = incoming.unwrap_or_else(|| {
                    args.first().cloned().unwrap_or(Value::Null)
                });
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
                    let param_node = ast.at(AstRef::new(param_idx))?;
                    let param_names: Vec<String> = if param_node.kind == "paren"
                        || param_node.kind == "array"
                    {
                        let param_children = param_node
                            .payload
                            .get("children")
                            .and_then(|c| c.as_array())
                            .ok_or_else(|| {
                                LangError::Engine(
                                    "closure: params missing children".to_string(),
                                )
                            })?;
                        let mut names = Vec::with_capacity(param_children.len());
                        for pc in param_children {
                            let pidx = pc.as_u64().ok_or_else(|| {
                                LangError::Engine(
                                    "closure: param index must be a number".to_string(),
                                )
                            })? as usize;
                            let pnode = ast.at(AstRef::new(pidx))?;
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
                let captured = env.capture();
                let closure = serde_json::json!({
                    "__arrow__": true,
                    "params": params,
                    "body": body,
                    "env": captured,
                });
                Ok((closure, false))
            }
            "array" => {
                // Two payload shapes:
                // - Rich: `{"elements": [v1, v2, ...]}` — literal values.
                // - Pratt-parsed: `{"children": [elem_idx, ...]}` — each
                //   element is an arena index to evaluate.
                if let Some(elements) = current_node.payload.get("elements") {
                    Ok((elements.clone(), false))
                } else if let Some(children) = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                {
                    let mut result = Vec::with_capacity(children.len());
                    for entry in children {
                        let idx = entry.as_u64().ok_or_else(|| {
                            LangError::Engine(
                                "array: element index must be a number".to_string(),
                            )
                        })? as usize;
                        let child_ref = AstRef::new(idx);
                        // Spread: if the child is a `rest` node (from
                        // `...expr` syntax), flatten its value into the
                        // array if it's an array.
                        if ast.get(child_ref).map(|n| n.kind.as_str()) == Some("rest") {
                            let val = self.eval_node(ast, child_ref, lang, env)?;
                            if let Some(arr) = val.as_array() {
                                result.extend(arr.iter().cloned());
                            } else {
                                result.push(val);
                            }
                        } else {
                            result.push(
                                self.eval_node(ast, child_ref, lang, env)?,
                            );
                        }
                    }
                    Ok((Value::Array(result), false))
                } else {
                    Err(LangError::Engine(
                        "array: missing elements/children in payload".to_string(),
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
                    Ok((Value::Object(obj), false))
                } else if let Some(children) = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                {
                    // ADR-0074 v0.1: `{ expr }` (bare-expression block) vs
                    // `{ key: val }` (object). When ALL children are non-entry
                    // nodes (bare expressions), treat as a block: evaluate
                    // each and return the last value. When any child is an
                    // `entry` node, treat as an object.
                    let all_bare = children.iter().all(|c| {
                        c.as_u64()
                            .and_then(|idx| ast.at(AstRef::new(idx as usize)).ok())
                            .map(|n| n.kind != "entry")
                            .unwrap_or(true)
                    });
                    if all_bare && !children.is_empty() {
                        // Block semantics: evaluate all, return last.
                        let mut last = Value::Null;
                        for entry in children {
                            let idx = entry.as_u64().ok_or_else(|| {
                                LangError::Engine(
                                    "object: entry index must be a number".to_string(),
                                )
                            })? as usize;
                            last = self.eval_node(ast, AstRef::new(idx), lang, env)?;
                        }
                        return Ok((last, false));
                    }
                    let mut obj = serde_json::Map::new();
                    for entry in children {
                        let entry_idx = entry.as_u64().ok_or_else(|| {
                            LangError::Engine(
                                "object: entry index must be a number".to_string(),
                            )
                        })? as usize;
                        let entry_node = ast.at(AstRef::new(entry_idx))?;
                        // Spread entry: if the child is not an `entry` node,
                        // evaluate it and merge its fields into the object.
                        // This implements `{ ...s, field: val }` syntax.
                        if entry_node.kind != "entry" {
                            let spread_val =
                                self.eval_node(ast, AstRef::new(entry_idx), lang, env)?;
                            if let Some(spread_obj) = spread_val.as_object() {
                                for (k, v) in spread_obj {
                                    obj.insert(k.clone(), v.clone());
                                }
                            }
                            continue;
                        }
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
                        // identifier) or `string` literal, use its payload
                        // as the key string directly — don't evaluate it.
                        // This makes `{ n: 5 }` use key "n" and `{ "s": 5 }`
                        // use key "s". Otherwise (computed key), evaluate
                        // the key expression and use its string
                        // representation. Bracket computed keys `[expr]:`
                        // are desugared to `paren { expr }` before eval
                        // (ADR-0058), so they fall into the `else` branch.
                        let key_node = ast.at(AstRef::new(key_idx))?;
                        let key = if key_node.kind == "var" {
                            key_node
                                .payload
                                .as_str()
                                .ok_or_else(|| {
                                    LangError::Engine(
                                        "object: var key missing string payload".to_string(),
                                    )
                                })?
                                .to_string()
                        } else if key_node.kind == "string" {
                            key_node
                                .payload
                                .as_str()
                                .ok_or_else(|| {
                                    LangError::Engine(
                                        "object: string key missing payload".to_string(),
                                    )
                                })?
                                .to_string()
                        } else {
                            let key_val =
                                self.eval_node(ast, AstRef::new(key_idx), lang, env)?;
                            if let Some(s) = key_val.as_str() {
                                s.to_string()
                            } else {
                                key_val.to_string()
                            }
                        };
                        let value =
                            self.eval_node(ast, AstRef::new(val_idx), lang, env)?;
                        if handl_language::is_reserved_value_key(&key) {
                            return Err(LangError::Engine(format!(
                                "object: key '{}' is reserved",
                                key
                            )));
                        }
                        obj.insert(key.to_string(), value);
                    }
                    Ok((Value::Object(obj), false))
                } else {
                    Err(LangError::Engine(
                        "object: missing entries/children in payload".to_string(),
                    ))
                }
            }
            "index" => {
                let target = current_node.payload.get("target").ok_or_else(|| {
                    LangError::Engine("index: missing target in payload".to_string())
                })?;
                let index = current_node.payload.get("index").ok_or_else(|| {
                    LangError::Engine("index: missing index in payload".to_string())
                })?;
                match (target, index) {
                    (Value::Array(arr), Value::Number(n)) => {
                        let i = n.as_u64().ok_or_else(|| {
                            LangError::Engine(
                                "index: index must be a non-negative integer".to_string(),
                            )
                        })? as usize;
                        let v = arr.get(i).cloned().ok_or_else(|| {
                            LangError::Engine(format!("index: index {} out of bounds", i))
                        })?;
                        Ok((v, false))
                    }
                    (Value::Object(obj), Value::String(key)) => {
                        let v = obj.get(key).cloned().ok_or_else(|| {
                            LangError::Engine(format!("index: key '{}' not found", key))
                        })?;
                        Ok((v, false))
                    }
                    _ => Err(LangError::Engine(
                        "index: target must be array/object and index must be number/string"
                            .to_string(),
                    )),
                }
            }
            "assign" => {
                // ADR-0053: `assign { children: [var, value] }` or
                // `assign { children: [var, value, body] }` —
                // evaluate value, optionally bind var in a new scope and
                // evaluate body, or just return the value.
                // Falls back to legacy `payload.value` for hand-built ASTs.
                if let Some(children) =
                    current_node.payload.get("children").and_then(|c| c.as_array())
                {
                    if children.len() >= 2 {
                        let value_idx = children[1].as_u64().ok_or_else(|| {
                            LangError::Engine("assign: value index must be a number".to_string())
                        })? as usize;
                        if children.len() == 3 {
                            // `x = y in body` — bind and evaluate body.
                            let var_idx = children[0].as_u64().ok_or_else(|| {
                                LangError::Engine("assign: var index must be a number".to_string())
                            })? as usize;
                            let body_idx = children[2].as_u64().ok_or_else(|| {
                                LangError::Engine("assign: body index must be a number".to_string())
                            })? as usize;
                            let var_name = {
                                let var_node = ast.at(AstRef::new(var_idx))?;
                                var_node.payload.as_str().ok_or_else(|| {
                                    LangError::Engine(
                                        "assign: LHS must be an identifier".to_string(),
                                    )
                                })?
                            };
                            let value = self.eval_node(ast, AstRef::new(value_idx), lang, env)?;
                            env.push_scope();
                            env.define(var_name, value);
                            let result = self.eval_node(ast, AstRef::new(body_idx), lang, env);
                            env.pop_scope();
                            result.map(|v| (v, false))
                        } else {
                            // `x = y` — just evaluate and return the value.
                            self.eval_node(ast, AstRef::new(value_idx), lang, env)
                                .map(|v| (v, false))
                        }
                    } else {
                        Err(LangError::Engine(
                            "assign: expected at least 2 children".to_string(),
                        ))
                    }
                } else {
                    // Legacy shape: `assign { target, value }` (hand-built
                    // ASTs). Just return the value.
                    let value = current_node.payload.get("value").ok_or_else(|| {
                        LangError::Engine("assign: missing value in payload".to_string())
                    })?;
                    Ok((value.clone(), false))
                }
            }
            "match" => self.prim_match(ast, lang, env, current_node),
            "handle" => self.prim_handle(ast, lang, env, current_node),
            "json" => self.prim_json(ast, lang, env, current_node),
            "bind" => self.prim_bind(ast, lang, env, current_node),
            // Arithmetic / comparison / logic.
            "add" => bin_add(args),
            "sub" => bin_num(args, |a, b| a - b),
            "mul" => bin_num(args, |a, b| a * b),
            "div" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                if b.to_f64() == 0.0 {
                    return Err(LangError::Engine("division by zero".to_string()));
                }
                Ok((num_value(a / b), false))
            }
            "mod" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                if b.to_f64() == 0.0 {
                    return Err(LangError::Engine("modulo by zero".to_string()));
                }
                let val = match (a, b) {
                    (Num::Int(a), Num::Int(b)) => num_value(Num::Int(a % b)),
                    (a, b) => num_value(Num::Float(a.to_f64() % b.to_f64())),
                };
                Ok((val, false))
            }
            "neg" => {
                let a = as_num(&args[0])?;
                let val = match a {
                    Num::Int(i) => num_value(Num::Int(-i)),
                    Num::Float(f) => num_value(Num::Float(-f)),
                };
                Ok((val, false))
            }
            "eq" => Ok((Value::Bool(args[0] == args[1]), false)),
            "neq" => Ok((Value::Bool(args[0] != args[1]), false)),
            "lt" => cmp_num(args, |a, b| a < b),
            "gt" => cmp_num(args, |a, b| a > b),
            "lte" => cmp_num(args, |a, b| a <= b),
            "gte" => cmp_num(args, |a, b| a >= b),
            "pow" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok((num_value(Num::Float(a.to_f64().powf(b.to_f64()))), false))
            }
            "compare" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                let cmp = a.partial_cmp(&b).ok_or_else(|| {
                    LangError::Engine("compare: incomparable values".to_string())
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
                let tag = lit
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        LangError::Engine("route_on_type: missing type tag literal".to_string())
                    })?;
                let val = args
                    .first()
                    .cloned()
                    .or(incoming)
                    .unwrap_or(Value::Null);
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
            // elements as individual arguments.
            "call_host" => {
                let callee = incoming
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        LangError::Engine("call_host: callee must be a string".to_string())
                    })?;
                let modules = self.modules.as_ref().ok_or_else(|| {
                    LangError::Engine("call_host: no module registry".to_string())
                })?;
                let func = modules.lookup(callee).ok_or_else(|| {
                    LangError::Engine(format!("call_host: function '{}' not found", callee))
                })?;
                let call_args: Vec<Value> = args
                    .first()
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().cloned().collect())
                    .unwrap_or_default();
                let mut pctx = handl_platform::EvalContext::new();
                pctx.set_value_callable(Rc::new(TreeWalkCallable));
                let val = func
                    .call(&call_args, &mut pctx)
                    .map_err(|e| LangError::Engine(e.to_string()))?;
                Ok((val, false))
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
                        LangError::Engine("proj_rest: expected children array in payload".to_string())
                    })?;
                let mut vals = Vec::new();
                for entry in children.iter().skip(1) {
                    let idx = entry.as_u64().ok_or_else(|| {
                        LangError::Engine("proj_rest: child index must be a number".to_string())
                    })? as usize;
                    vals.push(self.eval_node(ast, AstRef::new(idx), lang, env)?);
                }
                Ok((Value::Array(vals), false))
            }
            // eval_let — evaluate a `let` node whose children are
            // [assign, body]. The `assign` child has children
            // [var/type-annot, value]. We extract the binding name from
            // the LHS, evaluate the value, extend the environment, then
            // evaluate the body in the extended scope. This replaces the
            // old `$payload`-based scheme that relied on `payload.name`
            // (set by `payload_tokens`), which was removed so that typed
            // `let x: T = e in body` bindings parse correctly.
            "eval_let" => {
                let children = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::Engine(
                            "eval_let: expected children array in payload".to_string(),
                        )
                    })?;
                if children.len() < 2 {
                    return Err(LangError::Engine(
                        "eval_let: expected at least 2 children".to_string(),
                    ));
                }
                let body_idx = children[1].as_u64().ok_or_else(|| {
                    LangError::Engine("eval_let: body index must be a number".to_string())
                })? as usize;
                // Two payload formats are supported:
                //   1. Legacy/corpus: `{"name": "x", "children": [value, body]}`
                //      — the binding name is in `payload.name`, children
                //      are [value_idx, body_idx] directly.
                //   2. v0.1 grammar: `{"children": [assign, body]}` where
                //      `assign` has children [lhs, value]; the name is
                //      extracted from the LHS (a `var` or `type-annot`).
                let (var_name, value_idx) = if let Some(name) =
                    current_node.payload.get("name").and_then(|v| v.as_str())
                {
                    let value_idx = children[0].as_u64().ok_or_else(|| {
                        LangError::Engine("eval_let: value index must be a number".to_string())
                    })? as usize;
                    (name.to_string(), value_idx)
                } else {
                    let assign_idx = children[0].as_u64().ok_or_else(|| {
                        LangError::Engine("eval_let: assign index must be a number".to_string())
                    })? as usize;
                    let assign_node = ast.at(AstRef::new(assign_idx))?;
                    let assign_children = assign_node
                        .payload
                        .get("children")
                        .and_then(|c| c.as_array())
                        .ok_or_else(|| {
                            LangError::Engine(
                                "eval_let: assign node has no children array".to_string(),
                            )
                        })?;
                    if assign_children.len() < 2 {
                        return Err(LangError::Engine(
                            "eval_let: assign node needs at least 2 children (lhs, value)"
                                .to_string(),
                        ));
                    }
                    let lhs_idx = assign_children[0].as_u64().ok_or_else(|| {
                        LangError::Engine("eval_let: lhs index must be a number".to_string())
                    })? as usize;
                    let value_idx = assign_children[1].as_u64().ok_or_else(|| {
                        LangError::Engine("eval_let: value index must be a number".to_string())
                    })? as usize;
                    // The LHS is either a `var` (untyped: `let x = e`) or
                    // a `type-annot` (typed: `let x: T = e`), in which
                    // case the var is its child 0.
                    let lhs_node = ast.at(AstRef::new(lhs_idx))?;
                    let name = if lhs_node.kind == "type-annot" {
                        let ta_children = lhs_node
                            .payload
                            .get("children")
                            .and_then(|c| c.as_array())
                            .ok_or_else(|| {
                                LangError::Engine(
                                    "eval_let: type-annot has no children array".to_string(),
                                )
                            })?;
                        if ta_children.is_empty() {
                            return Err(LangError::Engine(
                                "eval_let: type-annot needs at least 1 child".to_string(),
                            ));
                        }
                        let inner_var_idx = ta_children[0].as_u64().ok_or_else(|| {
                            LangError::Engine(
                                "eval_let: type-annot var index must be a number".to_string(),
                            )
                        })? as usize;
                        let inner_var = ast.at(AstRef::new(inner_var_idx))?;
                        inner_var.payload.as_str().ok_or_else(|| {
                            LangError::Engine(
                                "eval_let: type-annot inner var payload is not a string".to_string(),
                            )
                        })?
                    } else {
                        lhs_node.payload.as_str().ok_or_else(|| {
                            LangError::Engine(
                                "eval_let: var node payload is not a string".to_string(),
                            )
                        })?
                    };
                    (name.to_string(), value_idx)
                };
                let value = self.eval_node(ast, AstRef::new(value_idx), lang, env)?;
                env.push_scope();
                env.define(&var_name, value);
                let result = self.eval_node(ast, AstRef::new(body_idx), lang, env);
                env.pop_scope();
                result.map(|v| (v, false))
            }
            // eval_block — evaluate all children of a block node in
            // sequence, returning the last child's value. Used by
            // `do { expr; expr; ... }` surface syntax (ADR-0054).
            "eval_block" => {
                let children = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::Engine("eval_block: expected children array in payload".to_string())
                    })?;
                if children.is_empty() {
                    return Ok((Value::Null, false));
                }
                let mut result = Value::Null;
                for entry in children.iter() {
                    let idx = entry.as_u64().ok_or_else(|| {
                        LangError::Engine("eval_block: child index must be a number".to_string())
                    })? as usize;
                    result = self.eval_node(ast, AstRef::new(idx), lang, env)?;
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
                let arg = args.first().or(incoming.as_ref()).unwrap_or(&Value::Null);
                Ok((Value::from(type_tag(arg)), false))
            }
            "not" => Ok((Value::Bool(!is_truthy(&args[0])), false)),
            "and" => Ok((Value::Bool(is_truthy(&args[0]) && is_truthy(&args[1])), false)),
            "or" => Ok((Value::Bool(is_truthy(&args[0]) || is_truthy(&args[1])), false)),
            // Utility unary numeric ops (used by derived-pattern tests in
            // plugin-lib-arrow-iter). These take the incoming value as
            // their single operand.
            "is_pos" => {
                let v = incoming.unwrap_or(Value::Null);
                let n = as_num(&v)?;
                Ok((Value::Bool(n > Num::Int(0)), false))
            }
            "dec" => {
                let v = incoming.unwrap_or(Value::Null);
                let n = as_num(&v)?;
                Ok((num_value(n - Num::Int(1)), false))
            }
            "inc" => {
                let v = incoming.unwrap_or(Value::Null);
                let n = as_num(&v)?;
                Ok((num_value(n + Num::Int(1)), false))
            }
            "half_floor" => {
                let v = incoming.unwrap_or(Value::Null);
                let n = as_num(&v)?;
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
                let inc = args.first().cloned().or(incoming).unwrap_or(Value::Null);
                let a = pair_fst(&inc)
                    .ok_or_else(|| {
                        LangError::Engine("inc_fst: input is not a pair".to_string())
                    })?
                    .clone();
                let b = pair_snd(&inc)
                    .ok_or_else(|| {
                        LangError::Engine("inc_fst: input is not a pair".to_string())
                    })?
                    .clone();
                let a_n = as_num(&a)?;
                let new_a = num_value(a_n + Num::Int(1));
                Ok((pair(new_a, b), false))
            }
            // Test helper: multi-step Loop countdown. Incoming is
            // pair(in, state) where in = pair(result, initial_count). On
            // the first step (state Null) the state is initialized to
            // initial_count. Then: state > 0 -> either_right(state - 1)
            // (continue); state == 0 -> either_left(result) (done).
            "countdown_step" => {
                let inc = args.first().cloned().or(incoming).unwrap_or(Value::Null);
                let a = pair_fst(&inc).cloned().ok_or_else(|| {
                    LangError::Engine("countdown_step: input is not a pair".to_string())
                })?;
                let b = pair_snd(&inc).cloned().unwrap_or(Value::Null);
                let result_val = pair_fst(&a).cloned().ok_or_else(|| {
                    LangError::Engine(
                        "countdown_step: in must be pair(result, initial)".to_string(),
                    )
                })?;
                let initial = pair_snd(&a).cloned().unwrap_or(Value::Null);
                let state = if b.is_null() { initial } else { b };
                let n = as_num(&state)?;
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
                let inc = args.first().cloned().or(incoming).unwrap_or(Value::Null);
                let b = pair_snd(&inc).cloned().unwrap_or(Value::Null);
                Ok((either_right(b), false))
            }
            // v0.1: `product` — categorical product `***` (ADR-0074).
            // Receives the two evaluated child results [f_result, g_result]
            // and pairs them as {"__pair__": [f_result, g_result]}.
            "product" => {
                let left = args.first().cloned().unwrap_or(Value::Null);
                let right = args.get(1).cloned().unwrap_or(Value::Null);
                Ok((pair(left, right), false))
            }
            // v0.1: `nullish` — nullish coalescing `??` (ADR-0074 A.3).
            // If left is some(v) ({"_tag":"Some","value":v}), return v;
            // if left is none ({"_tag":"None"}) or null, return right;
            // otherwise return right.
            // NOTE: The eval.nullish scheme uses Seq/Choice/tag combinators
            // which handle the null case via truthiness. This native op
            // provides full option-type semantics for direct dispatch.
            "nullish" => {
                let left = args.first().cloned().unwrap_or(Value::Null);
                let right = args.get(1).cloned().unwrap_or(Value::Null);
                if left.is_null() {
                    Ok((right, false))
                } else if left.get("_tag").and_then(|t| t.as_str()) == Some("Some") {
                    let inner = left.get("value").cloned().unwrap_or(Value::Null);
                    Ok((inner, false))
                } else if left.get("_tag").and_then(|t| t.as_str()) == Some("None") {
                    Ok((right, false))
                } else {
                    Ok((right, false))
                }
            }
            // v0.1: `opt_chain` — optional chaining `?.` (ADR-0074 A.4).
            // args = [input, field_name]. If input is some(v), return
            // some(v[field_name]); if input is none or null, return none.
            "opt_chain" => {
                let input = args.first().cloned().unwrap_or(Value::Null);
                let field_name = args
                    .get(1)
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        LangError::Engine(
                            "opt_chain: field name must be a string".to_string(),
                        )
                    })?;
                if input.is_null() {
                    // null → none
                    Ok((serde_json::json!({"_tag": "None"}), false))
                } else if input.get("_tag").and_then(|t| t.as_str()) == Some("Some") {
                    // some(v) → some(v.field)
                    let inner = input.get("value").cloned().unwrap_or(Value::Null);
                    let field_val = inner
                        .get(field_name)
                        .cloned()
                        .unwrap_or(Value::Null);
                    Ok((
                        serde_json::json!({"_tag": "Some", "value": field_val}),
                        false,
                    ))
                } else if input.get("_tag").and_then(|t| t.as_str()) == Some("None") {
                    // none → none
                    Ok((serde_json::json!({"_tag": "None"}), false))
                } else {
                    // Non-option value: treat as some(value) and extract field
                    let field_val = input.get(field_name).cloned().unwrap_or(Value::Null);
                    Ok((
                        serde_json::json!({"_tag": "Some", "value": field_val}),
                        false,
                    ))
                }
            }
            _ => Err(LangError::Engine(format!("unknown v2 native op: {}", op))),
        }
    }

    /// Apply an `App` callee: closure application only (ADR-0060).
    ///
    /// Host-function dispatch (string callee) has been moved to the
    /// `call_host` native op. This helper only handles arrow-value
    /// (closure) application: extract params/body/env, build a fresh
    /// scope from the captured env, bind params, evaluate the body.
    fn apply_app(
        &self,
        callee: &Value,
        args: &[Value],
        ast: &Ast,
        lang: &Language,
        _env: &mut ScopeStack,
    ) -> Result<Value, LangError> {
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
        // Arrow-value invocation (ADR-0052): the arrow carries its
        // definition-time env as data. We construct a fresh env from
        // that carried env, extend with params, and evaluate the body.
        // The caller's env is never mutated — no swap/restore hack.
        let mut arrow_env = ScopeStack::new();
        arrow_env.scopes[0] = captured_env;
        arrow_env.push_scope();
        for (param, arg) in params.iter().zip(args.iter()) {
            arrow_env.define(param, arg.clone());
        }
        self.eval_node(ast, body_ref, lang, &mut arrow_env)
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
        let node = ast.at(node_ref)?;
        let scheme_def = lang.eval.get_by_identity(&node.kind);
        match scheme_def {
            Some(def) => {
                self.eval_scheme(&def.scheme, ast, node_ref, lang, env, node, None)
            }
            None => {
                // Fallback for `foreign.<name>` node kinds.
                if let Some(foreign_name) = node.kind.strip_prefix("foreign.") {
                    let modules = self.modules.as_ref().ok_or_else(|| {
                        LangError::Engine(format!("unregistered foreign: {}", foreign_name))
                    })?;
                    let func = modules.lookup(foreign_name).ok_or_else(|| {
                        LangError::Engine(format!("unregistered foreign: {}", foreign_name))
                    })?;
                    let mut arg_vals = Vec::new();
                    if let Some(children) =
                        node.payload.get("children").and_then(|c| c.as_array())
                    {
                        for child_ref in children {
                            if let Some(idx) = child_ref.as_u64() {
                                let child_ref = AstRef::new(idx as usize);
                                arg_vals.push(self.eval_node(ast, child_ref, lang, env)?);
                            }
                        }
                    }
                    let mut pctx = handl_platform::EvalContext::new();
                    pctx.set_value_callable(Rc::new(TreeWalkCallable));
                    func.call(&arg_vals, &mut pctx)
                        .map_err(|e| LangError::Engine(e.to_string()))
                } else {
                    Ok(node.payload.clone())
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
        scheme: &Scheme,
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
    fn prim_bind(
        &self,
        ast: &Ast,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
    ) -> Result<(Value, bool), LangError> {
        let name = current_node
            .payload
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LangError::Engine("bind: missing name".to_string()))?;
        let children = current_node
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
        let value = self.eval_node(ast, AstRef::new(value_idx), lang, env)?;
        // Extend env with name = value, evaluate body, pop.
        env.push_scope();
        env.define(name, value);
        let result = self.eval_node(ast, AstRef::new(body_idx), lang, env);
        env.pop_scope();
        result.map(|v| (v, false))
    }

    /// `match` op: pattern-match a scrutinee against a list of arms.
    ///
    /// The `match` node's payload has `children: [scrut_idx, block_idx]`.
    /// The block node (kind `block`) has `children: [arm_idx, ...]`. Each
    /// arm node (kind `arm`) has `children: [pattern_idx, body_idx]`.
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
    ) -> Result<(Value, bool), LangError> {
        let children = current_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| LangError::Engine("match: missing children in payload".to_string()))?;
        let scrut_idx = children
            .get(0)
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LangError::Engine("match: missing scrutinee child".to_string()))?
            as usize;
        let block_idx = children
            .get(1)
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LangError::Engine("match: missing block child".to_string()))?
            as usize;

        // Evaluate the scrutinee.
        let scrut_val = self.eval_node(ast, AstRef::new(scrut_idx), lang, env)?;

        // Get the block node's children (the arm indices).
        let block_node = ast.at(AstRef::new(block_idx))?;
        let arm_indices = block_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| LangError::Engine("match: block node missing children".to_string()))?;

        // Try each arm.
        for arm_entry in arm_indices {
            let arm_idx = arm_entry
                .as_u64()
                .ok_or_else(|| LangError::Engine("match: arm index must be a number".to_string()))?
                as usize;
            let arm_node = ast.at(AstRef::new(arm_idx))?;
            let arm_children = arm_node
                .payload
                .get("children")
                .and_then(|c| c.as_array())
                .ok_or_else(|| LangError::Engine("match: arm node missing children".to_string()))?;
            let pattern_idx = arm_children
                .get(0)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| LangError::Engine("match: arm missing pattern child".to_string()))?
                as usize;
            let body_idx = arm_children
                .get(1)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| LangError::Engine("match: arm missing body child".to_string()))?
                as usize;

            // Interpret the pattern node structurally (ADR-0061: shared
            // pattern matching logic).
            env.push_scope();
            let mut ctx = TreeWalkPatternCtx { engine: self, lang, env };
            let matched = pattern_match::try_pattern(
                ast,
                AstRef::new(pattern_idx),
                &scrut_val,
                &mut ctx,
            )?;
            if matched {
                let result = self.eval_node(ast, AstRef::new(body_idx), lang, env);
                env.pop_scope();
                return result.map(|v| (v, false));
            }
            env.pop_scope();
        }

        Err(LangError::Engine("match: no arm matched".to_string()))
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
    ) -> Result<(Value, bool), LangError> {
        let children = current_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| LangError::Engine("handle: missing children in payload".to_string()))?;
        let body_idx = children
            .get(0)
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LangError::Engine("handle: missing body child".to_string()))?
            as usize;
        let block_idx = children
            .get(1)
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LangError::Engine("handle: missing block child".to_string()))?
            as usize;

        // Evaluate the body.
        let body_val = self.eval_node(ast, AstRef::new(body_idx), lang, env)?;

        // Get the block node's children (the clause indices).
        let block_node = ast.at(AstRef::new(block_idx))?;
        let clause_indices = block_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| LangError::Engine("handle: block node missing children".to_string()))?;

        // Try each clause.
        for clause_entry in clause_indices {
            let clause_idx = clause_entry.as_u64().ok_or_else(|| {
                LangError::Engine("handle: clause index must be a number".to_string())
            })? as usize;
            let clause_node = ast.at(AstRef::new(clause_idx))?;
            let clause_children = clause_node
                .payload
                .get("children")
                .and_then(|c| c.as_array())
                .ok_or_else(|| {
                    LangError::Engine("handle: clause node missing children".to_string())
                })?;
            let pattern_idx = clause_children
                .get(0)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    LangError::Engine("handle: clause missing pattern child".to_string())
                })? as usize;
            let handler_idx = clause_children
                .get(1)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    LangError::Engine("handle: clause missing handler child".to_string())
                })? as usize;

            env.push_scope();
            let mut ctx = TreeWalkPatternCtx { engine: self, lang, env };
            let matched = pattern_match::try_pattern(
                ast,
                AstRef::new(pattern_idx),
                &body_val,
                &mut ctx,
            )?;
            if matched {
                let result = self.eval_node(ast, AstRef::new(handler_idx), lang, env);
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
    ) -> Result<(Value, bool), LangError> {
        // ADR-0060: if the json node has children (desugared from
        // json{...}(args)), child 0 is the template node, children 1..N
        // are the args. Evaluate args and substitute $0, $1, … into the
        // template string.
        if let Some(children) = current_node.payload.get("children").and_then(|c| c.as_array()) {
            if children.len() >= 2 {
                // Child 0 is the original json template node.
                let template_idx = children[0].as_u64().ok_or_else(|| {
                    LangError::Engine("json: child index must be a number".to_string())
                })? as usize;
                let template_node = ast.at(AstRef::new(template_idx))?;
                let template = template_node.payload.as_str().ok_or_else(|| {
                    LangError::Engine("json: template payload must be a string".to_string())
                })?;
                let mut s = template.to_string();
                for (i, arg_entry) in children[1..].iter().enumerate() {
                    let idx = arg_entry.as_u64().ok_or_else(|| {
                        LangError::Engine("json: child index must be a number".to_string())
                    })? as usize;
                    let val = self.eval_node(ast, AstRef::new(idx), lang, env)?;
                    let replacement = serde_json::to_string(&val)
                        .map_err(|e| LangError::Engine(format!("json: {}", e)))?;
                    s = s.replace(&format!("${}", i), &replacement);
                }
                return serde_json::from_str(&s)
                    .map_err(|e| LangError::Engine(format!("json: parse error: {}", e)))
                    .map(|v| (v, false));
            }
        }
        // No children: parse the payload string as JSON directly.
        let expr = current_node.payload.as_str().ok_or_else(|| {
            LangError::Engine("json: payload must be a string".to_string())
        })?;
        serde_json::from_str(expr)
            .map_err(|e| LangError::Engine(format!("json: parse error: {}", e)))
            .map(|v| (v, false))
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
    ) -> Result<(Value, bool), LangError> {
        let children = current_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                LangError::Engine("eval_rec: expected children array in payload".to_string())
            })?;
        if children.is_empty() {
            return Ok((Value::Null, false));
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
        let mut prev_values: HashMap<String, Value> = HashMap::new();
        for entry in children {
            let child_idx = entry.as_u64().ok_or_else(|| {
                LangError::Engine("eval_rec: child index must be a number".to_string())
            })? as usize;
            let child_node = ast.at(AstRef::new(child_idx))?;
            if child_node.kind == "assign" {
                if let Some(ac) = child_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                {
                    if ac.len() >= 2 {
                        let var_idx = ac[0].as_u64().ok_or_else(|| {
                            LangError::Engine(
                                "eval_rec: assign var index must be a number".to_string(),
                            )
                        })? as usize;
                        let var_node = ast.at(AstRef::new(var_idx))?;
                        if let Some(name) = var_node.payload.as_str() {
                            // Seed from the enclosing scope (before the push).
                            // Since we already pushed a scope, lookup will
                            // search innermost-first, but the inner scope is
                            // empty at this point, so it falls through.
                            let outer_val =
                                env.lookup(name).cloned().unwrap_or(Value::Null);
                            prev_values.insert(name.to_string(), outer_val);
                        }
                    }
                }
            }
        }

        let mut result = Value::Null;
        const MAX_FUEL: usize = 10000;
        let mut converged = false;

        for _iter in 0..MAX_FUEL {
            let mut changed = false;

            for entry in children {
                let child_idx = entry.as_u64().ok_or_else(|| {
                    LangError::Engine("eval_rec: child index must be a number".to_string())
                })? as usize;
                let child_node = ast.at(AstRef::new(child_idx))?;

                // Handle `return` — exit the region with the return value.
                if child_node.kind == "return" {
                    if let Some(rc) = child_node
                        .payload
                        .get("children")
                        .and_then(|c| c.as_array())
                    {
                        if let Some(first) = rc.first() {
                            if let Some(ret_idx) = first.as_u64() {
                                result = self.eval_node(
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
                    return Ok((Value::Null, false));
                }

                // Handle `assign` bindings — evaluate the value expression
                // and update the binding in the current scope. Errors are
                // caught: a binding may reference another not-yet-computed
                // binding (still unbound or Null), which would error. The
                // binding is skipped for this pass and retried next pass.
                if child_node.kind == "assign" {
                    if let Some(ac) = child_node
                        .payload
                        .get("children")
                        .and_then(|c| c.as_array())
                    {
                        if ac.len() >= 2 {
                            let var_idx = ac[0].as_u64().ok_or_else(|| {
                                LangError::Engine(
                                    "eval_rec: assign var index must be a number".to_string(),
                                )
                            })? as usize;
                            let value_idx = ac[1].as_u64().ok_or_else(|| {
                                LangError::Engine(
                                    "eval_rec: assign value index must be a number".to_string(),
                                )
                            })? as usize;
                            let var_node = ast.at(AstRef::new(var_idx))?;
                            let var_name = var_node.payload.as_str().ok_or_else(|| {
                                LangError::Engine(
                                    "eval_rec: assign var payload must be a string".to_string(),
                                )
                            })?;

                            match self.eval_node(ast, AstRef::new(value_idx), lang, env) {
                                Ok(new_val) => {
                                    let prev = prev_values
                                        .get(var_name)
                                        .cloned()
                                        .unwrap_or(Value::Null);
                                    env.define(var_name, new_val.clone());
                                    if new_val != prev {
                                        changed = true;
                                        prev_values
                                            .insert(var_name.to_string(), new_val.clone());
                                    }
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
                result = self.eval_node(ast, AstRef::new(child_idx), lang, env)?;
            }

            if !changed {
                converged = true;
                break;
            }
        }

        if !converged {
            env.pop_scope();
            return Err(LangError::Engine(
                "rec: exceeded fuel limit (10000 iterations) without reaching fixpoint"
                    .to_string(),
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

impl EvalEngine for TreeWalkEngine {
    fn eval(&self, lang: &Language, req: &EvalRequest) -> Result<Value, LangError> {
        let ast = &req.ast;
        if ast.is_empty() {
            return Ok(Value::Null);
        }
        // Set thread-locals so host functions can call arrow-values via
        // ctx.try_call_value (see TreeWalkCallable below).
        let ast_rc = Rc::new(ast.clone());
        let lang_rc = Rc::new(lang.clone());
        let modules_rc = self.modules.clone();
        CURRENT_AST.with(|c| *c.borrow_mut() = Some(Rc::clone(&ast_rc)));
        CURRENT_LANG.with(|c| *c.borrow_mut() = Some(Rc::clone(&lang_rc)));
        CURRENT_MODULES.with(|c| *c.borrow_mut() = modules_rc);
        let mut env = ScopeStack::new();
        // Inject host-provided globals (e.g. `self` → language value) into
        // the root scope. ADR-0041 Phase 3: `self` is an ambient variable,
        // not a function call.
        for (name, val) in &self.globals {
            env.define(name, val.clone());
        }
        // The root is the last node in the arena (Pratt convention).
        let root_ref = AstRef::new(ast_rc.len() - 1);
        let result = self.eval_node(&ast_rc, root_ref, &lang_rc, &mut env);
        // Clear thread-locals.
        CURRENT_AST.with(|c| *c.borrow_mut() = None);
        CURRENT_LANG.with(|c| *c.borrow_mut() = None);
        CURRENT_MODULES.with(|c| *c.borrow_mut() = None);
        result
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
        let obj = callee
            .as_object()
            .ok_or_else(|| handl_platform::EvalError::Other("call_value: not an object".to_string()))?;
        if !obj.get("__arrow__").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Err(handl_platform::EvalError::Other(
                "call_value: not an arrow-value".to_string(),
            ));
        }
        let params: Vec<String> = obj
            .get("params")
            .and_then(|v| v.as_array())
            .ok_or_else(|| handl_platform::EvalError::Other("arrow-value missing params".to_string()))?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(String::from)
                    .ok_or_else(|| handl_platform::EvalError::Other("arrow-value param not string".to_string()))
            })
            .collect::<Result<_, _>>()?;
        let body_idx = obj
            .get("body")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| handl_platform::EvalError::Other("arrow-value missing body".to_string()))?;
        let captured_env: HashMap<String, Value> = obj
            .get("env")
            .and_then(|v| v.as_object())
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        if params.len() != args.len() {
            return Err(handl_platform::EvalError::Other(format!(
                "arrow-value expects {} args, got {}",
                params.len(),
                args.len()
            )));
        }
        // Build a fresh env from the arrow's captured env + params.
        let mut arrow_env = ScopeStack::new();
        arrow_env.scopes[0] = captured_env;
        arrow_env.push_scope();
        for (param, arg) in params.iter().zip(args.iter()) {
            arrow_env.define(param, arg.clone());
        }
        // Get AST and Language from thread-locals.
        let ast = CURRENT_AST.with(|c| c.borrow().clone())
            .ok_or_else(|| handl_platform::EvalError::Other("call_value: no AST in thread-local".to_string()))?;
        let lang = CURRENT_LANG.with(|c| c.borrow().clone())
            .ok_or_else(|| handl_platform::EvalError::Other("call_value: no Language in thread-local".to_string()))?;
        let engine = TreeWalkEngine::new();
        // Set modules if available.
        let engine = if let Some(mods) = CURRENT_MODULES.with(|c| c.borrow().clone()) {
            engine.with_modules(mods)
        } else {
            engine
        };
        engine
            .eval_node(&ast, AstRef::new(body_idx as usize), &lang, &mut arrow_env)
            .map_err(|e| handl_platform::EvalError::Other(e.to_string()))
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

/// Binary numeric op helper.
fn bin_num<F>(args: &[Value], f: F) -> Result<(Value, bool), LangError>
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
fn bin_add(args: &[Value]) -> Result<(Value, bool), LangError> {
    let a = &args[0];
    let b = &args[1];
    match (a, b) {
        (Value::String(s1), Value::String(s2)) => {
            Ok((Value::String(format!("{}{}", s1, s2)), false))
        }
        (Value::String(_), _) | (_, Value::String(_)) => Err(LangError::Engine(format!(
            "add: type mismatch ({} + {}): string concatenation requires both operands to be strings",
            a, b
        ))),
        _ => bin_num(args, |x, y| x + y),
    }
}

/// Numeric comparison helper.
fn cmp_num<F>(args: &[Value], f: F) -> Result<(Value, bool), LangError>
where
    F: FnOnce(Num, Num) -> bool,
{
    let a = as_num(&args[0])?;
    let b = as_num(&args[1])?;
    Ok((Value::Bool(f(a, b)), false))
}

// ---------------------------------------------------------------------------
// Pair / Either value-shape helpers
// ---------------------------------------------------------------------------

/// Return the type tag string for a JSON value (ADR-0060).
///
/// Used by `route_on_type` and the `typeof` native op. Returns one of
/// `"null"`, `"bool"`, `"number"`, `"string"`, `"array"`, `"object"`.
fn type_tag(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstNode, EvalSchemeDef, Language, Scheme};
    use handl_platform::{EvalContext, Function, Module, ModuleLookup};
    use handl_plugin_engine_eval::EvalRequest;
    use handl_plugin_module::ModuleRegistry;
    use serde_json::json;

    // --- v2 scheme constructor helpers (mirror the old v1 shapes) ---

    /// `proj_child n` — the v2 equivalent of the old `Scheme::Child(n)`.
    fn child(n: u64) -> Scheme {
        Scheme::arr_native_lit("proj_child", vec![], Value::from(n))
    }

    /// `proj_env name` — the v2 equivalent of the old `Scheme::Var(name)`.
    fn var(name: &str) -> Scheme {
        Scheme::arr_native_lit("proj_env", vec![], name)
    }

    /// `const v` — the v2 equivalent of the old `Scheme::Lit(v)`.
    fn lit(v: impl Into<Value>) -> Scheme {
        Scheme::arr_native_lit("const", vec![], v.into())
    }

    /// `arr_native op args` — the v2 equivalent of the old
    /// `Scheme::Prim { op, args }`.
    fn prim(op: &str, args: Vec<Scheme>) -> Scheme {
        Scheme::arr_native(op, args)
    }

    /// `arr_foreign name args` — the v2 equivalent of the old
    /// `Scheme::Foreign { name, args }`.
    fn foreign(name: &str, args: Vec<Scheme>) -> Scheme {
        Scheme::arr_foreign(name, args)
    }

    /// The `Bind` lowering:
    /// `Seq(Seq(value, extend_env name), body)`.
    fn bind(name: &str, value: Scheme, body: Scheme) -> Scheme {
        Scheme::seq(
            Scheme::seq(value, Scheme::arr_native_lit("extend_env", vec![], name)),
            body,
        )
    }

    /// The `Branch` lowering:
    /// `Seq(cond, Seq(arr tag, Choice(then_, else_)))`.
    fn branch(cond: Scheme, then_: Scheme, else_: Scheme) -> Scheme {
        Scheme::seq(
            cond,
            Scheme::seq(
                Scheme::arr_native("tag", vec![]),
                Scheme::choice(then_, else_),
            ),
        )
    }

    /// `app callee args` — the v2 equivalent of the old
    /// `Scheme::App { callee, args }`.
    fn app(callee: Scheme, args: Vec<Scheme>) -> Scheme {
        Scheme::app(callee, args)
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
        scheme: &Scheme,
        env: &mut ScopeStack,
    ) -> Result<Value, LangError> {
        let ast = make_ast(vec![dummy_node()]);
        let node = ast.at(AstRef::new(0)).unwrap();
        engine.eval_scheme(scheme, &ast, AstRef::new(0), &Language::new(), env, node, None)
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

    #[test]
    fn arr_const_returns_literal() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Scheme::arr_native_lit("const", vec![], json!(42));
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn arr_const_null_when_no_literal() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Scheme::arr_native("const", vec![]);
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, Value::Null);
    }

    // --- arr proj_env ----------------------------------------------------

    #[test]
    fn arr_proj_env_bound_lookup() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        env.define("x", json!(7));
        let s = Scheme::arr_native_lit("proj_env", vec![], "x");
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(7));
    }

    #[test]
    fn arr_proj_env_unbound_returns_name() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Scheme::arr_native_lit("proj_env", vec![], "unknown");
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, Value::from("unknown"));
    }

    // --- Seq const >>> id ------------------------------------------------

    #[test]
    fn seq_const_then_id() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], json!(5)),
            Scheme::arr_native("id", vec![]),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(5));
    }

    // --- Bind shape ------------------------------------------------------

    #[test]
    fn bind_shape_extend_env_then_proj_env() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Scheme::seq(
            Scheme::seq(
                Scheme::arr_native_lit("const", vec![], json!(5)),
                Scheme::arr_native_lit("extend_env", vec![], "x"),
            ),
            Scheme::arr_native_lit("proj_env", vec![], "x"),
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
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], pair(json!(3), json!(7))),
            Scheme::first(Scheme::arr_native("id", vec![])),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, pair(json!(3), json!(7)));
    }

    #[test]
    fn first_const_on_pair() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], pair(json!(3), json!(7))),
            Scheme::first(Scheme::arr_native_lit("const", vec![], json!(99))),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, pair(json!(99), json!(7)));
    }

    // --- Choice on Either ------------------------------------------------

    #[test]
    fn choice_left() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], either_left(json!(5))),
            Scheme::choice(
                Scheme::arr_native_lit("const", vec![], json!(1)),
                Scheme::arr_native_lit("const", vec![], json!(2)),
            ),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn choice_right() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], either_right(json!(5))),
            Scheme::choice(
                Scheme::arr_native_lit("const", vec![], json!(1)),
                Scheme::arr_native_lit("const", vec![], json!(2)),
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
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!("yes"));
    }

    #[test]
    fn tag_choice_false_yields_else() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
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
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], json!(10)),
            Scheme::loop_feedback(Scheme::arr_native("inc_fst", vec![])),
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
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], pair(json!(42), json!(5))),
            Scheme::loop_feedback(Scheme::arr_native("countdown_step", vec![])),
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
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], json!(1)),
            Scheme::loop_feedback(Scheme::arr_native("infinite_step", vec![])),
        );
        let err = eval_standalone(&engine, &s, &mut env).unwrap_err();
        assert!(matches!(err, LangError::Engine(ref m) if m.contains("max iterations")));
    }

    #[test]
    fn loop_multi_step_single_step_still_works() {
        // Backward compat: a body returning a pair (not an Either) runs
        // exactly once and returns fst. This is the original single-step
        // behavior, now the fallback when the result is not an Either.
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Scheme::seq(
            Scheme::arr_native_lit("const", vec![], json!(10)),
            Scheme::loop_feedback(Scheme::arr_native("inc_fst", vec![])),
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
            Scheme::arr_native_lit("const", vec![], closure),
            vec![Scheme::arr_native_lit("const", vec![], json!(41))],
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
        let s = Scheme::arr_native(
            "add",
            vec![
                Scheme::arr_native_lit("const", vec![], json!(2)),
                Scheme::arr_native_lit("const", vec![], json!(3)),
            ],
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn native_eq() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Scheme::arr_native(
            "eq",
            vec![
                Scheme::arr_native_lit("const", vec![], json!(2)),
                Scheme::arr_native_lit("const", vec![], json!(2)),
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
        let s = Scheme::arr_foreign(
            "double",
            vec![Scheme::arr_native_lit("const", vec![], json!(21))],
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn foreign_arr_no_modules_errors() {
        let engine = TreeWalkEngine::new();
        let mut env = ScopeStack::new();
        let s = Scheme::arr_foreign(
            "double",
            vec![Scheme::arr_native_lit("const", vec![], json!(21))],
        );
        let err = eval_standalone(&engine, &s, &mut env).unwrap_err();
        assert!(matches!(err, LangError::Engine(ref m) if m.contains("unregistered foreign")));
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
        let s = Scheme::arr_native("bogus", vec![]);
        let err = eval_standalone(&engine, &s, &mut env).unwrap_err();
        assert!(matches!(err, LangError::Engine(ref m) if m.contains("unknown v2 native op")));
    }

    // --- Globals injection ----------------------------------------------

    #[test]
    fn with_global_visible_in_env() {
        let engine = TreeWalkEngine::new().with_global("self", json!("my-lang"));
        let mut env = ScopeStack::new();
        for (name, val) in &engine.globals {
            env.define(name, val.clone());
        }
        let s = Scheme::arr_native_lit("proj_env", vec![], "self");
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
        assert!(matches!(err, LangError::Engine(_)));
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
        assert!(matches!(err, LangError::Engine(_)));
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
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                app(child(0), vec![]),
            ))
            .unwrap();
        // Callee is a lit (not a closure).
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(42)),
            AstNode::new("call").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
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
        assert!(matches!(err, LangError::Engine(_)));
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
        assert!(matches!(err, LangError::Engine(_)));
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
        // function expects 1 param, call passes 2 args.
        let ast = make_ast(vec![
            AstNode::new("function").with_payload(json!({"params": ["x"], "body": 0})),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("call").with_payload(json!({"children": [0, 1, 2]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    // --- Coverage: edge cases for error paths and type conversions ---

    #[test]
    fn lit_scheme_returns_value() {
        // Direct const evaluation via a node whose eval scheme is const.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.const",
                "const",
                lit(json!(99)),
            ))
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
        assert!(matches!(err, LangError::Engine(_)));
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
        assert!(matches!(err, LangError::Engine(_)));
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
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                app(child(0), vec![]),
            ))
            .unwrap();
        // Callee is an object {"foo": 1} — not a closure.
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!({"foo": 1})),
            AstNode::new("call").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
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
        assert!(matches!(err, LangError::Engine(_)));
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
        assert!(matches!(err, LangError::Engine(_)));
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
        assert!(matches!(err, LangError::Engine(_)));
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
        assert!(matches!(err, LangError::Engine(_)));
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
        // Closure has a non-string param → error.
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
        // params contains a non-string (number).
        let ast = make_ast(vec![
            AstNode::new("function").with_payload(json!({"params": [123], "body": 0})),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("call").with_payload(json!({"children": [0, 1]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn closure_missing_params_array_errors() {
        // Closure object has params but it's not an array.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                app(child(0), vec![]),
            ))
            .unwrap();
        // Callee is a "closure" with params as a string (not array).
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!({
                "__arrow__": true,
                "params": "not_an_array",
                "body": 0
            })),
            AstNode::new("call").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn closure_missing_body_errors() {
        // Closure object has no "body" field.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                app(child(0), vec![]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!({
                "__arrow__": true,
                "params": []
            })),
            AstNode::new("call").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn app_callee_not_object_errors() {
        // Callee is a number, not an object → as_object() returns None.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                app(child(0), vec![]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(42)),
            AstNode::new("call").with_payload(json!({"children": [0]})),
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
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
            AstNode::new("var").with_payload(json!("k")),                  // 0
            AstNode::new("paren").with_payload(json!({"children": [0]})),  // 1
            AstNode::new("value").with_payload(json!(2)),                  // 2
            AstNode::new("entry").with_payload(json!({"children": [1, 2]})), // 3
            AstNode::new("var").with_payload(json!("other")),              // 4
            AstNode::new("value").with_payload(json!(5)),                  // 5
            AstNode::new("entry").with_payload(json!({"children": [4, 5]})), // 6
            AstNode::new("object").with_payload(json!({"children": [3, 6]})), // 7
            AstNode::new("var").with_payload(json!("k")),                  // 8
            AstNode::new("function").with_payload(json!({"children": [8, 7]})), // 9
            AstNode::new("value").with_payload(json!("mykey")),            // 10
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
            AstNode::new("value").with_payload(json!(5)),        // 0: scrutinee
            AstNode::new("var").with_payload(json!("_")),        // 1: wildcard pattern
            AstNode::new("value").with_payload(json!(1)),        // 2: body
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})), // 3
            AstNode::new("block").with_payload(json!({"children": [3]})),  // 4
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
            AstNode::new("value").with_payload(json!(5)),        // 0: scrutinee
            AstNode::new("var").with_payload(json!("x")),        // 1: binder pattern
            AstNode::new("var").with_payload(json!("x")),        // 2: body (returns x)
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})), // 3
            AstNode::new("block").with_payload(json!({"children": [3]})),  // 4
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
            AstNode::new("value").with_payload(json!(3)),        // 0: scrutinee
            AstNode::new("value").with_payload(json!(1)),        // 1: pattern (literal 1)
            AstNode::new("value").with_payload(json!("a")),      // 2: body
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})), // 3
            AstNode::new("value").with_payload(json!(3)),        // 4: pattern (literal 3)
            AstNode::new("value").with_payload(json!("b")),      // 5: body
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
            AstNode::new("value").with_payload(json!(9)),        // 0: scrutinee
            AstNode::new("value").with_payload(json!(1)),        // 1: pattern (literal 1)
            AstNode::new("value").with_payload(json!("a")),      // 2: body
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})), // 3
            AstNode::new("block").with_payload(json!({"children": [3]})),  // 4
            AstNode::new("match").with_payload(json!({"children": [0, 4]})), // 5: root
        ]);
        let req = EvalRequest::new(ast);
        let err = engine.eval(&lang, &req).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn match_array_destructuring() {
        // match [1, 2] { [a, b]: a + b } → 3
        let engine = TreeWalkEngine::new();
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("value").with_payload(json!([1, 2])),   // 0: scrutinee
            AstNode::new("var").with_payload(json!("a")),        // 1: pattern elem a
            AstNode::new("var").with_payload(json!("b")),        // 2: pattern elem b
            AstNode::new("array").with_payload(json!({"children": [1, 2]})), // 3: pattern
            AstNode::new("var").with_payload(json!("a")),        // 4: body a
            AstNode::new("var").with_payload(json!("b")),        // 5: body b
            AstNode::new("binary.add").with_payload(json!({"children": [4, 5]})), // 6: a+b
            AstNode::new("arm").with_payload(json!({"children": [3, 6]})), // 7
            AstNode::new("block").with_payload(json!({"children": [7]})),  // 8
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
            AstNode::new("var").with_payload(json!("a")),        // 1: pattern elem a
            AstNode::new("var").with_payload(json!("rest")),     // 2: rest binder
            AstNode::new("rest").with_payload(json!({"children": [2]})), // 3: ...rest
            AstNode::new("array").with_payload(json!({"children": [1, 3]})), // 4: pattern [a, ...rest]
            AstNode::new("var").with_payload(json!("rest")),     // 5: body (returns rest)
            AstNode::new("arm").with_payload(json!({"children": [4, 5]})), // 6
            AstNode::new("block").with_payload(json!({"children": [6]})),  // 7
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
            AstNode::new("var").with_payload(json!("x")),       // 1: key x
            AstNode::new("var").with_payload(json!("a")),       // 2: val pattern a
            AstNode::new("entry").with_payload(json!({"children": [1, 2]})), // 3: x: a
            AstNode::new("var").with_payload(json!("y")),       // 4: key y
            AstNode::new("var").with_payload(json!("b")),       // 5: val pattern b
            AstNode::new("entry").with_payload(json!({"children": [4, 5]})), // 6: y: b
            AstNode::new("object").with_payload(json!({"children": [3, 6]})), // 7: pattern
            AstNode::new("var").with_payload(json!("a")),       // 8: body a
            AstNode::new("var").with_payload(json!("b")),       // 9: body b
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
            AstNode::new("var").with_payload(json!("x")),       // 1: key x
            AstNode::new("var").with_payload(json!("a")),       // 2: val pattern a
            AstNode::new("entry").with_payload(json!({"children": [1, 2]})), // 3: x: a
            AstNode::new("var").with_payload(json!("rest")),    // 4: rest binder
            AstNode::new("rest").with_payload(json!({"children": [4]})), // 5: ...rest
            AstNode::new("object").with_payload(json!({"children": [3, 5]})), // 6: pattern
            AstNode::new("var").with_payload(json!("rest")),    // 7: body (returns rest)
            AstNode::new("arm").with_payload(json!({"children": [6, 7]})), // 8
            AstNode::new("block").with_payload(json!({"children": [8]})),  // 9
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
            AstNode::new("value").with_payload(json!(5)),        // 0: body
            AstNode::new("var").with_payload(json!("x")),        // 1: pattern (binder x)
            AstNode::new("var").with_payload(json!("x")),        // 2: handler x
            AstNode::new("value").with_payload(json!(1)),        // 3: handler 1
            AstNode::new("binary.add").with_payload(json!({"children": [2, 3]})), // 4: x+1
            AstNode::new("arm").with_payload(json!({"children": [1, 4]})), // 5: clause
            AstNode::new("block").with_payload(json!({"children": [5]})),  // 6
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
            AstNode::new("value").with_payload(json!(5)),        // 0: body
            AstNode::new("value").with_payload(json!(1)),        // 1: pattern (literal 1)
            AstNode::new("value").with_payload(json!("a")),      // 2: handler
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})), // 3: clause
            AstNode::new("block").with_payload(json!({"children": [3]})),  // 4
            AstNode::new("handle").with_payload(json!({"children": [0, 4]})), // 5: root
        ]);
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(5));
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
            .merge(EvalSchemeDef::new(
                "e.rec",
                "rec",
                prim("eval_rec", vec![]),
            ))
            .unwrap();
        // `let` — bind name to child(0), evaluate child(1).
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.let",
                "let",
                bind("$payload", child(0), child(1)),
            ))
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
            AstNode::new("var").with_payload(json!("b")),                   // 0: b in b+1
            AstNode::new("value").with_payload(json!(1)),                   // 1: lit 1
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})), // 2: b+1
            AstNode::new("var").with_payload(json!("a")),                   // 3: LHS a = ...
            AstNode::new("assign").with_payload(json!({"children": [3, 2]})), // 4: a = b+1
            AstNode::new("value").with_payload(json!(0)),                   // 5: lit 0
            AstNode::new("var").with_payload(json!("b")),                   // 6: LHS b = ...
            AstNode::new("assign").with_payload(json!({"children": [6, 5]})), // 7: b = 0
            AstNode::new("var").with_payload(json!("a")),                   // 8: result expr
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
            AstNode::new("value").with_payload(json!(5)),                   // 0: n initial
            AstNode::new("var").with_payload(json!("n")),                   // 1: n in n>0
            AstNode::new("value").with_payload(json!(0)),                   // 2: 0 in n>0
            AstNode::new("binary.gt").with_payload(json!({"children": [1, 2]})), // 3: n>0
            AstNode::new("var").with_payload(json!("n")),                   // 4: n in n-1
            AstNode::new("value").with_payload(json!(1)),                   // 5: 1 in n-1
            AstNode::new("binary.sub").with_payload(json!({"children": [4, 5]})), // 6: n-1
            AstNode::new("var").with_payload(json!("n")),                   // 7: n (else)
            AstNode::new("if").with_payload(json!({"children": [3, 6, 7]})), // 8: if n>0 then n-1 else n
            AstNode::new("var").with_payload(json!("n")),                   // 9: LHS n = ...
            AstNode::new("assign").with_payload(json!({"children": [9, 8]})), // 10: n = if ...
            AstNode::new("var").with_payload(json!("n")),                   // 11: result expr
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
            AstNode::new("value").with_payload(json!(0)),                   // 0: x initial
            AstNode::new("var").with_payload(json!("x")),                   // 1: x in x<10
            AstNode::new("value").with_payload(json!(10)),                  // 2: 10 in x<10
            AstNode::new("binary.lt").with_payload(json!({"children": [1, 2]})), // 3: x<10
            AstNode::new("var").with_payload(json!("x")),                   // 4: x in x+1
            AstNode::new("value").with_payload(json!(1)),                   // 5: 1 in x+1
            AstNode::new("binary.add").with_payload(json!({"children": [4, 5]})), // 6: x+1
            AstNode::new("var").with_payload(json!("x")),                   // 7: x (else)
            AstNode::new("if").with_payload(json!({"children": [3, 6, 7]})), // 8: if x<10 then x+1 else x
            AstNode::new("var").with_payload(json!("x")),                   // 9: LHS x = ...
            AstNode::new("assign").with_payload(json!({"children": [9, 8]})), // 10: x = if ...
            AstNode::new("var").with_payload(json!("x")),                   // 11: result expr
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
            AstNode::new("value").with_payload(json!(0)),                   // 0: n initial
            AstNode::new("var").with_payload(json!("n")),                   // 1: n in n+1
            AstNode::new("value").with_payload(json!(1)),                   // 2: 1 in n+1
            AstNode::new("binary.add").with_payload(json!({"children": [1, 2]})), // 3: n+1
            AstNode::new("var").with_payload(json!("n")),                   // 4: LHS n = ...
            AstNode::new("assign").with_payload(json!({"children": [4, 3]})), // 5: n = n+1
            AstNode::new("var").with_payload(json!("n")),                   // 6: result expr
            AstNode::new("rec").with_payload(json!({"children": [5, 6]})),  // 7: rec
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
            AstNode::new("value").with_payload(json!(42)),                  // 0: lit 42
            AstNode::new("var").with_payload(json!("x")),                   // 1: LHS x = ...
            AstNode::new("assign").with_payload(json!({"children": [1, 0]})), // 2: x = 42
            AstNode::new("var").with_payload(json!("x")),                   // 3: return value
            AstNode::new("return").with_payload(json!({"children": [3]})),  // 4: return x
            AstNode::new("rec").with_payload(json!({"children": [2, 4]})),  // 5: root
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
            AstNode::new("value").with_payload(json!(1)),                   // 0: lit 1
            AstNode::new("var").with_payload(json!("x")),                   // 1: LHS x = ...
            AstNode::new("assign").with_payload(json!({"children": [1, 0]})), // 2: x = 1
            AstNode::new("var").with_payload(json!("x")),                   // 3: x in x+1
            AstNode::new("value").with_payload(json!(1)),                   // 4: 1 in x+1
            AstNode::new("binary.add").with_payload(json!({"children": [3, 4]})), // 5: x+1
            AstNode::new("var").with_payload(json!("y")),                   // 6: LHS y = ...
            AstNode::new("assign").with_payload(json!({"children": [6, 5]})), // 7: y = x+1
            AstNode::new("var").with_payload(json!("y")),                   // 8: return value
            AstNode::new("return").with_payload(json!({"children": [8]})),  // 9: return y
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
            AstNode::new("value").with_payload(json!(5)),                   // 0: lit 5
            AstNode::new("var").with_payload(json!("x")),                   // 1: LHS x = ...
            AstNode::new("assign").with_payload(json!({"children": [1, 0]})), // 2: x = 5
            AstNode::new("var").with_payload(json!("x")),                   // 3: result expr
            AstNode::new("rec").with_payload(json!({"children": [2, 3]})),  // 4: root
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
            AstNode::new("var").with_payload(json!("b")),                   // 0: b in b+1
            AstNode::new("value").with_payload(json!(1)),                   // 1: lit 1
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})), // 2: b+1
            AstNode::new("var").with_payload(json!("a")),                   // 3: LHS a = ...
            AstNode::new("assign").with_payload(json!({"children": [3, 2]})), // 4: a = b+1
            // b = a - 1
            AstNode::new("var").with_payload(json!("a")),                   // 5: a in a-1
            AstNode::new("value").with_payload(json!(1)),                   // 6: lit 1
            AstNode::new("binary.sub").with_payload(json!({"children": [5, 6]})), // 7: a-1
            AstNode::new("var").with_payload(json!("b")),                   // 8: LHS b = ...
            AstNode::new("assign").with_payload(json!({"children": [8, 7]})), // 9: b = a-1
            // result
            AstNode::new("var").with_payload(json!("a")),                   // 10: result expr
            AstNode::new("rec").with_payload(json!({"children": [4, 9, 10]})), // 11: rec
            // let a = 1 in rec { ... }
            AstNode::new("value").with_payload(json!(1)),                   // 12: a initial
            AstNode::new("let").with_payload(json!({"name": "a", "children": [12, 11]})), // 13: let a = 1 in rec
            // let b = 0 in (let a = 1 in rec { ... })  [root]
            AstNode::new("value").with_payload(json!(0)),                   // 14: b initial
            AstNode::new("let").with_payload(json!({"name": "b", "children": [14, 13]})), // 15: root
        ]);
        let req = EvalRequest::new(ast);
        let result = engine.eval(&lang, &req).unwrap();
        assert_eq!(result, json!(1));
    }
}
