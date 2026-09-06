//! Partial-evaluator typechecker foundation (ADR-0068, steps 1 & 4).
//!
//! ADR-0068 ("Predicates as Types") specifies that the typechecker is a
//! **partial evaluator with a termination guard**: to verify a type-position
//! proposition `lhs == rhs`, the checker normalizes both sides and compares
//! them. If normalization is blocked (a symbolic value it cannot reduce, an
//! effect, non-termination), the checker does **not** fail — it reports
//! [`Verdict::Blocked`], which the caller treats as "insert a runtime check".
//! That is the "graduated guarantees" model: proven statically where
//! normalizable, checked at runtime where not.
//!
//! Induction principles such as `array.rec` are library functions the checker
//! *recognizes*, not syntax. [`Arr::Rec`] models an `array.rec(base, step,
//! goal)` region; [`recognize_array_rec`] recovers its structure and
//! [`prove_array_rec`] runs structural **induction over the array** with two
//! checker directives:
//!
//! - [`Directive::ByEval`] — reduce both sides and compare their normal forms
//!   (no induction hypothesis). Used for the `base: by_eval` case.
//! - [`Directive::ByNorm`] — reduce both sides, applying the induction
//!   hypothesis as a rewrite, then compare. Used for the
//!   `step: (x, rest, ih) -> by_norm + ih` case.
//!
//! The canonical example from ADR-0068 §2 — proving the length lemma
//!
//! ```text
//! array.rec(base: by_eval, step: (x, rest, ih) -> by_norm + ih)
//!   : array.length(append(a, b)) == array.length(a) + array.length(b)
//! ```
//!
//! — is exposed directly as [`prove_length_append`]; the analogous
//! [`prove_length_map`] proves `|map(f, a)| == |a|`.

use std::collections::BTreeMap;
use std::fmt;

// ============================================================================
// Proof verdicts
// ============================================================================

// `Verdict` and `Proof` live in `handl_language::verdict` (ADR-0068) so the
// typecheck engine and the refinement crate share them. Re-export here for
// backward compatibility with existing callers of this module.
pub use handl_language::{Proof, Verdict};

// ============================================================================
// Checker directives
// ============================================================================

/// How the checker closes an induction case.
///
/// These are checker directives (ADR-0068 §2), not language constructs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Directive {
    /// Reduce both sides and compare their normal forms; no induction
    /// hypothesis is available or applied. This is the `base: by_eval` case.
    ByEval,
    /// Reduce both sides, apply the induction hypothesis as a rewrite, and
    /// compare. This is the `step: ... -> by_norm + ih` case.
    ByNorm,
}

impl fmt::Display for Directive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Directive::ByEval => write!(f, "by_eval"),
            Directive::ByNorm => write!(f, "by_norm + ih"),
        }
    }
}

// ============================================================================
// Array-algebra expressions
// ============================================================================

/// A small array-algebra expression used for type-position reasoning.
///
/// This is the symbolic surface the partial evaluator reduces. It is rich
/// enough to state and check the length lemmas from ADR-0068 §2
/// (`append`/`map`/`length`), while staying open to extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arr {
    /// A free symbolic variable (e.g. `a`, `b`, `f`, `n`).
    Var(String),
    /// The empty array `[]`.
    Empty,
    /// A cons cell `x :: rest`.
    Cons(Box<Arr>, Box<Arr>),
    /// Array concatenation (functional `array.append`).
    Append(Box<Arr>, Box<Arr>),
    /// Array map `map(f, a)` — `f` is a function symbol, `a` the array.
    Map(String, Box<Arr>),
    /// `array.length` of an array.
    Len(Box<Arr>),
    /// An integer literal.
    Num(i64),
    /// Addition (used in lengths: `n + m`).
    Add(Box<Arr>, Box<Arr>),
    /// Equality proposition `lhs == rhs` (the type-position goal).
    Eq(Box<Arr>, Box<Arr>),
    /// A boolean literal.
    Bool(bool),
    /// An unevaluated function application `f(x)` — used as the element of a
    /// mapped array. Length does not depend on element values, so this stays
    /// opaque during normalization.
    App(String, Box<Arr>),
    /// An `array.rec(base, step, goal)` induction region over `scrutinee`.
    Rec {
        base: Directive,
        step: Directive,
        scrutinee: String,
        goal: Box<Arr>,
    },
}

