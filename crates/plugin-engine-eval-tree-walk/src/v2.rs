//! # v2 arrow-calculus reference handler (ADR-0052)
//!
//! A self-contained **handler** that interprets
//! [`SchemeV2`](handl_language::scheme_v2::SchemeV2) directly — the v2
//! counterpart of the v1 `eval_scheme`. Per Sanada's model, the handler
//! interprets arrow descriptions; the kernel only constructs them.
//!
//! This is **additive**: the v1 interpreter in [`crate`] is untouched. The
//! handler reuses the v1 [`ScopeStack`](crate::ScopeStack) and the v1 value
//! conventions (closures, `$payload`) so v2 is fully compatible with v1
//! languages via [`lower_v1_to_v2`](handl_language::scheme_v2::lower_v1_to_v2).
//!
//! # Value shapes
//!
//! The arrow-calculus combinators operate on a few tagged JSON value shapes:
//!
//! - **Pair** (`first`): `{"__pair__": [a, b]}`. `first inner` runs `inner`
//!   on `a` and carries `b` through, producing `{"__pair__": [inner(a), b]}`.
//! - **Either** (`Choice` / `tag`): `{"__either__": "left"|"right", "value":
//!   v}`. `Choice { l, r }` routes a `left` Either to `l` and a `right`
//!   Either to `r`. The `tag` native op converts a truthy value into a
//!   `left` Either and a falsy value into a `right` Either (this is the
//!   `Branch` lowering: `c >>> arr tag >>> Choice(t, e)`).
//! - **Closure** (`App`): identical to v1 —
//!   `{"__closure__": true, "params": [...], "body": <idx>, "env": {...}}`.
//!   `App` evaluates the callee; if it is a string naming a registered host
//!   function, dispatches to it; otherwise applies the closure (swap-in
//!   captured env, bind params, eval body node, restore).
//!
//! # Loop semantics
//!
//! [`SchemeV2::Loop`] implements **causal/strict** feedback
//! (ADR-0052 §"Feedback semantics", open question #1 — the default). The
//! handler seeds `State` with [`Value::Null`], constructs the pair
//! `(In, State)`, and evaluates `body` with that pair as `incoming`. The
//! body's result shape selects one of two modes:
//!
//! - **Single-step (backward-compatible).** The body returns a **pair**
//!   `(Out, NewState)`; `Loop` returns `Out`. This is the original
//!   state-machine single step: the state is hidden in the feedback
//!   channel and the body runs exactly once.
//!
//! - **Multi-step (Either-based).** The body returns an **Either**
//!   (`{"__either__": "left"|"right", "value": v}`):
//!   - `left(out)` → **done**: `Loop` returns `out`.
//!   - `right(new_state)` → **continue**: re-run the body with
//!     `pair(original_input, new_state)`. Repeat until `left`.
//!
//!   A max-iteration guard (100_000) prevents infinite loops.
//!
//! The multi-step body is a step function `(In, State) ~> Either(Out,
//! State)`: `Left` terminates with `Out`; `Right` feeds `State` back and
//! re-runs. This is exactly `while`/`for` expressed as `Loop` + `Choice`
//! — the body uses `Choice` internally to produce the Either. The two
//! modes are distinguished by their JSON shape (`__pair__` vs
//! `__either__`), so the existing single-step tests stay green.

use std::collections::HashMap;
use std::rc::Rc;

use handl_language::scheme_v2::{ArrSource, SchemeV2, lower_v1_to_v2};
use handl_language::{Ast, AstNode, AstRef, LangError, Language, Value};
use handl_platform::ModuleLookup;

use crate::ScopeStack;

// ---------------------------------------------------------------------------
// Numeric helpers (mirror v1 `as_num`/`num_value`/`is_truthy` exactly)
// ---------------------------------------------------------------------------

/// A number that tracks int-vs-float, mirroring v1 `Num`.
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

/// Extract a number from a [`Value`] (mirrors v1 `as_num`).
fn as_num(v: &Value) -> Result<Num, LangError> {
    if let Some(i) = v.as_i64() {
        return Ok(Num::Int(i));
    }
    if let Some(f) = v.as_f64() {
        return Ok(Num::Float(f));
    }
    Err(LangError::Engine(format!("expected number, got {}", v)))
}

