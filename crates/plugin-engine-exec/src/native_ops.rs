//! Shared native-op semantics for the arrow-calculus kernel (ADR-0044 §7,
//! ADR-0052).
//!
//! `lang.eval` schemes express *what* a node does as `Term::Op` data; the
//! op names are the calculus vocabulary and their **semantics is defined
//! once here** — not re-implemented per backend. Each exec engine
//! (tree-walk, bytecode, cranelift) is a different *interpretation* of the
//! same schemes: it controls how `Term` is traversed and how op arguments
//! are produced (tree-walk evaluates sub-schemes lazily, bytecode compiles
//! them to instruction ranges, cranelift to native code), but when an op's
//! meaning is needed it delegates to [`apply_native_op`] /
//! [`step_decision`] with an [`NativeOpCtx`] implementation supplying only
//! the engine-local *globals*: scope stack, node evaluation, arrow
//! application.
//!
//! This eliminates the historical 3x copy-paste of `apply_native`
//! (~60 arms) plus the `Num`/`as_num`/`num_value`/`is_truthy` helpers that
//! had already drifted apart (e.g. string `add`, `countdown_step` int
//! preservation, `Arith` f64 round-tripping).
//!
//! Ops that genuinely require deep engine machinery (imports, module
//! calls, `eval_block`, `closure` capture shapes, FFI) stay behind
//! [`NativeOpCtx`] services — or remain backend arms until their service
//! is declared. [`apply_native_op`] returns `Ok(None)` for op names it
//! does not own, so backends compose it ahead of their residual arms.

use std::rc::Rc;

use handl_language::{AstRef, LangError, Value};
use handl_value::{
    TaggedValue, cell_comp_axis, cell_compose, cell2_comp_v, cell2_id, cell2_src, cell2_tgt,
    cell2_whisker, either_left, either_right, is_either_left, is_either_right, pair, pair_fst,
    pair_snd, record_proj,
};
use rustc_hash::FxHashMap;
use serde_json::json;

// ===========================================================================
// Num — int/float-preserving arithmetic (moved from the 3 backend copies)
// ===========================================================================

/// A number that tracks int-vs-float so integer arithmetic stays exact.
#[derive(Debug, Clone, Copy)]
pub enum Num {
    /// Integer value.
    Int(i64),
    /// Float value.
    Float(f64),
}

impl Num {
    /// Convert to `f64`.
    pub fn to_f64(self) -> f64 {
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

/// Extract a number from a [`TaggedValue`].
pub fn as_num(v: &TaggedValue) -> Result<Num, LangError> {
    match v {
        TaggedValue::Int(i) => Ok(Num::Int(*i)),
        TaggedValue::Float(f) => Ok(Num::Float(*f)),
        _ => Err(LangError::engine(format!("expected number, got {:?}", v))),
    }
}

/// Convert a [`Num`] back to a [`TaggedValue`].
pub fn num_value(n: Num) -> TaggedValue {
    match n {
        Num::Int(i) => TaggedValue::Int(i),
        Num::Float(f) => TaggedValue::Float(f),
    }
}

/// Whether a [`TaggedValue`] is truthy.
///
/// `false`, `null`, `0`, and `""` are falsy; everything else is truthy.
pub fn is_truthy(v: &TaggedValue) -> bool {
    match v {
        TaggedValue::Bool(b) => *b,
        TaggedValue::Null => false,
        TaggedValue::Int(i) => *i != 0,
        TaggedValue::Float(f) => *f != 0.0,
        TaggedValue::Str(s) => !s.is_empty(),
        _ => true,
    }
}

/// The type tag of a [`TaggedValue`] (the `typeof` op vocabulary).
pub fn type_tag(v: &TaggedValue) -> &'static str {
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

// ===========================================================================
// Numeric helpers (moved from the 3 backend copies)
// ===========================================================================

/// Binary numeric op helper.
pub fn bin_num<F>(args: &[TaggedValue], f: F) -> Result<TaggedValue, LangError>
where
    F: FnOnce(Num, Num) -> Num,
{
    let a = as_num(&args[0])?;
    let b = as_num(&args[1])?;
    Ok(num_value(f(a, b)))
}

/// Addition helper. Supports string concatenation when both operands are
/// strings, and numeric addition when both operands are numbers. Any other
/// combination of types is a type-mismatch error.
///
/// This is the canonical `add` semantics — the bytecode `Arith`/`add`
/// implementations that lacked string concatenation (and round-tripped
/// ints through `f64`) were drift, now unified here.
pub fn bin_add(args: &[TaggedValue]) -> Result<TaggedValue, LangError> {
    let a = &args[0];
    let b = &args[1];
    match (a, b) {
        (TaggedValue::Str(s1), TaggedValue::Str(s2)) => {
            Ok(TaggedValue::Str(Rc::from(format!("{}{}", s1, s2))))
        }
        (TaggedValue::Str(_), _) | (_, TaggedValue::Str(_)) => Err(LangError::engine(format!(
            "add: type mismatch ({} + {}): string concatenation requires both operands to be strings",
            handl_value::tagged_to_value(a),
            handl_value::tagged_to_value(b)
        ))),
        _ => bin_num(args, |x, y| x + y),
    }
}

/// Numeric comparison helper.
pub fn cmp_num<F>(args: &[TaggedValue], f: F) -> Result<TaggedValue, LangError>
where
    F: FnOnce(Num, Num) -> bool,
{
    let a = as_num(&args[0])?;
    let b = as_num(&args[1])?;
    Ok(TaggedValue::Bool(f(a, b)))
}

// ===========================================================================
// Engine services — the only things that differ per interpretation
// ===========================================================================

/// An engine service a shared op needs. [`NativeOpCtx::supports`] gates
/// each service so a backend that cannot provide it (e.g. the bytecode VM
/// cannot re-enter AST evaluation for `proj_child`) declines the op and
/// falls back to its own machinery instead of failing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpService {
    /// Evaluate an AST arena node (child projection: `proj_child`,
    /// `paren`, `assign`, …).
    EvalNode,
    /// Scope stack access (`proj_env`, `extend_env`, `closure` capture).
    Env,
    /// Apply an arrow value to arguments (`first`, `app`, `loop_step`, …).
    ApplyArrow,
}

/// Engine-specific globals the shared ops need.
///
/// Each exec backend implements this once; the op *semantics* then live in
/// this module. This is the same unification pattern as
/// [`crate::pattern_match::PatternMatchCtx`].
pub trait NativeOpCtx {
    /// Whether this engine can provide `service`. Returning `false` makes
    /// [`apply_native_op`] decline dependent ops (`Ok(None)`) so the
    /// backend's own machinery handles them.
    fn supports(&self, service: OpService) -> bool;

    /// The payload of the node whose scheme is running (`desugar_error`,
    /// `$payload` resolution). `None` when no node context is available.
    fn node_payload(&self) -> Option<&Value>;

    /// The payload of an arbitrary arena node — a *read*, not evaluation
    /// (`json`'s template child is a raw string leaf; evaluating it would
    /// run its own `json` scheme and parse the template prematurely).
    /// Engines that report [`OpService::EvalNode`] have the arena.
    fn node_payload_at(&mut self, r: AstRef) -> Option<Value>;

    /// Resolve the named child `role` of the node whose scheme is running
    /// to its position in the node's `children` array (ADR-0081 —
    /// `Language::child_role_index` semantics). `None` when the shape
    /// declares no such role or the engine has no `Language`; the op then
    /// applies its documented positional default.
    fn child_role_index(&mut self, role: &str) -> Option<usize>;

    /// The **arity-aware** counterpart of
    /// [`child_role_index`](Self::child_role_index) —
    /// `Language::child_role_index_for` semantics on the node whose
    /// scheme is running: roles declared after an `optional` slot shift
    /// when the slot is absent at the node's actual arity, and an
    /// `optional` role absent at this arity resolves to `None` (e.g.
    /// `assign`'s trailing `body` on the two-child form). `None` =
    /// undeclared or unoccupied; the op's positional default applies.
    fn child_role_index_for(&mut self, role: &str) -> Option<usize>;

    /// Whether the shape of the node at `r` declares child `role`
    /// (`Language::child_role_index` full-form semantics) — the
    /// `is_assign_node`/`is_return_node` checks `eval_block`/`eval_rec`
    /// apply to each child. `false` for role-less kinds, missing nodes,
    /// and engines without a `Language` (standalone bootstrap).
    fn role_declared_at(&mut self, r: AstRef, role: &str) -> bool;

    /// The arity-aware positional index of `role` among the children of
    /// the node at `r` (`Language::child_role_index_for` on that node) —
    /// `eval_block`/`eval_rec`/`binding_target_name` resolve *child* node
    /// roles this way. `None` = undeclared, unoccupied optional slot, or
    /// missing node.
    fn child_role_index_at(&mut self, r: AstRef, role: &str) -> Option<usize>;

