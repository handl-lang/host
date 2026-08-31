//! # handl-plugin-engine-eval-tree-walk
//!
//! The **reference interpreter** for the eval operation (ADR-0044 §3, §4).
//!
//! This engine interprets [`handl_language::Scheme`] **data** directly — it
//! looks up `lang.eval[node.kind]` → [`handl_language::EvalSchemeDef`] →
//! [`Scheme`], and interprets the scheme via tree-walking. It does **not**
//! match on a static `CanonicalKind` enum; it is entirely data-driven.
//!
//! ## Child resolution convention
//!
//! [`Scheme::Child`] references are **relative child indices**: `Child(n)`
//! means "the nth child of the current node". The current node's payload must
//! contain a `"children"` JSON array of arena indices; `Child(n)` reads
//! `payload["children"][n]` to get the absolute [`AstRef`], then recurses.
//!
//! ## `$payload` convention
//!
//! When a [`Scheme::Var`] name or [`Scheme::Bind`] name is the string
//! `"$payload"`, the engine reads the actual name from the current node's
//! payload:
//! - `Var("$payload")` → reads `node.payload` as a string (the variable name).
//! - `Bind { name: "$payload", ... }` → reads `node.payload["name"]` as a
//!   string (the bind name).
//!
//! This keeps evaluation data-driven: the scheme says "get the name from the
//! node", not "use this hardcoded name".
//!
//! ## No-scheme fallback
//!
//! If a node kind has no eval scheme in the `Language`, the engine returns
//! `node.payload` as the value directly. This handles literal nodes (e.g. a
//! `"lit"` node whose payload is `42`).
//!
//! ## Closure representation
//!
//! Closures are `Value::Object` with the shape:
//! ```json
//! {"__closure__": true, "params": ["x", ...], "body": <arena_idx>, "env": {...}}
//! ```
//! Created by the `"closure"` primitive (see [`Prim` handling](#primitives)).
//! [`Scheme::App`] evaluates the callee, checks for `__closure__`, binds
//! params to args in a new scope on top of the captured env, and evaluates
//! the body node at the stored arena index.
//!
//! ## Primitives
//!
//! The built-in primitive set: `add`, `sub`, `mul`, `div`, `mod`, `eq`, `lt`,
//! `gt`, `not`, `and`, `or`, `neg`, and `closure`. Unknown ops return
//! [`LangError::Engine`].
//!
//! ## Scope management
//!
//! Uses a `Vec<HashMap<String, Value>>` scope stack. `Bind` pushes a child
//! scope; lookup searches innermost-first. Closure calls swap in the captured
//! environment, bind params, evaluate the body, then restore the original
//! scope stack.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::rc::Rc;

use handl_language::{Ast, AstNode, AstRef, LangError, Language, Scheme, Value, ast_value};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};

/// The tree-walk eval engine.
///
/// Construct with [`TreeWalkEngine::new`] and optionally pass a module
/// registry via [`TreeWalkEngine::with_modules`].
#[derive(Clone)]
pub struct TreeWalkEngine {
    /// The module registry for `Foreign` dispatch (may be `None`).
    modules: Option<Rc<dyn handl_platform::ModuleLookup>>,
    /// Global variables injected by the host (e.g. `self` → language value).
    /// Pre-populated into the root scope on every eval call.
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
    pub fn with_modules(mut self, modules: Rc<dyn handl_platform::ModuleLookup>) -> Self {
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
}

impl Default for TreeWalkEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// A scope stack: `Vec<HashMap<String, Value>>` searched innermost-first.
struct ScopeStack {
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

/// Evaluation context passed through the recursive interpreter.
struct EvalCtx<'a> {
    /// The AST being evaluated.
    ast: &'a Ast,
    /// The current node's arena reference (retained for debugging and
    /// future use by engines that need absolute refs).
    #[allow(dead_code)]
    current_ref: AstRef,
    /// The current node.
    current_node: &'a AstNode,
    /// The language definition.
    lang: &'a Language,
    /// The scope stack.
    env: &'a mut ScopeStack,
    /// The module registry (for `Foreign` dispatch).
    modules: Option<&'a Rc<dyn handl_platform::ModuleLookup>>,
}

impl EvalEngine for TreeWalkEngine {
    fn eval(&self, lang: &Language, req: &EvalRequest) -> Result<Value, LangError> {
        let ast = &req.ast;
        if ast.is_empty() {
            return Ok(Value::Null);
        }
        let mut env = ScopeStack::new();
        // Inject host-provided globals (e.g. `self` → language value) into
        // the root scope. ADR-0041 Phase 3: `self` is an ambient variable,
        // not a function call.
        for (name, val) in &self.globals {
            env.define(name, val.clone());
        }
        // The root is the last node in the arena (Pratt convention).
        let root_ref = AstRef::new(ast.len() - 1);
        self.eval_node(ast, root_ref, lang, &mut env)
    }

    fn box_clone(&self) -> Box<dyn EvalEngine> {
        Box::new(self.clone())
    }

    fn with_modules(&self, modules: Rc<dyn handl_platform::ModuleLookup>) -> Box<dyn EvalEngine> {
        Box::new(self.clone().with_modules(modules))
    }