/// Convert a [`Num`] back to a [`Value`] (mirrors v1 `num_value`).
fn num_value(n: Num) -> Value {
    match n {
        Num::Int(i) => Value::from(i),
        Num::Float(f) => Value::from(f),
    }
}

/// Whether a [`Value`] is truthy (mirrors v1 `is_truthy`).
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
// Pair / Either value-shape helpers
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
// V2Interpreter
// ---------------------------------------------------------------------------

/// The v2 arrow-calculus reference handler (ADR-0052).
///
/// A tree-walk handler that interprets [`SchemeV2`] directly. It is the v2
/// counterpart of the v1 `eval_scheme`. Construct with [`V2Interpreter::new`]
/// and optionally attach a module registry via
/// [`V2Interpreter::with_modules`] or inject globals via
/// [`V2Interpreter::with_global`].
///
/// Reuses the v1 [`ScopeStack`] and value conventions (closures, `$payload`)
/// so v2 is compatible with v1 languages via
/// [`lower_v1_to_v2`].
#[derive(Clone)]
pub struct V2Interpreter {
    /// The module registry for `Foreign` dispatch (may be `None`).
    modules: Option<Rc<dyn ModuleLookup>>,
    /// Global variables injected by the host, pre-populated into the root
    /// scope on every eval call.
    globals: HashMap<String, Value>,
}

impl V2Interpreter {
    /// Create a new v2 handler with no module registry.
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
    pub fn with_global(mut self, name: impl Into<String>, value: Value) -> Self {
        self.globals.insert(name.into(), value);
        self
    }