impl Arr {
    pub fn var(s: impl Into<String>) -> Self {
        Arr::Var(s.into())
    }

    pub fn empty() -> Self {
        Arr::Empty
    }

    pub fn cons(head: Arr, tail: Arr) -> Self {
        Arr::Cons(Box::new(head), Box::new(tail))
    }

    pub fn append(a: Arr, b: Arr) -> Self {
        Arr::Append(Box::new(a), Box::new(b))
    }

    pub fn map(f: impl Into<String>, a: Arr) -> Self {
        Arr::Map(f.into(), Box::new(a))
    }

    pub fn len(a: Arr) -> Self {
        Arr::Len(Box::new(a))
    }

    pub fn num(n: i64) -> Self {
        Arr::Num(n)
    }

    pub fn add(a: Arr, b: Arr) -> Self {
        Arr::Add(Box::new(a), Box::new(b))
    }

    pub fn eq(a: Arr, b: Arr) -> Self {
        Arr::Eq(Box::new(a), Box::new(b))
    }
}

impl fmt::Display for Arr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arr::Var(s) => write!(f, "{}", s),
            Arr::Empty => write!(f, "[]"),
            Arr::Cons(x, rest) => write!(f, "{} :: {}", x, rest),
            Arr::Append(a, b) => write!(f, "{} ++ {}", a, b),
            Arr::Map(fn_sym, a) => write!(f, "map({}, {})", fn_sym, a),
            Arr::Len(a) => write!(f, "|{}|", a),
            Arr::Num(n) => write!(f, "{}", n),
            Arr::Add(a, b) => write!(f, "({} + {})", a, b),
            Arr::Eq(a, b) => write!(f, "{} == {}", a, b),
            Arr::Bool(b) => write!(f, "{}", b),
            Arr::App(fn_sym, x) => write!(f, "{}({})", fn_sym, x),
            Arr::Rec {
                base,
                step,
                scrutinee,
                goal,
            } => write!(
                f,
                "array.rec(base: {}, step: ({}, rest, ih) -> {}, {})",
                base, scrutinee, step, goal
            ),
        }
    }
}
// ============================================================================
// Sum normal form
// ============================================================================

/// A canonical additive normal form for length reasoning.
///
/// A value is a multiset of "length atoms": a `Len(...)` of some reduced
/// array expression, plus a running integer constant. Multiset (rather than
/// ordered) equality lets the checker prove associativity/commutativity of
/// `+` for free, e.g. `1 + (|rest| + |b|) == (1 + |rest|) + |b|`.
///
/// Keys: `"#"` holds the summed integer constant; any other key is the
/// pretty-printed reduced argument of a `Len(...)` atom (e.g. `"rest ++ b"`).
type Sum = BTreeMap<String, i64>;

/// Merge `coef` copies of atom `key` into `sum`, dropping zero coefficients.
fn add_atom(mut sum: Sum, key: &str, coef: i64) -> Sum {
    let total = sum.get(key).copied().unwrap_or(0) + coef;
    if total == 0 {
        sum.remove(key);
    } else {
        sum.insert(key.to_string(), total);
    }
    sum
}

/// Merge two sums, dropping zero coefficients.
fn merge_sum(mut a: Sum, b: Sum) -> Sum {
    for (key, coef) in b {
        a = add_atom(a, &key, coef);
    }
    a
}

// ============================================================================
// The partial evaluator
// ============================================================================

/// The termination-guarded partial evaluator.
///
/// Carries a step budget that doubles as the ADR-0068 termination guard: if
/// normalization would run unbounded, the budget is exhausted and the result
/// is [`Verdict::Blocked`] rather than a loop or a crash.
pub struct Pev {
    budget: u32,
}

impl Pev {
    pub fn new(budget: u32) -> Self {
        Pev { budget }
    }

    /// Consume one step of the budget; `None` when exhausted.
    fn tick(&mut self) -> Option<()> {
        if self.budget == 0 {
            None
        } else {
            self.budget -= 1;
            Some(())
        }
    }

