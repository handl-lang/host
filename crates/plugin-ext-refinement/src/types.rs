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
    /// A type variable (e.g. `α`, `β`).
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
    /// Sorted(var) — the list `var` is sorted.
    Sorted(String),
    /// |a| = |b| — length equality.
    LengthEq(String, String),
    /// |a| <= |b| — length inequality.
    LengthLe(String, String),
    /// forall x in list, pred(x) — all elements satisfy a predicate.
    AllSatisfy(String, String),
    /// a <= b — numeric less-than-or-equal.
    Le(String, String),
    /// elem is the head (first element) of list.
    HeadOf(String, String),
    /// |a| = |b| + 1 — output is one longer than input.
    LengthPlusOne(String, String),
    /// |a| = |b| - 1 — output is one shorter than input.
    LengthSubOne(String, String),
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
            Pred::And(a, b) => format!("({} ∧ {})", a.pretty(), b.pretty()),
            Pred::Or(a, b) => format!("({} ∨ {})", a.pretty(), b.pretty()),
            Pred::Implies(a, b) => format!("({} ⟹ {})", a.pretty(), b.pretty()),
            Pred::Forall(var, list, pred) => format!("∀{}. {}({})", var, pred, list),
        }
    }
}

impl fmt::Display for Pred {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.pretty())
    }
}

impl Pred {
    /// Simplify a predicate by applying basic logical equivalences.
    ///
    /// This is essential for fixpoint convergence detection when Z3 is not
    /// available: without simplification, `And(True, True)` is structurally
    /// distinct from `True`, so the toy implication checker cannot determine
    /// that `True ⟹ And(True, True)`, and the fixpoint never converges.
    ///
    /// Simplifications applied:
    /// - `And(True, x)` → `x`, `And(x, True)` → `x`
    /// - `And(False, _)` → `False`, `And(_, False)` → `False`
    /// - `Or(False, x)` → `x`, `Or(x, False)` → `x`
    /// - `Or(True, _)` → `True`, `Or(_, True)` → `True`
    /// - `Implies(True, x)` → `x`
    /// - `Implies(x, True)` → `True`
    /// - `Implies(False, _)` → `True`
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
            | Pred::Forall(_, _, _) => self.clone(),

            Pred::And(a, b) => {
                let a = a.simplify();
                let b = b.simplify();
                match (&a, &b) {
                    (Pred::True, _) => b,
                    (_, Pred::True) => a,
                    (Pred::False, _) | (_, Pred::False) => Pred::False,
                    // Dedup: And(x, x) → x (idempotence of conjunction).
                    // This is essential for fixpoint convergence when the
                    // toy_implies checker is used: without dedup, And(sc, sc)
                    // is structurally distinct from sc, and the toy checker
                    // cannot prove sc ⟹ And(sc, sc) because it decomposes
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
            RefTy::Prod(a, b) => format!("({} × {})", a.pretty(), b.pretty()),
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
    pub fn new(input: RefTy, output: RefTy, guarantee: Pred, termination_effect: Option<String>) -> Self {
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
    /// No measure discovered — termination not guaranteed.
    None,
    /// Length of a list: |c| decreases each iteration, bounded below by 0.
    ListLength(String),
    /// Integer counter: c decreases by `decrement` each iteration, bounded below by 0.
    Counter(String, i64),
    /// Constant (side-channel) — measure doesn't change, so no decrease.
    Constant,
    /// Nested — termination depends on inner loop's measure.
    Nested(Box<Measure>),
}

impl Measure {
    /// Pretty-print a measure for human-readable output.
    pub fn pretty(&self) -> String {
        match self {
            Measure::None => "None".to_string(),
            Measure::ListLength(v) => {
                format!("ListLength({}) — |{}| decreases by 1 each iteration", v, v)
            }
            Measure::Counter(v, d) => {
                format!("Counter({}, -{}) — {} decreases by {} each iteration", v, d, v, d)
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

/// The result of refinement inference on a `Scheme`.
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
}
