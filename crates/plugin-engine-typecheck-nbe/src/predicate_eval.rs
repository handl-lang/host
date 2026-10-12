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
use handl_plugin_engine_exec::EvalRequest;
use handl_plugin_engine_typecheck::TypecheckDiagnostic;
use std::collections::HashMap;
use std::sync::OnceLock;

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
    /// Integer inequality `a != b`.
    Neq(Box<SymExpr>, Box<SymExpr>),
    /// Integer less-than `a < b`.
    Lt(Box<SymExpr>, Box<SymExpr>),
    /// Integer greater-than `a > b`.
    Gt(Box<SymExpr>, Box<SymExpr>),
    /// Integer less-than-or-equal `a <= b`.
    Lte(Box<SymExpr>, Box<SymExpr>),
    /// Integer greater-than-or-equal `a >= b`.
    Gte(Box<SymExpr>, Box<SymExpr>),
    /// Boolean AND `a && b`.
    And(Box<SymExpr>, Box<SymExpr>),
    /// Boolean OR `a || b`.
    Or(Box<SymExpr>, Box<SymExpr>),
    /// Conditional `if c then t else e`.
    If(Box<SymExpr>, Box<SymExpr>, Box<SymExpr>),
    /// Function application `callee(args...)`.
    Call(Box<SymExpr>, Vec<SymExpr>),
    /// An expression the normalizer cannot handle.
    Stuck(String),
}

impl SymExpr {
    fn int(v: i64) -> Self {
        SymExpr::Int(v)
    }
    fn boolean(v: bool) -> Self {
        SymExpr::Bool(v)
    }
    fn var(s: impl Into<String>) -> Self {
        SymExpr::Var(s.into())
    }
    fn add(a: SymExpr, b: SymExpr) -> Self {
        SymExpr::Add(Box::new(a), Box::new(b))
    }
    fn sub(a: SymExpr, b: SymExpr) -> Self {
        SymExpr::Sub(Box::new(a), Box::new(b))
    }
    fn mul(a: SymExpr, b: SymExpr) -> Self {
        SymExpr::Mul(Box::new(a), Box::new(b))
    }
    fn mod_(a: SymExpr, b: SymExpr) -> Self {
        SymExpr::Mod(Box::new(a), Box::new(b))
    }
    fn eq(a: SymExpr, b: SymExpr) -> Self {
        SymExpr::Eq(Box::new(a), Box::new(b))
    }
    fn neq(a: SymExpr, b: SymExpr) -> Self {
        SymExpr::Neq(Box::new(a), Box::new(b))
    }
    fn lt(a: SymExpr, b: SymExpr) -> Self {
        SymExpr::Lt(Box::new(a), Box::new(b))
    }
    fn gt(a: SymExpr, b: SymExpr) -> Self {
        SymExpr::Gt(Box::new(a), Box::new(b))
    }
    fn lte(a: SymExpr, b: SymExpr) -> Self {
        SymExpr::Lte(Box::new(a), Box::new(b))
    }
    fn gte(a: SymExpr, b: SymExpr) -> Self {
        SymExpr::Gte(Box::new(a), Box::new(b))
    }
    fn and(a: SymExpr, b: SymExpr) -> Self {
        SymExpr::And(Box::new(a), Box::new(b))
    }
    fn or(a: SymExpr, b: SymExpr) -> Self {
        SymExpr::Or(Box::new(a), Box::new(b))
    }
    fn if_(c: SymExpr, t: SymExpr, e: SymExpr) -> Self {
        SymExpr::If(Box::new(c), Box::new(t), Box::new(e))
    }
    fn call(callee: SymExpr, args: Vec<SymExpr>) -> Self {
        SymExpr::Call(Box::new(callee), args)
    }
    fn stuck(msg: impl Into<String>) -> Self {
        SymExpr::Stuck(msg.into())
    }
}

