//! Core refinement types: predicates, refinement types, contracts, and results.
//!
//! These are ported from the research prototype's `refinement_inference_tests.rs`,
//! `termination_analysis_tests.rs`, and `real_programs_tests.rs`, adapted to
//! be self-contained (no dependency on the research crate).

use std::fmt;

// ============================================================================
// Base types
// ============================================================================

/// Base types for refinement inference.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BaseTy {
    /// Integer type.
    Int,
    /// Boolean type.
    Bool,
    /// A type variable (e.g. `a`, `b`).
    Var(String),
}

impl fmt::Display for BaseTy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaseTy::Int => write!(f, "Int"),
            BaseTy::Bool => write!(f, "Bool"),
            BaseTy::Var(v) => write!(f, "{}", v),
        }
    }
}

// ============================================================================
// Predicates
// ============================================================================

/// A symbolic predicate over values. Built from graph structure.
///
/// Carries variable names so the implication checker can reason structurally.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pred {
    /// Always true.
    True,
    /// Always false.
    False,
    /// Sorted(var) -- the list `var` is sorted.
    Sorted(String),
    /// |a| = |b| -- length equality.
    LengthEq(String, String),
    /// |a| <= |b| -- length inequality.
    LengthLe(String, String),
    /// forall x in list, pred(x) -- all elements satisfy a predicate.
    AllSatisfy(String, String),
    /// a <= b -- numeric less-than-or-equal.
    Le(String, String),
    /// elem is the head (first element) of list.
    HeadOf(String, String),
    /// |a| = |b| + 1 -- output is one longer than input.
    LengthPlusOne(String, String),
    /// |a| = |b| - 1 -- output is one shorter than input.
    LengthSubOne(String, String),
    /// IsPerm(a, b) -- list a is a permutation of list b (same elements, possibly different order).
    IsPerm(String, String),
    /// IsSuccess(var) -- the value is a successful Result (inl).
    IsSuccess(String),
    /// IsError(var) -- the value is a failed Result (inr).
    IsError(String),
    /// Conjunction.
    And(Box<Pred>, Box<Pred>),
    /// Disjunction.
    Or(Box<Pred>, Box<Pred>),
    /// Implication.
    Implies(Box<Pred>, Box<Pred>),
    /// forall var in list, pred_name(var).
    Forall(String, String, String),
}

impl Pred {
    /// Pretty-print a predicate for human-readable output.
    pub fn pretty(&self) -> String {
        match self {
            Pred::True => "True".to_string(),
            Pred::False => "False".to_string(),
            Pred::Sorted(x) => format!("Sorted({})", x),
            Pred::LengthEq(a, b) => format!("|{}| = |{}|", a, b),
            Pred::LengthLe(a, b) => format!("|{}| <= |{}|", a, b),
            Pred::AllSatisfy(list, pred) => format!("AllSatisfy({}, {})", list, pred),
            Pred::Le(a, b) => format!("{} <= {}", a, b),
            Pred::HeadOf(elem, list) => format!("HeadOf({}, {})", elem, list),
            Pred::LengthPlusOne(a, b) => format!("|{}| = |{}| + 1", a, b),
            Pred::LengthSubOne(a, b) => format!("|{}| = |{}| - 1", a, b),
            Pred::IsPerm(a, b) => format!("Perm({}, {})", a, b),
            Pred::IsSuccess(x) => format!("IsSuccess({})", x),
            Pred::IsError(x) => format!("IsError({})", x),
            Pred::And(a, b) => format!("({} && {})", a.pretty(), b.pretty()),
            Pred::Or(a, b) => format!("({} || {})", a.pretty(), b.pretty()),
            Pred::Implies(a, b) => format!("({} ==> {})", a.pretty(), b.pretty()),
            Pred::Forall(var, list, pred) => format!("forall{}. {}({})", var, pred, list),
        }
    }
}

impl fmt::Display for Pred {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.pretty())
    }
}

// ============================================================================
// Runtime predicate evaluation (ADR-0077 Phase 3)
// ============================================================================

/// Look up a named variable in a runtime JSON value.
///
/// For a JSON object, `name` is a key. For a JSON array, `name` is parsed as
/// an index (0, 1, …). Returns `None` if the name is not found or the value
/// is neither an object nor an array.
fn lookup_var<'a>(value: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    match value {
        serde_json::Value::Object(map) => map.get(name),
        serde_json::Value::Array(arr) => name.parse::<usize>().ok().and_then(|i| arr.get(i)),
        _ => None,
    }
}

