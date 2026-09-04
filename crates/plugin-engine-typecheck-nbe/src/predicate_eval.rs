//! Predicate-as-type checking via the bound eval engine (ADR-0068 step B1).
//!
//! When a type annotation is a predicate (a function value that returns
//! `type.bool`), the typechecker normalizes `predicate(expr)` via the bound
//! [`EvalEngine`]. The result determines the annotation outcome (ADR-0068 §1,
//! "graduated guarantees"):
//!
//! - `Value::Bool(true)` → [`Proof::ok`] — proven statically, no runtime check.
//! - `Value::Bool(false)` → [`Proof::no`] — disproved, the annotation is
//!   rejected.
//! - Any other value or an eval error → [`Proof::blocked`] — normalization
//!   could not finish; the caller accepts the annotation with a runtime
//!   backstop `assert T(e)`.
//!
//! This is the concrete-annotation case (B1): the expression value is fully
//! known, so the predicate is *run* rather than reasoned about symbolically.
//! Symbolic normalization for dependent annotations (B2) is a later step.
//!
//! # Closure body-index preservation
//!
//! A closure value carries a `body` index into the AST it was created from.
//! To evaluate `predicate(expr)` we build a synthetic AST by **cloning the
//! entire original AST** and appending a `call` node whose children are
//! `[type_expr_idx, expr_idx]`. Cloning preserves all original indices, so
//! when the `call` eval scheme evaluates the type-expression child (creating
//! the closure) and then applies it, the closure's body index is valid in the
//! cloned AST. Building a fresh 3-node AST would break this: the body index
//! would point outside the new arena.

use handl_language::{Ast, AstNode, AstRef, LangError, Language, Proof, Value, Verdict};
use handl_plugin_engine_eval::EvalRequest;
use handl_plugin_engine_typecheck::TypecheckDiagnostic;

use super::TypecheckContext;

// ---------------------------------------------------------------------------
// B2: Symbolic normalization for predicates with free variables
// ---------------------------------------------------------------------------

/// A symbolic expression built during partial evaluation of a predicate
/// body. This is a mini-IR that covers the common operations in predicate
/// bodies (arithmetic, equality, boolean logic). Anything outside this
/// subset normalizes to [`SymExpr::Stuck`], which maps to [`Proof::blocked`].
#[derive(Debug, Clone, PartialEq)]
enum SymExpr {
    /// A concrete integer.
    Int(i64),
    /// A concrete boolean.
    Bool(bool),
    /// A free symbolic variable (e.g. the predicate parameter).
    Var(String),
    /// Integer addition `a + b`.
    Add(Box<SymExpr>, Box<SymExpr>),
    /// Integer subtraction `a - b`.
    Sub(Box<SymExpr>, Box<SymExpr>),
    /// Integer multiplication `a * b`.
    Mul(Box<SymExpr>, Box<SymExpr>),
    /// Integer modulo `a % b`.
    Mod(Box<SymExpr>, Box<SymExpr>),
    /// Integer equality `a == b`.
    Eq(Box<SymExpr>, Box<SymExpr>),
    /// Boolean AND `a && b`.
    And(Box<SymExpr>, Box<SymExpr>),
    /// Boolean OR `a || b`.
    Or(Box<SymExpr>, Box<SymExpr>),
    /// An expression the normalizer cannot handle.
    Stuck(String),
}

impl SymExpr {
    fn int(v: i64) -> Self { SymExpr::Int(v) }
    fn boolean(v: bool) -> Self { SymExpr::Bool(v) }
    fn var(s: impl Into<String>) -> Self { SymExpr::Var(s.into()) }
    fn add(a: SymExpr, b: SymExpr) -> Self { SymExpr::Add(Box::new(a), Box::new(b)) }
    fn sub(a: SymExpr, b: SymExpr) -> Self { SymExpr::Sub(Box::new(a), Box::new(b)) }
    fn mul(a: SymExpr, b: SymExpr) -> Self { SymExpr::Mul(Box::new(a), Box::new(b)) }
    fn mod_(a: SymExpr, b: SymExpr) -> Self { SymExpr::Mod(Box::new(a), Box::new(b)) }
    fn eq(a: SymExpr, b: SymExpr) -> Self { SymExpr::Eq(Box::new(a), Box::new(b)) }
    fn and(a: SymExpr, b: SymExpr) -> Self { SymExpr::And(Box::new(a), Box::new(b)) }
    fn or(a: SymExpr, b: SymExpr) -> Self { SymExpr::Or(Box::new(a), Box::new(b)) }
    fn stuck(msg: impl Into<String>) -> Self { SymExpr::Stuck(msg.into()) }
}