/// If either operand is [`SymExpr::Stuck`], propagate the stuck reason.
/// This ensures the reason message survives through binary operations
/// instead of being lost in an unreduced wrapper (Phase 3.4).
fn propagate_stuck(a: &SymExpr, b: &SymExpr) -> Option<SymExpr> {
    match (a, b) {
        (SymExpr::Stuck(msg), _) => Some(SymExpr::stuck(msg.clone())),
        (_, SymExpr::Stuck(msg)) => Some(SymExpr::stuck(msg.clone())),
        _ => None,
    }
}

/// Convert a concrete [`Value`] (from a closure's captured env) into a
/// [`SymExpr`]. Only integers and booleans are supported; anything else
/// returns `None` (the caller skips the binding).
fn value_to_sym(v: &Value) -> Option<SymExpr> {
    if let Some(i) = v.as_i64() {
        Some(SymExpr::int(i))
    } else if let Some(b) = v.as_bool() {
        Some(SymExpr::boolean(b))
    } else {
        None
    }
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
                if let Some(stuck) = propagate_stuck(a, b) {
                    return Some(stuck);
                }
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
                if let Some(stuck) = propagate_stuck(a, b) {
                    return Some(stuck);
                }
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
                if let Some(stuck) = propagate_stuck(a, b) {
                    return Some(stuck);
                }
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
                if let Some(stuck) = propagate_stuck(a, b) {
                    return Some(stuck);
                }
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
                if let Some(stuck) = propagate_stuck(a, b) {
                    return Some(stuck);
                }
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::Eq(na, nb))
                } else {
                    None
                }
            }
        },

        // Inequality on concrete ints/bools.
        SymExpr::Neq(a, b) => match (&**a, &**b) {
            (SymExpr::Int(x), SymExpr::Int(y)) => Some(SymExpr::boolean(x != y)),
            (SymExpr::Bool(x), SymExpr::Bool(y)) => Some(SymExpr::boolean(x != y)),
            (SymExpr::Var(x), SymExpr::Var(y)) if x == y => Some(SymExpr::boolean(false)),
            _ => {
                if let Some(stuck) = propagate_stuck(a, b) {
                    return Some(stuck);
                }
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::Neq(na, nb))
                } else {
                    None
                }
            }
        },

        // Comparisons on concrete ints.
        SymExpr::Lt(a, b) => match (&**a, &**b) {
            (SymExpr::Int(x), SymExpr::Int(y)) => Some(SymExpr::boolean(x < y)),
            _ => {
                if let Some(stuck) = propagate_stuck(a, b) {
                    return Some(stuck);
                }
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::Lt(na, nb))
                } else {
                    None
                }
            }
        },
        SymExpr::Gt(a, b) => match (&**a, &**b) {
            (SymExpr::Int(x), SymExpr::Int(y)) => Some(SymExpr::boolean(x > y)),
            _ => {
                if let Some(stuck) = propagate_stuck(a, b) {
                    return Some(stuck);
                }
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::Gt(na, nb))
                } else {
                    None
                }
            }
        },
        SymExpr::Lte(a, b) => match (&**a, &**b) {
            (SymExpr::Int(x), SymExpr::Int(y)) => Some(SymExpr::boolean(x <= y)),
            _ => {
                if let Some(stuck) = propagate_stuck(a, b) {
                    return Some(stuck);
                }
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::Lte(na, nb))
                } else {
                    None
                }
            }
        },
        SymExpr::Gte(a, b) => match (&**a, &**b) {
            (SymExpr::Int(x), SymExpr::Int(y)) => Some(SymExpr::boolean(x >= y)),
            _ => {
                if let Some(stuck) = propagate_stuck(a, b) {
                    return Some(stuck);
                }
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::Gte(na, nb))
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
                if let Some(stuck) = propagate_stuck(a, b) {
                    return Some(stuck);
                }
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
                if let Some(stuck) = propagate_stuck(a, b) {
                    return Some(stuck);
                }
                let na = normalize_step(a).map(Box::new).unwrap_or_else(|| a.clone());
                let nb = normalize_step(b).map(Box::new).unwrap_or_else(|| b.clone());
                if &na != a || &nb != b {
                    Some(SymExpr::Or(na, nb))
                } else {
                    None
                }
            }
        },

        // Conditional: if true then t else e → t; if false then t else e → e.
        SymExpr::If(c, t, e) => match &**c {
            SymExpr::Bool(true) => Some((**t).clone()),
            SymExpr::Bool(false) => Some((**e).clone()),
            SymExpr::Stuck(msg) => Some(SymExpr::stuck(msg.clone())),
            _ => {
                let nc = normalize_step(c).map(Box::new).unwrap_or_else(|| c.clone());
                let nt = normalize_step(t).map(Box::new).unwrap_or_else(|| t.clone());
                let ne = normalize_step(e).map(Box::new).unwrap_or_else(|| e.clone());
                if &nc != c || &nt != t || &ne != e {
                    Some(SymExpr::If(nc, nt, ne))
                } else {
                    None
                }
            }
        },

        // Call: only normalizes if the callee or an arg is Stuck (propagate
        // the reason). Otherwise stuck — the symbolic normalizer cannot
        // beta-reduce without a closure body in SymExpr form.
        SymExpr::Call(callee, args) => {
            if let SymExpr::Stuck(msg) = &**callee {
                return Some(SymExpr::stuck(msg.clone()));
            }
            for arg in args {
                if let SymExpr::Stuck(msg) = arg {
                    return Some(SymExpr::stuck(msg.clone()));
                }
            }
            let nc = normalize_step(callee)
                .map(Box::new)
                .unwrap_or_else(|| callee.clone());
            let nargs: Vec<SymExpr> = args
                .iter()
                .map(|a| normalize_step(a).unwrap_or_else(|| a.clone()))
                .collect();
            if &nc != callee || nargs.iter().zip(args.iter()).any(|(n, o)| n != o) {
                Some(SymExpr::Call(nc, nargs))
            } else {
                Some(SymExpr::stuck("call: cannot beta-reduce symbolically"))
            }
        }
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