    /// Interpret a given [`SchemeV2`] in the context of `current_node`.
    ///
    /// This is the core handler entry point. `incoming` is the value fed by
    /// an enclosing `Seq`/`First`/`Choice`/`Loop`; top-level calls pass
    /// `None`.
    ///
    /// # Scope discipline
    ///
    /// `extend_env` pushes a scope and returns the value flowing through.
    /// The scope's lifetime follows the arrow pipeline: it is popped by the
    /// enclosing [`SchemeV2::Seq`] whose `fst` (transitively) contained the
    /// `extend_env`, after that `Seq`'s `snd` completes. This realizes the
    /// `Bind` lowering `value >>> extend_env "x" >>> body` =
    /// `Seq(Seq(value, extend_env), body)`: the scope pushed by
    /// `extend_env` (the inner `Seq`'s `snd`) is reported up through the
    /// inner `Seq` and popped by the outer `Seq` after `body`. A bare
    /// `extend_env` with no enclosing `Seq` is popped here.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_scheme_v2(
        &self,
        scheme: &SchemeV2,
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
    /// reported and propagates whatever `snd` reported (so an `extend_env`
    /// in the inner `Seq`'s `snd` position — the `Bind` lowering shape —
    /// is popped by the *enclosing* `Seq` after its `snd` completes). The
    /// other combinators (`First`/`Choice`/`Loop`/`App`) pop any scope
    /// their sub-evaluation pushed immediately, since they have no
    /// "binding persists down the pipeline" semantics.
    #[allow(clippy::too_many_arguments)]
    fn eval_inner(
        &self,
        scheme: &SchemeV2,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<Value>,
    ) -> Result<(Value, bool), LangError> {
        match scheme {
            SchemeV2::Arr {
                op,
                args,
                source,
                lit,
            } => self.eval_arr(
                op,
                args,
                source,
                lit.as_ref(),
                ast,
                node_ref,
                lang,
                env,
                current_node,
                incoming,
            ),
            SchemeV2::Seq { fst, snd } => {
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
            SchemeV2::First { inner } => {
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
            SchemeV2::Choice { l, r } => {
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
            SchemeV2::Loop { body } => {
                // Causal/strict feedback (ADR-0052 §"Feedback semantics",
                // open question #1 — the default). Two modes, selected by
                // the body's result shape (see the module-level "Loop
                // semantics" doc):
                //
                // 1. Single-step (backward-compatible): body returns a
                //    pair (Out, NewState); Loop returns Out.
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
                    // (Out, State) and return Out (backward-compatible).
                    let out = pair_fst(&result).ok_or_else(|| {
                        LangError::Engine(
                            "Loop: body did not return a pair or Either".to_string(),
                        )
                    })?;
                    return Ok((out.clone(), false));
                }
                Err(LangError::Engine("Loop: exceeded max iterations".to_string()))
            }
            SchemeV2::App { callee, args } => {
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
                let val = self.apply_app(&callee_val, &arg_vals, ast, lang, env)?;
                Ok((val, false))
            }
        }
    }

    /// Evaluate an `Arr` combinator. Returns the value and whether this Arr
    /// pushed a scope (`extend_env` pushes; the caller — `Seq` or
    /// `eval_scheme_v2` — is responsible for popping it).
    #[allow(clippy::too_many_arguments)]
    fn eval_arr(
        &self,
        op: &str,
        args: &[SchemeV2],
        source: &ArrSource,
        lit: Option<&Value>,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
        current_node: &AstNode,
        incoming: Option<Value>,
    ) -> Result<(Value, bool), LangError> {
        // Derived-pattern composite ops (while_step/for_step/fix_step) carry
        // sub-arrows as args that must be evaluated with the *current value*
        // extracted from the incoming pair, not with the pair itself. The
        // generic arg evaluation below would feed the pair to the sub-schemes,
        // which is wrong for these ops. Intercept before it. These are
        // handler-interpreted derived operations (ADR-0052 §"Derived forms"):
        // the description is primitive-only (`Loop` + `Arr`); the
        // interpretation routes the value through the carried cond/body
        // sub-arrows. This is NOT a new kernel primitive — `while_step` is an
        // `Arr` op (one of the six primitives); its interpretation is the
        // derived pattern.
        if matches!(source, ArrSource::Native)
            && matches!(op, "while_step" | "for_step" | "fix_step")
        {
            return self.eval_derived_step(
                op, args, ast, node_ref, lang, env, current_node, incoming,
            );
        }
        // Evaluate sub-scheme args (each gets the same incoming context).
        let mut arg_vals = Vec::with_capacity(args.len());
        for arg in args {
            arg_vals.push(self.eval_scheme_v2(
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
                let val = func
                    .call(&arg_vals, &mut pctx)
                    .map_err(|e| LangError::Engine(e.to_string()))?;
                Ok((val, false))
            }
        }
    }

    /// Evaluate a derived-pattern composite step op
    /// (`while_step`/`for_step`/`fix_step`). These are handler-interpreted
    /// derived operations (ADR-0052 §"Derived forms"): the `SchemeV2`
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
        raw_args: &[SchemeV2],
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
                let cond_val = self.eval_scheme_v2(
                    cond, ast, node_ref, lang, env, current_node, Some(current.clone()),
                )?;
                if is_truthy(&cond_val) {
                    let new_val = self.eval_scheme_v2(
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
                    let new_acc = self.eval_scheme_v2(
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
                let new_val = self.eval_scheme_v2(
                    body, ast, node_ref, lang, env, current_node, Some(current.clone()),
                )?;
                if new_val == current {
                    Ok((either_left(new_val), false))
                } else {
                    Ok((either_right(new_val), false))
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
                // Same fallback as v1 Var: unbound → name as a string.
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
                let val = self.eval_node_v2(ast, target_ref, lang, env)?;
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
                // responsibility (Seq or eval_scheme_v2).
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
                // Mirror v1 `closure` prim: read params/body from the
                // current node's payload (rich or Pratt-parsed shapes).
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
                    "__closure__": true,
                    "params": params,
                    "body": body,
                    "env": captured,
                });
                Ok((closure, false))
            }
            "array" => {
                // Two payload shapes (mirrors v1 `array` prim):
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
                        result.push(
                            self.eval_node_v2(ast, AstRef::new(idx), lang, env)?,
                        );
                    }
                    Ok((Value::Array(result), false))
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
                        obj.insert(key.to_string(), value.clone());
                    }
                    Ok((Value::Object(obj), false))
                } else if let Some(children) = current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                {
                    let mut obj = serde_json::Map::new();
                    for entry in children {
                        let entry_idx = entry.as_u64().ok_or_else(|| {
                            LangError::Engine(
                                "object: entry index must be a number".to_string(),
                            )
                        })? as usize;
                        let entry_node = ast.at(AstRef::new(entry_idx))?;
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
                        let key_val =
                            self.eval_node_v2(ast, AstRef::new(key_idx), lang, env)?;
                        let key = key_val.as_str().ok_or_else(|| {
                            LangError::Engine(
                                "object: entry key must evaluate to a string".to_string(),
                            )
                        })?;
                        let value =
                            self.eval_node_v2(ast, AstRef::new(val_idx), lang, env)?;
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
                let value = current_node.payload.get("value").ok_or_else(|| {
                    LangError::Engine("assign: missing value in payload".to_string())
                })?;
                Ok((value.clone(), false))
            }
            // Arithmetic / comparison / logic — mirror v1 apply_prim.
            "add" => bin_num(args, |a, b| a + b),
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
            "typeof" => {
                let type_name = match &args[0] {
                    Value::Null => "null",
                    Value::Bool(_) => "bool",
                    Value::Number(_) => "number",
                    Value::String(_) => "string",
                    Value::Array(_) => "array",
                    Value::Object(_) => "object",
                };
                Ok((Value::from(type_name), false))
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
            // Test helper: increment the first component of a pair, carrying
            // the second through. Used by the Loop causal-single-step test.
            // Documented as a test helper, not part of the ADR op set.
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
            // (continue); state == 0 -> either_left(result) (done). Used
            // by the multi-step Loop countdown test.
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
            _ => Err(LangError::Engine(format!("unknown v2 native op: {}", op))),
        }
    }

    /// Apply an `App` callee: host-function dispatch (string callee) or
    /// closure application, mirroring v1 `apply_closure`.
    fn apply_app(
        &self,
        callee: &Value,
        args: &[Value],
        ast: &Ast,
        lang: &Language,
        env: &mut ScopeStack,
    ) -> Result<Value, LangError> {
        // Host-function dispatch: a string callee naming a registered host fn.
        if let Some(name) = callee.as_str()
            && let Some(modules) = self.modules.as_ref()
            && let Some(func) = modules.lookup(name)
        {
            let mut pctx = handl_platform::EvalContext::new();
            return func
                .call(args, &mut pctx)
                .map_err(|e| LangError::Engine(e.to_string()));
        }
        // Closure application (mirrors v1 apply_closure).
        let obj = callee
            .as_object()
            .ok_or_else(|| LangError::Engine("callee is not a closure".to_string()))?;
        if !obj
            .get("__closure__")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return Err(LangError::Engine("callee is not a closure".to_string()));
        }
        let params: Vec<String> = obj
            .get("params")
            .and_then(|v| v.as_array())
            .ok_or_else(|| LangError::Engine("closure missing params".to_string()))?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(String::from)
                    .ok_or_else(|| LangError::Engine("closure param is not a string".to_string()))
            })
            .collect::<Result<_, _>>()?;
        let body_idx = obj
            .get("body")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LangError::Engine("closure missing body".to_string()))?;
        let body_ref = AstRef::new(body_idx as usize);
        let captured_env: HashMap<String, Value> = obj
            .get("env")
            .and_then(|v| v.as_object())
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        if params.len() != args.len() {
            return Err(LangError::Engine(format!(
                "closure expects {} args, got {}",
                params.len(),
                args.len()
            )));
        }
        // Swap in captured env + new scope with params, eval body, restore.
        let saved = std::mem::replace(env, ScopeStack::new());
        env.scopes[0] = captured_env;
        env.push_scope();
        for (param, arg) in params.iter().zip(args.iter()) {
            env.define(param, arg.clone());
        }
        let result = self.eval_node_v2(ast, body_ref, lang, env);
        *env = saved;
        result
    }

    /// Evaluate a single node by looking up its v1 eval scheme, lowering to
    /// v2, and interpreting. This is the child-node evaluation helper used
    /// by `proj_child` and closure bodies.
    ///
    /// Fallback (no scheme): `foreign.<name>` kinds dispatch to the host
    /// function `<name>` with evaluated children; other kinds return the
    /// payload as the value (mirrors v1 `eval_node`).
    pub fn eval_node_v2(
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
                let v2 = lower_v1_to_v2(&def.scheme)?;
                self.eval_scheme_v2(&v2, ast, node_ref, lang, env, node, None)
            }
            None => {
                // Fallback for `foreign.<name>` node kinds (mirrors v1).
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
                                arg_vals.push(self.eval_node_v2(ast, child_ref, lang, env)?);
                            }
                        }
                    }
                    let mut pctx = handl_platform::EvalContext::new();
                    func.call(&arg_vals, &mut pctx)
                        .map_err(|e| LangError::Engine(e.to_string()))
                } else {
                    Ok(node.payload.clone())
                }
            }
        }
    }

    /// Convenience: look up the v1 `EvalSchemeDef` for the node, lower it to
    /// v2 via [`lower_v1_to_v2`], and interpret. This demonstrates the v2
    /// handler running v1 languages backward-compatibly (ADR-0052).
    pub fn eval_lowered(
        &self,
        ast: &Ast,
        node_ref: AstRef,
        lang: &Language,
        env: &mut ScopeStack,
    ) -> Result<Value, LangError> {
        self.eval_node_v2(ast, node_ref, lang, env)
    }
}