/// Normalize a symbolic expression one step. Returns `None` if the
/// expression is already in normal form (or stuck).
fn normalize_step(e: &SymExpr) -> Option<SymExpr> {
    match e {
        // Already normal.
        SymExpr::Int(_) | SymExpr::Bool(_) | SymExpr::Var(_) | SymExpr::Stuck(_) => None,

        // Arithmetic on concrete ints.
        SymExpr::Add(a, b) => match (&**a, &**b) {
            (SymExpr::Int(x), SymExpr::Int(y)) => Some(SymExpr::int(x + y)),
            // Identity: x + 0 → x
            (SymExpr::Int(0), _) => Some((**b).clone()),
            // Identity: 0 + x → x
            (_, SymExpr::Int(0)) => Some((**a).clone()),
            _ => {
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::Add(na, nb))
                } else {
                    None
                }
            }
        },
        SymExpr::Sub(a, b) => match (&**a, &**b) {
            (SymExpr::Int(x), SymExpr::Int(y)) => Some(SymExpr::int(x - y)),
            // Identity: x - 0 → x
            (_, SymExpr::Int(0)) => Some((**a).clone()),
            _ => {
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::Sub(na, nb))
                } else {
                    None
                }
            }
        },
        SymExpr::Mul(a, b) => match (&**a, &**b) {
            (SymExpr::Int(x), SymExpr::Int(y)) => Some(SymExpr::int(x * y)),
            // Identity: x * 1 → x
            (SymExpr::Int(1), _) => Some((**b).clone()),
            // Identity: 1 * x → x
            (_, SymExpr::Int(1)) => Some((**a).clone()),
            // Zero: x * 0 → 0, 0 * x → 0
            (SymExpr::Int(0), _) | (_, SymExpr::Int(0)) => Some(SymExpr::int(0)),
            _ => {
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::Mul(na, nb))
                } else {
                    None
                }
            }
        },
        SymExpr::Mod(a, b) => match (&**a, &**b) {
            (SymExpr::Int(x), SymExpr::Int(y)) => {
                if *y == 0 {
                    Some(SymExpr::stuck("modulo by zero"))
                } else {
                    Some(SymExpr::int(x % y))
                }
            }
            _ => {
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::Mod(na, nb))
                } else {
                    None
                }
            }
        },

        // Equality on concrete ints.
        SymExpr::Eq(a, b) => match (&**a, &**b) {
            (SymExpr::Int(x), SymExpr::Int(y)) => Some(SymExpr::boolean(x == y)),
            (SymExpr::Bool(x), SymExpr::Bool(y)) => Some(SymExpr::boolean(x == y)),
            // If both sides are syntactically equal variables, it's true.
            (SymExpr::Var(x), SymExpr::Var(y)) if x == y => Some(SymExpr::boolean(true)),
            _ => {
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::Eq(na, nb))
                } else {
                    None
                }
            }
        },

        // Boolean logic on concrete bools.
        SymExpr::And(a, b) => match (&**a, &**b) {
            (SymExpr::Bool(x), SymExpr::Bool(y)) => Some(SymExpr::boolean(*x && *y)),
            // Short-circuit: true && b → b
            (SymExpr::Bool(true), _) => Some((**b).clone()),
            // Short-circuit: false && b → false
            (SymExpr::Bool(false), _) => Some(SymExpr::boolean(false)),
            _ => {
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::And(na, nb))
                } else {
                    None
                }
            }
        },
        SymExpr::Or(a, b) => match (&**a, &**b) {
            (SymExpr::Bool(x), SymExpr::Bool(y)) => Some(SymExpr::boolean(*x || *y)),
            // Short-circuit: false || b → b
            (SymExpr::Bool(false), _) => Some((**b).clone()),
            // Short-circuit: true || b → true
            (SymExpr::Bool(true), _) => Some(SymExpr::boolean(true)),
            _ => {
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::Or(na, nb))
                } else {
                    None
                }
            }
        },
    }
}

/// Normalize a symbolic expression to a fixed point (up to a step budget).
fn normalize_sym(e: &SymExpr, budget: u32) -> SymExpr {
    let mut current = e.clone();
    for _ in 0..budget {
        match normalize_step(&current) {
            Some(next) => current = next,
            None => break,
        }
    }
    current
}