/// Type alias for the symbolic environment (variable name → `SymExpr`).
type SymEnv = HashMap<String, SymExpr>;

/// Type of a symbolic translation handler registered in [`sym_registry`].
///
/// Each handler takes the full AST, the node to translate, and the symbolic
/// environment. It returns `Some(SymExpr)` if it can handle the node kind,
/// or `None` if not (the caller falls through to the `Stuck` case).
type SymFn = fn(&Ast, &AstNode, &SymEnv) -> Option<SymExpr>;

// --- Symbolic translation handlers --------------------------------------
//
// Each handler corresponds to one arm of the former `match` in
// `ast_to_sym`. They are registered in [`sym_registry`] at init time,
// making the dispatch table-driven rather than a hardcoded `match`.

fn sym_lit(_ast: &Ast, node: &AstNode, _env: &SymEnv) -> Option<SymExpr> {
    // Integer or boolean literal.
    if let Some(i) = node.payload.as_i64() {
        Some(SymExpr::int(i))
    } else if let Some(b) = node.payload.as_bool() {
        Some(SymExpr::boolean(b))
    } else {
        Some(SymExpr::stuck("non-integer/boolean literal"))
    }
}

fn sym_var(_ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    let name = node.payload.as_str().unwrap_or("");
    if let Some(v) = env.get(name) {
        Some(v.clone())
    } else {
        Some(SymExpr::var(name))
    }
}

fn sym_binary_add(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    let (a, b) = binary_children(node);
    Some(SymExpr::add(
        ast_to_sym(ast, AstRef::new(a), env),
        ast_to_sym(ast, AstRef::new(b), env),
    ))
}

fn sym_binary_sub(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    let (a, b) = binary_children(node);
    Some(SymExpr::sub(
        ast_to_sym(ast, AstRef::new(a), env),
        ast_to_sym(ast, AstRef::new(b), env),
    ))
}

fn sym_binary_mul(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    let (a, b) = binary_children(node);
    Some(SymExpr::mul(
        ast_to_sym(ast, AstRef::new(a), env),
        ast_to_sym(ast, AstRef::new(b), env),
    ))
}

