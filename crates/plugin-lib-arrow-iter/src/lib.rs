//! # handl-plugin-lib-arrow-iter
//!
//! Derived iteration patterns over `Loop` + `Choice` (ADR-0052 §"Derived
//! forms").
//!
//! ADR-0052 establishes that `while`/`for`/`fix` are **not** kernel
//! primitives and **not** grammar rules — they are *derived patterns*
//! expressed via the six arrow-calculus primitives. The load-bearing
//! principle: **feedback is primitive; iteration is derived.** This crate
//! is the concrete payoff: it builds [`SchemeV2`] descriptions of `while`/
//! `for`/`fix` from `Loop` + `Arr`, with no new kernel constructs.
//!
//! Each builder returns a [`SchemeV2::Loop`] whose body is a single
//! `Arr` composite step op (`while_step`/`for_step`/`fix_step`) carrying
//! the user-supplied sub-arrows (`cond`/`body`) as `args`. The
//! description is primitive-only (`Loop` + `Arr`); the *interpretation*
//! of the composite op lives in the v2 reference handler
//! (`handl-plugin-engine-eval-tree-walk`), which routes the current
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

#![warn(missing_docs)]

use handl_language::SchemeV2;

/// `while cond body` — derived from `Loop` + `Choice`
/// (ADR-0052 §"Derived forms").
///
/// Repeatedly applies `body` while `cond` holds. The body receives the
/// current value; `cond` tests it. When `cond` is falsy, the loop
/// returns the current value.
///
/// Arrow-calculus form: a [`SchemeV2::Loop`] whose body is the
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
pub fn while_loop(cond: SchemeV2, body: SchemeV2) -> SchemeV2 {
    SchemeV2::loop_feedback(SchemeV2::arr_native("while_step", vec![cond, body]))
}

/// `for body` — derived from `Loop` + `Choice`
/// (ADR-0052 §"Derived forms").
///
/// Runs `body` a fixed number of times, accumulating the result. The
/// input is `pair(count, initial)`; `body` is applied to the accumulator
/// on each step. Returns the accumulated value after `count` steps.
///
/// Arrow-calculus form: a [`SchemeV2::Loop`] whose body is the
/// `for_step` composite op carrying `body` as a sub-arrow. The state is
/// `pair(counter, acc)`, initialized from the input on the first step.
/// While `counter > 0`, the step runs `body` on `acc` and continues with
/// `pair(counter - 1, body(acc))`; when `counter == 0`, it terminates
/// with `acc`.
pub fn for_loop(body: SchemeV2) -> SchemeV2 {
    SchemeV2::loop_feedback(SchemeV2::arr_native("for_step", vec![body]))
}

/// `fix body` — derived from `Loop` + `Choice`
/// (ADR-0052 §"Derived forms").
///
/// Repeatedly applies `body` until the result stabilizes (equals the
/// input to that step). Returns the fixpoint.
///
/// Arrow-calculus form: a [`SchemeV2::Loop`] whose body is the
/// `fix_step` composite op carrying `body` as a sub-arrow. The step runs
/// `body` on the current value (the original input on the first
/// iteration; the evolving state thereafter). If the result equals the
/// current value, the fixpoint is reached — terminate with it; else
/// continue with the result. The `Loop` max-iteration guard bounds
/// non-converging bodies.
pub fn fix_loop(body: SchemeV2) -> SchemeV2 {
    SchemeV2::loop_feedback(SchemeV2::arr_native("fix_step", vec![body]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_language::{Ast, AstNode, AstRef, LangError, Language, Value};
    use handl_plugin_engine_eval_tree_walk::ScopeStack;
    use handl_plugin_engine_eval_tree_walk::v2::V2Interpreter;
    use serde_json::json;

    /// Construct a pair value `{"__pair__": [a, b]}`.
    fn pair(a: Value, b: Value) -> Value {
        json!({"__pair__": [a, b]})
    }

    /// Build a minimal one-node AST and evaluate a standalone scheme with
    /// a fresh env, passing `None` as incoming.
    fn eval_standalone(
        engine: &V2Interpreter,
        scheme: &SchemeV2,
        env: &mut ScopeStack,
    ) -> Result<Value, LangError> {
        let mut ast = Ast::new();
        ast.push(AstNode::new("test").with_payload(json!({})));
        let node = ast.at(AstRef::new(0)).unwrap();
        engine.eval_scheme_v2(scheme, &ast, AstRef::new(0), &Language::new(), env, node, None)
    }

    #[test]
    fn while_loop_counts_down_to_zero() {
        // cond = is_pos (incoming > 0), body = dec (incoming - 1).
        // Start at 5: 5 -> 4 -> 3 -> 2 -> 1 -> 0 (cond falsy) -> return 0.
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let cond = SchemeV2::arr_native("is_pos", vec![]);
        let body = SchemeV2::arr_native("dec", vec![]);
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], json!(5)),
            while_loop(cond, body),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(0));
    }

    #[test]
    fn for_loop_accumulates_increments() {
        // body = inc (incoming + 1). Input = pair(5, 0): run 5 times.
        // 0 -> 1 -> 2 -> 3 -> 4 -> 5 (counter hits 0) -> return 5.
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let body = SchemeV2::arr_native("inc", vec![]);
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], pair(json!(5), json!(0))),
            for_loop(body),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(5));
    }

    #[test]
    fn fix_loop_halves_to_zero() {
        // body = half_floor (incoming / 2, floored). Start at 100.
        // 100 -> 50 -> 25 -> 12 -> 6 -> 3 -> 1 -> 0 -> 0 (fixpoint) -> 0.
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let body = SchemeV2::arr_native("half_floor", vec![]);
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], json!(100)),
            fix_loop(body),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(0));
    }

    #[test]
    fn while_loop_zero_iterations() {
        // cond = is_pos, body = dec. Start at 0: cond falsy immediately.
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let cond = SchemeV2::arr_native("is_pos", vec![]);
        let body = SchemeV2::arr_native("dec", vec![]);
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], json!(0)),
            while_loop(cond, body),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(0));
    }

    #[test]
    fn for_loop_zero_iterations() {
        // body = inc. Input = pair(0, 99): zero iterations -> return 99.
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let body = SchemeV2::arr_native("inc", vec![]);
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], pair(json!(0), json!(99))),
            for_loop(body),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(99));
    }

    #[test]
    fn fix_loop_already_at_fixpoint() {
        // body = half_floor. Start at 0: 0 -> 0 (fixpoint) -> return 0.
        let engine = V2Interpreter::new();
        let mut env = ScopeStack::new();
        let body = SchemeV2::arr_native("half_floor", vec![]);
        let s = SchemeV2::seq(
            SchemeV2::arr_native_lit("const", vec![], json!(0)),
            fix_loop(body),
        );
        let result = eval_standalone(&engine, &s, &mut env).unwrap();
        assert_eq!(result, json!(0));
    }
}