impl Default for V2Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Small numeric-op helpers
// ---------------------------------------------------------------------------

/// Binary numeric op helper.
fn bin_num<F>(args: &[Value], f: F) -> Result<(Value, bool), LangError>
where
    F: FnOnce(Num, Num) -> Num,
{
    let a = as_num(&args[0])?;
    let b = as_num(&args[1])?;
    Ok((num_value(f(a, b)), false))
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{AstNode, EvalSchemeDef, Language, Scheme};
    use handl_platform::{EvalContext, Function, Module, ModuleLookup};
    use handl_plugin_module::ModuleRegistry;
    use serde_json::json;

    /// Build a simple AST for testing (mirrors the v1 test helper).
    fn make_ast(nodes: Vec<AstNode>) -> Ast {
        let mut ast = Ast::new();
        for n in nodes {
            ast.push(n);
        }
        ast
    }

    /// A minimal node used as the "current node" for scheme-only tests that
    /// do not need child resolution. Its payload is an empty object.
    fn dummy_node() -> AstNode {
        AstNode::new("test").with_payload(json!({}))
    }

    /// Evaluate a standalone scheme with a fresh env and a dummy current
    /// node, passing `None` as incoming.
    fn eval_standalone(
        engine: &V2Interpreter,
        scheme: &SchemeV2,
        env: &mut ScopeStack,
    ) -> Result<Value, LangError> {
        let ast = make_ast(vec![dummy_node()]);
        let node = ast.at(AstRef::new(0)).unwrap();
        engine.eval_scheme_v2(scheme, &ast, AstRef::new(0), &Language::new(), env, node, None)
    }

    // 1. arr const ---------------------------------------------------------

    #[test]
    fn arr_const_returns_literal() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::arr_native_lit("const", vec![], json!(42));
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn arr_const_null_when_no_literal() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::arr_native("const", vec![]);
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, Value::Null);
    }

    // 2. arr proj_env ------------------------------------------------------

    #[test]
    fn arr_proj_env_bound_lookup() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        env.define("x", json!(7));
        let s = SchemeV2::arr_native_lit("proj_env", vec![], "x");
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(7));
    }

    #[test]
    fn arr_proj_env_unbound_returns_name() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::arr_native_lit("proj_env", vec![], "unknown");
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, Value::from("unknown"));
    }

    // 3. Seq const >>> id --------------------------------------------------

    #[test]
    fn seq_const_then_id() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], json!(5)),
            SchemeV2::arr_native("id", vec![]),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(5));
    }

    // 4. Bind shape: Seq(Seq(const 5, extend_env "x"), proj_env "x") -------

    #[test]
    fn bind_shape_extend_env_then_proj_env() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::seq(
            SchemeV2::seq(
                SchemeV2::arr_native_lit("const", vec![], json!(5)),
                SchemeV2::arr_native_lit("extend_env", vec![], "x"),
            ),
            SchemeV2::arr_native_lit("proj_env", vec![], "x"),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(5));
        // Scope must be popped after the Seq: "x" no longer visible.
        assert!(env.lookup("x").is_none());
    }

    // 5. First on a pair ---------------------------------------------------

    #[test]
    fn first_id_on_pair() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        // Top-level: First needs an incoming pair. Wrap in a const that
        // produces the pair, then Seq into First.
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], pair(json!(3), json!(7))),
            SchemeV2::first(SchemeV2::arr_native("id", vec![])),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, pair(json!(3), json!(7)));
    }

    #[test]
    fn first_const_on_pair() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], pair(json!(3), json!(7))),
            SchemeV2::first(SchemeV2::arr_native_lit("const", vec![], json!(99))),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, pair(json!(99), json!(7)));
    }

    // 6. Choice on Either --------------------------------------------------

    #[test]
    fn choice_left() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], either_left(json!(5))),
            SchemeV2::choice(
                SchemeV2::arr_native_lit("const", vec![], json!(1)),
                SchemeV2::arr_native_lit("const", vec![], json!(2)),
            ),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn choice_right() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], either_right(json!(5))),
            SchemeV2::choice(
                SchemeV2::arr_native_lit("const", vec![], json!(1)),
                SchemeV2::arr_native_lit("const", vec![], json!(2)),
            ),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(2));
    }

    // 7. tag + Choice (Branch lowering) ------------------------------------

    #[test]
    fn tag_choice_true_yields_then() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::seq(
            SchemeV2::seq(
                SchemeV2::arr_native_lit("const", vec![], json!(true)),
                SchemeV2::arr_native("tag", vec![]),
            ),
            SchemeV2::choice(
                SchemeV2::arr_native_lit("const", vec![], "yes"),
                SchemeV2::arr_native_lit("const", vec![], "no"),
            ),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!("yes"));
    }

    #[test]
    fn tag_choice_false_yields_else() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::seq(
            SchemeV2::seq(
                SchemeV2::arr_native_lit("const", vec![], json!(false)),
                SchemeV2::arr_native("tag", vec![]),
            ),
            SchemeV2::choice(
                SchemeV2::arr_native_lit("const", vec![], "yes"),
                SchemeV2::arr_native_lit("const", vec![], "no"),
            ),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!("no"));
    }

    // 8. Loop causal single-step ------------------------------------------

    #[test]
    fn loop_causal_single_step() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        // body: inc_fst maps pair(in, state) -> pair(in+1, state).
        // Loop seeds state=Null, feeds pair(in, Null), returns out=in+1.
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], json!(10)),
            SchemeV2::loop_feedback(SchemeV2::arr_native("inc_fst", vec![])),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(11));
    }

    // 8b. Loop multi-step (Either-based) ----------------------------------

    #[test]
    fn loop_multi_step_either_countdown() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        // input = pair(42, 5): result=42, countdown from 5.
        // countdown_step: state>0 -> either_right(state-1) (continue);
        // state==0 -> either_left(42) (done). Runs 5 steps, returns 42.
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], pair(json!(42), json!(5))),
            SchemeV2::loop_feedback(SchemeV2::arr_native("countdown_step", vec![])),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn loop_multi_step_max_iterations_guard() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        // infinite_step always returns either_right(state) — never
        // terminates. The Loop max-iteration guard must fire.
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], json!(1)),
            SchemeV2::loop_feedback(SchemeV2::arr_native("infinite_step", vec![])),
        );
        let err = eval_standalone(&engine, &s, &mut env).unwrap_err();
        assert!(matches!(err, LangError::Engine(ref m) if m.contains("max iterations")));
    }

    #[test]
    fn loop_multi_step_single_step_still_works() {
        // Backward compat: a body returning a pair (not an Either) runs
        // exactly once and returns fst. This is the original single-step
        // behavior, now the fallback when the result is not an Either.
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], json!(10)),
            SchemeV2::loop_feedback(SchemeV2::arr_native("inc_fst", vec![])),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(11));
    }

    // 9. App with a closure value -----------------------------------------

    #[test]
    fn app_closure_applies_body() {
        let engine = V2Interpreter::new();
        // AST:
        // 0: var("x") payload "x"  -- body node; its scheme returns proj_env "x"
        // 1: test node (current)   -- the node whose scheme is the App
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("test").with_payload(json!({})),
        ]);
        // Build a closure value pointing at body node 0, param ["x"].
        let closure = json!({
            "__closure__": true,
            "params": ["x"],
            "body": 0,
            "env": {},
        });
        // App(callee = const <closure>, args = [const 41])
        let s = SchemeV2::app(
            SchemeV2::arr_native_lit("const", vec![], closure),
            vec![SchemeV2::arr_native_lit("const", vec![], json!(41))],
        );
        let mut env = ScopeStack::new();
        let node = ast.at(AstRef::new(1)).unwrap();
        let result = engine
            .eval_scheme_v2(&s, &ast, AstRef::new(1), &lang, &mut env, node, None)
            .unwrap();
        assert_eq!(result, json!(41));
    }

    // 10. eval_lowered end-to-end -----------------------------------------

    #[test]
    fn eval_lowered_lit_node() {
        let engine = V2Interpreter::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.lit", "lit", Scheme::lit(42)))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!(42))]);
        let mut env = ScopeStack::new();
        let result = engine
            .eval_lowered(&ast, AstRef::new(0), &lang, &mut env)
            .unwrap();
        assert_eq!(result, json!(42));
    }

    // 11. lower_v1_to_v2 + Branch -----------------------------------------

    #[test]
    fn lowered_branch_true_takes_then() {
        let engine = V2Interpreter::new();
        let mut lang = Language::new();
        // "if" node: Branch { cond: child(0), then: child(1), else: child(2) }
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
        let engine = V2Interpreter::new();
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
        let mut env = ScopeStack::new();
        let result = engine
            .eval_lowered(&ast, AstRef::new(3), &lang, &mut env)
            .unwrap();
        assert_eq!(result, json!(2));
    }

    // --- Arithmetic / comparison parity with v1 --------------------------

    #[test]
    fn native_add() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::arr_native(
            "add",
            vec![
                SchemeV2::arr_native_lit("const", vec![], json!(2)),
                SchemeV2::arr_native_lit("const", vec![], json!(3)),
            ],
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn native_eq() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::arr_native(
            "eq",
            vec![
                SchemeV2::arr_native_lit("const", vec![], json!(2)),
                SchemeV2::arr_native_lit("const", vec![], json!(2)),
            ],
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(true));
    }

    // --- Foreign dispatch -------------------------------------------------

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
        let engine = V2Interpreter::new().with_modules(double_registry());
        let mut env = ScopeStack::new();
        let s = SchemeV2::arr_foreign(
            "double",
            vec![SchemeV2::arr_native_lit("const", vec![], json!(21))],
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn foreign_arr_no_modules_errors() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::arr_foreign(
            "double",
            vec![SchemeV2::arr_native_lit("const", vec![], json!(21))],
        );
        let err = eval_standalone(&engine, &s, &mut env).unwrap_err();
        assert!(matches!(err, LangError::Engine(ref m) if m.contains("unregistered foreign")));
    }

    // --- proj_child via eval_lowered --------------------------------------

    #[test]
    fn proj_child_evaluates_child_node() {
        let engine = V2Interpreter::new();
        let mut lang = Language::new();
        // "binary.add": add(child(0), child(1)) lowered to v2.
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
        let mut env = ScopeStack::new();
        let result = engine
            .eval_lowered(&ast, AstRef::new(2), &lang, &mut env)
            .unwrap();
        assert_eq!(result, json!(5));
    }

    // --- Unknown op -------------------------------------------------------

    #[test]
    fn unknown_native_op_errors() {
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let s = SchemeV2::arr_native("bogus", vec![]);
        let err = eval_standalone(&engine, &s, &mut env).unwrap_err();
        assert!(matches!(err, LangError::Engine(ref m) if m.contains("unknown v2 native op")));
    }

    // --- Globals injection ------------------------------------------------

    #[test]
    fn with_global_visible_in_env() {
        let engine = V2Interpreter::new().with_global("self", json!("my-lang"));
        let mut env = ScopeStack::new();
        for (name, val) in &engine.globals {
            env.define(name, val.clone());
        }
        let s = SchemeV2::arr_native_lit("proj_env", vec![], "self");
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!("my-lang"));
    }
}