fn sym_binary_mod(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    let (a, b) = binary_children(node);
    Some(SymExpr::mod_(
        ast_to_sym(ast, AstRef::new(a), env),
        ast_to_sym(ast, AstRef::new(b), env),
    ))
}

fn sym_binary_eq(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    let (a, b) = binary_children(node);
    Some(SymExpr::eq(
        ast_to_sym(ast, AstRef::new(a), env),
        ast_to_sym(ast, AstRef::new(b), env),
    ))
}

fn sym_binary_neq(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    let (a, b) = binary_children(node);
    Some(SymExpr::neq(
        ast_to_sym(ast, AstRef::new(a), env),
        ast_to_sym(ast, AstRef::new(b), env),
    ))
}

fn sym_binary_lt(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    let (a, b) = binary_children(node);
    Some(SymExpr::lt(
        ast_to_sym(ast, AstRef::new(a), env),
        ast_to_sym(ast, AstRef::new(b), env),
    ))
}

fn sym_binary_gt(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    let (a, b) = binary_children(node);
    Some(SymExpr::gt(
        ast_to_sym(ast, AstRef::new(a), env),
        ast_to_sym(ast, AstRef::new(b), env),
    ))
}

fn sym_binary_lte(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    let (a, b) = binary_children(node);
    Some(SymExpr::lte(
        ast_to_sym(ast, AstRef::new(a), env),
        ast_to_sym(ast, AstRef::new(b), env),
    ))
}

fn sym_binary_gte(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    let (a, b) = binary_children(node);
    Some(SymExpr::gte(
        ast_to_sym(ast, AstRef::new(a), env),
        ast_to_sym(ast, AstRef::new(b), env),
    ))
}

fn sym_binary_and(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    let (a, b) = binary_children(node);
    Some(SymExpr::and(
        ast_to_sym(ast, AstRef::new(a), env),
        ast_to_sym(ast, AstRef::new(b), env),
    ))
}

fn sym_binary_or(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    let (a, b) = binary_children(node);
    Some(SymExpr::or(
        ast_to_sym(ast, AstRef::new(a), env),
        ast_to_sym(ast, AstRef::new(b), env),
    ))
}

fn sym_if(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    // if{children: [cond, then, else]}
    let (cond, then_idx, else_idx) = if_children(node);
    Some(SymExpr::if_(
        ast_to_sym(ast, AstRef::new(cond), env),
        ast_to_sym(ast, AstRef::new(then_idx), env),
        ast_to_sym(ast, AstRef::new(else_idx), env),
    ))
}

fn sym_call(ast: &Ast, node: &AstNode, env: &SymEnv) -> Option<SymExpr> {
    // call{children: [callee, arg1, arg2, ...]}
    let (callee_idx, arg_idxs) = call_children(node);
    let callee_sym = ast_to_sym(ast, AstRef::new(callee_idx), env);
    let arg_syms: Vec<SymExpr> = arg_idxs
        .iter()
        .map(|&i| ast_to_sym(ast, AstRef::new(i), env))
        .collect();
    Some(SymExpr::call(callee_sym, arg_syms))
}

/// Build (once) and return the static sym registry — a table mapping node
/// kind strings to symbolic translation handler functions.
///
/// This is a data-driven dispatch: the registry is a `HashMap` table, not
/// a hardcoded `match` statement. New node kinds can be added by inserting
/// a new entry here (or, in the future, by loading entries from language
/// data). The handlers themselves still know about the kinds they
/// translate — this is inherent to a symbolic translator — but the
/// *dispatch* is table-driven and extensible.
fn sym_registry() -> &'static HashMap<&'static str, SymFn> {
    static REGISTRY: OnceLock<HashMap<&'static str, SymFn>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert("lit", sym_lit as SymFn);
        m.insert("var", sym_var as SymFn);
        m.insert("binary.add", sym_binary_add as SymFn);
        m.insert("binary.sub", sym_binary_sub as SymFn);
        m.insert("binary.mul", sym_binary_mul as SymFn);
        m.insert("binary.mod", sym_binary_mod as SymFn);
        m.insert("binary.eq", sym_binary_eq as SymFn);
        m.insert("binary.neq", sym_binary_neq as SymFn);
        m.insert("binary.lt", sym_binary_lt as SymFn);
        m.insert("binary.gt", sym_binary_gt as SymFn);
        m.insert("binary.lte", sym_binary_lte as SymFn);
        m.insert("binary.gte", sym_binary_gte as SymFn);
        m.insert("binary.and", sym_binary_and as SymFn);
        m.insert("binary.or", sym_binary_or as SymFn);
        m.insert("if", sym_if as SymFn);
        m.insert("call", sym_call as SymFn);
        m
    })
}