    fn with_global(&self, name: &str, val: Value) -> Box<dyn EvalEngine> {
        Box::new(self.clone().with_global(name, val))
    }
}

impl TreeWalkEngine {
    /// Evaluate a single node by looking up its eval scheme and interpreting
    /// it. If no scheme exists for the node kind:
    /// - `foreign.<name>` kinds dispatch to host function `<name>` (ADR-0041
    ///   Phase 2 — makes `eval`/`self`/`compile` handl-callable).
    /// - Other kinds return the payload as the value.
    fn eval_node(
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
                let mut ctx = EvalCtx {
                    ast,
                    current_ref: node_ref,
                    current_node: node,
                    lang,
                    env,
                    modules: self.modules.as_ref(),
                };
                self.eval_scheme(&def.scheme, &mut ctx)
            }
            None => {
                // Fallback for `foreign.<name>` node kinds (ADR-0041 Phase 2):
                // if no explicit scheme exists but the kind is `foreign.<name>`,
                // dispatch to the host function `<name>` with children as args.
                // This makes `eval`/`self`/`compile` handl-callable without
                // requiring per-function scheme entries in the Language data.
                if let Some(foreign_name) = node.kind.strip_prefix("foreign.") {
                    let modules = self.modules.as_ref().ok_or_else(|| {
                        LangError::Engine(format!("unregistered foreign: {}", foreign_name))
                    })?;
                    let func = modules.lookup(foreign_name).ok_or_else(|| {
                        LangError::Engine(format!("unregistered foreign: {}", foreign_name))
                    })?;
                    let mut arg_vals = Vec::new();
                    if let Some(children) = node.payload.get("children").and_then(|c| c.as_array())
                    {
                        for child_ref in children {
                            if let Some(idx) = child_ref.as_u64() {
                                let child_ref = AstRef::new(idx as usize);
                                arg_vals.push(self.eval_node(ast, child_ref, lang, env)?);
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

    /// Interpret a [`Scheme`] combinator in the given context.
    fn eval_scheme(&self, scheme: &Scheme, ctx: &mut EvalCtx<'_>) -> Result<Value, LangError> {
        match scheme {
            Scheme::Lit(v) => Ok(v.clone()),

            Scheme::Var(name) => {
                let var_name = if name == "$payload" {
                    ctx.current_node
                        .payload
                        .as_str()
                        .ok_or_else(|| {
                            LangError::Engine(
                                "expected string payload for $payload var".to_string(),
                            )
                        })?
                        .to_string()
                } else {
                    name.clone()
                };
                // ADR-0041 Phase 2: if the variable is not bound in scope,
                // return its name as a string value. This allows unbound
                // identifiers like `eval` to flow into `App` dispatch, where
                // `apply_closure` / `Scheme::App` checks host functions.
                // A bound variable shadows this fallback (the common case).
                ctx.env
                    .lookup(&var_name)
                    .cloned()
                    .map(Ok)
                    .unwrap_or_else(|| Ok(Value::from(var_name)))
            }

            Scheme::Child(child_ref) => {
                let children = ctx
                    .current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                    .ok_or_else(|| {
                        LangError::Engine("expected children array in payload".to_string())
                    })?;
                let arena_idx = children
                    .get(child_ref.index())
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        LangError::Engine(format!("child {} not found", child_ref.index()))
                    })?;
                let target_ref = AstRef::new(arena_idx as usize);
                // Save and restore env across child eval (child shares the
                // same scope, no new scope pushed).
                self.eval_node(ctx.ast, target_ref, ctx.lang, ctx.env)
            }

            Scheme::Bind { name, value, body } => {
                let val = self.eval_scheme(value, ctx)?;
                let bind_name = if name == "$payload" {
                    ctx.current_node
                        .payload
                        .get("name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            LangError::Engine(
                                "expected name in payload for $payload bind".to_string(),
                            )
                        })?
                        .to_string()
                } else {
                    name.clone()
                };
                ctx.env.push_scope();
                ctx.env.define(&bind_name, val);
                let result = self.eval_scheme(body, ctx);
                ctx.env.pop_scope();
                result
            }

            Scheme::App { callee, args } => {
                let closure_val = self.eval_scheme(callee, ctx)?;
                // ADR-0041 Phase 2: evaluate args, tolerating missing children
                // (the call grammar is fixed arity 2 = callee + 1 arg, but
                // the scheme lists 2 args; Child(2) may be out of bounds for
                // source-parsed calls with 1 arg). Missing children are
                // skipped, so `eval("1 + 2")` dispatches with 1 arg.
                let mut arg_vals = Vec::new();
                for arg in args {
                    match self.eval_scheme(arg, ctx) {
                        Ok(v) => arg_vals.push(v),
                        Err(LangError::Engine(ref msg))
                            if msg.contains("child ") && msg.contains("not found") =>
                        {
                            // Skip missing child — variable-arity call.
                        }
                        Err(e) => return Err(e),
                    }
                }
                // If the callee is a string naming a registered host
                // function, dispatch to it. This makes `eval("1 + 2")` work:
                // `var("eval")` evaluates to the string "eval", and App
                // dispatches to the eval host fn.
                if let Some(name) = closure_val.as_str() {
                    if let Some(modules) = ctx.modules {
                        if let Some(func) = modules.lookup(name) {
                            let mut pctx = handl_platform::EvalContext::new();
                            return func
                                .call(&arg_vals, &mut pctx)
                                .map_err(|e| LangError::Engine(e.to_string()));
                        }
                    }
                }
                self.apply_closure(&closure_val, &arg_vals, ctx)
            }

            Scheme::Branch { cond, then_, else_ } => {
                let cond_val = self.eval_scheme(cond, ctx)?;
                if is_truthy(&cond_val) {
                    self.eval_scheme(then_, ctx)
                } else {
                    self.eval_scheme(else_, ctx)
                }
            }

            Scheme::Prim { op, args } => {
                let mut arg_vals = Vec::with_capacity(args.len());
                for arg in args {
                    arg_vals.push(self.eval_scheme(arg, ctx)?);
                }
                self.apply_prim(op, &arg_vals, ctx)
            }

            Scheme::Foreign { name, args } => {
                let mut arg_vals = Vec::with_capacity(args.len());
                for arg in args {
                    arg_vals.push(self.eval_scheme(arg, ctx)?);
                }
                let modules = ctx
                    .modules
                    .ok_or_else(|| LangError::Engine(format!("unregistered foreign: {}", name)))?;
                let func = modules
                    .lookup(name)
                    .ok_or_else(|| LangError::Engine(format!("unregistered foreign: {}", name)))?;
                let mut pctx = handl_platform::EvalContext::new();
                func.call(&arg_vals, &mut pctx)
                    .map_err(|e| LangError::Engine(e.to_string()))
            }

            Scheme::Vau { params, body } => {
                // The vau escape (ADR-0041 §6, ADR-0045): do NOT evaluate
                // the children. Instead, bind each param to the
                // unevaluated child subtree (a tagged `__ast__` value) and
                // evaluate the body in the extended scope. The body can
                // call `eval(self, ast)` to evaluate a child on demand.
                let children = ctx
                    .current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array());
                ctx.env.push_scope();
                for (i, param) in params.iter().enumerate() {
                    let arena_idx = children
                        .and_then(|arr| arr.get(i))
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| LangError::Engine(format!("vau: child {} not found", i)))?;
                    let sub = ctx.ast.subtree(AstRef::new(arena_idx as usize))?;
                    ctx.env.define(param, ast_value(&sub));
                }
                let result = self.eval_scheme(body, ctx);
                ctx.env.pop_scope();
                result
            }
        }
    }