    /// Substitute every `Var(var)` inside `e` with `val`.
    ///
    /// Used by the induction machinery to specialize the scrutinee to `[]`
    /// (base) and to `x :: rest` (step), and to build the induction
    /// hypothesis by specializing to `rest`.
    pub fn subst(&self, e: &Arr, var: &str, val: &Arr) -> Arr {
        match e {
            Arr::Var(s) if s == var => val.clone(),
            Arr::Var(_) => e.clone(),
            Arr::Empty => Arr::Empty,
            Arr::Cons(x, rest) => Arr::cons(self.subst(x, var, val), self.subst(rest, var, val)),
            Arr::Append(a, b) => Arr::append(self.subst(a, var, val), self.subst(b, var, val)),
            Arr::Map(f, a) => Arr::map(f.clone(), self.subst(a, var, val)),
            Arr::Len(a) => Arr::len(self.subst(a, var, val)),
            Arr::Num(n) => Arr::Num(*n),
            Arr::Add(a, b) => Arr::add(self.subst(a, var, val), self.subst(b, var, val)),
            Arr::Eq(a, b) => Arr::eq(self.subst(a, var, val), self.subst(b, var, val)),
            Arr::Bool(b) => Arr::Bool(*b),
            Arr::App(f, x) => Arr::App(f.clone(), Box::new(self.subst(x, var, val))),
            Arr::Rec {
                base,
                step,
                scrutinee,
                goal,
            } => Arr::Rec {
                base: *base,
                step: *step,
                scrutinee: scrutinee.clone(),
                goal: Box::new(self.subst(goal, var, val)),
            },
        }
    }
    /// Partially evaluate an expression, applying the array-recursion rewrite
    /// rules (the definitions of `append`, `map`, and `length`) until no more
    /// progress can be made or the budget is exhausted.
    ///
    /// The result is still an `Arr`, but with every reducible head collapsed:
    /// `append([], b) -> b`, `append(x :: rest, b) -> x :: append(rest, b)`,
    /// `map(f, []) -> []`, `map(f, x :: rest) -> f(x) :: map(f, rest)`,
    /// `length([]) -> 0`, `length(x :: rest) -> 1 + length(rest)`. Lengths of
    /// genuinely symbolic arrays (e.g. `length(rest)` where `rest` is a free
    /// variable) are left stuck for the caller to extract as atoms.
    pub fn reduce(&mut self, e: &Arr) -> Result<Arr, String> {
        self.tick()
            .ok_or_else(|| "termination guard: budget exhausted".to_string())?;
        match e {
            Arr::Var(_) | Arr::Empty | Arr::Num(_) | Arr::Bool(_) => Ok(e.clone()),

            Arr::Cons(x, rest) => {
                let x = self.reduce(x)?;
                let rest = self.reduce(rest)?;
                Ok(Arr::cons(x, rest))
            }

            Arr::Append(a, b) => {
                let a = self.reduce(a)?;
                let b = self.reduce(b)?;
                match (&a, &b) {
                    // append([], b) = b
                    (Arr::Empty, _) => Ok(b),
                    // append(x :: rest, b) = x :: append(rest, b)
                    (Arr::Cons(x, rest), _) => {
                        let inner = self.reduce(&Arr::append((**rest).clone(), b))?;
                        Ok(Arr::cons((**x).clone(), inner))
                    }
                    // append(a, []) = a
                    (_, Arr::Empty) => Ok(a),
                    _ => Ok(Arr::append(a, b)),
                }
            }

            Arr::Map(f, a) => {
                let a = self.reduce(a)?;
                match &a {
                    // map(f, []) = []
                    Arr::Empty => Ok(Arr::Empty),
                    // map(f, x :: rest) = f(x) :: map(f, rest)
                    Arr::Cons(x, rest) => {
                        let mapped = Arr::App(f.clone(), Box::new((**x).clone()));
                        let inner = self.reduce(&Arr::map(f.clone(), (**rest).clone()))?;
                        Ok(Arr::cons(mapped, inner))
                    }
                    _ => Ok(Arr::map(f.clone(), a)),
                }
            }

            Arr::Len(a) => {
                let a = self.reduce(a)?;
                match &a {
                    // length([]) = 0
                    Arr::Empty => Ok(Arr::Num(0)),
                    // length(x :: rest) = 1 + length(rest)
                    Arr::Cons(_, rest) => {
                        let inner = self.reduce(&Arr::len((**rest).clone()))?;
                        Ok(Arr::add(Arr::Num(1), inner))
                    }
                    _ => Ok(Arr::len(a)),
                }
            }

            Arr::Add(a, b) => {
                let a = self.reduce(a)?;
                let b = self.reduce(b)?;
                Ok(Arr::add(a, b))
            }

            Arr::Eq(a, b) => {
                let a = self.reduce(a)?;
                let b = self.reduce(b)?;
                Ok(Arr::eq(a, b))
            }

            Arr::App(f, x) => Ok(Arr::App(f.clone(), Box::new(self.reduce(x)?))),

            // array.rec regions are handled by prove_array_rec, not reduced.
            Arr::Rec { .. } => Ok(e.clone()),
        }
    }