/// Resolve a single-variable predicate's operand against a value.
///
/// If `lookup_var` finds the named field/element, use it; otherwise fall back
/// to the whole value (the variable name is a placeholder like `"x"` or
/// `"output"` that refers to the value itself).
fn resolve_single<'a>(value: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    lookup_var(value, name).unwrap_or(value)
}

/// Resolve a two-variable predicate's operands against a value.
///
/// Each name is looked up via `lookup_var`; if not found, the whole value is
/// used as the fallback (so e.g. `Le("output", "1")` against a bare number
/// resolves `output` to the number and `1` to the number too — but `1` is
/// typically a literal placeholder, so callers should treat literal-ish names
/// specially when needed).
fn resolve_pair<'a>(
    value: &'a serde_json::Value,
    a: &str,
    b: &str,
) -> (&'a serde_json::Value, &'a serde_json::Value) {
    let av = lookup_var(value, a).unwrap_or(value);
    let bv = lookup_var(value, b).unwrap_or(value);
    (av, bv)
}

/// Try to read a JSON value as an `f64` number.
fn as_f64(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
}

/// Try to read a JSON value as an array.
fn as_array(v: &serde_json::Value) -> Option<&[serde_json::Value]> {
    v.as_array().map(|v| &**v)
}

/// Check whether a JSON array of numbers is sorted in non-decreasing order.
fn is_sorted_array(arr: &[serde_json::Value]) -> bool {
    let mut prev: Option<f64> = None;
    for elem in arr {
        let n = match as_f64(elem) {
            Some(n) => n,
            None => return false,
        };
        if prev.is_some_and(|p| n < p) {
            return false;
        }
        prev = Some(n);
    }
    true
}

/// Check whether two JSON arrays are permutations of each other (same
/// multiset of elements, any order). Uses string serialization for element
/// comparison, which is sufficient for JSON values.
fn is_permutation(a: &[serde_json::Value], b: &[serde_json::Value]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut a_keys: Vec<String> = a.iter().map(|v| v.to_string()).collect();
    let mut b_keys: Vec<String> = b.iter().map(|v| v.to_string()).collect();
    a_keys.sort();
    b_keys.sort();
    a_keys == b_keys
}