    /// Apply a closure value to argument values.
    fn apply_closure(
        &self,
        closure: &Value,
        args: &[Value],
        ctx: &mut EvalCtx<'_>,
    ) -> Result<Value, LangError> {
        // ADR-0041 Phase 2: if the callee is a string that names a registered
        // host function, dispatch to it. This makes `eval(self, "1 + 2")`
        // parse as `call(var("eval"), ...)` and dispatch to the `eval` host
        // function — no special syntax needed.
        if let Some(name) = closure.as_str() {
            if let Some(modules) = ctx.modules {
                if let Some(func) = modules.lookup(name) {
                    let mut pctx = handl_platform::EvalContext::new();
                    return func
                        .call(args, &mut pctx)
                        .map_err(|e| LangError::Engine(e.to_string()));
                }
            }
        }

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
        let saved = std::mem::replace(ctx.env, ScopeStack::new());
        ctx.env.scopes[0] = captured_env;
        ctx.env.push_scope();
        for (param, arg) in params.iter().zip(args.iter()) {
            ctx.env.define(param, arg.clone());
        }

        let result = self.eval_node(ctx.ast, body_ref, ctx.lang, ctx.env);

        // Restore the original scope stack.
        *ctx.env = saved;
        result
    }

    /// Apply a primitive operation.
    fn apply_prim(
        &self,
        op: &str,
        args: &[Value],
        ctx: &mut EvalCtx<'_>,
    ) -> Result<Value, LangError> {
        match op {
            "add" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok(num_value(a + b))
            }
            "sub" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok(num_value(a - b))
            }
            "mul" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok(num_value(a * b))
            }
            "div" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                if b.to_f64() == 0.0 {
                    return Err(LangError::Engine("division by zero".to_string()));
                }
                Ok(num_value(a / b))
            }
            "mod" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                if b.to_f64() == 0.0 {
                    return Err(LangError::Engine("modulo by zero".to_string()));
                }
                match (a, b) {
                    (Num::Int(a), Num::Int(b)) => Ok(num_value(Num::Int(a % b))),
                    (a, b) => Ok(num_value(Num::Float(a.to_f64() % b.to_f64()))),
                }
            }
            "pow" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                let result = a.to_f64().powf(b.to_f64());
                Ok(num_value(Num::Float(result)))
            }
            "eq" => Ok(Value::Bool(args[0] == args[1])),
            "lt" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok(Value::Bool(a < b))
            }
            "gt" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok(Value::Bool(a > b))
            }
            "gte" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok(Value::Bool(a >= b))
            }
            "lte" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                Ok(Value::Bool(a <= b))
            }
            "neq" => Ok(Value::Bool(args[0] != args[1])),
            "compare" => {
                let a = as_num(&args[0])?;
                let b = as_num(&args[1])?;
                let cmp = a
                    .partial_cmp(&b)
                    .ok_or_else(|| LangError::Engine("compare: incomparable values".to_string()))?;
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
            "neg" => {
                let a = as_num(&args[0])?;
                match a {
                    Num::Int(i) => Ok(num_value(Num::Int(-i))),
                    Num::Float(f) => Ok(num_value(Num::Float(-f))),
                }
            }
            "closure" => {
                // Create a closure from the current node's payload.
                //
                // Three payload shapes are supported:
                // - **Rich**: `{"params": ["x", ...], "body": <arena_idx>}`
                // - **Pratt-parsed (bare param)**: `{"children": [var_idx,
                //   body_idx]}` where `var_idx` points to a `var` node whose
                //   payload is the parameter name string.
                // - **Pratt-parsed (paren params)**: `{"children": [paren_idx,
                //   body_idx]}` where `paren_idx` points to a `paren` node
                //   whose children are `var` nodes — supports multi-param
                //   `(x, y) -> body`.
                let payload = &ctx.current_node.payload;
                let (params, body) = if let (Some(p), Some(b)) = (
                    payload.get("params").and_then(|v| v.as_array()),
                    payload.get("body").and_then(|v| v.as_u64()),
                ) {
                    (p.clone(), b as usize)
                } else if let Some(children) = payload.get("children").and_then(|c| c.as_array()) {
                    let param_idx = children.get(0).and_then(|v| v.as_u64()).ok_or_else(|| {
                        LangError::Engine("closure prim: missing param child".to_string())
                    })? as usize;
                    let body_idx = children.get(1).and_then(|v| v.as_u64()).ok_or_else(|| {
                        LangError::Engine("closure prim: missing body child".to_string())
                    })? as usize;
                    let param_node = ctx.ast.at(AstRef::new(param_idx))?;
                    // Extract param names: if the param node is a `paren`,
                    // unwrap its children (multi-param). If it's a `var`,
                    // use its string payload directly (bare single param).
                    let param_names: Vec<String> = if param_node.kind == "paren" {
                        let paren_children = param_node
                            .payload
                            .get("children")
                            .and_then(|c| c.as_array())
                            .ok_or_else(|| {
                                LangError::Engine(
                                    "closure prim: paren params missing children".to_string(),
                                )
                            })?;
                        let mut names = Vec::with_capacity(paren_children.len());
                        for pc in paren_children {
                            let pidx = pc.as_u64().ok_or_else(|| {
                                LangError::Engine(
                                    "closure prim: param index must be a number".to_string(),
                                )
                            })? as usize;
                            let pnode = ctx.ast.at(AstRef::new(pidx))?;
                            let pname = pnode.payload.as_str().ok_or_else(|| {
                                LangError::Engine(
                                    "closure prim: param must be a var with string payload"
                                        .to_string(),
                                )
                            })?;
                            names.push(pname.to_string());
                        }
                        names
                    } else {
                        let pname = param_node.payload.as_str().ok_or_else(|| {
                            LangError::Engine(
                                "closure prim: param node must be a var with string payload"
                                    .to_string(),
                            )
                        })?;
                        vec![pname.to_string()]
                    };
                    (param_names.into_iter().map(Value::from).collect(), body_idx)
                } else {
                    return Err(LangError::Engine(
                        "closure prim: missing params/body in payload".to_string(),
                    ));
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
                // Two payload shapes:
                // - **Rich**: `{"elements": [v1, v2, ...]}` — literal values.
                // - **Pratt-parsed**: `{"children": [elem_idx, ...]}` — each
                //   element is an arena index to evaluate.
                if let Some(elements) = ctx.current_node.payload.get("elements") {
                    Ok(elements.clone())
                } else if let Some(children) = ctx
                    .current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                {
                    let mut result = Vec::with_capacity(children.len());
                    for entry in children {
                        let idx = entry.as_u64().ok_or_else(|| {
                            LangError::Engine("array: element index must be a number".to_string())
                        })? as usize;
                        result.push(self.eval_node(
                            ctx.ast,
                            AstRef::new(idx),
                            ctx.lang,
                            ctx.env,
                        )?);
                    }
                    Ok(Value::Array(result))
                } else {
                    Err(LangError::Engine(
                        "array: missing elements/children in payload".to_string(),
                    ))
                }
            }
            "object" => {
                // Two payload shapes:
                // - **Rich**: `{"entries": [{"key": "k", "value": v}, ...]}`.
                // - **Pratt-parsed**: `{"children": [entry_idx, ...]}` where
                //   each entry node has `{"children": [key_idx, value_idx]}`.
                if let Some(entries) = ctx
                    .current_node
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
                    Ok(Value::Object(obj))
                } else if let Some(children) = ctx
                    .current_node
                    .payload
                    .get("children")
                    .and_then(|c| c.as_array())
                {
                    let mut obj = serde_json::Map::new();
                    for entry in children {
                        let entry_idx = entry.as_u64().ok_or_else(|| {
                            LangError::Engine("object: entry index must be a number".to_string())
                        })? as usize;
                        let entry_node = ctx.ast.at(AstRef::new(entry_idx))?;
                        let entry_children = entry_node
                            .payload
                            .get("children")
                            .and_then(|c| c.as_array())
                            .ok_or_else(|| {
                                LangError::Engine("object: entry node missing children".to_string())
                            })?;
                        let key_idx =
                            entry_children
                                .get(0)
                                .and_then(|v| v.as_u64())
                                .ok_or_else(|| {
                                    LangError::Engine("object: entry missing key child".to_string())
                                })? as usize;
                        let val_idx =
                            entry_children
                                .get(1)
                                .and_then(|v| v.as_u64())
                                .ok_or_else(|| {
                                    LangError::Engine(
                                        "object: entry missing value child".to_string(),
                                    )
                                })? as usize;
                        let key_val =
                            self.eval_node(ctx.ast, AstRef::new(key_idx), ctx.lang, ctx.env)?;
                        let key = key_val.as_str().ok_or_else(|| {
                            LangError::Engine(
                                "object: entry key must evaluate to a string".to_string(),
                            )
                        })?;
                        let value =
                            self.eval_node(ctx.ast, AstRef::new(val_idx), ctx.lang, ctx.env)?;
                        obj.insert(key.to_string(), value);
                    }
                    Ok(Value::Object(obj))
                } else {
                    Err(LangError::Engine(
                        "object: missing entries/children in payload".to_string(),
                    ))
                }
            }
            "index" => {
                let target = ctx.current_node.payload.get("target").ok_or_else(|| {
                    LangError::Engine("index: missing target in payload".to_string())
                })?;
                let index = ctx.current_node.payload.get("index").ok_or_else(|| {
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
                let value = ctx.current_node.payload.get("value").ok_or_else(|| {
                    LangError::Engine("assign: missing value in payload".to_string())
                })?;
                Ok(value.clone())
            }
            "match" => self.prim_match(ctx),
            "handle" => self.prim_handle(ctx),
            "json" => self.prim_json(ctx),
            _ => Err(LangError::Engine(format!("unknown prim: {}", op))),
        }
    }

    /// `match` prim: pattern-match a scrutinee against a list of arms.
    ///
    /// The `match` node's payload has `children: [scrut_idx, block_idx]`. The
    /// block node (kind `block`) has `children: [arm_idx, ...]`. Each arm
    /// node (kind `arm`) has `children: [pattern_idx, body_idx]`.
    ///
    /// Pattern matching interprets the pattern AST node structurally:
    /// - A `var` node named `_` or starting with `_` → **wildcard** (always
    ///   matches, binds nothing).
    /// - A `var` node with any other name → **binder** (always matches, binds
    ///   the name to the scrutinee value in the current scope).
    /// - Any other node → **literal**: the pattern is evaluated as a value
    ///   and compared by equality (`==`) to the scrutinee.
    ///
    /// The first matching arm's body is evaluated lazily; non-matching arms
    /// are never evaluated.
    fn prim_match(&self, ctx: &mut EvalCtx<'_>) -> Result<Value, LangError> {
        let children = ctx
            .current_node
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
        let scrut_val = self.eval_node(ctx.ast, AstRef::new(scrut_idx), ctx.lang, ctx.env)?;

        // Get the block node's children (the arm indices).
        let block_node = ctx.ast.at(AstRef::new(block_idx))?;
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
            let arm_node = ctx.ast.at(AstRef::new(arm_idx))?;
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

            // Interpret the pattern node structurally.
            ctx.env.push_scope();
            let matched = self.try_pattern(
                ctx.ast,
                AstRef::new(pattern_idx),
                &scrut_val,
                ctx.lang,
                ctx.env,
            )?;
            if matched {
                let result = self.eval_node(ctx.ast, AstRef::new(body_idx), ctx.lang, ctx.env);
                ctx.env.pop_scope();
                return result;
            }
            ctx.env.pop_scope();
        }

        Err(LangError::Engine("match: no arm matched".to_string()))
    }

    /// `handle` prim: evaluate a body, then dispatch the result to the first
    /// matching handler clause.
    ///
    /// Structurally identical to `match` (children = [body_idx, block_idx],
    /// block contains `arm` nodes with [pattern_idx, handler_idx]). The body
    /// is evaluated first; its result is matched against clause patterns. If
    /// no clause matches, the body value is returned as-is.
    fn prim_handle(&self, ctx: &mut EvalCtx<'_>) -> Result<Value, LangError> {
        let children = ctx
            .current_node
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
        let body_val = self.eval_node(ctx.ast, AstRef::new(body_idx), ctx.lang, ctx.env)?;

        // Get the block node's children (the clause indices).
        let block_node = ctx.ast.at(AstRef::new(block_idx))?;
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
            let clause_node = ctx.ast.at(AstRef::new(clause_idx))?;
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

            ctx.env.push_scope();
            let matched = self.try_pattern(
                ctx.ast,
                AstRef::new(pattern_idx),
                &body_val,
                ctx.lang,
                ctx.env,
            )?;
            if matched {
                let result = self.eval_node(ctx.ast, AstRef::new(handler_idx), ctx.lang, ctx.env);
                ctx.env.pop_scope();
                return result;
            }
            ctx.env.pop_scope();
        }

        // No clause matched — return the body value as-is.
        Ok(body_val)
    }

    /// `json` prim: parse a JSON template string from the node payload.
    ///
    /// The payload carries `expr` (a JSON string, optionally with `$0`, `$1`,
    /// … placeholders) and `args` (an array of arena indices whose evaluated
    /// values are substituted into the placeholders before parsing). If no
    /// `args` are present, `expr` is parsed directly.
    fn prim_json(&self, ctx: &mut EvalCtx<'_>) -> Result<Value, LangError> {
        let payload = &ctx.current_node.payload;
        let expr = payload
            .get("expr")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LangError::Engine("json: missing expr string in payload".to_string()))?;

        // If there are args, substitute placeholders $0, $1, … with their
        // evaluated JSON representations.
        let final_expr = if let Some(args_arr) = payload.get("args").and_then(|v| v.as_array()) {
            let mut s = expr.to_string();
            for (i, arg_entry) in args_arr.iter().enumerate() {
                // Each arg entry is an arena index referencing a child node.
                let replacement = if let Some(idx) = arg_entry.as_u64() {
                    let val =
                        self.eval_node(ctx.ast, AstRef::new(idx as usize), ctx.lang, ctx.env)?;
                    serde_json::to_string(&val)
                        .map_err(|e| LangError::Engine(format!("json: {}", e)))?
                } else {
                    // Already a literal value — serialize directly.
                    serde_json::to_string(arg_entry)
                        .map_err(|e| LangError::Engine(format!("json: {}", e)))?
                };
                s = s.replace(&format!("${}", i), &replacement);
            }
            s
        } else {
            expr.to_string()
        };

        serde_json::from_str(&final_expr)
            .map_err(|e| LangError::Engine(format!("json: parse error: {}", e)))
    }

    /// Interpret a pattern AST node structurally against a scrutinee value.
    ///
    /// - `var` node named `_` or `_*` → wildcard (always matches).
    /// - `var` node with another name → binder (binds name → scrut, always
    ///   matches).
    /// - `array` node → array destructuring: positional sub-patterns with
    ///   optional `...rest` capture.
    /// - `object` node → object destructuring: key → sub-pattern entries
    ///   with optional `...rest` capture.
    /// - Any other node → literal: evaluate the pattern as a value and
    ///   compare by equality.
    ///
    /// Returns `true` if the pattern matches. Bindings are inserted into
    /// `env` (the caller is responsible for push/pop scope).
    fn try_pattern(
        &self,
        ast: &Ast,
        pattern_ref: AstRef,
        scrut: &Value,
        lang: &Language,
        env: &mut ScopeStack,
    ) -> Result<bool, LangError> {
        let pattern_node = ast.at(pattern_ref)?;
        // Check if it's a `var` node (identifier pattern).
        if pattern_node.kind == "var" {
            if let Some(name) = pattern_node.payload.as_str() {
                if name == "_" || name.starts_with('_') {
                    // Wildcard — always matches, binds nothing.
                    return Ok(true);
                }
                // Binder — always matches, binds name → scrutinee.
                env.define(name, scrut.clone());
                return Ok(true);
            }
        }
        // Array destructuring pattern: [a, b, ...rest]
        if pattern_node.kind == "array" {
            return self.try_array_pattern(ast, pattern_ref, scrut, lang, env);
        }
        // Object destructuring pattern: { x: a, y: b, ...rest }
        if pattern_node.kind == "object" {
            return self.try_object_pattern(ast, pattern_ref, scrut, lang, env);
        }
        // Literal pattern — evaluate and compare by equality.
        let pat_val = self.eval_node(ast, pattern_ref, lang, env)?;
        Ok(pat_val == *scrut)
    }

    /// Extract the binder name from a `rest` pattern node (`...name`).
    ///
    /// Returns `Some(name)` if the rest node binds a name, or `None` if it's
    /// a wildcard rest (`..._` or `..._*`). Returns `Err` if the rest node is
    /// malformed.
    fn rest_binder_name(&self, ast: &Ast, rest_ref: AstRef) -> Result<Option<String>, LangError> {
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
                    return Ok(None); // Wildcard rest — binds nothing.
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
        &self,
        ast: &Ast,
        pattern_ref: AstRef,
        scrut: &Value,
        lang: &Language,
        env: &mut ScopeStack,
    ) -> Result<bool, LangError> {
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

        // Separate positional patterns from the optional rest pattern.
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
            return Ok(false); // Exact length required without rest.
        }
        if scrut_arr.len() < n {
            return Ok(false); // Not enough elements for positional patterns.
        }

        // Match positional sub-patterns.
        for (i, &pat_idx) in positional.iter().enumerate() {
            if !self.try_pattern(ast, AstRef::new(pat_idx), &scrut_arr[i], lang, env)? {
                return Ok(false);
            }
        }

        // Bind remaining elements to the rest binder.
        if let Some(ridx) = rest_idx {
            if let Some(name) = self.rest_binder_name(ast, AstRef::new(ridx))? {
                let rest_vals: Vec<Value> = scrut_arr[n..].to_vec();
                env.define(&name, Value::Array(rest_vals));
            }
        }

        Ok(true)
    }

    /// Object destructuring: `{ x: a, y: b, ...rest }`.
    ///
    /// Each entry's key is a field name (from a `var` or string `value` node);
    /// the entry's value is a sub-pattern matched against the corresponding
    /// field. A `...rest` element captures the unmatched keys as an object.
    fn try_object_pattern(
        &self,
        ast: &Ast,
        pattern_ref: AstRef,
        scrut: &Value,
        lang: &Language,
        env: &mut ScopeStack,
    ) -> Result<bool, LangError> {
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

        // Separate entries from the optional rest pattern.
        let mut rest_idx: Option<usize> = None;
        let mut entries: Vec<(String, usize)> = Vec::new(); // (key, value_pattern_idx)
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
                    .ok_or_else(|| {
                        LangError::Engine("object pattern: malformed entry".to_string())
                    })?;
                let key_idx = entry_children[0].as_u64().ok_or_else(|| {
                    LangError::Engine("object pattern: bad entry key index".to_string())
                })? as usize;
                let val_idx = entry_children[1].as_u64().ok_or_else(|| {
                    LangError::Engine("object pattern: bad entry val index".to_string())
                })? as usize;
                let key_node = ast.at(AstRef::new(key_idx))?;
                // Key can be a `var` (identifier) or a `value` (string literal).
                let key = if key_node.kind == "var" {
                    key_node.payload.as_str().unwrap_or("").to_string()
                } else {
                    // Evaluate the key expression to get the field name.
                    let key_val = self.eval_node(ast, AstRef::new(key_idx), lang, env)?;
                    key_val.as_str().unwrap_or("").to_string()
                };
                entries.push((key, val_idx));
            }
        }

        // Match each entry against the corresponding field.
        let mut matched_keys: Vec<String> = Vec::new();
        for (key, val_idx) in &entries {
            match scrut_obj.get(key) {
                Some(scrut_val) => {
                    matched_keys.push(key.clone());
                    if !self.try_pattern(ast, AstRef::new(*val_idx), scrut_val, lang, env)? {
                        return Ok(false);
                    }
                }
                None => return Ok(false), // Missing key — no match.
            }
        }

        // Bind remaining keys to the rest binder.
        if let Some(ridx) = rest_idx {
            if let Some(name) = self.rest_binder_name(ast, AstRef::new(ridx))? {
                let rest_obj: serde_json::Map<String, Value> = scrut_obj
                    .iter()
                    .filter(|(k, _)| !matched_keys.contains(k))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                env.define(&name, Value::Object(rest_obj));
            }
        }

        Ok(true)
    }
}