    /// Reduce an expression to its sum normal form.
    ///
    /// Returns `Err(reason)` if the expression is not normalizable to a sum
    /// (the source of [`Blocked`](Verdict::Blocked)).
    pub fn to_sum(&mut self, e: &Arr) -> Result<Sum, String> {
        self.tick()
            .ok_or_else(|| "termination guard: budget exhausted".to_string())?;
        let r = self.reduce(e)?;
        match r {
            Arr::Num(n) => Ok(add_atom(Sum::new(), "#", n)),
            Arr::Add(a, b) => {
                let sa = self.to_sum(&a)?;
                let sb = self.to_sum(&b)?;
                Ok(merge_sum(sa, sb))
            }
            Arr::Len(a) => self.len_sum(&a),
            Arr::Bool(_) => Err("boolean in numeric position".to_string()),
            Arr::Var(_) => {
                // A bare symbolic numeric variable: treat it as a scalar
                // length atom (e.g. `n + 1` where `n` is a free length).
                Ok(add_atom(Sum::new(), &r.to_string(), 1))
            }
            Arr::Empty | Arr::Cons(_, _) | Arr::Append(_, _) | Arr::Map(_, _) | Arr::App(_, _) => {
                Err(format!("array expression {} in numeric position", r))
            }
            Arr::Eq(_, _) => Err(format!("equality {} used as a number", r)),
            Arr::Rec { .. } => Err("array.rec region used as a number".to_string()),
        }
    }
    /// Convert a reduced array expression into length-atom summands.
    fn len_sum(&mut self, a: &Arr) -> Result<Sum, String> {
        self.tick()
            .ok_or_else(|| "termination guard: budget exhausted".to_string())?;
        let a = self.reduce(a)?;
        match &a {
            // |[]| = 0
            Arr::Empty => Ok(Sum::new()),
            // |x :: rest| = 1 + |rest|
            Arr::Cons(_, rest) => {
                let one = add_atom(Sum::new(), "#", 1);
                let rest_sum = self.len_sum(rest)?;
                Ok(merge_sum(one, rest_sum))
            }
            // A stuck symbolic array: one length atom keyed by its rendering.
            _ => Ok(add_atom(Sum::new(), &a.to_string(), 1)),
        }
    }

    /// Check an equality by normalization, optionally applying an induction
    /// hypothesis as a rewrite.
    fn check(&mut self, lhs: &Arr, rhs: &Arr, ih: Option<&Arr>) -> Result<Verdict, String> {
        self.tick()
            .ok_or_else(|| "termination guard: budget exhausted".to_string())?;
        let l = self.to_sum(lhs)?;
        let r = self.to_sum(rhs)?;
        if l == r {
            return Ok(Verdict::Proven);
        }
        if let Some(ih) = ih {
            if let Some((key, repl)) = self.ih_rewrite(ih)? {
                let l = self.rewrite(l, &key, &repl);
                let r = self.rewrite(r, &key, &repl);
                if l == r {
                    return Ok(Verdict::Proven);
                }
            }
        }
        Ok(Verdict::Disproved)
    }

    /// From an induction hypothesis `Eq(ih_lhs, ih_rhs)`, derive a rewrite
    /// rule `atom_key -> replacement_sum`, when the hypothesis has the shape
    /// of a single length atom (e.g. `|append(rest, b)| == |rest| + |b|`).
    fn ih_rewrite(&mut self, ih: &Arr) -> Result<Option<(String, Sum)>, String> {
        self.tick()
            .ok_or_else(|| "termination guard: budget exhausted".to_string())?;
        let ir = self.reduce(ih)?;
        // The IH must be an equality; otherwise there is nothing to derive
        // (an honest "no rewrite available", not a failure).
        let Arr::Eq(il, irhs) = ir else {
            return Ok(None);
        };
        let il = self.to_sum(&il)?;
        // The IH's left side must be exactly one length atom (no constant,
        // no mixture) so we can use it as a targeted rewrite.
        let non_const: Vec<(&String, &i64)> =
            il.iter().filter(|(k, _)| k.as_str() != "#").collect();
        if non_const.len() != 1 {
            return Ok(None);
        }
        let (key, &coef) = non_const[0];
        if coef != 1 {
            return Ok(None);
        }
        if il.get("#").copied().unwrap_or(0) != 0 {
            return Ok(None);
        }
        let repl = self.to_sum(&irhs)?;
        Ok(Some((key.clone(), repl)))
    }