/// Translate a Handl AST node into a symbolic expression, given a
/// mapping from variable names to their symbolic values (the predicate
/// parameter is bound to the expression's symbolic representation).
///
/// Dispatch is table-driven via [`sym_registry`]: the node's kind is
/// looked up in the registry and, if a handler is registered, it is
/// invoked. Unregistered kinds produce [`SymExpr::Stuck`].
fn ast_to_sym(ast: &Ast, node_ref: AstRef, env: &SymEnv) -> SymExpr {
    let node = match ast.get(node_ref) {
        Some(n) => n,
        None => return SymExpr::stuck("node out of bounds"),
    };
    match sym_registry().get(node.kind.as_str()) {
        Some(handler) => handler(ast, node, env).unwrap_or_else(|| {
            SymExpr::stuck(format!("handler returned None for kind: {}", node.kind))
        }),
        None => SymExpr::stuck(format!("unsupported node kind: {}", node.kind)),
    }
}

/// Get the two child indices of a binary node.
fn binary_children(node: &AstNode) -> (usize, usize) {
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

/// Get the three child indices of an `if` node: (cond, then, else).
fn if_children(node: &AstNode) -> (usize, usize, usize) {
    let empty = Vec::new();
    let children = node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .unwrap_or(&empty);
    let c = children.first().and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let t = children.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let e = children.get(2).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    (c, t, e)
}

/// Get the callee index and argument indices of a `call` node.
fn call_children(node: &AstNode) -> (usize, Vec<usize>) {
    let empty = Vec::new();
    let children = node
        .payload
        .get("children")
        .and_then(|c| c.as_array())
        .unwrap_or(&empty);
    let callee = children.first().and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let args: Vec<usize> = children
        .iter()
        .skip(1)
        .filter_map(|v| v.as_u64().map(|i| i as usize))
        .collect();
    (callee, args)
}

/// Extract parameter names and body index from a `function` node, handling
/// both the rich `params`/`body` payload and the legacy `children` array
/// format (Phase 3.2).
///
/// - Rich format: `{params: ["name", ...], body: idx}` — names are inline.
/// - Legacy format: `{children: [param_idx, body_idx]}` — `param_idx` points
///   to a `var` node (or a `paren`/`array` wrapper of `var` nodes) whose
///   payload is the param name.
///
/// Returns `(param_names, body_idx)`.
fn function_param_body(ast: &Ast, node: &AstNode, lang: &Language) -> Option<(Vec<String>, usize)> {
    // Rich format: {params: ["name", ...], body: idx}
    if let (Some(params), Some(body)) = (
        node.payload.get("params").and_then(|v| v.as_array()),
        node.payload.get("body").and_then(|v| v.as_u64()),
    ) {
        let names: Vec<String> = params
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        return Some((names, body as usize));
    }
    // Legacy format: {children: [param_idx, body_idx]} — only `function`
    // nodes (a core calculus primitive) carry this layout. A `call` node
    // also has two children, but is not a function.
    if node.kind != "function" {
        return None;
    }
    let children = node.payload.get("children").and_then(|c| c.as_array())?;
    if children.len() < 2 {
        return None;
    }
    let param_idx = children[0].as_u64()? as usize;
    let body_idx = children[1].as_u64()? as usize;
    let param_node = ast.get(AstRef::new(param_idx))?;
    // A bare `var` (or `type-annot` wrapping one) carries the name directly;
    // a `paren`/`array` wrapper holds multiple `var` children.
    let names = if let Ok(name) = lang.param_name(ast, param_node) {
        vec![name]
    } else {
        let param_children = param_node
            .payload
            .get("children")
            .and_then(|c| c.as_array())?;
        param_children
            .iter()
            .filter_map(|pc| {
                let pidx = pc.as_u64()? as usize;
                ast.get(AstRef::new(pidx))
                    .and_then(|pn| pn.payload.as_str())
                    .map(String::from)
            })
            .collect()
    };
    Some((names, body_idx))
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
            Ok(other) => {
                Proof::blocked(format!("predicate did not reduce to bool, got: {}", other))
            }
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
    /// `predicate_ref` points to the predicate. If it is a `function` node,
    /// the param name and body index are read directly from the node's
    /// payload (supporting both the `children` array and `params`/`body`
    /// rich formats — Phase 3.2). If it is any other kind (e.g. `call` for
    /// a named/parametric predicate like `DivisibleBy(3)`), the type
    /// expression is *evaluated* via the bound eval engine to obtain the
    /// closure value; the closure's `body` index and captured `env` are then
    /// used for symbolic normalization (Phase 3.1).
    /// `expr_ref` points to the annotated expression node.
    pub(crate) fn check_predicate_symbolic(
        &self,
        lang: &Language,
        ast: &Ast,
        predicate_ref: AstRef,
        expr_ref: AstRef,
        budget: u32,
    ) -> Proof {
        let pred_node = match ast.get(predicate_ref) {
            Some(n) => n,
            None => return Proof::blocked("predicate node out of bounds"),
        };

        // Resolve the predicate body, param name, and any captured env.
        //
        // Case 1: the node has a function-like payload (params/body or a
        //         children array that `function_param_body` can parse) — read
        //         param/body directly from the AST. This is data-driven: we
        //         detect the function shape by payload structure, not by
        //         checking `kind == "function"` (ADR-0081 engine genericity).
        // Case 2: any other shape (e.g. `call`) — evaluate the type expression
        //         to get the closure value, then use its body index (valid in
        //         the subtree) and captured env.
        let (body_ast, body_idx, param_name, captured_env): (Ast, usize, String, Option<Value>) =
            if let Some((param_names, body_idx)) = function_param_body(ast, pred_node, lang) {
                // Phase 3.2: handle both children-array and params/body shapes.
                let param_name = param_names
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| "_".to_string());
                (ast.clone(), body_idx, param_name, None)
            } else {
                // Phase 3.1: evaluate the type expression to get the closure.
                if self.pure {
                    if let Err(_) = super::check_pure(lang, ast, predicate_ref) {
                        return Proof::blocked(
                            "impure type expression in symbolic predicate check",
                        );
                    }
                }
                let sub = match ast.subtree(predicate_ref) {
                    Ok(s) => s,
                    Err(e) => return Proof::blocked(format!("subtree error: {}", e)),
                };
                let req = EvalRequest::new(sub.clone());
                match self.eval_engine.eval(lang, &req) {
                    Ok(type_val) if Self::is_closure_value(&type_val) => {
                        let params: Vec<String> = type_val
                            .get("params")
                            .and_then(|p| p.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let body_idx =
                            type_val.get("body").and_then(|b| b.as_u64()).unwrap_or(0) as usize;
                        let param_name =
                            params.into_iter().next().unwrap_or_else(|| "_".to_string());
                        let env = type_val.get("env").cloned();
                        (sub, body_idx, param_name, env)
                    }
                    Ok(other) => {
                        return Proof::blocked(format!(
                            "type expression did not evaluate to closure, got: {}",
                            other
                        ));
                    }
                    Err(e) => return Proof::blocked(format!("type expression eval error: {}", e)),
                }
            };

        // Build the symbolic representation of the annotated expression.
        // This is computed from the *original* AST (the expression lives
        // there, not in the subtree).
        let empty_env = HashMap::new();
        let expr_sym = ast_to_sym(ast, expr_ref, &empty_env);

        // Bind the parameter to the expression's symbolic form.
        let mut env = HashMap::new();
        env.insert(param_name.clone(), expr_sym);

        // Add captured env bindings from the closure (if any). These are
        // concrete values (e.g. `k = 3` for `DivisibleBy(3)`).
        if let Some(captured) = captured_env {
            if let Some(obj) = captured.as_object() {
                for (k, v) in obj {
                    if let Some(sym) = value_to_sym(v) {
                        env.insert(k.clone(), sym);
                    }
                }
            }
        }

        // Translate the predicate body to a symbolic expression. The body
        // index is valid in `body_ast` (the original AST for `function` nodes,
        // or the subtree for evaluated closures).
        let body_sym = ast_to_sym(&body_ast, AstRef::new(body_idx), &env);

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

    /// Check purity of a function by inspecting its effect row (Phase 7.2).
    ///
    /// `expr_type` is the synthesized type of the annotated expression. If it
    /// is a `type.function` with an `effects` field, the effect row is
    /// inspected:
    /// - Empty ops array → [`Proof::ok`] (pure, no effects).
    /// - Non-empty ops array → [`Proof::no`] (impure, has effects).
    /// - No `effects` field → [`Proof::ok`] (no effects declared = pure by
    ///   default).
    ///
    /// If the type is not a `type.function`, the check is blocked (we can't
    /// determine purity of a non-function).
    pub(crate) fn check_purity(&self, expr_type: &Value) -> Proof {
        // Check if it's a type.function.
        let is_function = self.registry_id_check(expr_type);
        if !is_function {
            return Proof::blocked(format!(
                "purity: expression type is not a function, cannot check purity"
            ));
        }

        // Read the effects field from the function type.
        let effects = expr_type.get("effects");
        match effects {
            Some(eff) => {
                // Check if the effects value is a type.effects with ops.
                let ops = eff.get("ops").and_then(|o| o.as_array());
                match ops {
                    Some(ops_arr) if ops_arr.is_empty() => Proof::ok("function has no effects"),
                    Some(ops_arr) => {
                        let effect_names: Vec<String> = ops_arr
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                        Proof::no(format!("function has effects: {}", effect_names.join(", ")))
                    }
                    None => {
                        // effects field present but no ops array — treat as
                        // empty (pure).
                        Proof::ok("function has no effects (effects field without ops)")
                    }
                }
            }
            None => {
                // No effects field — pure by default.
                Proof::ok("function has no effects (no effects field)")
            }
        }
    }

    /// Helper: check if a value is a `type.function` from this engine's
    /// registry. Uses the registry id string comparison since we don't have
    /// direct access to the TypeRegistry here (it's in the context).
    fn registry_id_check(&self, v: &Value) -> bool {
        v.get("__tag__").and_then(|t| t.as_str()) == Some("type.function")
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

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn sym_registry_contains_expected_entries() {
        // The sym registry must be table-driven and contain the node kinds
        // the symbolic normalizer supports. This test guards against
        // accidental removal of entries during refactoring.
        let reg = sym_registry();
        assert!(reg.contains_key("lit"));
        assert!(reg.contains_key("var"));
        assert!(reg.contains_key("binary.add"));
        assert!(reg.contains_key("binary.sub"));
        assert!(reg.contains_key("binary.mul"));
        assert!(reg.contains_key("binary.mod"));
        assert!(reg.contains_key("binary.eq"));
        assert!(reg.contains_key("binary.neq"));
        assert!(reg.contains_key("binary.lt"));
        assert!(reg.contains_key("binary.gt"));
        assert!(reg.contains_key("binary.lte"));
        assert!(reg.contains_key("binary.gte"));
        assert!(reg.contains_key("binary.and"));
        assert!(reg.contains_key("binary.or"));
        assert!(reg.contains_key("if"));
        assert!(reg.contains_key("call"));
        // An unregistered kind should not be present.
        assert!(!reg.contains_key("nonexistent.kind"));
    }
}