/// A number that can be either integer or float.
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

    // --- Scheme combinator tests ---

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
        // Create a "var" node with scheme Var("$payload"), payload = "x".
        // But we need "x" to be in scope. We'll use a Bind to put it there.
        let engine = TreeWalkEngine::new();
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
        // ADR-0041 Phase 2: unbound variables now return their name as a
        // string value (so identifiers like `eval` can flow into host-function
        // dispatch via `App`). This test verifies the new behavior.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("unknown")))
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
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
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
    fn prim_add() {
        let engine = TreeWalkEngine::new();
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
    fn prim_sub() {
        let engine = TreeWalkEngine::new();
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
    fn prim_mul() {
        let engine = TreeWalkEngine::new();
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
    fn prim_div() {
        let engine = TreeWalkEngine::new();
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
    fn prim_div_by_zero_errors() {
        let engine = TreeWalkEngine::new();
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
    fn prim_eq() {
        let engine = TreeWalkEngine::new();
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
    fn prim_lt_gt() {
        let engine = TreeWalkEngine::new();
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
    fn foreign_dispatches_to_registered_host() {
        let engine = TreeWalkEngine::new();
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
                Scheme::foreign("double", vec![Scheme::child(0)]),
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
                Scheme::foreign("double", vec![Scheme::child(0)]),
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
        // "function" node: creates a closure via Prim("closure").
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                Scheme::prim("closure", vec![]),
            ))
            .unwrap();
        // "var" node: looks up a variable.
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
            .unwrap();
        // "binary.add": adds two children.
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
            ))
            .unwrap();
        // "call": applies callee to args.
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
                Scheme::App {
                    callee: Box::new(Scheme::child(0)),
                    args: vec![],
                },
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
        // Ast:
        // 0: lit(3)
        // 1: var("x") — inner let body references x
        // 2: let {name: "x", children: [0, ?]} — inner let, but body is the outer let
        // Actually, let me build this more carefully.
        //
        // let x = 3 in (let y = 4 in (x + y))
        //
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
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::prim("closure", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                Scheme::App {
                    callee: Box::new(Scheme::child(0)),
                    args: vec![Scheme::child(1), Scheme::child(2)],
                },
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
        // Direct Scheme::Lit evaluation via a node whose eval scheme is Lit.
        let engine = TreeWalkEngine::new();
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
    fn var_payload_not_string_errors() {
        // Var("$payload") but payload is not a string.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new("e.var", "var", Scheme::var("$payload")))
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
                Scheme::Bind {
                    name: "$payload".into(),
                    value: Box::new(Scheme::Child(AstRef::new(0))),
                    body: Box::new(Scheme::Child(AstRef::new(1))),
                },
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
                Scheme::foreign("triple", vec![Scheme::child(0)]),
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
        // Callee is an object without __closure__ marker.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.call",
                "call",
                Scheme::App {
                    callee: Box::new(Scheme::child(0)),
                    args: vec![],
                },
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
        // "closure" prim but payload has no "params" field.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                Scheme::prim("closure", vec![]),
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
        // "closure" prim but payload has no "body" field.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.fn",
                "function",
                Scheme::prim("closure", vec![]),
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
                Scheme::prim("sub", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::prim("mul", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::prim("div", vec![Scheme::child(0), Scheme::child(1)]),
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
    fn num_div_int_by_zero_returns_float_inf() {
        // Directly test Num::Div with int zero to cover that path.
        let result = Num::Int(1) / Num::Int(0);
        assert!(result.to_f64().is_infinite());
    }

    #[test]
    fn num_partial_eq() {
        // Cover PartialEq for Num (used by prim "eq").
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
    fn prim_add_non_number_errors() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim("add", vec![Scheme::child(0), Scheme::child(1)]),
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
                Scheme::Branch {
                    cond: Box::new(Scheme::child(0)),
                    then_: Box::new(Scheme::child(1)),
                    else_: Box::new(Scheme::child(2)),
                },
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
                Scheme::Branch {
                    cond: Box::new(Scheme::child(0)),
                    then_: Box::new(Scheme::child(1)),
                    else_: Box::new(Scheme::child(2)),
                },
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
                Scheme::App {
                    callee: Box::new(Scheme::child(0)),
                    args: vec![],
                },
            ))
            .unwrap();
        // Callee is a "closure" with params as a string (not array).
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!({
                "__closure__": true,
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
                Scheme::App {
                    callee: Box::new(Scheme::child(0)),
                    args: vec![],
                },
            ))
            .unwrap();
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!({
                "__closure__": true,
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
                Scheme::App {
                    callee: Box::new(Scheme::child(0)),
                    args: vec![],
                },
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

    // --- Vau (fexpr) tests (ADR-0041 §6, ADR-0045) ---

    #[test]
    fn vau_does_not_force_child() {
        // A vau node binds its child (unevaluated) but the body returns a
        // constant. The child is a node that would error if evaluated
        // (division by zero), proving the child is never forced.
        let engine = TreeWalkEngine::new();
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

    #[test]
    fn vau_eval_escape_evaluates_child_on_demand() {
        // A vau node binds its child (unevaluated) and the body calls
        // `eval(self, x)` to evaluate it — the ArrowApply escape. Requires
        // a host `eval` fn and a `self` global. We register a custom eval
        // host fn that re-enters the tree-walk engine.
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
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
        // The vau body: eval(self, x) where x is the unevaluated child.
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.vau",
                "vau",
                Scheme::vau(
                    vec!["x".into()],
                    Scheme::App {
                        callee: Box::new(Scheme::var("eval")),
                        args: vec![Scheme::var("self"), Scheme::var("x")],
                    },
                ),
            ))
            .unwrap();

        // The child to receive unevaluated: 1 + 2 → 3
        // 0: lit(1), 1: lit(2), 2: binary.add {children:[0,1]}
        // 3: vau {children:[2]}
        let ast = make_ast(vec![
            AstNode::new("lit").with_payload(json!(1)),
            AstNode::new("lit").with_payload(json!(2)),
            AstNode::new("binary.add").with_payload(json!({"children": [0, 1]})),
            AstNode::new("vau").with_payload(json!({"children": [2]})),
        ]);

        // A host eval fn that re-enters the engine. It needs the language
        // and host; we capture them via a registry that re-enters with a
        // fresh engine (no nested host needed for this self-contained
        // child).
        struct EvalHost {
            lang: Language,
        }
        impl Function for EvalHost {
            fn call(
                &self,
                args: &[Value],
                _ctx: &mut EvalContext,
            ) -> Result<Value, handl_platform::EvalError> {
                // eval(self, x): args[0] is the language value (unused —
                // we use the captured lang), args[1] is the __ast__ value.
                let ast_val = args.get(1).ok_or_else(|| {
                    handl_platform::EvalError::Other("eval: expected (self, ast)".into())
                })?;
                let ast = handl_language::as_ast(ast_val).ok_or_else(|| {
                    handl_platform::EvalError::Other("eval: expected an __ast__ value".into())
                })?;
                let engine = TreeWalkEngine::new();
                let req = EvalRequest::new(ast);
                engine
                    .eval(&self.lang, &req)
                    .map_err(|e| handl_platform::EvalError::Other(e.to_string()))
            }
        }
        struct EvalModule {
            eval: EvalHost,
        }
        impl Module for EvalModule {
            fn name(&self) -> &str {
                "eval"
            }
            fn function(&self, name: &str) -> Option<&dyn Function> {
                if name == "eval" {
                    Some(&self.eval)
                } else {
                    None
                }
            }
        }
        let eval_mod = EvalModule {
            eval: EvalHost { lang: lang.clone() },
        };
        let registry: Rc<dyn ModuleLookup> = Rc::new(ModuleRegistry::new().with(eval_mod));
        let engine = engine
            .with_modules(registry)
            .with_global("self", handl_language::language_value(&lang));
        let req = EvalRequest::new(ast);
        assert_eq!(engine.eval(&lang, &req).unwrap(), json!(3));
    }

    // -----------------------------------------------------------------
    // Pratt-parsed literal tests (children-based payloads)
    // -----------------------------------------------------------------

    /// Build a minimal language for Pratt-parsed literal eval tests:
    /// `value` (lit), `var`, `binary.add`, `array`, `object`, `entry`,
    /// `function` (closure prim), `call` (App).
    fn literal_eval_lang() -> Language {
        let mut lang = Language::new();
        // `value` nodes have no scheme — the eval fallback returns
        // `node.payload.clone()`, which is the literal value.
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
                "e.array",
                "array",
                Scheme::prim("array", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.object",
                "object",
                Scheme::prim("object", vec![]),
            ))
            .unwrap();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.function",
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
    fn pratt_function_paren_params_eval() {
        // (x, y) -> x + y, called with (3, 4) → 7
        // AST:
        // 0: var("x"), 1: var("y"), 2: paren{children:[0,1]}
        // 3: var("x"), 4: var("y"), 5: binary.add{children:[3,4]}
        // 6: function{children:[2, 5]}
        // 7: value(3), 8: value(4)
        // 9: call{children:[6, 7]}  (only 1 arg slot in App — multi-arg call
        //    isn't supported by the simple App scheme; test single-param path
        //    separately below)
        // Since App only supports 1 arg, test single-param function instead.
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
        // The closure prim unwraps the paren node to get [x, y] params.
        // But App only passes 1 arg. So we test the closure creation itself
        // (not the call) — verify the closure has 2 params.
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
        assert_eq!(result["__closure__"], json!(true));
        assert_eq!(result["params"], json!(["x", "y"]));
    }

    // -----------------------------------------------------------------
    // match / handle / destructuring conformance tests
    //
    // These are the reference conformance tests for the `match` and
    // `handle` prims (including array/object/`...rest` destructuring
    // patterns). The bytecode and cranelift engines replicate this
    // behavior exactly; the same AST shapes are tested in all three.
    // -----------------------------------------------------------------

    /// Build a minimal language for match/handle eval tests:
    /// `value` (no scheme → payload fallback), `var`, `binary.add`,
    /// `match`, `handle`.
    fn match_lang() -> Language {
        let mut lang = Language::new();
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
}