    /// Replace every occurrence of atom `key` in `sum` with `repl`.
    fn rewrite(&self, mut sum: Sum, key: &str, repl: &Sum) -> Sum {
        if let Some(&coef) = sum.get(key) {
            sum.remove(key);
            for (k, v) in repl {
                sum = add_atom(sum, k, v * coef);
            }
        }
        sum
    }
}
// ============================================================================
// array.rec recognition + induction
// ============================================================================

/// The structure of an `array.rec` induction, as recognized by the checker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayRecSpec {
    /// Directive closing the base case (`a := []`).
    pub base: Directive,
    /// Directive closing the step case (`a := x :: rest`, using the IH).
    pub step: Directive,
    /// The array variable being inducted on.
    pub scrutinee: String,
    /// Left-hand side of the type-position equality (a `Len`/`Add` of the
    /// scrutinee and the free variables).
    pub lhs: Arr,
    /// Right-hand side of the type-position equality.
    pub rhs: Arr,
}

/// Recognize an `array.rec` region inside a goal `Arr`, returning its
/// [`ArrayRecSpec`] if the expression is an equality guarded by induction.
///
/// This is the "the checker recognizes `array.rec` as a library function"
/// behavior from ADR-0068 §2: `array.rec` is not a normal runtime function
/// the evaluator digests — it is an induction principle the checker unpacks.
pub fn recognize_array_rec(e: &Arr) -> Option<ArrayRecSpec> {
    match e {
        Arr::Rec {
            base,
            step,
            scrutinee,
            goal,
        } => {
            // The goal must itself be a proposition (an equality) so that the
            // induction has something to prove.
            let Arr::Eq(l, r) = &**goal else {
                return None;
            };
            Some(ArrayRecSpec {
                base: *base,
                step: *step,
                scrutinee: scrutinee.clone(),
                lhs: (**l).clone(),
                rhs: (**r).clone(),
            })
        }
        _ => None,
    }
}

/// Check a single proposition `lhs == rhs` under a checker directive.
///
/// - `by_eval` normalizes both sides and compares — no induction hypothesis.
/// - `by_norm` additionally applies `ih` (if present) as a rewrite.
///
/// A normalization failure becomes [`Blocked`](Verdict::Blocked) (runtime
/// fallback), never a crash.
pub fn check_proposition(
    lhs: &Arr,
    rhs: &Arr,
    directive: Directive,
    ih: Option<&Arr>,
    budget: u32,
) -> Proof {
    let mut pev = Pev::new(budget);
    let verdict = match directive {
        Directive::ByEval => pev.check(lhs, rhs, None),
        Directive::ByNorm => pev.check(lhs, rhs, ih),
    };
    match verdict {
        Ok(Verdict::Proven) => Proof::ok(format!(
            "{}: {} normalizes to true",
            directive,
            Arr::eq(lhs.clone(), rhs.clone())
        )),
        Ok(Verdict::Disproved) => Proof::no(format!(
            "{}: {} normalizes to false",
            directive,
            Arr::eq(lhs.clone(), rhs.clone())
        )),
        Ok(Verdict::Blocked) => Proof::blocked(format!(
            "{}: {} blocked (runtime fallback)",
            directive,
            Arr::eq(lhs.clone(), rhs.clone())
        )),
        Err(reason) => Proof::blocked(format!(
            "{}: {} -- {}",
            directive,
            Arr::eq(lhs.clone(), rhs.clone()),
            reason
        )),
    }
}