impl Pred {
    /// Evaluate this predicate against a runtime JSON value.
    ///
    /// This is the runtime enforcement path (ADR-0077 Phase 3): the proof
    /// system infers contracts, and this method evaluates the contract's
    /// `precondition`/`guarantee` predicates against actual call arguments
    /// and return values at eval time.
    ///
    /// The `value` is the runtime argument (for `requires`) or return value
    /// (for `guarantee`). Variable names in the predicate (`Pred::Sorted(x)`,
    /// `Pred::Le(a, b)`, etc.) refer to fields/elements of this value:
    /// - For a single named variable like `Pred::Sorted(x)`, `x` is the value
    ///   itself (the predicate applies to the whole value).
    /// - For two-variable predicates like `Pred::Le(a, b)`, `a` and `b` are
    ///   looked up as keys in a JSON object, or as elements 0/1 of a JSON
    ///   array.
    /// - `Pred::True` always holds; `Pred::False` never holds.
    /// - Logical combinators (`And`, `Or`, `Implies`) evaluate recursively.
    /// - Predicates that cannot be evaluated against the given value (e.g.
    ///   `Sorted` on a non-array, or a named variable not found) return
    ///   `false` (fail-closed: if we can't confirm the predicate holds, we
    ///   reject).
    pub fn eval(&self, value: &serde_json::Value) -> bool {
        match self {
            Pred::True => true,
            Pred::False => false,

            Pred::Sorted(x) => {
                let v = resolve_single(value, x);
                match as_array(v) {
                    Some(arr) => is_sorted_array(arr),
                    None => false,
                }
            }

            Pred::LengthEq(a, b) => {
                let (av, bv) = resolve_pair(value, a, b);
                match (as_array(av), as_array(bv)) {
                    (Some(a), Some(b)) => a.len() == b.len(),
                    _ => false,
                }
            }

            Pred::LengthLe(a, b) => {
                let (av, bv) = resolve_pair(value, a, b);
                match (as_array(av), as_array(bv)) {
                    (Some(a), Some(b)) => a.len() <= b.len(),
                    _ => false,
                }
            }

            Pred::AllSatisfy(list, pred) => {
                let v = resolve_single(value, list);
                let arr = match as_array(v) {
                    Some(arr) => arr,
                    None => return false,
                };
                // `pred` is a string naming a predicate to apply to each
                // element. Handle the common cases.
                match pred.as_str() {
                    "Sorted" => arr.iter().all(|e| match as_array(e) {
                        Some(sub) => is_sorted_array(sub),
                        None => false,
                    }),
                    "True" => true,
                    "False" => false,
                    // Unknown predicate name → fail-closed.
                    _ => false,
                }
            }

            Pred::Le(a, b) => {
                let (av, bv) = resolve_pair(value, a, b);
                match (as_f64(av), as_f64(bv)) {
                    (Some(x), Some(y)) => x <= y,
                    _ => false,
                }
            }

            Pred::HeadOf(elem, list) => {
                let (ev, lv) = resolve_pair(value, elem, list);
                match as_array(lv) {
                    Some(arr) if !arr.is_empty() => arr.first() == Some(ev),
                    _ => false,
                }
            }

            Pred::LengthPlusOne(a, b) => {
                let (av, bv) = resolve_pair(value, a, b);
                match (as_array(av), as_array(bv)) {
                    (Some(a), Some(b)) => a.len() == b.len() + 1,
                    _ => false,
                }
            }

            Pred::LengthSubOne(a, b) => {
                let (av, bv) = resolve_pair(value, a, b);
                match (as_array(av), as_array(bv)) {
                    (Some(a), Some(b)) => a.len() + 1 == b.len(),
                    _ => false,
                }
            }

            Pred::IsPerm(a, b) => {
                let (av, bv) = resolve_pair(value, a, b);
                match (as_array(av), as_array(bv)) {
                    (Some(a), Some(b)) => is_permutation(a, b),
                    _ => false,
                }
            }

            Pred::IsSuccess(x) => {
                let v = resolve_single(value, x);
                match v {
                    serde_json::Value::Object(map) => {
                        // Convention: __choice__ == "left" (inl/success) OR
                        // has an "ok" field (Result-like).
                        map.get("__choice__").and_then(|c| c.as_str()) == Some("left")
                            || map.contains_key("ok")
                    }
                    _ => false,
                }
            }

            Pred::IsError(x) => {
                let v = resolve_single(value, x);
                match v {
                    serde_json::Value::Object(map) => {
                        // Convention: __choice__ == "right" (inr/error) OR
                        // has an "err"/"error" field (Result-like).
                        map.get("__choice__").and_then(|c| c.as_str()) == Some("right")
                            || map.contains_key("err")
                            || map.contains_key("error")
                    }
                    _ => false,
                }
            }

            Pred::And(a, b) => a.eval(value) && b.eval(value),
            Pred::Or(a, b) => a.eval(value) || b.eval(value),
            Pred::Implies(a, b) => !a.eval(value) || b.eval(value),

            Pred::Forall(_var, list, pred) => {
                let v = resolve_single(value, list);
                let arr = match as_array(v) {
                    Some(arr) => arr,
                    None => return false,
                };
                // `pred` is a string naming a predicate to apply to each
                // element (same handling as AllSatisfy).
                match pred.as_str() {
                    "Sorted" => arr.iter().all(|e| match as_array(e) {
                        Some(sub) => is_sorted_array(sub),
                        None => false,
                    }),
                    "True" => true,
                    "False" => false,
                    _ => false,
                }
            }
        }
    }
}

