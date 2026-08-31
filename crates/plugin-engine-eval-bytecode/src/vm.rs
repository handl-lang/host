//! Bytecode VM internals: scope stack, numeric helpers, and the execution
//! frame.
//!
//! The [`VmFrame`] executes a compiled instruction program against an operand
//! stack and a scope stack. [`Instr::EvalChild`] and [`Instr::Call`] recurse
//! into [`VmFrame::exec_node`], which compiles (via a per-node cache) and
//! executes child or closure-body nodes. This per-node compilation cache is
//! the mechanism that erases data dispatch (ADR-0044 §7): once a node's
//! scheme is resolved to instructions, subsequent executions skip the
//! `lang.eval[node.kind]` hash lookup entirely.

use std::collections::HashMap;
use std::rc::Rc;

use handl_language::{Ast, AstNode, AstRef, LangError, Language, Value, ast_value};

use crate::compile;
use crate::instr::Instr;

// ---------------------------------------------------------------------------
// Scope stack — identical semantics to the tree-walk interpreter
// ---------------------------------------------------------------------------

/// A scope stack: `Vec<HashMap<String, Value>>` searched innermost-first.
///
/// Mirrors the tree-walk's `ScopeStack` exactly: `Bind` pushes a child scope;
/// lookup searches innermost-first; closure calls swap in the captured
/// environment, bind params, evaluate the body, then restore the original
/// scope stack.
///
/// This type is `pub(crate)` so the v2 VM ([`crate::vm_v2`], ADR-0052) can
/// reuse the same scope discipline. This is a visibility-only change — no
/// v1 logic is altered. The `scopes` field is `pub(crate)` for the
/// closure-env swap-in that `apply_closure` performs.
pub(crate) struct ScopeStack {
    /// The stack of scopes, innermost last. `pub(crate)` so the v2 VM can
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
///
/// `pub(crate)` so the v2 VM ([`crate::vm_v2`]) can reuse the same
/// numeric semantics. This is a visibility-only change.
#[derive(Debug, Clone, Copy, PartialEq)]
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
// VmFrame — the execution context
// ---------------------------------------------------------------------------

/// The execution frame for the bytecode VM.
///
/// Owns the scope stack and a per-node compilation cache. The `ast` and
/// `lang` references are immutable; `modules` is the optional module
/// registry for `Foreign` dispatch.
pub(crate) struct VmFrame<'a, 'h> {
    /// The AST being evaluated.
    ast: &'a Ast,
    /// The language definition (for scheme lookup during compilation).
    lang: &'a Language,
    /// The module registry (for `Foreign` dispatch).
    modules: Option<&'h Rc<dyn handl_platform::ModuleLookup>>,
    /// Per-node compilation cache: `AstRef index → compiled instructions`.
    /// This is the data-dispatch erasure mechanism (ADR-0044 §7).
    cache: HashMap<usize, Vec<Instr>>,
    /// The scope stack.
    scopes: ScopeStack,
}

impl<'a, 'h> VmFrame<'a, 'h> {
    /// Create a new frame with a fresh cache and scope stack.
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

    /// Define a global variable in the root scope (used by the host to inject
    /// `self` and other ambient values before execution).
    pub(crate) fn define_global(&mut self, name: &str, value: Value) {
        self.scopes.define(name, value);
    }

    /// Compile (if not cached) and execute a single node.
    ///
    /// Looks up `lang.eval[node.kind]` to find the node's [`Scheme`], compiles
    /// it to instructions (or uses the no-scheme fallback `[LoadPayload,
    /// Return]`), caches the result, and executes. On subsequent calls for
    /// the same [`AstRef`], the cached instructions are reused — no
    /// `lang.eval` lookup occurs.
    fn exec_node(&mut self, node_ref: AstRef) -> Result<Value, LangError> {
        let node = self.ast.at(node_ref)?;

        // Compile-on-demand with cache (erases data dispatch).
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
        self.exec_instrs(&instrs, node)
    }

    /// Execute a program of instructions against the given current node.
    ///
    /// Uses a fresh operand stack. The scope stack (`self.scopes`) is shared
    /// across recursive `exec_node` calls.
    pub(crate) fn exec_instrs(
        &mut self,
        instrs: &[Instr],
        node: &AstNode,
    ) -> Result<Value, LangError> {
        let mut stack: Vec<Value> = Vec::new();
        let mut ip = 0;
        while ip < instrs.len() {
            match &instrs[ip] {
                Instr::Push(v) => {
                    stack.push(v.clone());
                }
                Instr::Load(name) => {
                    let val = self
                        .scopes
                        .lookup(name)
                        .cloned()
                        .ok_or_else(|| LangError::UnknownVariable { name: name.clone() })?;
                    stack.push(val);
                }
                Instr::LoadPayloadVar => {
                    let var_name = node
                        .payload
                        .as_str()
                        .ok_or_else(|| {
                            LangError::Engine(
                                "expected string payload for $payload var".to_string(),
                            )
                        })?
                        .to_string();
                    let val = self
                        .scopes
                        .lookup(&var_name)
                        .cloned()
                        .ok_or(LangError::UnknownVariable { name: var_name })?;
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
                    let val = stack.pop().ok_or_else(|| {
                        LangError::Engine("stack underflow on StorePayloadName".to_string())
                    })?;
                    let bind_name = node
                        .payload
                        .get("name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            LangError::Engine(
                                "expected name in payload for $payload bind".to_string(),
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
                    let result = self.apply_closure(&callee, &args)?;
                    stack.push(result);
                }
                Instr::JumpIfFalse(target) => {
                    let cond = stack.pop().ok_or_else(|| {
                        LangError::Engine("stack underflow on JumpIfFalse".to_string())
                    })?;
                    if !is_truthy(&cond) {
                        ip = *target;
                        continue;
                    }
                }
                Instr::Jump(target) => {
                    ip = *target;
                    continue;
                }
                Instr::Prim(op, n) => {
                    let mut args = Vec::with_capacity(*n);
                    for _ in 0..*n {
                        args.push(stack.pop().ok_or_else(|| {
                            LangError::Engine("stack underflow on Prim".to_string())
                        })?);
                    }
                    args.reverse();
                    let result = self.apply_prim(op, &args, node)?;
                    stack.push(result);
                }
                Instr::Foreign(name, n) => {
                    let mut args = Vec::with_capacity(*n);
                    for _ in 0..*n {
                        args.push(stack.pop().ok_or_else(|| {
                            LangError::Engine("stack underflow on Foreign".to_string())
                        })?);
                    }
                    args.reverse();
                    let modules = self.modules.ok_or_else(|| {
                        LangError::Engine(format!("unregistered foreign: {}", name))
                    })?;
                    let func = modules.lookup(name).ok_or_else(|| {
                        LangError::Engine(format!("unregistered foreign: {}", name))
                    })?;
                    let mut pctx = handl_platform::EvalContext::new();
                    let result = func
                        .call(&args, &mut pctx)
                        .map_err(|e| LangError::Engine(e.to_string()))?;
                    stack.push(result);
                }
                Instr::VauEnter => {
                    // Push a new scope for the vau parameter bindings.
                    self.scopes.push_scope();
                }
                Instr::VauBind(name, child_n) => {
                    // Bind the unevaluated child subtree to `name` in the
                    // current scope (the vau escape, ADR-0041 §6). The child
                    // is NOT evaluated.
                    let children = node.payload.get("children").and_then(|c| c.as_array());
                    let arena_idx = children
                        .and_then(|arr| arr.get(*child_n))
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| {
                            LangError::Engine(format!("vau: child {} not found", child_n))
                        })?;
                    let sub = self.ast.subtree(AstRef::new(arena_idx as usize))?;
                    self.scopes.define(name, ast_value(&sub));
                }
                Instr::Return => {
                    return stack
                        .pop()
                        .ok_or_else(|| LangError::Engine("stack underflow on Return".to_string()));
                }
            }
            ip += 1;
        }
        // No Return encountered — return top of stack (defensive).
        stack
            .pop()
            .ok_or_else(|| LangError::Engine("empty operand stack at end of program".to_string()))
    }