/// Run structural induction over an array to prove `lhs == rhs` for all
/// arrays of the scrutinee, per `array.rec`.
///
/// - **Base** (`a := []`): proven via the spec's `base` directive (normally
///   `by_eval` — there is no smaller array to invoke an IH on).
/// - **Step** (`a := x :: rest`): proven via the spec's `step` directive
///   (normally `by_norm + ih`, where `ih` is `goal[scrutinee := rest]`).
///
/// Verdicts combine so that any blocked case yields [`Blocked`](Verdict::Blocked)
/// (the caller inserts a runtime check), any refuted case yields
/// [`Disproved`](Verdict::Disproved), and both cases provable yield
/// [`Proven`](Verdict::Proven).
pub fn prove_array_rec(spec: &ArrayRecSpec, budget: u32) -> Proof {
    let mut pev = Pev::new(budget);

    // --- Base case: scrutinee := [] ---
    let empty = Arr::Empty;
    let base_lhs = pev.subst(&spec.lhs, &spec.scrutinee, &empty);
    let base_rhs = pev.subst(&spec.rhs, &spec.scrutinee, &empty);
    let base = check_proposition(&base_lhs, &base_rhs, spec.base, None, pev.budget);
    if base.verdict == Verdict::Disproved {
        return Proof::no(format!(
            "base case ({}) refuted: {}",
            spec.scrutinee, base.reason
        ));
    }
    if base.verdict == Verdict::Blocked {
        return Proof::blocked(format!(
            "base case ({}) could not be normalized: {}",
            spec.scrutinee, base.reason
        ));
    }

    // --- Step case: scrutinee := x :: rest, with ih := goal[rest] ---
    let x = Arr::var("x");
    let rest = Arr::var("rest");
    // ih: the original goal specialized to the induction tail.
    let ih = Arr::eq(
        pev.subst(&spec.lhs, &spec.scrutinee, &rest),
        pev.subst(&spec.rhs, &spec.scrutinee, &rest),
    );
    let step_lhs = pev.subst(
        &spec.lhs,
        &spec.scrutinee,
        &Arr::cons(x.clone(), rest.clone()),
    );
    let step_rhs = pev.subst(&spec.rhs, &spec.scrutinee, &Arr::cons(x, rest));
    let step = check_proposition(&step_lhs, &step_rhs, spec.step, Some(&ih), pev.budget);
    if step.verdict == Verdict::Disproved {
        return Proof::no(format!("step case refuted: {}", step.reason));
    }
    if step.verdict == Verdict::Blocked {
        return Proof::blocked(format!(
            "step case could not be normalized: {}",
            step.reason
        ));
    }

    Proof::ok(format!(
        "array.rec base ({}) proven by {}; step (x :: rest) proven by {} with ih == {}",
        spec.scrutinee, spec.base, spec.step, ih
    ))
}
// ============================================================================
// Ready-made length lemmas (ADR-0068 §2)
// ============================================================================

/// Prove `|append(a, b)| == |a| + |b|` for all arrays, by `array.rec` with
/// `base: by_eval` and `step: (x, rest, ih) -> by_norm + ih`.
pub fn prove_length_append() -> Proof {
    let spec = ArrayRecSpec {
        base: Directive::ByEval,
        step: Directive::ByNorm,
        scrutinee: "a".to_string(),
        lhs: Arr::len(Arr::append(Arr::var("a"), Arr::var("b"))),
        rhs: Arr::add(Arr::len(Arr::var("a")), Arr::len(Arr::var("b"))),
    };
    prove_array_rec(&spec, 10_000)
}