impl Pred {
    /// Simplify a predicate by applying basic logical equivalences.
    ///
    /// This is essential for fixpoint convergence detection when Z3 is not
    /// available: without simplification, `And(True, True)` is structurally
    /// distinct from `True`, so the toy implication checker cannot determine
    /// that `True ==> And(True, True)`, and the fixpoint never converges.
    ///
    /// Simplifications applied:
    /// - `And(True, x)` -> `x`, `And(x, True)` -> `x`
    /// - `And(False, _)` -> `False`, `And(_, False)` -> `False`
    /// - `Or(False, x)` -> `x`, `Or(x, False)` -> `x`
    /// - `Or(True, _)` -> `True`, `Or(_, True)` -> `True`
    /// - `Implies(True, x)` -> `x`
    /// - `Implies(x, True)` -> `True`
    /// - `Implies(False, _)` -> `True`
    pub fn simplify(&self) -> Pred {
        match self {
            Pred::True | Pred::False => self.clone(),
            Pred::Sorted(_)
            | Pred::LengthEq(_, _)
            | Pred::LengthLe(_, _)
            | Pred::AllSatisfy(_, _)
            | Pred::Le(_, _)
            | Pred::HeadOf(_, _)
            | Pred::LengthPlusOne(_, _)
            | Pred::LengthSubOne(_, _)
            | Pred::IsPerm(_, _)
            | Pred::IsSuccess(_)
            | Pred::IsError(_)
            | Pred::Forall(_, _, _) => self.clone(),

            Pred::And(a, b) => {
                let a = a.simplify();
                let b = b.simplify();
                match (&a, &b) {
                    (Pred::True, _) => b,
                    (_, Pred::True) => a,
                    (Pred::False, _) | (_, Pred::False) => Pred::False,
                    // Dedup: And(x, x) -> x (idempotence of conjunction).
                    // This is essential for fixpoint convergence when the
                    // toy_implies checker is used: without dedup, And(sc, sc)
                    // is structurally distinct from sc, and the toy checker
                    // cannot prove sc ==> And(sc, sc) because it decomposes
                    // the left side first (And-decomposition) before the
                    // right side, and no single conjunct of sc implies the
                    // full And(sc, sc).
                    _ if a == b => a,
                    _ => Pred::And(Box::new(a), Box::new(b)),
                }
            }

            Pred::Or(a, b) => {
                let a = a.simplify();
                let b = b.simplify();
                match (&a, &b) {
                    (Pred::False, _) => b,
                    (_, Pred::False) => a,
                    (Pred::True, _) | (_, Pred::True) => Pred::True,
                    _ => Pred::Or(Box::new(a), Box::new(b)),
                }
            }

            Pred::Implies(a, b) => {
                let a = a.simplify();
                let b = b.simplify();
                match (&a, &b) {
                    (Pred::True, _) => b,
                    (_, Pred::True) => Pred::True,
                    (Pred::False, _) => Pred::True,
                    _ => Pred::Implies(Box::new(a), Box::new(b)),
                }
            }
        }
    }
}

// ============================================================================
// Refinement types
// ============================================================================

/// A refinement type: `{ x : T | P(x) }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RefTy {
    /// Base type with no refinement.
    Plain(BaseTy),
    /// Refinement: `{ x : T | pred(x) }`.
    Refine(BaseTy, Pred),
    /// Product with refinements on components.
    Prod(Box<RefTy>, Box<RefTy>),
    /// Sum with refinements.
    Sum(Box<RefTy>, Box<RefTy>),
    /// List with element type and optional refinements.
    List(Box<RefTy>),
}

impl RefTy {
    /// Pretty-print a refinement type.
    pub fn pretty(&self) -> String {
        match self {
            RefTy::Plain(b) => b.to_string(),
            RefTy::Refine(b, p) => {
                format!("{{ x : {} | {} }}", b, p.pretty())
            }
            RefTy::Prod(a, b) => format!("({} * {})", a.pretty(), b.pretty()),
            RefTy::Sum(a, b) => format!("({} + {})", a.pretty(), b.pretty()),
            RefTy::List(e) => format!("List {}", e.pretty()),
        }
    }
}

// ============================================================================
// Contracts
// ============================================================================