    /// Evaluate the AST node at `r`.
    ///
    /// # Errors
    ///
    /// Implementations that cannot re-enter AST evaluation should return
    /// `Err` and report [`OpService::EvalNode`] as unsupported.
    fn eval_node(&mut self, r: AstRef) -> Result<TaggedValue, LangError>;

    /// Look up `name` in the scope stack.
    fn env_lookup(&mut self, name: &str) -> Option<TaggedValue>;

    /// Define `name = value` in the innermost scope.
    fn env_define(&mut self, name: &str, value: TaggedValue);

    /// Push a new innermost scope.
    fn env_push(&mut self);

    /// Pop the innermost scope.
    fn env_pop(&mut self);

    /// `x = y` statement semantics (ADR-0074 feedback wires): update the
    /// nearest existing binding in an enclosing scope (innermost-first),
    /// or define it in the innermost scope when `name` is not already
    /// bound. A cross-scope write that changes a value raises the
    /// `enclosing_updated` signal [`env_take_enclosing_updated`](Self::env_take_enclosing_updated)
    /// reads.
    fn env_assign(&mut self, name: &str, value: TaggedValue);

    /// Read and clear the scope stack's cross-scope-write flag — the
    /// `enclosing_updated` signal `eval_rec`'s fixpoint check consumes:
    /// `true` means a nested [`env_assign`](Self::env_assign) updated an
    /// enclosing binding during this pass, so the pass must not count as
    /// converged.
    fn env_take_enclosing_updated(&mut self) -> bool;

    /// Discard any deferred error a failed
    /// [`eval_node`](Self::eval_node) left behind. The cranelift backend
    /// routes errors through a `ctx.error` slot that a skipped `eval_rec`
    /// binding must not leak into the next pass; engines that return
    /// errors directly keep the default no-op.
    fn clear_eval_error(&mut self) {}

    /// Capture the visible environment for a closure.
    fn env_capture(&mut self) -> Rc<FxHashMap<Rc<str>, TaggedValue>>;

