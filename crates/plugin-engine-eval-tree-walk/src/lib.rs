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
//! The built-in primitive set: `add`, `sub`, `mul`, `div`, `eq`, `lt`, `gt`,
//! `not`, `and`, `or`, and `closure`. Unknown ops return
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

use handl_language::{Ast, AstNode, AstRef, LangError, Language, Scheme, Value};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest, HostFunctions};

/// The tree-walk eval engine.
///
/// Construct with [`TreeWalkEngine::new`] and optionally pass a host
/// functions registry via [`TreeWalkEngine::with_host`].
pub struct TreeWalkEngine<'h> {
    /// The host functions registry for `Foreign` dispatch (may be `None`).
    host: Option<&'h dyn HostFunctions>,
}

impl<'h> TreeWalkEngine<'h> {
    /// Create a new tree-walk engine with no host functions.
    pub fn new() -> Self {
        Self { host: None }
    }

    /// Attach a host functions registry (builder style).
    pub fn with_host(mut self, host: &'h dyn HostFunctions) -> Self {
        self.host = Some(host);
        self
    }
}

impl<'h> Default for TreeWalkEngine<'h> {
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
struct EvalCtx<'a, 'h> {
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
    /// The host functions registry.
    host: Option<&'h dyn HostFunctions>,
}

impl<'h> EvalEngine for TreeWalkEngine<'h> {
    fn eval(&self, lang: &Language, req: &EvalRequest) -> Result<Value, LangError> {
        let ast = &req.ast;
        if ast.is_empty() {
            return Ok(Value::Null);
        }
        let mut env = ScopeStack::new();
        // The root is the last node in the arena (Pratt convention).
        let root_ref = AstRef::new(ast.len() - 1);
        self.eval_node(ast, root_ref, lang, &mut env)
    }
}

impl<'h> TreeWalkEngine<'h> {
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
                    host: self.host,
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
                    let host = self.host.ok_or_else(|| {
                        LangError::Engine(format!(
                            "unregistered foreign: {}",
                            foreign_name
                        ))
                    })?;
                    let host_fn = host.lookup(foreign_name).ok_or_else(|| {
                        LangError::Engine(format!(
                            "unregistered foreign: {}",
                            foreign_name
                        ))
                    })?;
                    let mut arg_vals = Vec::new();
                    if let Some(children) = node.payload.get("children").and_then(|c| c.as_array()) {
                        for child_ref in children {
                            if let Some(idx) = child_ref.as_u64() {
                                let child_ref = AstRef::new(idx as usize);
                                arg_vals.push(self.eval_node(ast, child_ref, lang, env)?);
                            }
                        }
                    }
                    host_fn.call(&arg_vals)
                } else {
                    Ok(node.payload.clone())
                }
            }
        }
    }

    /// Interpret a [`Scheme`] combinator in the given context.
    fn eval_scheme(&self, scheme: &Scheme, ctx: &mut EvalCtx<'_, 'h>) -> Result<Value, LangError> {
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
                ctx.env
                    .lookup(&var_name)
                    .cloned()
                    .ok_or_else(|| LangError::UnknownVariable { name: var_name })
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
                let mut arg_vals = Vec::with_capacity(args.len());
                for arg in args {
                    arg_vals.push(self.eval_scheme(arg, ctx)?);
                }
                self.apply_closure(&closure_val, &arg_vals, ctx)
            }

            Scheme::Branch {
                cond,
                then_,
                else_,
            } => {
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
                let host = ctx.host.ok_or_else(|| {
                    LangError::Engine(format!("unregistered foreign: {}", name))
                })?;
                let host_fn = host.lookup(name).ok_or_else(|| {
                    LangError::Engine(format!("unregistered foreign: {}", name))
                })?;
                host_fn.call(&arg_vals)
            }
        }
    }

    /// Apply a closure value to argument values.
    fn apply_closure(
        &self,
        closure: &Value,
        args: &[Value],
        ctx: &mut EvalCtx<'_, 'h>,
    ) -> Result<Value, LangError> {
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
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
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
        ctx: &mut EvalCtx<'_, 'h>,
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
            "not" => Ok(Value::Bool(!is_truthy(&args[0]))),
            "and" => Ok(Value::Bool(is_truthy(&args[0]) && is_truthy(&args[1]))),
            "or" => Ok(Value::Bool(is_truthy(&args[0]) || is_truthy(&args[1]))),
            "closure" => {
                // Create a closure from the current node's payload.
                let payload = &ctx.current_node.payload;
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
                let env = ctx.env.capture();
                let closure = serde_json::json!({
                    "__closure__": true,
                    "params": params,
                    "body": body,
                    "env": env,
                });
                Ok(closure)
            }
            _ => Err(LangError::Engine(format!("unknown prim: {}", op))),
        }
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
    use handl_plugin_engine_eval::{EvalRequest, HostFn, HostFunctions};
    use serde_json::json;
    use std::collections::HashMap as StdHashMap;

    /// Build a simple AST for testing.
    fn make_ast(nodes: Vec<AstNode>) -> Ast {
        let mut ast = Ast::new();
        for n in nodes {
            ast.push(n);
        }
        ast
    }

    /// A simple host function registry for tests.
    struct MapHost {
        fns: StdHashMap<String, Box<dyn HostFn>>,
    }

    impl MapHost {
        fn new() -> Self {
            Self {
                fns: StdHashMap::new(),
            }
        }

        fn register(&mut self, name: impl Into<String>, f: Box<dyn HostFn>) {
            self.fns.insert(name.into(), f);
        }
    }

    impl HostFunctions for MapHost {
        fn lookup(&self, name: &str) -> Option<&dyn HostFn> {
            self.fns.get(name).map(|f| f.as_ref())
        }
    }

    /// A host function that doubles its numeric argument.
    struct DoubleFn;

    impl HostFn for DoubleFn {
        fn call(&self, args: &[Value]) -> Result<Value, LangError> {
            let n = args[0].as_i64().unwrap();
            Ok(Value::from(n * 2))
        }
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
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::var("$payload"),
            ))
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
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::var("unknown"),
            ))
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
    fn child_recurses_into_child_node() {
        let engine = TreeWalkEngine::new();
        let mut lang = Language::new();
        lang.eval
            .merge(EvalSchemeDef::new(
                "e.add",
                "binary.add",
                Scheme::prim(
                    "add",
                    vec![Scheme::child(0), Scheme::child(1)],
                ),
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
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::var("$payload"),
            ))
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
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::var("$payload"),
            ))
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

        let mut host = MapHost::new();
        host.register("double", Box::new(DoubleFn));
        let engine = engine.with_host(&host);
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
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::var("$payload"),
            ))
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
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::var("$payload"),
            ))
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
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::var("$payload"),
            ))
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
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::var("$payload"),
            ))
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

        let mut host = MapHost::new();
        // Register "double" but not "triple".
        host.register("double", Box::new(DoubleFn));
        let engine = engine.with_host(&host);
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
            .merge(EvalSchemeDef::new(
                "e.var",
                "var",
                Scheme::var("x"),
            ))
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
}