/// Translate a Handl AST node into a symbolic expression, given a
/// mapping from variable names to their symbolic values (the predicate
/// parameter is bound to the expression's symbolic representation).
fn ast_to_sym(
    ast: &Ast,
    node_ref: AstRef,
    env: &std::collections::HashMap<String, SymExpr>,
) -> SymExpr {
    let node = match ast.get(node_ref) {
        Some(n) => n,
        None => return SymExpr::stuck("node out of bounds"),
    };
    match node.kind.as_str() {
        "lit" => {
            // Integer or boolean literal.
            if let Some(i) = node.payload.as_i64() {
                SymExpr::int(i)
            } else if let Some(b) = node.payload.as_bool() {
                SymExpr::boolean(b)
            } else {
                SymExpr::stuck("non-integer/boolean literal")
            }
        }
        "var" => {
            let name = node.payload.as_str().unwrap_or("");
            if let Some(v) = env.get(name) {
                v.clone()
            } else {
                SymExpr::var(name)
            }
        }
        "binary.add" => {
            let (a, b) = binary_children(ast, node);
            SymExpr::add(ast_to_sym(ast, AstRef::new(a), env), ast_to_sym(ast, AstRef::new(b), env))
        }
        "binary.sub" => {
            let (a, b) = binary_children(ast, node);
            SymExpr::sub(ast_to_sym(ast, AstRef::new(a), env), ast_to_sym(ast, AstRef::new(b), env))
        }
        "binary.mul" => {
            let (a, b) = binary_children(ast, node);
            SymExpr::mul(ast_to_sym(ast, AstRef::new(a), env), ast_to_sym(ast, AstRef::new(b), env))
        }
        "binary.mod" => {
            let (a, b) = binary_children(ast, node);
            SymExpr::mod_(ast_to_sym(ast, AstRef::new(a), env), ast_to_sym(ast, AstRef::new(b), env))
        }
        "binary.eq" => {
            let (a, b) = binary_children(ast, node);
            SymExpr::eq(ast_to_sym(ast, AstRef::new(a), env), ast_to_sym(ast, AstRef::new(b), env))
        }
        "binary.and" => {
            let (a, b) = binary_children(ast, node);
            SymExpr::and(ast_to_sym(ast, AstRef::new(a), env), ast_to_sym(ast, AstRef::new(b), env))
        }
        "binary.or" => {
            let (a, b) = binary_children(ast, node);
            SymExpr::or(ast_to_sym(ast, AstRef::new(a), env), ast_to_sym(ast, AstRef::new(b), env))
        }
        _ => SymExpr::stuck(format!("unsupported node kind: {}", node.kind)),
    }
}

/// Get the two child indices of a binary node.
fn binary_children(ast: &Ast, node: &AstNode) -> (usize, usize) {
    let empty = Vec::new();
    let children = node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .unwrap_or(&empty);
    let a = children.first().and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let b = children.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    (a, b)
}

impl super::NbeTypecheckEngine {
    /// Check a predicate annotation by evaluating `predicate(expr)` via the
    /// bound eval engine.
    ///
    /// `type_expr_ref` is the AST ref of the type expression (which evaluates
    /// to the predicate closure); `expr_ref` is the AST ref of the annotated
    /// expression (which evaluates to the argument). The synthetic AST is a
    /// clone of `ast` with a `call` node appended at the end; the eval engine
    /// evaluates the last node (the `call`), per Pratt convention.
    ///
    /// `budget` is reserved for a future custom step counter; for now the
    /// eval engine's built-in loop guards provide termination protection,
    /// and any error (including timeout) maps to [`Proof::blocked`].
    pub(crate) fn check_predicate_application(
        &self,
        lang: &Language,
        ast: &Ast,
        type_expr_ref: AstRef,
        expr_ref: AstRef,
        _budget: u32,
    ) -> Proof {
        // Clone the full AST so closure body indices remain valid, then
        // append a `call` node whose children are the type expression and
        // the expression. The eval engine evaluates the last node.
        let mut synth = ast.clone();
        let call_node = AstNode::new("call").with_payload(serde_json::json!({
            "children": [type_expr_ref.index(), expr_ref.index()]
        }));
        synth.push(call_node);

        let req = EvalRequest::new(synth);
        match self.eval_engine.eval(lang, &req) {
            Ok(Value::Bool(true)) => Proof::ok("predicate evaluated to true"),
            Ok(Value::Bool(false)) => Proof::no("predicate evaluated to false"),
            Ok(other) => Proof::blocked(format!(
                "predicate did not reduce to bool, got: {}",
                other
            )),
            Err(e) => Proof::blocked(format!("predicate evaluation error: {}", e)),
        }
    }