    /// Apply an arrow value to `args`.
    ///
    /// # Errors
    ///
    /// Implementations that cannot apply arrows should return `Err` and
    /// report [`OpService::ApplyArrow`] as unsupported.
    fn apply_arrow(
        &mut self,
        f: &TaggedValue,
        args: &[TaggedValue],
    ) -> Result<TaggedValue, LangError>;
}

// ===========================================================================
// The unified op table
// ===========================================================================

/// The result of a shared op: the produced value plus whether the op
/// pushed a scope the caller must later pop (the `extend_env` contract —
/// see the tree-walk's `(TaggedValue, bool)` convention).
pub type OpResult = Result<(TaggedValue, bool), LangError>;

/// Apply a native op by name.
///
/// `args` are the already-evaluated argument values (each backend produces
/// them its own way — that is the interpretation). `lit` is the op's
/// inline literal operand; `incoming` is the value flowing through the
/// arrow; `ctx` supplies engine globals.
///
/// Returns `Ok(None)` when `op` is not owned by the shared table (the
/// backend then runs its residual arms), and `Ok(None)` when a required
/// [`OpService`] is unsupported by `ctx`.
///
/// # Errors
///
/// Returns `Err` when the op is owned but its semantics fail (type
/// mismatch, missing literal, service error).
pub fn apply_native_op(
    op: &str,
    args: &[TaggedValue],
    lit: Option<&Value>,
    incoming: Option<TaggedValue>,
    ctx: &mut dyn NativeOpCtx,
) -> Result<Option<(TaggedValue, bool)>, LangError> {
    let result: OpResult = match op {
        "const" => Ok((
            TaggedValue::from(lit.cloned().unwrap_or(Value::Null)),
            false,
        )),
        "id" => Ok((
            incoming
                .or_else(|| args.first().cloned())
                .unwrap_or(TaggedValue::Null),
            false,
        )),
        "tag" => {
            let v = incoming
                .or_else(|| args.first().cloned())
                .unwrap_or(TaggedValue::Null);
            if is_truthy(&v) {
                Ok((either_left(v), false))
            } else {
                Ok((either_right(v), false))
            }
        }
        "desugar_error" => {
            // The `desugar-error` node kind / `data` payload carry the
            // reason; see eval.yaml. Reads `current_node.payload` first,
            // then the scheme `lit`.
            let payload = ctx.node_payload().cloned().unwrap_or(Value::Null);
            let lit_obj = lit.and_then(|v| v.as_object());
            let reason = payload
                .get("reason")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    lit_obj
                        .and_then(|o| o.get("reason"))
                        .and_then(|v| v.as_str())
                })
                .unwrap_or("unknown desugar error");
            let kind = payload
                .get("kind")
                .and_then(|v| v.as_str())
                .or_else(|| lit_obj.and_then(|o| o.get("kind")).and_then(|v| v.as_str()))
                .unwrap_or("desugar-error");
            return Err(LangError::engine(format!("{}: {}", kind, reason)));
        }
        // --- env ops ---------------------------------------------------
        "proj_env" => {
            if !ctx.supports(OpService::Env) {
                return Ok(None);
            }
            let name = lit
                .and_then(|v| v.as_str())
                .ok_or_else(|| LangError::engine("proj_env: missing name literal".to_string()))?;
            let var_name = if name == "$payload" {
                ctx.node_payload()
                    .and_then(|p| p.as_str())
                    .ok_or_else(|| {
                        LangError::engine(
                            "expected string payload for $payload proj_env".to_string(),
                        )
                    })?
                    .to_string()
            } else {
                name.to_string()
            };
            // Unbound → name as a string (allows identifiers like `eval`
            // to flow into `App` host-function dispatch).
            let val = ctx
                .env_lookup(&var_name)
                .unwrap_or_else(|| TaggedValue::Str(Rc::from(var_name)));
            Ok((val, false))
        }
        "extend_env" => {
            if !ctx.supports(OpService::Env) {
                return Ok(None);
            }
            let name = lit
                .and_then(|v| v.as_str())
                .ok_or_else(|| LangError::engine("extend_env: missing name literal".to_string()))?;
            let bind_name = if name == "$payload" {
                ctx.node_payload()
                    .and_then(|p| p.get("name"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        LangError::engine(
                            "expected name in payload for $payload extend_env".to_string(),
                        )
                    })?
                    .to_string()
            } else {
                name.to_string()
            };
            ctx.env_push();
            let val = incoming.unwrap_or(TaggedValue::Null);
            ctx.env_define(&bind_name, val.clone());
            Ok((val, true))
        }
        // --- scope ops (EvalNode + Env) ----------------------------------
        // ADR-0053: `assign { children: [target, value] }` or
        // `assign { children: [target, value, body] }` — evaluate `value`,
        // optionally bind `target` in a new scope and evaluate `body`, or
        // just return the value. `target`/`value`/`body` positions resolve
        // by named role (ADR-0081); `body` is an `optional` role so its
        // presence resolves arity-aware (`child_role_index_for`), with the
        // `children.len() == 3` check as the role-less fallback. Falls
        // back to legacy `payload.value` for hand-built ASTs.
        "assign" => {
            if !ctx.supports(OpService::EvalNode) || !ctx.supports(OpService::Env) {
                return Ok(None);
            }
            let children = ctx
                .node_payload()
                .and_then(|p| p.get("children"))
                .and_then(|c| c.as_array())
                .cloned();
            match children {
                Some(children) if children.len() >= 2 => {
                    let value_idx = ctx
                        .child_role_index_for("value")
                        .and_then(|pos| children.get(pos).and_then(|v| v.as_u64()))
                        .or_else(|| children.get(1).and_then(|v| v.as_u64()))
                        .map(|i| i as usize)
                        .ok_or_else(|| {
                            LangError::engine("assign: value index must be a number".to_string())
                        })?;
                    let body_idx = match ctx.child_role_index_for("body") {
                        Some(pos) => children.get(pos),
                        // Role-less fallback: a third positional child is
                        // the body.
                        None if children.len() == 3 => children.get(2),
                        None => None,
                    }
                    .map(|v| {
                        v.as_u64().map(|i| i as usize).ok_or_else(|| {
                            LangError::engine("assign: body index must be a number".to_string())
                        })
                    })
                    .transpose()?;
                    if let Some(body_idx) = body_idx {
                        // `x = y in body` — bind `target` in a fresh scope
                        // and evaluate `body` under it; the scope pops
                        // even when `body` errors.
                        let var_idx = ctx
                            .child_role_index_for("target")
                            .and_then(|pos| children.get(pos).and_then(|v| v.as_u64()))
                            .or_else(|| children.first().and_then(|v| v.as_u64()))
                            .map(|i| i as usize)
                            .ok_or_else(|| {
                                LangError::engine("assign: var index must be a number".to_string())
                            })?;
                        let var_name = binding_target_name(ctx, var_idx, "assign")?;
                        let value = ctx.eval_node(AstRef::new(value_idx))?;
                        ctx.env_push();
                        ctx.env_define(&var_name, value);
                        let result = ctx.eval_node(AstRef::new(body_idx));
                        ctx.env_pop();
                        result.map(|v| (v, false))
                    } else {
                        // `x = y` — just evaluate and return the value.
                        ctx.eval_node(AstRef::new(value_idx)).map(|v| (v, false))
                    }
                }
                Some(_) => Err(LangError::engine(
                    "assign: expected at least 2 children".to_string(),
                )),
                // Legacy shape: `assign { target, value }` (hand-built
                // ASTs). Just return the value.
                None => {
                    let value = ctx
                        .node_payload()
                        .and_then(|p| p.get("value").cloned())
                        .ok_or_else(|| {
                            LangError::engine("assign: missing value in payload".to_string())
                        })?;
                    Ok((TaggedValue::from(value), false))
                }
            }
        }
        // `bind` — arrow-calculus env extension (ADR-0052): the node's
        // payload has `name` and `children: [value_idx, body_idx]`;
        // evaluate value, extend env with `name = value`, evaluate body,
        // pop.
        "bind" => {
            if !ctx.supports(OpService::EvalNode) || !ctx.supports(OpService::Env) {
                return Ok(None);
            }
            let payload = ctx.node_payload().cloned().unwrap_or(Value::Null);
            let name = payload
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| LangError::engine("bind: missing name".to_string()))?
                .to_string();
            let children = payload
                .get("children")
                .and_then(|c| c.as_array())
                .ok_or_else(|| {
                    LangError::engine("bind: expected children array in payload".to_string())
                })?;
            let value_idx = children
                .first()
                .and_then(|v| v.as_u64())
                .ok_or_else(|| LangError::engine("bind: missing value child".to_string()))?
                as usize;
            let body_idx = children
                .get(1)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| LangError::engine("bind: missing body child".to_string()))?
                as usize;
            // Evaluate the value in the current env.
            let value = ctx.eval_node(AstRef::new(value_idx))?;
            // Extend env with name = value, evaluate body, pop.
            ctx.env_push();
            ctx.env_define(&name, value);
            let result = ctx.eval_node(AstRef::new(body_idx));
            ctx.env_pop();
            result.map(|v| (v, false))
        }
        // `eval_block` — evaluate all children of a block node in sequence
        // in a fresh scope, returning the last child's value. `assign`
        // children (shapes declaring `target`/`value` roles, two-child
        // `x = v` form only — a let-form `x = v in body` evaluates under
        // its own pushed scope via the `assign` op) bind through
        // `env_assign`: an already-bound `x` updates the nearest enclosing
        // binding (ADR-0074 feedback wires), else it defines a
        // block-local binding. Used by `do { expr; expr; ... }` surface
        // syntax (ADR-0054). Every exit pops the pushed scope.
        "eval_block" => {
            if !ctx.supports(OpService::EvalNode) || !ctx.supports(OpService::Env) {
                return Ok(None);
            }
            let children = ctx
                .node_payload()
                .and_then(|p| p.get("children"))
                .and_then(|c| c.as_array())
                .cloned()
                .ok_or_else(|| {
                    LangError::engine("eval_block: expected children array in payload".to_string())
                })?;
            if children.is_empty() {
                return Ok(Some((TaggedValue::Null, false)));
            }
            ctx.env_push();
            let mut result = TaggedValue::Null;
            let mut eval_err: Option<LangError> = None;
            'block: for entry in children.iter() {
                let idx = match entry.as_u64() {
                    Some(i) => i as usize,
                    None => {
                        eval_err = Some(LangError::engine(
                            "eval_block: child index must be a number".to_string(),
                        ));
                        break 'block;
                    }
                };
                let child = AstRef::new(idx);
                if is_assign_node(ctx, child) {
                    if let Some(ac) = node_children(ctx, child) {
                        if ac.len() == 2 {
                            // Binding target/value by child role
                            // (ADR-0081), falling back to the positional
                            // children for role-less shapes.
                            let var_idx = match role_child_arena(ctx, child, "target")
                                .or_else(|| ac[0].as_u64().map(|i| i as usize))
                            {
                                Some(i) => i,
                                None => {
                                    eval_err = Some(LangError::engine(
                                        "eval_block: assign var index must be a number".to_string(),
                                    ));
                                    break 'block;
                                }
                            };
                            let value_idx = match role_child_arena(ctx, child, "value")
                                .or_else(|| ac[1].as_u64().map(|i| i as usize))
                            {
                                Some(i) => i,
                                None => {
                                    eval_err = Some(LangError::engine(
                                        "eval_block: assign value index must be a number"
                                            .to_string(),
                                    ));
                                    break 'block;
                                }
                            };
                            let var_name = match binding_target_name(ctx, var_idx, "eval_block") {
                                Ok(n) => n,
                                Err(e) => {
                                    eval_err = Some(e);
                                    break 'block;
                                }
                            };
                            match ctx.eval_node(AstRef::new(value_idx)) {
                                Ok(value) => {
                                    ctx.env_assign(&var_name, value.clone());
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
                match ctx.eval_node(child) {
                    Ok(v) => result = v,
                    Err(e) => {
                        eval_err = Some(e);
                        break 'block;
                    }
                }
            }
            ctx.env_pop();
            if let Some(e) = eval_err {
                return Err(e);
            }
            Ok((result, false))
        }
        // `eval_rec` — recursive computation region (ADR-0074). The `rec`
        // node's children are the flattened block bindings: `assign`
        // children (`a = expr`) define bindings that may reference each
        // other cyclically, `return` children exit the region early, and
        // any other expression is evaluated for its result. The op
        // iterates to a fixpoint: each pass re-evaluates every binding's
        // value expression using the current values of other bindings;
        // when no tracked binding's value changes (and no nested
        // cross-scope write was observed — `env_take_enclosing_updated`),
        // the fixpoint is reached. A fuel limit (10000 passes) prevents
        // infinite loops. Binding-eval errors are caught and the binding
        // retried next pass, so mutually-dependent bindings converge
        // regardless of textual order. The scope pops on every exit.
        "eval_rec" => {
            if !ctx.supports(OpService::EvalNode) || !ctx.supports(OpService::Env) {
                return Ok(None);
            }
            let children = ctx
                .node_payload()
                .and_then(|p| p.get("children"))
                .and_then(|c| c.as_array())
                .cloned()
                .ok_or_else(|| {
                    LangError::engine("eval_rec: expected children array in payload".to_string())
                })?;
            if children.is_empty() {
                return Ok(Some((TaggedValue::Null, false)));
            }
            ctx.env_push();
            let region = eval_rec_region(ctx, &children);
            ctx.env_pop();
            region.map(|v| (v, false))
        }
        // --- child projection ops ---------------------------------------
        "proj_child" | "proj_child_or" => {
            if !ctx.supports(OpService::EvalNode) {
                return Ok(None);
            }
            let idx = lit
                .and_then(|v| v.as_u64())
                .ok_or_else(|| LangError::engine(format!("{op}: missing index literal")))?;
            let children = ctx
                .node_payload()
                .and_then(|p| p.get("children"))
                .and_then(|c| c.as_array())
                .ok_or_else(|| {
                    LangError::engine(format!("{op}: expected children array in payload"))
                })?;
            if op == "proj_child_or" && (idx as usize) >= children.len() {
                return Ok(Some((TaggedValue::Null, false)));
            }
            let arena_idx = children
                .get(idx as usize)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| LangError::engine(format!("{op}: child {} not found", idx)))?;
            let val = ctx.eval_node(AstRef::new(arena_idx as usize))?;
            Ok((val, false))
        }
        "paren" => {
            if !ctx.supports(OpService::EvalNode) {
                return Ok(None);
            }
            let children = ctx
                .node_payload()
                .and_then(|p| p.get("children"))
                .and_then(|c| c.as_array())
                .ok_or_else(|| {
                    LangError::engine("paren: expected children array in payload".to_string())
                })?
                .clone();
            if children.is_empty() {
                Ok((TaggedValue::Null, false))
            } else if children.len() == 1 {
                let idx = children[0].as_u64().ok_or_else(|| {
                    LangError::engine("paren: child index must be a number".to_string())
                })? as usize;
                let val = ctx.eval_node(AstRef::new(idx))?;
                Ok((val, false))
            } else {
                let mut vals = Vec::with_capacity(children.len());
                for c in &children {
                    let idx = c.as_u64().ok_or_else(|| {
                        LangError::engine("paren: child index must be a number".to_string())
                    })? as usize;
                    vals.push(ctx.eval_node(AstRef::new(idx))?);
                }
                Ok((TaggedValue::Arr(Rc::from(vals)), false))
            }
        }
        // ADR-0060: `proj_rest` — evaluate all children after index 0 and
        // return them as an array. `eval.call` uses it to collect the
        // variable-arity argument list into a single array value.
        "proj_rest" => {
            if !ctx.supports(OpService::EvalNode) {
                return Ok(None);
            }
            let children = ctx
                .node_payload()
                .and_then(|p| p.get("children"))
                .and_then(|c| c.as_array())
                .cloned()
                .ok_or_else(|| {
                    LangError::engine("proj_rest: expected children array in payload".to_string())
                })?;
            let mut vals = Vec::new();
            for entry in children.iter().skip(1) {
                let idx = entry.as_u64().ok_or_else(|| {
                    LangError::engine("proj_rest: child index must be a number".to_string())
                })? as usize;
                vals.push(ctx.eval_node(AstRef::new(idx))?);
            }
            Ok((TaggedValue::Arr(Rc::from(vals)), false))
        }
        // --- arithmetic / comparison -------------------------------------
        "add" => bin_add(args).map(|v| (v, false)),
        "sub" => bin_num(args, |a, b| a - b).map(|v| (v, false)),
        "mul" => bin_num(args, |a, b| a * b).map(|v| (v, false)),
        "div" => {
            let a = as_num(&args[0])?;
            let b = as_num(&args[1])?;
            if b.to_f64() == 0.0 {
                return Err(LangError::engine("division by zero".to_string()));
            }
            Ok((num_value(a / b), false))
        }
        "mod" => {
            let a = as_num(&args[0])?;
            let b = as_num(&args[1])?;
            if b.to_f64() == 0.0 {
                return Err(LangError::engine("modulo by zero".to_string()));
            }
            Ok((num_value(a % b), false))
        }
        "neg" => {
            let a = as_num(&args[0])?;
            Ok((
                num_value(match a {
                    Num::Int(i) => Num::Int(-i),
                    Num::Float(f) => Num::Float(-f),
                }),
                false,
            ))
        }
        "pow" => {
            let a = as_num(&args[0])?;
            let b = as_num(&args[1])?;
            Ok((num_value(Num::Float(a.to_f64().powf(b.to_f64()))), false))
        }
        "eq" => Ok((
            TaggedValue::Bool(handl_value::eq(&args[0], &args[1])),
            false,
        )),
        "neq" => Ok((
            TaggedValue::Bool(!handl_value::eq(&args[0], &args[1])),
            false,
        )),
        "lt" => cmp_num(args, |a, b| a < b).map(|v| (v, false)),
        "gt" => cmp_num(args, |a, b| a > b).map(|v| (v, false)),
        "lte" => cmp_num(args, |a, b| a <= b).map(|v| (v, false)),
        "gte" => cmp_num(args, |a, b| a >= b).map(|v| (v, false)),
        "compare" => {
            let a = as_num(&args[0])?;
            let b = as_num(&args[1])?;
            let cmp = a
                .partial_cmp(&b)
                .ok_or_else(|| LangError::engine("compare: incomparable values".to_string()))?;
            Ok((
                num_value(Num::Int(match cmp {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                })),
                false,
            ))
        }
        "number_add" => {
            // Collect the two addends from explicit args, from the
            // second component of an incoming pair, or from an incoming
            // array. This lets a handler `Term::Op { name: "number_add",
            // args: [] }` consume the `(incoming, evaluated_args)` pair
            // that `dispatch_op` passes. Operands are `Number`-tagged
            // records; the result is a re-minted `Number` tag.
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
                return Err(LangError::engine(
                    "number_add: expected at least two Number inputs".to_string(),
                ));
            }
            let a_num = as_num(operands[0].get("value").ok_or_else(|| {
                LangError::engine("number_add: first Number has no value".to_string())
            })?)?;
            if operands[0].get("__tag__").and_then(|t| t.as_str()) != Some("Number") {
                return Err(LangError::engine(
                    "number_add: first input is not a Number".to_string(),
                ));
            }
            let b_num = as_num(operands[1].get("value").ok_or_else(|| {
                LangError::engine("number_add: second Number has no value".to_string())
            })?)?;
            if operands[1].get("__tag__").and_then(|t| t.as_str()) != Some("Number") {
                return Err(LangError::engine(
                    "number_add: second input is not a Number".to_string(),
                ));
            }
            let sum = a_num + b_num;
            let first_obj = handl_value::tagged_to_value(&operands[0]);
            let mut payload = first_obj.as_object().cloned().unwrap_or_default();
            payload.remove("__tag__");
            payload.insert("value".to_string(), Value::from(num_value(sum)));
            let result =
                handl_language::tag_value("Number", &Value::Object(payload)).ok_or_else(|| {
                    LangError::engine("number_add: failed to mint Number result".to_string())
                })?;
            Ok((TaggedValue::from(result), false))
        }
        // --- logic --------------------------------------------------------
        "not" => Ok((TaggedValue::Bool(!is_truthy(&args[0])), false)),
        "and" => Ok((
            TaggedValue::Bool(is_truthy(&args[0]) && is_truthy(&args[1])),
            false,
        )),
        "or" => Ok((
            TaggedValue::Bool(is_truthy(&args[0]) || is_truthy(&args[1])),
            false,
        )),
        // --- unary numeric helpers (arrow_iter / loop tests) ---------------
        "is_pos" => {
            let v = incoming.unwrap_or(TaggedValue::Null);
            let n = as_num(&v)?;
            Ok((TaggedValue::Bool(n > Num::Int(0)), false))
        }
        "dec" => {
            let v = incoming.unwrap_or(TaggedValue::Null);
            let n = as_num(&v)?;
            Ok((num_value(n - Num::Int(1)), false))
        }
        "inc" => {
            let v = incoming.unwrap_or(TaggedValue::Null);
            let n = as_num(&v)?;
            Ok((num_value(n + Num::Int(1)), false))
        }
        "half_floor" => {
            let v = incoming.unwrap_or(TaggedValue::Null);
            let n = as_num(&v)?;
            let result = match n {
                Num::Int(i) => Num::Int(i / 2),
                Num::Float(f) => Num::Float((f / 2.0).floor()),
            };
            Ok((num_value(result), false))
        }
        "inc_fst" => {
            let inc = args
                .first()
                .cloned()
                .or(incoming)
                .unwrap_or(TaggedValue::Null);
            let a = pair_fst(&inc)
                .ok_or_else(|| LangError::engine("inc_fst: input is not a pair".to_string()))?
                .clone();
            let b = pair_snd(&inc)
                .ok_or_else(|| LangError::engine("inc_fst: input is not a pair".to_string()))?
                .clone();
            let a_n = as_num(&a)?;
            Ok((pair(num_value(a_n + Num::Int(1)), b), false))
        }
        "countdown_step" => {
            let inc = args
                .first()
                .cloned()
                .or(incoming)
                .unwrap_or(TaggedValue::Null);
            let a = pair_fst(&inc).cloned().ok_or_else(|| {
                LangError::engine("countdown_step: input is not a pair".to_string())
            })?;
            let b = pair_snd(&inc).cloned().unwrap_or(TaggedValue::Null);
            let result_val = pair_fst(&a).cloned().ok_or_else(|| {
                LangError::engine("countdown_step: in must be pair(result, initial)".to_string())
            })?;
            let initial = pair_snd(&a).cloned().unwrap_or(TaggedValue::Null);
            let state = if b.is_null() { initial } else { b };
            let n = as_num(&state)?;
            if n > Num::Int(0) {
                Ok((either_right(num_value(n - Num::Int(1))), false))
            } else {
                Ok((either_left(result_val), false))
            }
        }
        "infinite_step" => {
            let inc = args
                .first()
                .cloned()
                .or(incoming)
                .unwrap_or(TaggedValue::Null);
            let b = pair_snd(&inc).cloned().unwrap_or(TaggedValue::Null);
            Ok((either_right(b), false))
        }
        // --- product / sum / routing --------------------------------------
        "product" => {
            let left = args.first().cloned().unwrap_or(TaggedValue::Null);
            let right = args.get(1).cloned().unwrap_or(TaggedValue::Null);
            Ok((pair(left, right), false))
        }
        "sum" => {
            if !ctx.supports(OpService::ApplyArrow) {
                return Ok(None);
            }
            let f = args.first().cloned().unwrap_or(TaggedValue::Null);
            let g = args.get(1).cloned().unwrap_or(TaggedValue::Null);
            let input = incoming
                .or_else(|| args.get(2).cloned())
                .unwrap_or(TaggedValue::Null);
            if is_either_left(&input) {
                let v = handl_value::either_value(&input)
                    .cloned()
                    .unwrap_or(TaggedValue::Null);
                let fv = ctx.apply_arrow(&f, &[v])?;
                Ok((either_left(fv), false))
            } else if is_either_right(&input) {
                let v = handl_value::either_value(&input)
                    .cloned()
                    .unwrap_or(TaggedValue::Null);
                let gv = ctx.apply_arrow(&g, &[v])?;
                Ok((either_right(gv), false))
            } else {
                Err(LangError::engine("sum: input is not an either".to_string()))
            }
        }
        "first" => {
            if !ctx.supports(OpService::ApplyArrow) {
                return Ok(None);
            }
            let f = args.first().cloned().unwrap_or(TaggedValue::Null);
            let input = incoming
                .or_else(|| args.get(1).cloned())
                .unwrap_or(TaggedValue::Null);
            let a = pair_fst(&input)
                .cloned()
                .ok_or_else(|| LangError::engine("first: input is not a pair".to_string()))?;
            let b = pair_snd(&input).cloned().unwrap_or(TaggedValue::Null);
            let fa = ctx.apply_arrow(&f, &[a])?;
            Ok((pair(fa, b), false))
        }
        "second" => {
            if !ctx.supports(OpService::ApplyArrow) {
                return Ok(None);
            }
            let f = args.first().cloned().unwrap_or(TaggedValue::Null);
            let input = incoming
                .or_else(|| args.get(1).cloned())
                .unwrap_or(TaggedValue::Null);
            let a = pair_fst(&input).cloned().unwrap_or(TaggedValue::Null);
            let b = pair_snd(&input)
                .cloned()
                .ok_or_else(|| LangError::engine("second: input is not a pair".to_string()))?;
            let fb = ctx.apply_arrow(&f, &[b])?;
            Ok((pair(a, fb), false))
        }
        "left" => {
            if !ctx.supports(OpService::ApplyArrow) {
                return Ok(None);
            }
            let f = args.first().cloned().unwrap_or(TaggedValue::Null);
            let input = incoming
                .or_else(|| args.get(1).cloned())
                .unwrap_or(TaggedValue::Null);
            if is_either_left(&input) {
                let v = handl_value::either_value(&input)
                    .cloned()
                    .unwrap_or(TaggedValue::Null);
                let fv = ctx.apply_arrow(&f, &[v])?;
                Ok((either_left(fv), false))
            } else if is_either_right(&input) {
                Ok((input, false))
            } else {
                Err(LangError::engine(
                    "left: input is not an either".to_string(),
                ))
            }
        }
        "right" => {
            if !ctx.supports(OpService::ApplyArrow) {
                return Ok(None);
            }
            let f = args.first().cloned().unwrap_or(TaggedValue::Null);
            let input = incoming
                .or_else(|| args.get(1).cloned())
                .unwrap_or(TaggedValue::Null);
            if is_either_right(&input) {
                let v = handl_value::either_value(&input)
                    .cloned()
                    .unwrap_or(TaggedValue::Null);
                let fv = ctx.apply_arrow(&f, &[v])?;
                Ok((either_right(fv), false))
            } else if is_either_left(&input) {
                Ok((input, false))
            } else {
                Err(LangError::engine(
                    "right: input is not an either".to_string(),
                ))
            }
        }
        "app" => {
            if !ctx.supports(OpService::ApplyArrow) {
                return Ok(None);
            }
            let f = args.first().cloned().unwrap_or(TaggedValue::Null);
            let x = args
                .get(1)
                .cloned()
                .or(incoming)
                .unwrap_or(TaggedValue::Null);
            let v = ctx.apply_arrow(&f, &[x])?;
            Ok((v, false))
        }
        // --- option / field ops -------------------------------------------
        "nullish" => {
            // `a ?? b`: `null`/`None` coalesce to `b`, `Some(v)` unwraps
            // to `v`, and any other (non-null) value passes through as
            // itself. The final arm used to return `right` — `3 ?? 7`
            // yielded `7` — contradicting the `eval.nullish` scheme,
            // which routes a truthy left through `tag`/`Choice` back to
            // `proj_child 0`, i.e. `a` itself. Pass-through is the
            // intended semantics; `None` still coalesces (restored arm).
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
                Ok((left, false))
            }
        }
        "opt_chain" => {
            let input = args.first().cloned().unwrap_or(TaggedValue::Null);
            let field_name = args.get(1).and_then(|v| v.as_str()).ok_or_else(|| {
                LangError::engine("opt_chain: field name must be a string".to_string())
            })?;
            if input.is_null() {
                Ok((opt_none(), false))
            } else if input.get("_tag").and_then(|t| t.as_str()) == Some("Some") {
                let inner = input.get("value").cloned().unwrap_or(TaggedValue::Null);
                let field_val = inner.get(field_name).cloned().unwrap_or(TaggedValue::Null);
                Ok((opt_some(field_val), false))
            } else if input.get("_tag").and_then(|t| t.as_str()) == Some("None") {
                Ok((opt_none(), false))
            } else {
                let field_val = input.get(field_name).cloned().unwrap_or(TaggedValue::Null);
                Ok((opt_some(field_val), false))
            }
        }
        "index" => {
            // Two surfaces, one semantics:
            // - value form: `target`/`index` arrive as evaluated args
            //   (the `prim("index", [child0, child1])` scheme surface);
            // - node form: the node carries `children: [t, i]` — or the
            //   legacy `payload.target`/`payload.index` — evaluated via
            //   `EvalNode` (the compiled backends' surface).
            let (target, index) = if args.len() >= 2 {
                (args[0].clone(), args[1].clone())
            } else {
                if !ctx.supports(OpService::EvalNode) {
                    return Ok(None);
                }
                let payload = ctx.node_payload().cloned().unwrap_or(Value::Null);
                if let Some(children) = payload.get("children").and_then(|c| c.as_array()) {
                    if children.len() < 2 {
                        return Err(LangError::engine(
                            "index: expected at least 2 children".to_string(),
                        ));
                    }
                    let target_idx = children[0].as_u64().ok_or_else(|| {
                        LangError::engine("index: target index must be a number".to_string())
                    })? as usize;
                    let index_idx = children[1].as_u64().ok_or_else(|| {
                        LangError::engine("index: index index must be a number".to_string())
                    })? as usize;
                    (
                        ctx.eval_node(AstRef::new(target_idx))?,
                        ctx.eval_node(AstRef::new(index_idx))?,
                    )
                } else {
                    let t = payload
                        .get("target")
                        .cloned()
                        .map(TaggedValue::from)
                        .ok_or_else(|| {
                            LangError::engine("index: missing target in payload".to_string())
                        })?;
                    let i = payload
                        .get("index")
                        .cloned()
                        .map(TaggedValue::from)
                        .ok_or_else(|| {
                            LangError::engine("index: missing index in payload".to_string())
                        })?;
                    (t, i)
                }
            };
            match (target, index) {
                (TaggedValue::Arr(arr), index) => {
                    let i = index
                        .as_f64()
                        .filter(|n| *n >= 0.0 && n.fract() == 0.0)
                        .map(|n| n as usize)
                        .ok_or_else(|| {
                            LangError::engine(
                                "index: index must be a non-negative integer".to_string(),
                            )
                        })?;
                    arr.get(i).cloned().map(|v| (v, false)).ok_or_else(|| {
                        LangError::engine(format!("index: index {} out of bounds", i))
                    })
                }
                (TaggedValue::Obj(map), TaggedValue::Str(key)) => map
                    .get(key.as_ref())
                    .cloned()
                    .map(|v| (v, false))
                    .ok_or_else(|| LangError::engine(format!("index: key '{}' not found", key))),
                (target, index) => Err(LangError::engine(format!(
                    "index: target must be array/object and index must be number/string; got {} and {}",
                    type_tag(&target),
                    type_tag(&index),
                ))),
            }
        }
        // `field_access` — dynamic field access `target.field` /
        // `target[i]` (ADR-0060). The field resolves from the node's
        // `payload.field` first (the `field-access` surface), then the
        // second evaluated arg (`eval.field-access`'s `proj_child 1`),
        // then the op literal (the arrow-lowered
        // `op_native_lit("field_access", [target], field)` surface —
        // backends that cannot carry `lit` simply never reach that arm).
        // Unlike `index`, a missing object key yields `null`; the
        // pair/either/arrow projections expose the wire shapes' named
        // views (tree-walk semantics — canonical).
        "field_access" => {
            let target = args.first().cloned().unwrap_or(TaggedValue::Null);
            let field = ctx
                .node_payload()
                .and_then(|p| p.get("field"))
                .and_then(|v| v.as_str())
                .map(|s| TaggedValue::Str(Rc::from(s)))
                .or_else(|| args.get(1).cloned())
                .or_else(|| lit.cloned().map(TaggedValue::from))
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
                            LangError::engine(
                                "field_access: array index must be a non-negative integer"
                                    .to_string(),
                            )
                        })?;
                    arr.get(i).cloned().map(|v| (v, false)).ok_or_else(|| {
                        LangError::engine(format!(
                            "field_access: index {i} out of bounds (len {})",
                            arr.len()
                        ))
                    })
                }
                (TaggedValue::Pair(p), TaggedValue::Str(key)) => match key.as_ref() {
                    "first" => Ok((p.0.clone(), false)),
                    "second" => Ok((p.1.clone(), false)),
                    "_tag" => Ok((TaggedValue::Str(Rc::from("Pair")), false)),
                    _ => Ok((TaggedValue::Null, false)),
                },
                (TaggedValue::EitherLeft(v), TaggedValue::Str(key)) => match key.as_ref() {
                    "_tag" | "tag" => Ok((TaggedValue::Str(Rc::from("left")), false)),
                    "value" => Ok((v.as_ref().clone(), false)),
                    _ => Ok((TaggedValue::Null, false)),
                },
                (TaggedValue::EitherRight(v), TaggedValue::Str(key)) => match key.as_ref() {
                    "_tag" | "tag" => Ok((TaggedValue::Str(Rc::from("right")), false)),
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
                        Some(r) => Ok((TaggedValue::Str(r.clone()), false)),
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
                (target, field) => Err(LangError::engine(format!(
                    "field_access: target must be object/array, field must be string/number; got {} and {}",
                    type_tag(&target),
                    type_tag(&field),
                ))),
            }
        }
        // ADR-0060 `json` — template substitution: `$i` holes in the
        // template string are replaced by the JSON serialization of the
        // argument values, then the result parses as JSON. Two surfaces,
        // one semantics:
        // - node form (`eval.json` scheme): the node's `payload.children`
        //   hold `[template_leaf, arg…]`; the template is the leaf's
        //   *payload string* (read, not evaluated), the args are
        //   evaluated via `EvalNode`.
        // - term form (arrow-lowered `op_native_lit("json", args, t)`):
        //   `lit` is the template and `args` are already evaluated —
        //   the `term_interpreter` convention.
        // With neither, a string payload parses directly.
        "json" => {
            if let Some(template) = lit.and_then(|v| v.as_str()) {
                let mut s = template.to_string();
                for (i, arg) in args.iter().enumerate() {
                    let replacement = serde_json::to_string(&handl_value::tagged_to_value(arg))
                        .map_err(|e| LangError::engine(format!("json: {}", e)))?;
                    s = s.replace(&format!("${}", i), &replacement);
                }
                return serde_json::from_str::<Value>(&s)
                    .map_err(|e| LangError::engine(format!("json: parse error: {}", e)))
                    .map(|v| Some((TaggedValue::from(v), false)));
            }
            let children = ctx
                .node_payload()
                .and_then(|p| p.get("children"))
                .and_then(|c| c.as_array())
                .cloned();
            if let Some(children) = children {
                if children.len() >= 2 {
                    if !ctx.supports(OpService::EvalNode) {
                        return Ok(None);
                    }
                    let template_idx = children[0].as_u64().ok_or_else(|| {
                        LangError::engine("json: child index must be a number".to_string())
                    })? as usize;
                    let template = ctx
                        .node_payload_at(AstRef::new(template_idx))
                        .and_then(|p| p.as_str().map(str::to_string))
                        .ok_or_else(|| {
                            LangError::engine("json: template payload must be a string".to_string())
                        })?;
                    let mut s = template;
                    for (i, entry) in children[1..].iter().enumerate() {
                        let idx = entry.as_u64().ok_or_else(|| {
                            LangError::engine("json: child index must be a number".to_string())
                        })? as usize;
                        let val = ctx.eval_node(AstRef::new(idx))?;
                        let replacement =
                            serde_json::to_string(&handl_value::tagged_to_value(&val))
                                .map_err(|e| LangError::engine(format!("json: {}", e)))?;
                        s = s.replace(&format!("${}", i), &replacement);
                    }
                    return serde_json::from_str::<Value>(&s)
                        .map_err(|e| LangError::engine(format!("json: parse error: {}", e)))
                        .map(|v| Some((TaggedValue::from(v), false)));
                }
            }
            let expr = ctx
                .node_payload()
                .and_then(|p| p.as_str())
                .ok_or_else(|| LangError::engine("json: payload must be a string".to_string()))?;
            serde_json::from_str::<Value>(expr)
                .map_err(|e| LangError::engine(format!("json: parse error: {}", e)))
                .map(|v| (TaggedValue::from(v), false))
        }
        // ADR-0068 `runtime_assert` — graduated guarantees: the runtime
        // backstop for predicate annotations the typechecker could not
        // normalize statically (`Verdict::Blocked`). The node carries two
        // children — `predicate` (a closure over the expr's value
        // returning bool) and `expr` — whose positions resolve by named
        // role (`ctx.child_role_index`), falling back to positional
        // [0, 1] for role-less shapes. A non-arrow predicate passes the
        // value through unchecked (the typechecker already accepted it
        // statically).
        "runtime_assert" => {
            if !ctx.supports(OpService::EvalNode) || !ctx.supports(OpService::ApplyArrow) {
                return Ok(None);
            }
            let children = ctx
                .node_payload()
                .and_then(|p| p.get("children"))
                .and_then(|c| c.as_array())
                .cloned()
                .ok_or_else(|| {
                    LangError::engine(
                        "runtime_assert: expected children array in payload".to_string(),
                    )
                })?;
            let pred_idx = ctx.child_role_index("predicate").unwrap_or(0);
            let expr_idx = ctx.child_role_index("expr").unwrap_or(1);
            let pred_arena = children
                .get(pred_idx)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    LangError::engine(format!(
                        "runtime_assert: predicate child {} not found",
                        pred_idx
                    ))
                })? as usize;
            let expr_arena = children
                .get(expr_idx)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    LangError::engine(format!("runtime_assert: expr child {} not found", expr_idx))
                })? as usize;
            let pred_val = ctx.eval_node(AstRef::new(pred_arena))?;
            let expr_val = ctx.eval_node(AstRef::new(expr_arena))?;
            if !matches!(pred_val, TaggedValue::Arrow { .. }) {
                return Ok(Some((expr_val, false)));
            }
            let check_result = ctx.apply_arrow(&pred_val, &[expr_val.clone()])?;
            if is_truthy(&check_result) {
                Ok((expr_val, false))
            } else {
                Err(LangError::engine(
                    "runtime_assert: assertion failed — predicate returned false".to_string(),
                ))
            }
        }
        op if op == "route_on_type" || op.starts_with("route_on_type:") => {
            // The tag arrives as the op literal; the cranelift backend
            // bakes it into the op name (`route_on_type:<tag>`) instead.
            let tag = lit
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or_else(|| op.strip_prefix("route_on_type:").map(str::to_string))
                .filter(|t| !t.is_empty())
                .ok_or_else(|| {
                    LangError::engine("route_on_type: missing type tag literal".to_string())
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
        "typeof" => {
            let arg = args
                .first()
                .or(incoming.as_ref())
                .unwrap_or(&TaggedValue::Null);
            Ok((TaggedValue::Str(Rc::from(type_tag(arg))), false))
        }
        "tag.construct" => {
            if args.len() != 2 {
                return Err(LangError::engine(format!(
                    "tag.construct: expected 2 arguments (tag_name, payload), got {}",
                    args.len()
                )));
            }
            let tag_name = args[0].as_str().ok_or_else(|| {
                LangError::engine("tag.construct: tag_name must be a string".to_string())
            })?;
            let mut obj = match &args[1] {
                TaggedValue::Null => FxHashMap::default(),
                TaggedValue::Obj(map) => map.as_ref().clone(),
                _ => {
                    return Err(LangError::engine(
                        "tag.construct: payload must be an object or null".to_string(),
                    ));
                }
            };
            if obj.contains_key("__tag__") || obj.contains_key("__registry__") {
                return Err(LangError::engine(
                    "tag.construct: payload contains reserved __tag__ or __registry__ key"
                        .to_string(),
                ));
            }
            obj.insert(Rc::from("__tag__"), TaggedValue::Str(Rc::from(tag_name)));
            obj.insert(
                Rc::from("__registry__"),
                TaggedValue::Str(Rc::from("handl.canonical")),
            );
            Ok((TaggedValue::Obj(Rc::new(obj)), false))
        }
        "transport" => Ok((args.get(1).cloned().unwrap_or(TaggedValue::Null), false)),
        "export" => {
            // Export marking is a no-op for now.
            Ok((args.first().cloned().unwrap_or(TaggedValue::Null), false))
        }
        "record_proj" => {
            let record = args.first().cloned().unwrap_or(TaggedValue::Null);
            let field = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
            Ok((record_proj(&record, field), false))
        }
        // --- `Cell2` record ops (ADR-0104b frontier value model) -------
        // The derived term tags (`CellId`/`Src`/`Tgt`/`CellCompV`/
        // `Whisker`) lower to these op names on the compiled backends.
        "cell2_id" => Ok((cell2_id(args.first().unwrap_or(&TaggedValue::Null)), false)),
        "cell2_src" => Ok((cell2_src(args.first().unwrap_or(&TaggedValue::Null)), false)),
        "cell2_tgt" => Ok((cell2_tgt(args.first().unwrap_or(&TaggedValue::Null)), false)),
        "cell2_comp_v" => {
            let first = args.first().unwrap_or(&TaggedValue::Null);
            let second = args.get(1).unwrap_or(&TaggedValue::Null);
            cell2_comp_v(first, second)
                .map(|v| (v, false))
                .map_err(LangError::engine)
        }
        "cell2_whisker_left" | "cell2_whisker_right" => {
            let side = if op == "cell2_whisker_left" {
                "Left"
            } else {
                "Right"
            };
            let arrow = args.first().unwrap_or(&TaggedValue::Null);
            let cell = args.get(1).unwrap_or(&TaggedValue::Null);
            Ok((cell2_whisker(side, arrow, cell), false))
        }
        // `compose(diagram)` — generalized pasting/Kan filling
        // (ADR-0120): the n-ary generalization of `Seq{axis}`. The
        // diagram is an array of `{axis, cell}` steps (a bare cell is
        // axis 0); the first element seeds the composite. A single array
        // argument is the diagram; several arguments are the steps
        // themselves; with no args the incoming value is the diagram.
        "compose" => {
            let diagram = match args.len() {
                0 => incoming.unwrap_or(TaggedValue::Null),
                1 => args[0].clone(),
                _ => TaggedValue::Arr(Rc::from(args.to_vec())),
            };
            cell_compose(&diagram)
                .map(|v| (v, false))
                .map_err(LangError::engine)
        }
        op if cell_axis_op_dim(op).is_some() => {
            // `Seq{axis: k}` in the pasting regime (`k < dim(fst)`) lowers
            // to this op on the compiled backends: args are
            // `[axis, fst, snd]`, all evaluated on the same incoming —
            // the `∘ₖ` globular paste at the value level. The declared
            // cell dimension arrives as the op literal, or baked into the
            // op name (`cell_comp_axis:<dim>`; the legacy
            // `cell2_comp_axis:<dim>` spelling is accepted too) on
            // backends that cannot pass `lit` — the `route_on_type`
            // convention. It defaults to `2`, the canonical surface lift.
            // `cell_comp_axis` itself resolves the *effective* dimension
            // as `max(declared, cell_dim(fst), cell_dim(snd))` — values
            // carrying a higher dimension (`Cell`/`__dim__` records)
            // paste at their own height.
            let dim = lit
                .and_then(|v| v.as_u64())
                .unwrap_or_else(|| cell_axis_op_dim(op).unwrap_or(2));
            if args.len() != 3 {
                return Err(LangError::engine(format!(
                    "cell_comp_axis: expected 3 arguments (axis, fst, snd), got {}",
                    args.len()
                )));
            }
            let k = args[0]
                .as_f64()
                .filter(|n| *n >= 0.0 && n.fract() == 0.0)
                .map(|n| n as u64)
                .ok_or_else(|| {
                    LangError::engine(format!(
                        "Seq: axis evaluated to a non-natural value: {:?}",
                        args[0]
                    ))
                })?;
            cell_comp_axis(k, dim, &args[1], &args[2])
                .map(|v| (v, false))
                .map_err(LangError::engine)
        }
        // --- ops still owned by the backends --------------------------------
        _ => return Ok(None),
    };
    result.map(Some)
}

/// `None` option value for `opt_chain`.
fn opt_none() -> TaggedValue {
    TaggedValue::from(json!({"_tag": "None"}))
}

/// `Some(v)` option value for `opt_chain`.
fn opt_some(v: TaggedValue) -> TaggedValue {
    let mut obj = FxHashMap::default();
    obj.insert(Rc::from("_tag"), TaggedValue::Str(Rc::from("Some")));
    obj.insert(Rc::from("value"), v);
    TaggedValue::Obj(Rc::new(obj))
}

/// The declared operand dimension baked into a `cell_comp_axis[:N]` op
/// name (the legacy `cell2_comp_axis[:N]` spelling is accepted too).
/// Bare `cell_comp_axis`/`cell2_comp_axis` declare dimension 2 — the
/// canonical `>>>[k]` surface lift. Returns `None` for unrelated ops.
fn cell_axis_op_dim(op: &str) -> Option<u64> {
    for prefix in ["cell_comp_axis", "cell2_comp_axis"] {
        if op == prefix {
            return Some(2);
        }
        if let Some(suffix) = op.strip_prefix(prefix).and_then(|s| s.strip_prefix(':')) {
            return suffix.parse::<u64>().ok().map(|d| d.max(1));
        }
    }
    None
}

// ===========================================================================
// Binding/scope helpers (assign / bind / eval_block / eval_rec)
// ===========================================================================

/// The `children` array of the node at `r` (cloned — the `ctx` borrow
/// must end before [`NativeOpCtx::eval_node`] re-enters the engine).
fn node_children(ctx: &mut dyn NativeOpCtx, r: AstRef) -> Option<Vec<Value>> {
    ctx.node_payload_at(r)
        .and_then(|p| p.get("children").and_then(|c| c.as_array()).cloned())
}

/// `role_child_arena` (ADR-0081) on the node at `r`: `role` resolves to
/// its arity-aware position ([`NativeOpCtx::child_role_index_at`]), then
/// to the child arena index stored there. `None` = undeclared, the
/// optional slot is absent at this arity, or the slot is malformed —
/// callers keep their positional fallback.
fn role_child_arena(ctx: &mut dyn NativeOpCtx, r: AstRef, role: &str) -> Option<usize> {
    let pos = ctx.child_role_index_at(r, role)?;
    node_children(ctx, r)?
        .get(pos)
        .and_then(|v| v.as_u64())
        .map(|i| i as usize)
}

/// Whether the node at `r` is a binding (assign-like) node: its shape
/// declares both `target` and `value` roles — the tree-walk (canonical)
/// definition. The compiled backends historically checked `target` only;
/// the two coincide on canonical shapes (`assign` declares both), and a
/// `target`-only kind reaches the same outcome through the `value`-role
/// resolution that follows either way.
fn is_assign_node(ctx: &mut dyn NativeOpCtx, r: AstRef) -> bool {
    ctx.role_declared_at(r, "target") && ctx.role_declared_at(r, "value")
}

/// Whether the node at `r` is a return node: its shape declares a `value`
/// role but no `target` role (a binding declares both).
fn is_return_node(ctx: &mut dyn NativeOpCtx, r: AstRef) -> bool {
    ctx.role_declared_at(r, "value") && !ctx.role_declared_at(r, "target")
}

/// Resolve the bound name of a binding's `target` child: a bare
/// `var`-like node carries the name in its payload; a wrapper whose shape
/// declares a `subject` role (e.g. a type annotation — the
/// `subject`/`annot` role convention, ADR-0081) delegates to that child.
/// This makes `f: T = v`-shaped statements bind `f` without any kind
/// check.
fn binding_target_name(
    ctx: &mut dyn NativeOpCtx,
    target_idx: usize,
    err_ctx: &str,
) -> Result<String, LangError> {
    let mut r = AstRef::new(target_idx);
    let mut payload = ctx
        .node_payload_at(r)
        .ok_or(LangError::AstIndexOutOfBounds { index: target_idx })?;
    // Unwrap `subject`-roled wrappers (annotation-like nodes).
    for _ in 0..8 {
        if let Some(name) = payload.as_str() {
            return Ok(name.to_string());
        }
        let subject_idx = match role_child_arena(ctx, r, "subject") {
            Some(i) => i,
            None => break,
        };
        r = AstRef::new(subject_idx);
        payload = ctx
            .node_payload_at(r)
            .ok_or(LangError::AstIndexOutOfBounds { index: subject_idx })?;
    }
    payload.as_str().map(str::to_string).ok_or_else(|| {
        LangError::engine(format!("{err_ctx}: binding target must be an identifier"))
    })
}

/// Convergence comparison for `eval_rec` bindings.
///
/// Closures (`TaggedValue::Arrow`) are compared by params and body index
/// only, NOT by their captured env. In a `rec` fixpoint, each pass
/// re-evaluates a closure binding and creates a new closure that captures
/// the current scope (which now includes other rec bindings). The
/// captured env naturally differs each pass even though the closure's
/// code is identical, so a full structural `eq` would never converge.
/// Comparing only params + body lets the fixpoint converge after the env
/// stabilizes. Non-closure values use the standard structural `eq`.
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

/// The `eval_rec` region body — run under a scope the caller pushed and
/// pops regardless of the returned `Result` (ADR-0074).
///
/// Bindings are NOT pre-initialized. On the first pass, a binding that
/// references another not-yet-computed binding either finds it in an
/// enclosing scope (if it exists there) or errors; errors during binding
/// evaluation are caught and the binding is skipped for that pass — it
/// is retried on the next pass once its dependencies have values. This
/// allows mutually-dependent bindings to converge regardless of their
/// textual order.
fn eval_rec_region(
    ctx: &mut dyn NativeOpCtx,
    children: &[Value],
) -> Result<TaggedValue, LangError> {
    // Track the previous value of each binding for change detection.
    // Initialize from the enclosing scope so we can detect the first
    // change correctly: the caller already pushed the rec scope, so the
    // `env_lookup` below searches innermost-first, but the inner scope is
    // empty at this point and falls through.
    let mut prev_values: FxHashMap<Rc<str>, TaggedValue> = FxHashMap::default();
    let mut binding_names: Vec<Rc<str>> = Vec::new();
    for entry in children {
        let child_idx = entry.as_u64().ok_or_else(|| {
            LangError::engine("eval_rec: child index must be a number".to_string())
        })? as usize;
        let child = AstRef::new(child_idx);
        if is_assign_node(ctx, child) {
            if let Some(ac) = node_children(ctx, child) {
                if ac.len() >= 2 {
                    let var_idx = role_child_arena(ctx, child, "target")
                        .or_else(|| ac[0].as_u64().map(|i| i as usize))
                        .ok_or_else(|| {
                            LangError::engine(
                                "eval_rec: assign var index must be a number".to_string(),
                            )
                        })?;
                    if let Ok(name) = binding_target_name(ctx, var_idx, "eval_rec") {
                        let outer_val = ctx.env_lookup(&name).unwrap_or(TaggedValue::Null);
                        let name_rc: Rc<str> = Rc::from(name.as_str());
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
                LangError::engine("eval_rec: child index must be a number".to_string())
            })? as usize;
            let child = AstRef::new(child_idx);

            // `return` — exit the region with the return value.
            if is_return_node(ctx, child) {
                if let Some(rc) = node_children(ctx, child) {
                    if let Some(ret_idx) = rc.first().and_then(|v| v.as_u64()) {
                        return ctx.eval_node(AstRef::new(ret_idx as usize));
                    }
                }
                // `return` with no value → exit with Null.
                return Ok(TaggedValue::Null);
            }

            // `assign` bindings — evaluate the value expression and update
            // the binding in the current scope. Errors are caught: a
            // binding may reference another not-yet-computed binding
            // (still unbound or Null), which would error. The binding is
            // skipped for this pass and retried next pass.
            if is_assign_node(ctx, child) {
                if let Some(ac) = node_children(ctx, child) {
                    if ac.len() >= 2 {
                        let var_idx = role_child_arena(ctx, child, "target")
                            .or_else(|| ac[0].as_u64().map(|i| i as usize))
                            .ok_or_else(|| {
                                LangError::engine(
                                    "eval_rec: assign var index must be a number".to_string(),
                                )
                            })?;
                        let value_idx = role_child_arena(ctx, child, "value")
                            .or_else(|| ac[1].as_u64().map(|i| i as usize))
                            .ok_or_else(|| {
                                LangError::engine(
                                    "eval_rec: assign value index must be a number".to_string(),
                                )
                            })?;
                        let var_name = binding_target_name(ctx, var_idx, "eval_rec")?;
                        match ctx.eval_node(AstRef::new(value_idx)) {
                            Ok(new_val) => {
                                ctx.env_define(&var_name, new_val.clone());
                                result = new_val;
                            }
                            Err(_) => {
                                // Skip — binding couldn't be computed
                                // this pass (likely references a
                                // not-yet-computed binding). Discard any
                                // deferred error the failed eval left
                                // behind (cranelift `ctx.error`) so it
                                // does not leak into the next pass.
                                ctx.clear_eval_error();
                            }
                        }
                        continue;
                    }
                }
            }

            // Non-binding expression — evaluate normally.
            result = ctx.eval_node(child)?;
        }

        // End-of-pass fixpoint check. A binding's value can also change
        // via a nested update — `x = y` inside a block updates the
        // nearest enclosing binding (`env_assign`), which is how the
        // `while`/`do-while`/`for` desugars rewrite the rec-scope
        // `__continue`/`__xs` slots from inside the `if`/`match` branch
        // blocks. Comparing only at rec-level assign-eval time misses
        // those writes and converges one pass early (or oscillates), so
        // compare each tracked binding's *current* env value against the
        // value it held at the end of the previous pass — and treat any
        // cross-scope write observed this pass (e.g. a body rebinding an
        // outer variable) as a change.
        if ctx.env_take_enclosing_updated() {
            changed = true;
        }
        for name in &binding_names {
            let cur = ctx.env_lookup(name).unwrap_or(TaggedValue::Null);
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
        return Err(LangError::engine(
            "rec: exceeded fuel limit (10000 iterations) without reaching fixpoint".to_string(),
        ));
    }
    Ok(result)
}

// ===========================================================================
// Derived step ops — the loop-step *decision*, unified
// ===========================================================================

/// The decision half of a derived step op (`while_step`, `for_step`,
/// `fix_step`) emitted by the `while`/`for`/`fix` desugar rules.
///
/// Each backend produces the step's argument values by its own
/// interpretation — tree-walk evaluates the sub-schemes lazily through
/// `eval_arg` (so `while_step` only runs the body when the condition
/// holds), the compiled backends execute pre-compiled instruction ranges
/// and return cached results from `eval_arg`. The *decision* —
/// `(in, state) -> either` — is the shared op semantics, defined once.
///
/// `incoming` is the loop pair `(in, state)`: `in` on the first step
/// (when `state` is `Null`), `state` thereafter.
///
/// # Errors
///
/// Returns `Err` on malformed inputs or when `eval_arg` fails.
pub fn step_decision(
    op: &str,
    incoming: &TaggedValue,
    eval_arg: &mut dyn FnMut(usize, &TaggedValue) -> Result<TaggedValue, LangError>,
) -> Result<TaggedValue, LangError> {
    let in_val = pair_fst(incoming).cloned().unwrap_or(TaggedValue::Null);
    let state = pair_snd(incoming).cloned().unwrap_or(TaggedValue::Null);
    let current = if state.is_null() { &in_val } else { &state };
    match op {
        // args = [cond, body]. Truthy cond → continue with body's result;
        // else terminate with the current value.
        "while_step" => {
            let cond_val = eval_arg(0, current)?;
            if is_truthy(&cond_val) {
                let body_val = eval_arg(1, current)?;
                Ok(either_right(body_val))
            } else {
                Ok(either_left(current.clone()))
            }
        }
        // args = [body]. State is pair(counter, acc). Counter > 0 →
        // continue with pair(counter-1, body(acc)); else terminate with acc.
        "for_step" => {
            let (counter, acc) = if state.is_null() {
                // First step: in = pair(count, initial).
                (
                    pair_fst(&in_val).cloned().ok_or_else(|| {
                        LangError::engine(
                            "for_step: input must be pair(count, initial) when state is Null"
                                .to_string(),
                        )
                    })?,
                    pair_snd(&in_val).cloned().unwrap_or(TaggedValue::Null),
                )
            } else {
                // Later steps: state = pair(counter, acc).
                (
                    pair_fst(&state).cloned().ok_or_else(|| {
                        LangError::engine("for_step: state must be pair(counter, acc)".to_string())
                    })?,
                    pair_snd(&state).cloned().unwrap_or(TaggedValue::Null),
                )
            };
            let count_n = as_num(&counter)?;
            if count_n > Num::Int(0) {
                let new_acc = eval_arg(0, &acc)?;
                Ok(either_right(pair(
                    num_value(count_n - Num::Int(1)),
                    new_acc,
                )))
            } else {
                Ok(either_left(acc))
            }
        }
        // args = [body]. Run body; result == current → fixpoint, terminate;
        // else continue with the result.
        "fix_step" => {
            let new_val = eval_arg(0, current)?;
            if handl_value::eq(&new_val, current) {
                Ok(either_left(current.clone()))
            } else {
                Ok(either_right(new_val))
            }
        }
        _ => Err(LangError::engine(format!(
            "step_decision: unknown derived step op {op}"
        ))),
    }
}

/// The `loop_step` argument builder (ADR-0052): how the current loop
/// state maps onto the body closure's parameters.
///
/// - Single param (or non-object state): `[current]`.
/// - Array-pattern arrow: `[arr_of_fields]` — one array whose elements are
///   the object's fields bound by param name.
/// - Multi-param arrow over an object state: `[field_by_param_name, ...]`
///   — each param destructures the same-named field.
pub fn build_loop_step_args(closure_val: &TaggedValue, current: &TaggedValue) -> Vec<TaggedValue> {
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
                // Destructure: bind each param to the corresponding field
                // by name.
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

/// The `loop_step` outcome mapping (the ergonomic `loop` surface,
/// ADR-0052). After the backend applies the body closure to the current
/// state, the result maps:
///   `{ state: v }` → `either_right(v)` (continue)
///   `{ out: v }`    → `either_left(v)`  (terminate, backward compat)
///   bare `v`        → `either_left(v)`  (terminate, v0.1 `return v`)
pub fn loop_step_outcome(result: TaggedValue) -> TaggedValue {
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
    either_left(result)
}