/// A contract for a base arrow: its input/output refinement types,
/// precondition, guarantee predicate, and termination effect on the feedback
/// channel.
///
/// `precondition` is the predicate `P(x)` that must hold on the input for the
/// `guarantee` to apply. For unconditional contracts (the common case) it is
/// `Pred::True`. Conditional contracts carry a non-trivial precondition: the
/// guarantee only holds when the input satisfies `P(x)`.
///
/// `termination_effect` describes what the arrow does to C when used as the
/// right-hand side of a loop body composition `comp(_, arr)`:
///   - `"decrements_length"` : removes one element from a list C (|C| decreases)
///   - `"decrements_counter"`: decrements an integer counter C by 1
///   - `"constant"`          : leaves C unchanged (side-channel)
///   - `"none"`              : no known structural effect on C
#[derive(Debug, Clone)]
pub struct Contract {
    /// The input refinement type.
    pub input: RefTy,
    /// The output refinement type.
    pub output: RefTy,
    /// The precondition `P(x)` that must hold on the input for the guarantee
    /// to apply. `Pred::True` means the guarantee is unconditional.
    pub precondition: Pred,
    /// The guarantee predicate (what the arrow ensures about its output),
    /// valid whenever `precondition` holds on the input.
    pub guarantee: Pred,
    /// The termination effect on the feedback channel C.
    pub termination_effect: Option<String>,
}

impl Contract {
    /// Construct an *unconditional* contract (precondition `True`).
    ///
    /// This is the common case and is backward compatible with the original
    /// 4-field `Contract` construction.
    pub fn new(
        input: RefTy,
        output: RefTy,
        guarantee: Pred,
        termination_effect: Option<String>,
    ) -> Self {
        Self {
            input,
            output,
            precondition: Pred::True,
            guarantee,
            termination_effect,
        }
    }

    /// Construct a *conditional* contract: the `guarantee` only applies when
    /// `precondition` holds on the input.
    pub fn conditional(
        input: RefTy,
        output: RefTy,
        precondition: Pred,
        guarantee: Pred,
        termination_effect: Option<String>,
    ) -> Self {
        Self {
            input,
            output,
            precondition,
            guarantee,
            termination_effect,
        }
    }
}

// ============================================================================
// Termination measure
// ============================================================================

/// A termination measure on the feedback channel C.
///
/// A well-founded measure guarantees termination if it strictly decreases
/// each iteration and is bounded below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Measure {
    /// No measure discovered -- termination not guaranteed.
    None,
    /// Length of a list: |c| decreases each iteration, bounded below by 0.
    ListLength(String),
    /// Integer counter: c decreases by `decrement` each iteration, bounded below by 0.
    Counter(String, i64),
    /// Constant (side-channel) -- measure doesn't change, so no decrease.
    Constant,
    /// Nested -- termination depends on inner loop's measure.
    Nested(Box<Measure>),
}

impl Measure {
    /// Pretty-print a measure for human-readable output.
    pub fn pretty(&self) -> String {
        match self {
            Measure::None => "None".to_string(),
            Measure::ListLength(v) => {
                format!("ListLength({}) -- |{}| decreases by 1 each iteration", v, v)
            }
            Measure::Counter(v, d) => {
                format!(
                    "Counter({}, -{}) -- {} decreases by {} each iteration",
                    v, d, v, d
                )
            }
            Measure::Constant => "Constant (C unchanged)".to_string(),
            Measure::Nested(inner) => format!("Nested({})", inner.pretty()),
        }
    }
}

// ============================================================================
// Loop invariant
// ============================================================================

/// A discovered loop invariant for a `Loop` combinator.
#[derive(Debug, Clone)]
pub struct LoopInvariant {
    /// A description of the graph the invariant applies to.
    pub graph: String,
    /// The invariant predicate.
    pub invariant: Pred,
    /// How the invariant was discovered (e.g. "fixpoint iteration").
    pub how_discovered: String,
    /// Whether the invariant was verified by SMT.
    pub verified_by_smt: bool,
}

// ============================================================================
// Refinement result
// ============================================================================

/// The result of refinement inference on a `Term`.
#[derive(Debug, Clone)]
pub struct RefinementResult {
    /// The inferred input refinement type.
    pub input: RefTy,
    /// The inferred output refinement type.
    pub output: RefTy,
    /// The inferred guarantee predicate.
    pub guarantee: Pred,
    /// Loop invariants discovered for any `Loop` combinators.
    pub loop_invariants: Vec<LoopInvariant>,
    /// Termination measure, if any loop was analyzed.
    pub termination: Option<Measure>,
    /// Law annotations inferred from the graph's wiring structure
    /// (annotation-free trait inference, mirroring Lean4
    /// `TraitInference.lean`). Each annotation records an equational law
    /// the graph satisfies (e.g. functor identity) and how it strengthens
    /// the inferred contract.
    pub law_annotations: Vec<crate::trait_inference::LawAnnotation>,
}