    /// Apply a closure value to argument values.
    ///
    /// Mirrors the tree-walk's `apply_closure`: extract params/body/env from
    /// the closure object, save the current scope stack, swap in the captured
    /// env with a new scope for params, evaluate the body node, then restore
    /// the original scope stack.
    fn apply_closure(&mut self, closure: &Value, args: &[Value]) -> Result<Value, LangError> {
        let obj = closure
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

        // Swap in the captured env + new scope with params.
        let saved = std::mem::replace(&mut self.scopes, ScopeStack::new());
        self.scopes.scopes[0] = captured_env;
        self.scopes.push_scope();
        for (param, arg) in params.iter().zip(args.iter()) {
            self.scopes.define(param, arg.clone());
        }

        let result = self.exec_node(body_ref);

        // Restore the original scope stack.
        self.scopes = saved;
        result
    }

    /// Apply a primitive operation.
    ///
    /// Replicates the tree-walk's primitive semantics exactly: `add`, `sub`,
    /// `mul`, `div`, `mod`, `eq`, `lt`, `gt`, `not`, `and`, `or`, `neg`,
    /// `closure`.
    fn apply_prim(&mut self, op: &str, args: &[Value], node: &AstNode) -> Result<Value, LangError> {
        match op {
            "add" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                match (a, b) {
                    (Num::Int(a), Num::Int(b)) => Ok(Value::from(a + b)),
                    (a, b) => Ok(Value::from(a.to_f64() + b.to_f64())),
                }
            }
            "sub" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                match (a, b) {
                    (Num::Int(a), Num::Int(b)) => Ok(Value::from(a - b)),
                    (a, b) => Ok(Value::from(a.to_f64() - b.to_f64())),
                }
            }
            "mul" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                match (a, b) {
                    (Num::Int(a), Num::Int(b)) => Ok(Value::from(a * b)),
                    (a, b) => Ok(Value::from(a.to_f64() * b.to_f64())),
                }
            }
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
                match (a, b) {
                    (Num::Int(a), Num::Int(b)) => Ok(Value::from(a % b)),
                    (a, b) => Ok(Value::from(a.to_f64() % b.to_f64())),
                }
            }
            "eq" => Ok(Value::Bool(args[0] == args[1])),
            "lt" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok(Value::Bool(a.to_f64() < b.to_f64()))
            }
            "gt" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok(Value::Bool(a.to_f64() > b.to_f64()))
            }
            "pow" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok(Value::from(a.to_f64().powf(b.to_f64())))
            }
            "gte" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok(Value::Bool(a.to_f64() >= b.to_f64()))
            }
            "lte" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok(Value::Bool(a.to_f64() <= b.to_f64()))
            }
            "neq" => Ok(Value::Bool(args[0] != args[1])),
            "compare" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                let af = a.to_f64();
                let bf = b.to_f64();
                if af < bf {
                    Ok(Value::from(-1i64))
                } else if af > bf {
                    Ok(Value::from(1i64))
                } else {
                    Ok(Value::from(0i64))
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
                let a = as_num(&args[0])?;
                match a {
                    Num::Int(i) => Ok(Value::from(-i)),
                    Num::Float(f) => Ok(Value::from(-f)),
                }
            }
            "closure" => {
                let payload = &node.payload;
                let params = payload
                    .get("params")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| {
                        LangError::Engine("closure prim: missing params in payload".to_string())
                    })?;
                let body = payload
                    .get("body")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        LangError::Engine("closure prim: missing body in payload".to_string())
                    })?;
                let env = self.scopes.capture();
                Ok(serde_json::json!({
                    "__closure__": true,
                    "params": params,
                    "body": body,
                    "env": env,
                }))
            }
            "array" => {
                let elements = node.payload.get("elements").ok_or_else(|| {
                    LangError::Engine("array: missing elements in payload".to_string())
                })?;
                Ok(elements.clone())
            }
            "object" => {
                let entries = node
                    .payload
                    .get("entries")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| {
                        LangError::Engine("object: missing entries in payload".to_string())
                    })?;
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
                Ok(Value::Object(obj))
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
                let value = node.payload.get("value").ok_or_else(|| {
                    LangError::Engine("assign: missing value in payload".to_string())
                })?;
                Ok(value.clone())
            }
            "match" => self.prim_match(node),
            "handle" => self.prim_handle(node),
            _ => Err(LangError::Engine(format!("unknown prim: {}", op))),
        }
    }

    // --- match / handle primitives (mirror the tree-walk interpreter) ---

    /// `match`: pattern-match a scrutinee against a list of arms.
    ///
    /// The node payload is `{"children": [scrut_idx, block_idx]}`; the block
    /// node's payload is `{"children": [arm_idx, ...]}`; each arm node's
    /// payload is `{"children": [pattern_idx, body_idx]}`. The first matching
    /// arm's body is evaluated; non-matching arms are never evaluated. Returns
    /// an error if no arm matches.
    fn prim_match(&mut self, node: &AstNode) -> Result<Value, LangError> {
        let children = node
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
        let scrut_val = self.exec_node(AstRef::new(scrut_idx))?;
        let block_node = self.ast.at(AstRef::new(block_idx))?;
        let arm_indices = block_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| LangError::Engine("match: block node missing children".to_string()))?;
        for arm_entry in arm_indices {
            let arm_idx = arm_entry
                .as_u64()
                .ok_or_else(|| LangError::Engine("match: arm index must be a number".to_string()))?
                as usize;
            let arm_node = self.ast.at(AstRef::new(arm_idx))?;
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
            self.scopes.push_scope();
            let matched = self.try_pattern(AstRef::new(pattern_idx), &scrut_val)?;
            if matched {
                let result = self.exec_node(AstRef::new(body_idx));
                self.scopes.pop_scope();
                return result;
            }
            self.scopes.pop_scope();
        }
        Err(LangError::Engine("match: no arm matched".to_string()))
    }

    /// `handle`: evaluate a body, then dispatch the result to the first
    /// matching handler clause. Structurally identical to `match`. If no
    /// clause matches, the body value is returned as-is.
    fn prim_handle(&mut self, node: &AstNode) -> Result<Value, LangError> {
        let children = node
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
        let body_val = self.exec_node(AstRef::new(body_idx))?;
        let block_node = self.ast.at(AstRef::new(block_idx))?;
        let clause_indices = block_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .ok_or_else(|| LangError::Engine("handle: block node missing children".to_string()))?;
        for clause_entry in clause_indices {
            let clause_idx = clause_entry
                .as_u64()
                .ok_or_else(|| {
                    LangError::Engine("handle: clause index must be a number".to_string())
                })? as usize;
            let clause_node = self.ast.at(AstRef::new(clause_idx))?;
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
            self.scopes.push_scope();
            let matched = self.try_pattern(AstRef::new(pattern_idx), &body_val)?;
            if matched {
                let result = self.exec_node(AstRef::new(handler_idx));
                self.scopes.pop_scope();
                return result;
            }
            self.scopes.pop_scope();
        }
        Ok(body_val) // No clause matched — return the body value as-is.
    }

    /// Interpret a pattern AST node structurally against a scrutinee value.
    fn try_pattern(
        &mut self,
        pattern_ref: AstRef,
        scrut: &Value,
    ) -> Result<bool, LangError> {
        let pattern_node = self.ast.at(pattern_ref)?;
        if pattern_node.kind == "var" {
            if let Some(name) = pattern_node.payload.as_str() {
                if name == "_" || name.starts_with('_') {
                    return Ok(true); // wildcard
                }
                self.scopes.define(name, scrut.clone()); // binder
                return Ok(true);
            }
        }
        if pattern_node.kind == "array" {
            return self.try_array_pattern(pattern_ref, scrut);
        }
        if pattern_node.kind == "object" {
            return self.try_object_pattern(pattern_ref, scrut);
        }
        let pat_val = self.exec_node(pattern_ref)?; // literal
        Ok(pat_val == *scrut)
    }

    /// Resolve the binder name of a `rest` pattern node.
    ///
    /// Returns `None` for a wildcard rest (`_` / `_*`), `Some(name)` for a
    /// binder, or an error if the rest child is not a `var`.
    fn rest_binder_name(&self, rest_ref: AstRef) -> Result<Option<String>, LangError> {
        let rest_node = self.ast.at(rest_ref)?;
        let children = rest_node
            .payload
            .get("children")
            .and_then(|v| v.as_array())
            .ok_or_else(|| LangError::Engine("rest pattern: missing children".to_string()))?;
        let var_idx = children
            .first()
            .and_then(|v| v.as_u64())
            .ok_or_else(|| LangError::Engine("rest pattern: missing binder".to_string()))?;
        let var_node = self.ast.at(AstRef::new(var_idx as usize))?;
        if var_node.kind == "var" {
            if let Some(name) = var_node.payload.as_str() {
                if name == "_" || name.starts_with('_') {
                    return Ok(None); // wildcard rest
                }
                return Ok(Some(name.to_string()));
            }
        }
        Err(LangError::Engine("rest pattern: binder is not a var".to_string()))
    }

    /// Array destructuring pattern: positional sub-patterns with an optional
    /// `...rest`. Without rest, the array length must match exactly; with
    /// rest, the remaining elements are captured as an array bound to the rest
    /// name.
    fn try_array_pattern(
        &mut self,
        pattern_ref: AstRef,
        scrut: &Value,
    ) -> Result<bool, LangError> {
        let pattern_node = self.ast.at(pattern_ref)?;
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
            let child_node = self.ast.at(AstRef::new(idx))?;
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
            if !self.try_pattern(AstRef::new(pat_idx), &scrut_arr[i])? {
                return Ok(false);
            }
        }
        if let Some(ridx) = rest_idx {
            if let Some(name) = self.rest_binder_name(AstRef::new(ridx))? {
                let rest_vals: Vec<Value> = scrut_arr[n..].to_vec();
                self.scopes.define(&name, Value::Array(rest_vals));
            }
        }
        Ok(true)
    }

    /// Object destructuring pattern: each child is an `entry` node
    /// (`{"children": [key_idx, val_pattern_idx]}`) or a `rest` node. The key
    /// can be a `var` (identifier name) or evaluated to a string. An optional
    /// `...rest` captures unmatched keys as an object.
    fn try_object_pattern(
        &mut self,
        pattern_ref: AstRef,
        scrut: &Value,
    ) -> Result<bool, LangError> {
        let pattern_node = self.ast.at(pattern_ref)?;
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
            let child_node = self.ast.at(AstRef::new(idx))?;
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
                let key_node = self.ast.at(AstRef::new(key_idx))?;
                let key = if key_node.kind == "var" {
                    key_node.payload.as_str().unwrap_or("").to_string()
                } else {
                    let key_val = self.exec_node(AstRef::new(key_idx))?;
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
                    if !self.try_pattern(AstRef::new(*val_idx), scrut_val)? {
                        return Ok(false);
                    }
                }
                None => return Ok(false),
            }
        }
        if let Some(ridx) = rest_idx {
            if let Some(name) = self.rest_binder_name(AstRef::new(ridx))? {
                let rest_obj: serde_json::Map<String, Value> = scrut_obj
                    .iter()
                    .filter(|(k, _)| !matched_keys.contains(k))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                self.scopes.define(&name, Value::Object(rest_obj));
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{Ast, AstNode, Language, Scheme, SchemeVersion};
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

    // --- ScopeStack tests ---

    #[test]
    fn scope_stack_lookup_innermost_first() {
        let mut s = ScopeStack::new();
        s.define("x", json!(1));
        s.push_scope();
        s.define("x", json!(2));
        assert_eq!(s.lookup("x"), Some(&json!(2)));
        s.pop_scope();
        assert_eq!(s.lookup("x"), Some(&json!(1)));
    }

    #[test]
    fn scope_stack_pop_never_removes_last() {
        let mut s = ScopeStack::new();
        s.pop_scope();
        assert_eq!(s.scopes.len(), 1);
    }

    #[test]
    fn scope_stack_capture_flattens() {
        let mut s = ScopeStack::new();
        s.define("a", json!(1));
        s.push_scope();
        s.define("b", json!(2));
        let cap = s.capture();
        assert_eq!(cap.get("a"), Some(&json!(1)));
        assert_eq!(cap.get("b"), Some(&json!(2)));
    }

    // --- Numeric helper tests ---

    #[test]
    fn num_to_f64() {
        assert_eq!(Num::Int(42).to_f64(), 42.0);
        assert_eq!(Num::Float(3.14).to_f64(), 3.14);
    }

    #[test]
    fn as_num_extracts_int_and_float() {
        assert_eq!(as_num(&json!(42)).unwrap(), Num::Int(42));
        assert_eq!(as_num(&json!(3.14)).unwrap(), Num::Float(3.14));
        assert!(as_num(&json!("not a number")).is_err());
    }

    #[test]
    fn vm_prim_sub_non_number_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!("not a number")),
            Instr::Push(json!(1)),
            Instr::Prim("sub".into(), 2),
            Instr::Return,
        ];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_prim_mul_non_number_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!("not a number")),
            Instr::Push(json!(1)),
            Instr::Prim("mul".into(), 2),
            Instr::Return,
        ];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_prim_lt_non_number_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!("not a number")),
            Instr::Push(json!(1)),
            Instr::Prim("lt".into(), 2),
            Instr::Return,
        ];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_prim_gt_non_number_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!("not a number")),
            Instr::Push(json!(1)),
            Instr::Prim("gt".into(), 2),
            Instr::Return,
        ];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_prim_div_float_by_zero_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(1.0)),
            Instr::Push(json!(0.0)),
            Instr::Prim("div".into(), 2),
            Instr::Return,
        ];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_prim_mixed_arithmetic() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(1)),
            Instr::Push(json!(2.5)),
            Instr::Prim("add".into(), 2),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(3.5));
    }

    #[test]
    fn vm_prim_lt_float() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(3.0)),
            Instr::Push(json!(5)),
            Instr::Prim("lt".into(), 2),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(true));
    }

    // --- is_truthy tests ---

    #[test]
    fn is_truthy_bool() {
        assert!(is_truthy(&json!(true)));
        assert!(!is_truthy(&json!(false)));
    }

    #[test]
    fn is_truthy_null() {
        assert!(!is_truthy(&Value::Null));
    }

    #[test]
    fn is_truthy_number() {
        assert!(!is_truthy(&json!(0)));
        assert!(is_truthy(&json!(42)));
        assert!(!is_truthy(&json!(0.0)));
        assert!(is_truthy(&json!(3.14)));
    }

    #[test]
    fn is_truthy_string() {
        assert!(!is_truthy(&json!("")));
        assert!(is_truthy(&json!("hello")));
    }

    #[test]
    fn is_truthy_object_and_array() {
        assert!(is_truthy(&json!({"a": 1})));
        assert!(is_truthy(&json!([1, 2])));
    }

    // --- VmFrame instruction execution tests ---

    /// Helper: execute a raw instruction program against a root node.
    fn exec_raw(
        lang: &Language,
        ast: &Ast,
        instrs: &[Instr],
        modules: Option<&Rc<dyn ModuleLookup>>,
    ) -> Result<Value, LangError> {
        if ast.is_empty() {
            return Ok(Value::Null);
        }
        let root_ref = AstRef::new(ast.len() - 1);
        let root_node = ast.at(root_ref)?;
        let mut frame = VmFrame {
            ast,
            lang,
            modules,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        frame.exec_instrs(instrs, root_node)
    }

    #[test]
    fn vm_push_return() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![Instr::Push(json!(42)), Instr::Return];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn vm_load_payload() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("lit").with_payload(json!("hello"))]);
        let instrs = vec![Instr::LoadPayload, Instr::Return];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn vm_load_unknown_variable_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![Instr::Load("x".into()), Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert_eq!(err, LangError::UnknownVariable { name: "x".into() });
    }

    #[test]
    fn vm_load_payload_var() {
        let lang = Language::new();
        // Root node is a "var" with payload "x".
        // We need "x" in scope. Use Store first.
        let ast = make_ast(vec![AstNode::new("var").with_payload(json!("x"))]);
        let instrs = vec![
            Instr::Push(json!(99)),
            Instr::Store("x".into()),
            Instr::LoadPayloadVar,
            Instr::PopScope,
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(99));
    }

    #[test]
    fn vm_load_payload_var_not_string_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("var").with_payload(json!(42))]);
        let instrs = vec![Instr::LoadPayloadVar, Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_store_and_load() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(10)),
            Instr::Store("x".into()),
            Instr::Load("x".into()),
            Instr::PopScope,
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(10));
    }

    #[test]
    fn vm_store_payload_name() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("let").with_payload(json!({"name": "x"}))]);
        let instrs = vec![
            Instr::Push(json!(42)),
            Instr::StorePayloadName,
            Instr::Load("x".into()),
            Instr::PopScope,
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn vm_store_payload_name_missing_name_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("let").with_payload(json!({}))]);
        let instrs = vec![
            Instr::Push(json!(42)),
            Instr::StorePayloadName,
            Instr::Return,
        ];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_pop_scope_after_bind() {
        // After Store + PopScope, the variable should not be visible.
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(10)),
            Instr::Store("x".into()),
            Instr::PopScope,
            Instr::Load("x".into()),
            Instr::Return,
        ];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert_eq!(err, LangError::UnknownVariable { name: "x".into() });
    }

    #[test]
    fn vm_jump_if_false_takes_branch() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(false)),
            Instr::JumpIfFalse(3),
            Instr::Push(json!(1)),
            Instr::Push(json!(2)),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(2));
    }

    #[test]
    fn vm_jump_if_false_does_not_jump_when_truthy() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(true)),
            Instr::JumpIfFalse(4),
            Instr::Push(json!(1)),
            Instr::Jump(5),
            Instr::Push(json!(2)),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn vm_jump_unconditional() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(1)),
            Instr::Jump(3),
            Instr::Push(json!(2)),
            Instr::Push(json!(3)),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn vm_prim_add() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(3)),
            Instr::Push(json!(4)),
            Instr::Prim("add".into(), 2),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(7));
    }

    #[test]
    fn vm_prim_sub() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(10)),
            Instr::Push(json!(3)),
            Instr::Prim("sub".into(), 2),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(7));
    }

    #[test]
    fn vm_prim_mul() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(6)),
            Instr::Push(json!(7)),
            Instr::Prim("mul".into(), 2),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn vm_prim_div() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(20)),
            Instr::Push(json!(4)),
            Instr::Prim("div".into(), 2),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn vm_prim_div_by_zero_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(1)),
            Instr::Push(json!(0)),
            Instr::Prim("div".into(), 2),
            Instr::Return,
        ];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_prim_eq() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(5)),
            Instr::Push(json!(5)),
            Instr::Prim("eq".into(), 2),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(true));
    }

    #[test]
    fn vm_prim_lt_gt() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(3)),
            Instr::Push(json!(5)),
            Instr::Prim("lt".into(), 2),
            Instr::Return,
        ];
        assert_eq!(exec_raw(&lang, &ast, &instrs, None).unwrap(), json!(true));

        let instrs = vec![
            Instr::Push(json!(3)),
            Instr::Push(json!(5)),
            Instr::Prim("gt".into(), 2),
            Instr::Return,
        ];
        assert_eq!(exec_raw(&lang, &ast, &instrs, None).unwrap(), json!(false));
    }

    #[test]
    fn vm_prim_not_and_or() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);

        // not true → false
        let instrs = vec![
            Instr::Push(json!(true)),
            Instr::Prim("not".into(), 1),
            Instr::Return,
        ];
        assert_eq!(exec_raw(&lang, &ast, &instrs, None).unwrap(), json!(false));

        // true and false → false
        let instrs = vec![
            Instr::Push(json!(true)),
            Instr::Push(json!(false)),
            Instr::Prim("and".into(), 2),
            Instr::Return,
        ];
        assert_eq!(exec_raw(&lang, &ast, &instrs, None).unwrap(), json!(false));

        // true or false → true
        let instrs = vec![
            Instr::Push(json!(true)),
            Instr::Push(json!(false)),
            Instr::Prim("or".into(), 2),
            Instr::Return,
        ];
        assert_eq!(exec_raw(&lang, &ast, &instrs, None).unwrap(), json!(true));
    }

    #[test]
    fn vm_prim_unknown_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![Instr::Prim("unknown_op".into(), 0), Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unknown prim"));
    }

    #[test]
    fn vm_prim_add_non_number_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!("not a number")),
            Instr::Push(json!(1)),
            Instr::Prim("add".into(), 2),
            Instr::Return,
        ];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_prim_closure() {
        let lang = Language::new();
        let ast = make_ast(vec![
            AstNode::new("function").with_payload(json!({"params": ["x"], "body": 0})),
        ]);
        let instrs = vec![Instr::Prim("closure".into(), 0), Instr::Return];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        let obj = result.as_object().unwrap();
        assert_eq!(obj.get("__closure__"), Some(&json!(true)));
        assert_eq!(obj.get("params"), Some(&json!(["x"])));
        assert_eq!(obj.get("body"), Some(&json!(0)));
        assert!(obj.get("env").is_some());
    }

    #[test]
    fn vm_prim_closure_missing_params_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![
            AstNode::new("function").with_payload(json!({"body": 0})),
        ]);
        let instrs = vec![Instr::Prim("closure".into(), 0), Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_prim_closure_missing_body_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![
            AstNode::new("function").with_payload(json!({"params": ["x"]})),
        ]);
        let instrs = vec![Instr::Prim("closure".into(), 0), Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    // --- Foreign tests ---

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

    #[test]
    fn vm_foreign_dispatches_to_host() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(21)),
            Instr::Foreign("double".into(), 1),
            Instr::Return,
        ];
        let reg = conformance_registry();
        let result = exec_raw(&lang, &ast, &instrs, Some(&reg)).unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn vm_foreign_no_host_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(21)),
            Instr::Foreign("double".into(), 1),
            Instr::Return,
        ];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unregistered foreign"));
    }

    #[test]
    fn vm_foreign_host_present_but_fn_not_found_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(21)),
            Instr::Foreign("triple".into(), 1),
            Instr::Return,
        ];
        let reg = conformance_registry();
        let err = exec_raw(&lang, &ast, &instrs, Some(&reg)).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("unregistered foreign"));
    }

    // --- EvalChild tests (via exec_node) ---

    #[test]
    fn vm_eval_child() {
        let mut lang = Language::new();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
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
        // Test via exec_node which handles EvalChild properly (compiles
        // child nodes on demand via the cache).
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let root_ref = AstRef::new(ast.len() - 1);
        let result = frame.exec_node(root_ref).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn vm_eval_child_missing_children_errors() {
        let mut lang = Language::new();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        let ast = make_ast(vec![AstNode::new("binary.add").with_payload(json!({}))]);
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let root_ref = AstRef::new(ast.len() - 1);
        let err = frame.exec_node(root_ref).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_eval_child_index_out_of_range_errors() {
        let mut lang = Language::new();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("binary.add").with_payload(json!({"children": [0]})),
        ]);
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let root_ref = AstRef::new(ast.len() - 1);
        let err = frame.exec_node(root_ref).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_no_scheme_returns_payload() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("custom").with_payload(json!("hello"))]);
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let root_ref = AstRef::new(ast.len() - 1);
        let result = frame.exec_node(root_ref).unwrap();
        assert_eq!(result, json!("hello"));
    }

    #[test]
    fn vm_compilation_cache_reuses() {
        // Execute a node twice; the cache should contain the compiled instrs.
        let mut lang = Language::new();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
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
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let root_ref = AstRef::new(ast.len() - 1);
        let r1 = frame.exec_node(root_ref).unwrap();
        assert_eq!(r1, json!(5));
        // Cache should now have entries for all 3 nodes.
        assert_eq!(frame.cache.len(), 3);
        // Second execution uses cache (same result).
        let r2 = frame.exec_node(root_ref).unwrap();
        assert_eq!(r2, json!(5));
        // Cache size unchanged (no recompilation).
        assert_eq!(frame.cache.len(), 3);
    }

    // --- Call / closure tests ---

    #[test]
    fn vm_call_closure() {
        let mut lang = Language::new();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.fn",
                "function",
                Scheme::prim("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::var("$payload"),
            ))
            .unwrap();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.call",
                "call",
                Scheme::App {
                    callee: Box::new(Scheme::child(0)),
                    args: vec![Scheme::child(1)],
                },
            ))
            .unwrap();
        // Ast for: fn(x) { x + 1 } applied to 5 → 6
        // 0: var("x")
        // 1: lit(1)
        // 2: binary.add {children: [0, 1]}
        // 3: function {params: ["x"], body: 2}
        // 4: lit(5)
        // 5: call {children: [3, 4]}  — App{callee: child(0), args: [child(1)]}
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
            AstNode::new("function").with_payload(json!({"params": ["x"], "body": 2})),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("call").with_payload(json!({"children": [3, 4]})),
        ]);
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let root_ref = AstRef::new(ast.len() - 1);
        let result = frame.exec_node(root_ref).unwrap();
        assert_eq!(result, json!(6));
    }

    #[test]
    fn vm_call_non_closure_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(42)),
            AstNode::new("test"),
        ]);
        let closure = json!(42);
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let err = frame.apply_closure(&closure, &[]).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_call_object_not_closure_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let closure = json!({"foo": 1});
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let err = frame.apply_closure(&closure, &[]).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_call_wrong_arity_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let closure = json!({
            "__closure__": true,
            "params": ["x"],
            "body": 0,
            "env": {}
        });
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let err = frame
            .apply_closure(&closure, &[json!(1), json!(2)])
            .unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_call_closure_missing_params_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let closure = json!({
            "__closure__": true,
            "params": "not_an_array",
            "body": 0
        });
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let err = frame.apply_closure(&closure, &[]).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_call_closure_missing_body_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let closure = json!({
            "__closure__": true,
            "params": []
        });
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let err = frame.apply_closure(&closure, &[]).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_call_closure_param_not_string_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let closure = json!({
            "__closure__": true,
            "params": [123],
            "body": 0
        });
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let err = frame.apply_closure(&closure, &[json!(1)]).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_closure_captures_env() {
        let mut lang = Language::new();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::var("$payload"),
            ))
            .unwrap();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
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
            .merge(handl_language::EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.fn",
                "function",
                Scheme::prim("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
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
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let root_ref = AstRef::new(ast.len() - 1);
        let result = frame.exec_node(root_ref).unwrap();
        assert_eq!(result, json!(15));
    }

    // --- Version check test ---

    #[test]
    fn version_mismatch_errors() {
        let mut lang = Language::new();
        lang.scheme_version = SchemeVersion::new("v2");
        // The engine should reject v2.
        assert!(!SchemeVersion::v1().supports(&lang.scheme_version));
    }

    // --- Stack underflow tests ---

    #[test]
    fn vm_return_empty_stack_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_no_return_returns_top_of_stack() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![Instr::Push(json!(99))];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(99));
    }

    #[test]
    fn vm_no_return_empty_stack_errors() {
        // No Return and empty stack — should error.
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs: Vec<Instr> = vec![];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_float_arithmetic() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(1.5)),
            Instr::Push(json!(2.5)),
            Instr::Prim("add".into(), 2),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(4.0));
    }

    #[test]
    fn vm_float_sub() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(5.5)),
            Instr::Push(json!(1.5)),
            Instr::Prim("sub".into(), 2),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(4.0));
    }

    #[test]
    fn vm_float_mul() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(2.5)),
            Instr::Push(json!(3.0)),
            Instr::Prim("mul".into(), 2),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(7.5));
    }

    #[test]
    fn vm_div_floats() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(7.5)),
            Instr::Push(json!(2.5)),
            Instr::Prim("div".into(), 2),
            Instr::Return,
        ];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(3.0));
    }

    #[test]
    fn vm_div_int_by_zero_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(1)),
            Instr::Push(json!(0)),
            Instr::Prim("div".into(), 2),
            Instr::Return,
        ];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    // --- AstIndexOutOfBounds test ---

    #[test]
    fn vm_ast_index_out_of_bounds_errors() {
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let mut frame = VmFrame {
            ast: &ast,
            lang: &lang,
            modules: None,
            cache: HashMap::new(),
            scopes: ScopeStack::new(),
        };
        let err = frame.exec_node(AstRef::new(99)).unwrap_err();
        assert_eq!(err, LangError::AstIndexOutOfBounds { index: 99 });
    }

    // --- Coverage: stack underflow and edge cases ---

    #[test]
    fn vm_load_payload_var_unknown_variable_errors() {
        // LoadPayloadVar with a valid string payload but the variable
        // is not in scope.
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("var").with_payload(json!("unknown"))]);
        let instrs = vec![Instr::LoadPayloadVar, Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert_eq!(
            err,
            LangError::UnknownVariable {
                name: "unknown".into()
            }
        );
    }

    #[test]
    fn vm_store_payload_name_stack_underflow_errors() {
        // StorePayloadName with an empty operand stack.
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("let").with_payload(json!({"name": "x"}))]);
        let instrs = vec![Instr::StorePayloadName, Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_jump_if_false_stack_underflow_errors() {
        // JumpIfFalse with an empty operand stack.
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![Instr::JumpIfFalse(1), Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_foreign_stack_underflow_errors() {
        // Foreign with an empty operand stack (expects 1 arg but none pushed).
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let reg = conformance_registry();
        let instrs = vec![Instr::Foreign("double".into(), 1), Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, Some(&reg)).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_exec_raw_empty_ast_returns_null() {
        // The exec_raw helper's empty-ast early return.
        let lang = Language::new();
        let ast = Ast::new();
        let instrs = vec![Instr::Return];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn vm_store_stack_underflow_errors() {
        // Store with an empty operand stack.
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![Instr::Store("x".into()), Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_call_stack_underflow_callee_errors() {
        // Call with an empty operand stack (no callee).
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![Instr::Call(0), Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_call_stack_underflow_args_errors() {
        // Call(2) with only 1 value on the stack (callee + 1 arg, but
        // expects 2 args).
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![Instr::Push(json!(1)), Instr::Call(2), Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_prim_stack_underflow_errors() {
        // Prim("add", 2) with an empty operand stack.
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![Instr::Prim("add".into(), 2), Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_prim_partial_stack_underflow_errors() {
        // Prim("add", 2) with only 1 value on the stack.
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let instrs = vec![
            Instr::Push(json!(1)),
            Instr::Prim("add".into(), 2),
            Instr::Return,
        ];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_call_closure_body_exec_error_restores_scope() {
        // When a closure body execution errors, the scope stack should
        // still be restored (the saved scopes are put back).
        let lang = Language::new();
        let ast = make_ast(vec![AstNode::new("test")]);
        let closure = json!({
            "__closure__": true,
            "params": [],
            "body": 99,  // invalid body index
            "env": {}
        });
        let mut frame = VmFrame::new(&ast, &lang, None);
        let err = frame.apply_closure(&closure, &[]).unwrap_err();
        assert!(matches!(err, LangError::AstIndexOutOfBounds { .. }));
    }

    #[test]
    fn vm_call_closure_with_env_no_object() {
        // Closure with no "env" field (defaults to empty).
        let mut lang = Language::new();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::var("$payload"),
            ))
            .unwrap();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.call",
                "call",
                Scheme::App {
                    callee: Box::new(Scheme::child(0)),
                    args: vec![Scheme::child(1)],
                },
            ))
            .unwrap();
        // fn(x) { x + 1 } called with 5, but closure has no env field
        let ast = make_ast(vec![
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("call").with_payload(json!({"children": [3, 3]})),
        ]);
        // Manually create a closure and call it
        let closure = json!({
            "__closure__": true,
            "params": ["x"],
            "body": 2,
        });
        let mut frame = VmFrame::new(&ast, &lang, None);
        let result = frame.apply_closure(&closure, &[json!(5)]).unwrap();
        assert_eq!(result, json!(6));
    }

    // --- match / handle primitive tests ---

    /// A language with `var`, `match`, `handle`, `array`, `object`, and
    /// `binary.add` schemes.
    fn match_lang() -> Language {
        let mut lang = Language::new();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::var("$payload"),
            ))
            .unwrap();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.match",
                "match",
                Scheme::prim("match", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.handle",
                "handle",
                Scheme::prim("handle", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.array",
                "array",
                Scheme::prim("array", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.object",
                "object",
                Scheme::prim("object", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(handl_language::EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        lang
    }

    #[test]
    fn vm_match_wildcard() {
        // match 5 { _: 1 } → 1
        // 0: lit(5), 1: var("_"), 2: lit(1), 3: arm{[1,2]}, 4: block{[3]}, 5: match{[0,4]}
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("var").with_payload(json!("_")),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})),
            AstNode::new("block").with_payload(json!({"children": [3]})),
            AstNode::new("match").with_payload(json!({"children": [0, 4]})),
        ]);
        let instrs = vec![Instr::Prim("match".into(), 0), Instr::Return];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn vm_match_binder() {
        // match 5 { x: x } → 5
        // 0: lit(5), 1: var("x") (pattern), 2: var("x") (body), 3: arm{[1,2]}, 4: block{[3]}, 5: match{[0,4]}
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})),
            AstNode::new("block").with_payload(json!({"children": [3]})),
            AstNode::new("match").with_payload(json!({"children": [0, 4]})),
        ]);
        let instrs = vec![Instr::Prim("match".into(), 0), Instr::Return];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn vm_match_literal() {
        // match 3 { 1: "a", 3: "b" } → "b"
        // 0: lit(3), 1: lit(1), 2: lit("a"), 3: arm{[1,2]},
        // 4: lit(3), 5: lit("b"), 6: arm{[4,5]},
        // 7: block{[3,6]}, 8: match{[0,7]}
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
        let instrs = vec![Instr::Prim("match".into(), 0), Instr::Return];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!("b"));
    }

    #[test]
    fn vm_match_no_arm_errors() {
        // match 9 { 1: "a" } → error
        // 0: lit(9), 1: lit(1), 2: lit("a"), 3: arm{[1,2]}, 4: block{[3]}, 5: match{[0,4]}
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(9)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!("a")),
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})),
            AstNode::new("block").with_payload(json!({"children": [3]})),
            AstNode::new("match").with_payload(json!({"children": [0, 4]})),
        ]);
        let instrs = vec![Instr::Prim("match".into(), 0), Instr::Return];
        let err = exec_raw(&lang, &ast, &instrs, None).unwrap_err();
        assert!(matches!(err, LangError::Engine(_)));
    }

    #[test]
    fn vm_match_array_destructuring() {
        // match [1,2] { [a, b]: a + b } → 3
        // 0: array{elements:[1,2]}, 1: var("a"), 2: var("b"),
        // 3: array{children:[1,2]} (pattern), 4: var("a"), 5: var("b"),
        // 6: binary.add{children:[4,5]}, 7: arm{[3,6]}, 8: block{[7]}, 9: match{[0,8]}
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("array").with_payload(json!({"elements": [1, 2]})),
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
        let instrs = vec![Instr::Prim("match".into(), 0), Instr::Return];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn vm_match_array_rest() {
        // match [1,2,3,4] { [a, ...rest]: rest } → [2,3,4]
        // (one positional pattern `a` binds to 1; rest captures indices 1..)
        // 0: array{elements:[1,2,3,4]}, 1: var("a"), 2: var("rest"),
        // 3: rest{children:[2]}, 4: array{children:[1,3]} (pattern),
        // 5: var("rest") (body), 6: arm{[4,5]}, 7: block{[6]}, 8: match{[0,7]}
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("array").with_payload(json!({"elements": [1, 2, 3, 4]})),
            AstNode::new("var").with_payload(json!("a")),
            AstNode::new("var").with_payload(json!("rest")),
            AstNode::new("rest").with_payload(json!({"children": [2]})),
            AstNode::new("array").with_payload(json!({"children": [1, 3]})),
            AstNode::new("var").with_payload(json!("rest")),
            AstNode::new("arm").with_payload(json!({"children": [4, 5]})),
            AstNode::new("block").with_payload(json!({"children": [6]})),
            AstNode::new("match").with_payload(json!({"children": [0, 7]})),
        ]);
        let instrs = vec![Instr::Prim("match".into(), 0), Instr::Return];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!([2, 3, 4]));
    }

    #[test]
    fn vm_match_object_destructuring() {
        // match {x: 1, y: 2} { {x: a, y: b}: a + b } → 3
        // 0: object{entries:[{key:"x",value:1},{key:"y",value:2}]},
        // 1: var("x"), 2: var("a"), 3: entry{children:[1,2]},
        // 4: var("y"), 5: var("b"), 6: entry{children:[4,5]},
        // 7: object{children:[3,6]} (pattern),
        // 8: var("a"), 9: var("b"), 10: binary.add{children:[8,9]},
        // 11: arm{[7,10]}, 12: block{[11]}, 13: match{[0,12]}
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("object").with_payload(json!({"entries": [
                {"key": "x", "value": 1},
                {"key": "y", "value": 2},
            ]})),
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
        let instrs = vec![Instr::Prim("match".into(), 0), Instr::Return];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(3));
    }

    #[test]
    fn vm_match_object_rest() {
        // match {x: 1, y: 2, z: 3} { {x: a, ...rest}: rest } → {y: 2, z: 3}
        // 0: object{entries:[{key:"x",value:1},{key:"y",value:2},{key:"z",value:3}]},
        // 1: var("x"), 2: var("a"), 3: entry{children:[1,2]},
        // 4: var("rest"), 5: rest{children:[4]},
        // 6: object{children:[3,5]} (pattern),
        // 7: var("rest") (body), 8: arm{[6,7]}, 9: block{[8]}, 10: match{[0,9]}
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("object").with_payload(json!({"entries": [
                {"key": "x", "value": 1},
                {"key": "y", "value": 2},
                {"key": "z", "value": 3},
            ]})),
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
        let instrs = vec![Instr::Prim("match".into(), 0), Instr::Return];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!({"y": 2, "z": 3}));
    }

    #[test]
    fn vm_handle_dispatches() {
        // handle 5 { x: x + 1 } → 6
        // 0: lit(5), 1: var("x"), 2: lit(1),
        // 3: binary.add{children:[1,2]}, 4: arm{[1,3]},
        // 5: block{[4]}, 6: handle{[0,5]}
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("var").with_payload(json!("x")),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("binary.add").with_payload(json!({"children": [1, 2]})),
            AstNode::new("arm").with_payload(json!({"children": [1, 3]})),
            AstNode::new("block").with_payload(json!({"children": [4]})),
            AstNode::new("handle").with_payload(json!({"children": [0, 5]})),
        ]);
        let instrs = vec![Instr::Prim("handle".into(), 0), Instr::Return];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(6));
    }

    #[test]
    fn vm_handle_no_clause_returns_body() {
        // handle 5 { 1: "a" } → 5 (body returned as-is, no clause matches)
        // 0: lit(5), 1: lit(1), 2: lit("a"), 3: arm{[1,2]},
        // 4: block{[3]}, 5: handle{[0,4]}
        let lang = match_lang();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(5)),
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!("a")),
            AstNode::new("arm").with_payload(json!({"children": [1, 2]})),
            AstNode::new("block").with_payload(json!({"children": [3]})),
            AstNode::new("handle").with_payload(json!({"children": [0, 4]})),
        ]);
        let instrs = vec![Instr::Prim("handle".into(), 0), Instr::Return];
        let result = exec_raw(&lang, &ast, &instrs, None).unwrap();
        assert_eq!(result, json!(5));
    }
}