/// Prove `|map(f, a)| == |a|` for all arrays, by `array.rec` with
/// `base: by_eval` and `step: (x, rest, ih) -> by_norm + ih`.
pub fn prove_length_map() -> Proof {
    let spec = ArrayRecSpec {
        base: Directive::ByEval,
        step: Directive::ByNorm,
        scrutinee: "a".to_string(),
        lhs: Arr::len(Arr::map("f", Arr::var("a"))),
        rhs: Arr::len(Arr::var("a")),
    };
    prove_array_rec(&spec, 10_000)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_length_append_proven() {
        let p = prove_length_append();
        assert_eq!(p.verdict, Verdict::Proven, "{}", p.reason);
    }

    #[test]
    fn test_length_map_proven() {
        let p = prove_length_map();
        assert_eq!(p.verdict, Verdict::Proven, "{}", p.reason);
    }

    #[test]
    fn test_by_eval_base_append() {
        // |append([], b)| == |[]| + |b|  -- by_eval, no IH needed.
        let lhs = Arr::len(Arr::append(Arr::empty(), Arr::var("b")));
        let rhs = Arr::add(Arr::len(Arr::empty()), Arr::len(Arr::var("b")));
        let p = check_proposition(&lhs, &rhs, Directive::ByEval, None, 1000);
        assert_eq!(p.verdict, Verdict::Proven, "{}", p.reason);
    }

    #[test]
    fn test_by_norm_step_uses_ih() {
        // Step of the map lemma directly:
        //   |map(f, x :: rest)| == |x :: rest|  with ih |map(f,rest)| == |rest|
        let lhs = Arr::len(Arr::map("f", Arr::cons(Arr::var("x"), Arr::var("rest"))));
        let rhs = Arr::len(Arr::cons(Arr::var("x"), Arr::var("rest")));
        let ih = Arr::eq(
            Arr::len(Arr::map("f", Arr::var("rest"))),
            Arr::len(Arr::var("rest")),
        );
        // Without the IH, by_eval cannot close the step -> disproved.
        let by_eval = check_proposition(&lhs, &rhs, Directive::ByEval, None, 1000);
        let by_norm = check_proposition(&lhs, &rhs, Directive::ByNorm, Some(&ih), 1000);
        assert_eq!(
            by_eval.verdict,
            Verdict::Disproved,
            "by_eval should be too weak"
        );
        assert_eq!(by_norm.verdict, Verdict::Proven, "{}", by_norm.reason);
    }

    #[test]
    fn test_by_eval_disproves_false_equality() {
        let p = check_proposition(&Arr::num(0), &Arr::num(1), Directive::ByEval, None, 1000);
        assert_eq!(p.verdict, Verdict::Disproved, "{}", p.reason);
    }

    #[test]
    fn test_non_normalizable_blocks_not_crashes() {
        // A boolean in numeric position must yield Blocked (err -> Blocked),
        // never a panic.
        let p = check_proposition(
            &Arr::Bool(true),
            &Arr::num(1),
            Directive::ByEval,
            None,
            1000,
        );
        assert_eq!(p.verdict, Verdict::Blocked, "{}", p.reason);
    }

    #[test]
    fn test_termination_guard_blocks() {
        // A tiny budget that cannot even run a single normalization still
        // yields Blocked, proving the guard is a fallback, not a panic.
        let lhs = Arr::len(Arr::append(Arr::var("a"), Arr::var("b")));
        let rhs = Arr::add(Arr::len(Arr::var("a")), Arr::len(Arr::var("b")));
        let p = check_proposition(&lhs, &rhs, Directive::ByEval, None, 0);
        assert_eq!(p.verdict, Verdict::Blocked, "{}", p.reason);
    }

    #[test]
    fn test_recognize_array_rec() {
        let goal = Arr::eq(
            Arr::len(Arr::append(Arr::var("a"), Arr::var("b"))),
            Arr::add(Arr::len(Arr::var("a")), Arr::len(Arr::var("b"))),
        );
        let e = Arr::Rec {
            base: Directive::ByEval,
            step: Directive::ByNorm,
            scrutinee: "a".to_string(),
            goal: Box::new(goal),
        };
        let spec = recognize_array_rec(&e).expect("should recognize");
        assert_eq!(spec.scrutinee, "a");
        assert_eq!(spec.base, Directive::ByEval);
        assert_eq!(spec.step, Directive::ByNorm);
        // Not an equality-guarded rec is not recognized.
        assert!(recognize_array_rec(&Arr::Num(1)).is_none());
        let non_eq = Arr::Rec {
            base: Directive::ByEval,
            step: Directive::ByNorm,
            scrutinee: "a".to_string(),
            goal: Box::new(Arr::Num(2)),
        };
        assert!(recognize_array_rec(&non_eq).is_none());
    }

    #[test]
    fn test_reduce_append_and_length() {
        let mut pev = Pev::new(1000);
        // append([], b) ==> b
        let r = pev
            .reduce(&Arr::append(Arr::empty(), Arr::var("b")))
            .unwrap();
        assert_eq!(r, Arr::var("b"));
        // |x :: rest| ==> 1 + |rest|
        let r = pev
            .reduce(&Arr::len(Arr::cons(Arr::num(1), Arr::var("rest"))))
            .unwrap();
        assert_eq!(r, Arr::add(Arr::num(1), Arr::len(Arr::var("rest"))));
        // map(f, []) ==> []
        let r = pev.reduce(&Arr::map("f", Arr::empty())).unwrap();
        assert_eq!(r, Arr::empty());
    }

    #[test]
    fn test_verdict_display() {
        assert_eq!(Verdict::Proven.to_string(), "proven");
        assert_eq!(Verdict::Disproved.to_string(), "disproved");
        assert!(Verdict::Blocked.to_string().contains("runtime fallback"));
    }
}
