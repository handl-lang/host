//! Derived iteration patterns over `Loop` + `Choice` (ADR-0052 §"Derived
//! forms").
//!
//! ADR-0052 establishes that `while`/`for`/`fix` are **not** kernel
//! primitives and **not** grammar rules — they are *derived patterns*
//! expressed via the six arrow-calculus primitives. The load-bearing
//! principle: **feedback is primitive; iteration is derived.** These
//! builders construct [`Term`] descriptions of `while`/`for`/`fix` from
//! `Loop` + `Arr`, with no new kernel constructs.
//!
//! Each builder returns a [`Term::Loop`] whose body is a single
//! `Arr` composite step op (`while_step`/`for_step`/`fix_step`) carrying
//! the user-supplied sub-arrows (`cond`/`body`) as `args`. The
//! description is primitive-only (`Loop` + `Arr`); the *interpretation*
//! of the composite op lives in the v2 reference handler
//! (`hnl-engine-exec-tree-walk`), which routes the current
//! value through the carried sub-arrows and produces an `Either`
//! (`left` = done, `right` = continue) consumed by the multi-step
//! `Loop` handler. This is consistent with ADR-0052's "handlers
//! interpret" principle: the kernel constructs; the handler decides how
//! to run.
//!
//! The multi-step `Loop` semantics (Either-based: `left` terminates,
//! `right` feeds the new state back and re-runs) is what makes these
//! patterns iterate — a single-step `Loop` could not express repetition
//! on its own.
//!
//! The `arrow_iter` capability surface is declared in
//! `handl-packages/arrow_iter/arrow_iter.hnl` and folded into the
//! [`crate::packages`] module table (module `"arrow_iter"`, package
//! `"arrow_iter"`).

use handl_language::Term;

/// `while cond body` — derived from `Loop` + `Choice`
/// (ADR-0052 §"Derived forms").
///
/// Repeatedly applies `body` while `cond` holds. The body receives the
/// current value; `cond` tests it. When `cond` is falsy, the loop
/// returns the current value.
///
/// Arrow-calculus form: a [`Term::Loop`] whose body is the
/// `while_step` composite op carrying `cond` and `body` as sub-arrows.
/// The step extracts the current value (the original input on the first
/// iteration, when the state is `Null`; the evolving state thereafter),
/// runs `cond` on it, and routes via `Choice`:
/// - `cond` truthy → `either_right(body(value))`  (continue with the
///   body's output as the new state)
/// - `cond` falsy → `either_left(value)`          (done, return value)
///
/// The `Loop` handler re-runs the body on `right` and returns the
/// carried value on `left`.
pub fn while_loop(cond: Term, body: Term) -> Term {
    Term::loop_feedback(Term::arr_native("while_step", vec![cond, body]))
}

/// `for body` — derived from `Loop` + `Choice`
/// (ADR-0052 §"Derived forms").
///
/// Runs `body` a fixed number of times, accumulating the result. The
/// input is `pair(count, initial)`; `body` is applied to the accumulator
/// on each step. Returns the accumulated value after `count` steps.
///
/// Arrow-calculus form: a [`Term::Loop`] whose body is the
/// `for_step` composite op carrying `body` as a sub-arrow. The state is
/// `pair(counter, acc)`, initialized from the input on the first step.
/// While `counter > 0`, the step runs `body` on `acc` and continues with
/// `pair(counter - 1, body(acc))`; when `counter == 0`, it terminates
/// with `acc`.
pub fn for_loop(body: Term) -> Term {
    Term::loop_feedback(Term::arr_native("for_step", vec![body]))
}

/// `fix body` — derived from `Loop` + `Choice`
/// (ADR-0052 §"Derived forms").
///
/// Repeatedly applies `body` until the result stabilizes (equals the
/// input to that step). Returns the fixpoint.
///
/// Arrow-calculus form: a [`Term::Loop`] whose body is the
/// `fix_step` composite op carrying `body` as a sub-arrow. The step runs
/// `body` on the current value (the original input on the first
/// iteration; the evolving state thereafter). If the result equals the
/// current value, the fixpoint is reached — terminate with it; else
/// continue with the result. The `Loop` max-iteration guard bounds
/// non-converging bodies.
pub fn fix_loop(body: Term) -> Term {
    Term::loop_feedback(Term::arr_native("fix_step", vec![body]))
}