    /// Check a predicate annotation by symbolically normalizing the
    /// predicate body with the expression substituted for the parameter
    /// (ADR-0068 step B2).
    ///
    /// Unlike [`check_predicate_application`] (B1), which runs the predicate
    /// via the eval engine, this function works on the AST directly and can
    /// handle expressions with free variables. The predicate body is
    /// translated to a [`SymExpr`], the parameter is bound to a symbolic
    /// representation of the annotated expression, and the result is
    /// normalized to a fixed point.
    ///
    /// - If the normalized result is `SymExpr::Bool(true)` → [`Proof::ok`].
    /// - If the normalized result is `SymExpr::Bool(false)` → [`Proof::no`].
    /// - Otherwise → [`Proof::blocked`] (the normalizer could not reduce
    ///   the predicate to a concrete boolean).
    ///
    /// `predicate_ref` points to the `function` node (the predicate). The
    /// function reads the param name and body index from the node's children.
    /// `expr_ref` points to the annotated expression node.
    pub(crate) fn check_predicate_symbolic(
        &self,
        ast: &Ast,
        predicate_ref: AstRef,
        expr_ref: AstRef,
        budget: u32,
    ) -> Proof {
        let pred_node = match ast.get(predicate_ref) {
            Some(n) => n,
            None => return Proof::blocked("predicate node out of bounds"),
        };
        if pred_node.kind != "function" {
            return Proof::blocked(format!(
                "expected function node for predicate, got '{}'",
                pred_node.kind
            ));
        }
        let empty = Vec::new();
        let children = pred_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())
            .unwrap_or(&empty);
        if children.len() < 2 {
            return Proof::blocked("predicate function has < 2 children");
        }
        let param_idx = children[0].as_u64().unwrap_or(0) as usize;
        let body_idx = children[1].as_u64().unwrap_or(0) as usize;

        // Read the parameter name from the param node.
        let param_name = ast
            .get(AstRef::new(param_idx))
            .and_then(|n| n.payload.as_str())
            .unwrap_or("_")
            .to_string();

        // Build the symbolic representation of the annotated expression.
        let empty_env = std::collections::HashMap::new();
        let expr_sym = ast_to_sym(ast, expr_ref, &empty_env);

        // Bind the parameter to the expression's symbolic form.
        let mut env = std::collections::HashMap::new();
        env.insert(param_name.clone(), expr_sym);

        // Translate the predicate body to a symbolic expression.
        let body_sym = ast_to_sym(ast, AstRef::new(body_idx), &env);

        // Normalize to a fixed point.
        let result = normalize_sym(&body_sym, budget);

        match result {
            SymExpr::Bool(true) => Proof::ok("predicate symbolically normalized to true"),
            SymExpr::Bool(false) => Proof::no("predicate symbolically normalized to false"),
            SymExpr::Stuck(msg) => Proof::blocked(format!("symbolic normalization stuck: {}", msg)),
            other => Proof::blocked(format!(
                "predicate did not reduce to bool, got: {:?}",
                other
            )),
        }
    }

    /// Check whether a value is a closure (arrow-value).
    ///
    /// A closure is a JSON object with `"__arrow__": true`. This is the
    /// data-driven predicate detection: we inspect the value's shape, not the
    /// AST node kind (per the engine-genericity rule).
    pub(crate) fn is_closure_value(v: &Value) -> bool {
        v.get("__arrow__").and_then(|b| b.as_bool()) == Some(true)
    }

    /// Map a [`Proof`] to a typecheck result (ADR-0068 graduated guarantees).
    ///
    /// - [`Verdict::Proven`] → return `accept_type` (annotation accepted
    ///   statically, no runtime check).
    /// - [`Verdict::Disproved`] → `Err` (annotation rejected).
    /// - [`Verdict::Blocked`] → return `accept_type` and record a
    ///   `runtime_assert` diagnostic so the host/runtime knows a runtime
    ///   `assert T(e)` must be emitted at this annotation site (ADR-0068 §1
    ///   "graduated guarantees": runtime backstop where the checker cannot
    ///   normalize).
    pub(crate) fn resolve_predicate_proof(
        &self,
        proof: Proof,
        _expr_type: &Value,
        accept_type: &Value,
        ctx: &mut TypecheckContext,
    ) -> Result<Value, LangError> {
        match proof.verdict {
            Verdict::Proven => Ok(accept_type.clone()),
            Verdict::Disproved => Err(LangError::engine(format!(
                "predicate annotation disproved: {}",
                proof.reason
            ))),
            Verdict::Blocked => {
                // Runtime fallback: accept the annotation but record a
                // diagnostic so the host/runtime knows to emit
                // `assert T(e)` at this site.
                ctx.diagnostics.push(TypecheckDiagnostic {
                    severity: "runtime_assert".to_string(),
                    message: format!(
                        "predicate annotation could not be proven statically; \
                         runtime assert required: {}",
                        proof.reason
                    ),
                    span: None,
                });
                Ok(accept_type.clone())
            }
        }
    }
}
