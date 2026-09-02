//! Tests for the refinement inference engine.
//!
//! These tests port the research prototype's test cases to handl's `Scheme` IR,
//! verifying that the inference produces the same results:
//! - The 5 real programs: insertion sort, map, filter, fold/sum, pipeline
//! - Loop invariant discovery on side-channel, accumulator, filter, sortedness patterns
//! - Termination analysis on decreasing measure patterns
//! - SMT implication checks

use handl_language::{ArrSource, Scheme};

use crate::contracts::ContractRegistry;
use crate::inference::infer_refinement;
use crate::loop_invariant::{discover_invariant_fixpoint, discover_invariant_fixpoint_real, weakest_precondition};
use crate::smt::{implies, implies_with_report, simplify_with_arithmetic, toy_implies};
use crate::termination::analyze_termination;
use crate::types::{BaseTy, Measure, Pred, RefTy};

// ============================================================================
// Helpers: build the 5 real programs as Scheme graphs
// ============================================================================

/// Convenience: native arr with no args.
fn arr(op: &str) -> Scheme {
    Scheme::Arr {
        op: op.to_string(),
        args: vec![],
        source: ArrSource::Native,
        lit: None,
    }
}

/// Convenience: first(inner)
fn first(inner: Scheme) -> Scheme {
    Scheme::First {
        inner: Box::new(inner),
    }
}

/// Convenience: seq(fst, snd)
fn seq(fst: Scheme, snd: Scheme) -> Scheme {
    Scheme::Seq {
        fst: Box::new(fst),
        snd: Box::new(snd),
    }
}

/// Convenience: loop_feedback(body)
fn loop_fb(body: Scheme) -> Scheme {
    Scheme::Loop {
        body: Box::new(body),
    }
}

/// Convenience: choice(l, r)
fn choice(l: Scheme, r: Scheme) -> Scheme {
    Scheme::Choice {
        l: Box::new(l),
        r: Box::new(r),
    }
}

/// Insertion sort.
///
/// State threaded by the loop: (remaining_input, sorted_accumulator).
/// Each iteration:
///   1. first(head_and_tail) -- split remaining input into (head, tail)
///   2. first(insert)         -- insert head into the sorted accumulator
///   3. consume_one           -- consume one element from the input (|C|--)
///
/// The loop terminates because consume_one decrements |remaining_input|.
/// The accumulator stays sorted because insert's guarantee is Sorted(output).
fn insertion_sort() -> Scheme {
    loop_fb(seq(
        seq(
            first(arr("head_and_tail")),
            first(arr("insert")),
        ),
        arr("consume_one"),
    ))
}

/// Map.
///
/// State: (remaining_input, output_accumulator).
/// Each iteration:
///   1. first(head_and_tail)  -- split remaining into (head, tail)
///   2. first(apply_f)         -- apply f to head
///   3. comp(first(consume_one), append_to_output) -- consume input, append to output
fn map_impl() -> Scheme {
    loop_fb(seq(
        seq(
            first(arr("head_and_tail")),
            first(arr("apply_f")),
        ),
        seq(
            first(arr("consume_one")),
            arr("append_to_output"),
        ),
    ))
}

/// Filter.
///
/// State: (remaining_input, output_accumulator).
/// Each iteration:
///   1. first(head_and_tail)     -- split remaining into (head, tail)
///   2. first(test_predicate)    -- test if head satisfies predicate
///   3. left(append_to_output)   -- if predicate true, append to output; if false, skip
///   4. consume_one              -- consume one element from remaining input (|C|--)
fn filter_impl() -> Scheme {
    loop_fb(seq(
        seq(
            first(arr("head_and_tail")),
            first(arr("test_predicate")),
        ),
        seq(
            choice(arr("append_to_output"), arr("id")),
            arr("consume_one"),
        ),
    ))
}

/// Fold / Sum.
///
/// State: (remaining_input, accumulator).
/// Each iteration:
///   1. first(head_and_tail)  -- split remaining into (head, tail)
///   2. first(add_to_acc)      -- add head to accumulator
///   3. consume_one            -- consume one element from the input
fn sum_impl() -> Scheme {
    loop_fb(seq(
        seq(
            first(arr("head_and_tail")),
            first(arr("add_to_acc")),
        ),
        arr("consume_one"),
    ))
}

/// Pipeline: filter -> sort -> map.
///
/// Composes the three real implementations. The inference composes the
/// refinements: AllSatisfy (from filter), Sorted (from sort), and
/// LengthPreserved (from map, on the post-sort list).
fn pipeline() -> Scheme {
    seq(
        seq(
            filter_impl(),
            arr("sort"),
        ),
        arr("map_step"),
    )
}

// ============================================================================
// 1. Base arrow inference tests
// ============================================================================

#[test]
fn test_sort_base_arrow() {
    let registry = ContractRegistry::standard();
    let g = arr("sort");
    let result = infer_refinement(&g, &registry).unwrap();
    assert_eq!(
        result.guarantee,
        Pred::Sorted("output".to_string())
    );
}

#[test]
fn test_map_base_arrow() {
    let registry = ContractRegistry::standard();
    let g = arr("map");
    let result = infer_refinement(&g, &registry).unwrap();
    assert_eq!(
        result.guarantee,
        Pred::LengthEq("input".to_string(), "output".to_string())
    );
}

#[test]
fn test_filter_base_arrow() {
    let registry = ContractRegistry::standard();
    let g = arr("filter");
    let result = infer_refinement(&g, &registry).unwrap();
    // filter's guarantee is And(AllSatisfy, LengthLe)
    assert!(result.guarantee != Pred::True);
}

#[test]
fn test_id_base_arrow() {
    let registry = ContractRegistry::standard();
    let g = arr("id");
    let result = infer_refinement(&g, &registry).unwrap();
    assert_eq!(result.guarantee, Pred::True);
}

// ============================================================================
// 2. Composition tests
// ============================================================================

#[test]
fn test_comp_sort_sort() {
    let registry = ContractRegistry::standard();
    let g = seq(arr("sort"), arr("sort"));
    let _result = infer_refinement(&g, &registry).unwrap();
    // Verify via the SMT checker that Sorted => Sorted (idempotence)
    let s = Pred::Sorted("output".to_string());
    assert!(implies(&s, &s));
}

#[test]
fn test_first_sort() {
    let registry = ContractRegistry::standard();
    let g = first(arr("sort"));
    let result = infer_refinement(&g, &registry).unwrap();
    // first(sort) preserves sortedness on the first component AND preserves
    // the side channel: |input.1| = |output.1|.
    let expected = Pred::And(
        Box::new(Pred::Sorted("output".to_string())),
        Box::new(Pred::LengthEq("input.1".to_string(), "output.1".to_string())),
    );
    assert_eq!(result.guarantee, expected);
}

#[test]
fn test_comp_map_sort() {
    let registry = ContractRegistry::standard();
    let g = seq(arr("map"), arr("sort"));
    let result = infer_refinement(&g, &registry).unwrap();
    // Guarantee is And(LengthEq, Sorted)
    assert!(result.guarantee != Pred::True);
}

// ============================================================================
// 3. Real program tests: insertion sort, map, filter, fold, pipeline
// ============================================================================

#[test]
fn test_insertion_sort() {
    let registry = ContractRegistry::standard();
    let g = insertion_sort();
    let result = infer_refinement(&g, &registry).unwrap();
    // The inference should discover loop invariants
    assert!(!result.loop_invariants.is_empty(), "should discover loop invariant");
    // Termination: consume_one decrements |C| -> guaranteed
    let term = analyze_termination(&g, &registry);
    assert!(term.terminates, "insertion sort must terminate");
    assert_eq!(term.measure, Measure::ListLength("c".to_string()));
}

#[test]
fn test_map_impl() {
    let registry = ContractRegistry::standard();
    let g = map_impl();
    let result = infer_refinement(&g, &registry).unwrap();
    assert!(!result.loop_invariants.is_empty(), "should discover loop invariant");
    // Termination: consume_one decrements |C| -> guaranteed
    let term = analyze_termination(&g, &registry);
    assert!(term.terminates, "map must terminate");
    assert_eq!(term.measure, Measure::ListLength("c".to_string()));
}

#[test]
fn test_filter_impl() {
    let registry = ContractRegistry::standard();
    let g = filter_impl();
    let result = infer_refinement(&g, &registry).unwrap();
    assert!(!result.loop_invariants.is_empty(), "should discover loop invariant");
    // Termination: consume_one decrements |C| -> guaranteed
    let term = analyze_termination(&g, &registry);
    assert!(term.terminates, "filter must terminate");
    assert_eq!(term.measure, Measure::ListLength("c".to_string()));
}

#[test]
fn test_sum_impl() {
    let registry = ContractRegistry::standard();
    let g = sum_impl();
    let result = infer_refinement(&g, &registry).unwrap();
    assert!(!result.loop_invariants.is_empty(), "should discover loop invariant");
    // Termination: consume_one decrements |C| -> guaranteed
    let term = analyze_termination(&g, &registry);
    assert!(term.terminates, "sum must terminate");
    assert_eq!(term.measure, Measure::ListLength("c".to_string()));
}

#[test]
fn test_pipeline() {
    let registry = ContractRegistry::standard();
    let g = pipeline();
    let result = infer_refinement(&g, &registry).unwrap();
    // Pipeline composes filter_impl, sort, map_step
    // The guarantee should be a conjunction of sub-guarantees
    assert!(result.guarantee != Pred::True, "pipeline should have a non-trivial guarantee");
    // No loops at the top level (loops are inside filter_impl)
    // Termination: filter_impl terminates, sort and map_step are base arrows
    let term = analyze_termination(&g, &registry);
    assert!(term.terminates, "pipeline must terminate");
}

// ============================================================================
// 4. Loop invariant discovery tests
// ============================================================================

#[test]
fn test_loop_invariant_side_channel() {
    // loop(first(id)) -- C passes through unchanged.
    let registry = ContractRegistry::standard();
    let body = first(arr("id"));
    let (inv, how, _verified) = discover_invariant_fixpoint(&body, &registry);
    // Side-channel: invariant is True
    assert_eq!(inv, Pred::True);
    assert!(how.contains("fixpoint"));
    assert!(_verified, "True is trivially preserved");
}

#[test]
fn test_loop_invariant_accumulator() {
    // loop(seq(f, id)) -- accumulator pattern, id leaves C unchanged.
    let registry = ContractRegistry::standard();
    let body = seq(arr("f"), arr("id"));
    let (inv, _how, _verified) = discover_invariant_fixpoint(&body, &registry);
    // The invariant should be True (f has guarantee True, id is True)
    // wp(seq(f, id), True) = wp(f, wp(id, True)) = wp(f, True) = True
    // So I_0 = True, wp = True, I_1 = True && True = True -> fixpoint at 0
    assert_eq!(inv, Pred::True);
}

#[test]
fn test_loop_invariant_consume_pattern() {
    // loop(seq(insert, consume_one)) -- consume pattern
    let registry = ContractRegistry::standard();
    let body = seq(arr("insert"), arr("consume_one"));
    let (inv, how, _verified) = discover_invariant_fixpoint(&body, &registry);
    // insert's guarantee is Sorted(output), consume_one's is True
    // wp(seq(insert, consume_one), True) = wp(insert, wp(consume_one, True))
    //   = wp(insert, True) = True (since consume_one guarantee True => True)
    // So I_0 = True, fixpoint at 0
    assert_eq!(inv, Pred::True);
    assert!(how.contains("fixpoint"));
}

#[test]
fn test_loop_invariant_real_fixpoint_iteration() {
    // Test the real fixpoint iteration function with trace
    let registry = ContractRegistry::standard();
    let body = first(arr("id"));
    let (inv, how, trace, verified) =
        discover_invariant_fixpoint_real(&body, &registry, &Pred::True, 20);
    assert_eq!(inv, Pred::True);
    assert!(how.contains("converged"));
    assert!(!trace.is_empty());
    assert!(verified);
}

#[test]
fn test_weakest_precondition_id() {
    let registry = ContractRegistry::standard();
    let post = Pred::Sorted("output".to_string());
    // wp(id, I) = I
    let wp = weakest_precondition(&arr("id"), &registry, &post);
    assert_eq!(wp, post);
}

#[test]
fn test_weakest_precondition_arr_guarantee_implies_post() {
    let registry = ContractRegistry::standard();
    let post = Pred::Sorted("output".to_string());
    // wp(sort, Sorted) = True (sort's guarantee implies Sorted)
    let wp = weakest_precondition(&arr("sort"), &registry, &post);
    assert_eq!(wp, Pred::True);
}

#[test]
fn test_weakest_precondition_arr_guarantee_does_not_imply_post() {
    let registry = ContractRegistry::standard();
    let post = Pred::Sorted("output".to_string());
    // wp(map, Sorted) = True (map's guarantee LengthEq doesn't imply Sorted,
    // so the WP is conservative -- True, imposing no constraint).
    let wp = weakest_precondition(&arr("map"), &registry, &post);
    // With the new WP rule, when the guarantee does not imply the post,
    // we conservatively return True (no constraint from this branch).
    assert_eq!(wp, Pred::True);
}

#[test]
fn test_weakest_precondition_seq() {
    let registry = ContractRegistry::standard();
    let post = Pred::Sorted("output".to_string());
    // wp(seq(sort, id), Sorted) = wp(sort, wp(id, Sorted)) = wp(sort, Sorted) = True
    let wp = weakest_precondition(&seq(arr("sort"), arr("id")), &registry, &post);
    assert_eq!(wp, Pred::True);
}

#[test]
fn test_weakest_precondition_first() {
    let registry = ContractRegistry::standard();
    let post = Pred::Sorted("output".to_string());
    // wp(first(g), I) = I (C passes through unchanged)
    let wp = weakest_precondition(&first(arr("sort")), &registry, &post);
    assert_eq!(wp, post);
}

// ============================================================================
// 5. Termination analysis tests
// ============================================================================

#[test]
fn test_termination_side_channel_loop() {
    // loop(first(id)) -- C passes through unchanged.
    let registry = ContractRegistry::standard();
    let g = loop_fb(first(arr("id")));
    let result = analyze_termination(&g, &registry);
    assert!(!result.terminates, "side-channel loop must NOT be guaranteed");
    assert_eq!(result.measure, Measure::Constant);
}

#[test]
fn test_termination_list_consuming_loop() {
    // loop(seq(process_one, consume_one)) -- C is a list, consume_one removes one element.
    let registry = ContractRegistry::standard();
    let g = loop_fb(seq(arr("process_one"), arr("consume_one")));
    let result = analyze_termination(&g, &registry);
    assert!(result.terminates, "list-consuming loop must terminate");
    assert_eq!(result.measure, Measure::ListLength("c".to_string()));
}

#[test]
fn test_termination_counter_loop() {
    // loop(seq(step, decrement)) -- C is a counter, decrement decrements by 1.
    let registry = ContractRegistry::standard();
    let g = loop_fb(seq(arr("step"), arr("decrement")));
    let result = analyze_termination(&g, &registry);
    assert!(result.terminates, "counter loop must terminate");
    assert_eq!(result.measure, Measure::Counter("c".to_string(), 1));
}

#[test]
fn test_termination_fixed_point_loop() {
    // loop(seq(f, id)) -- C updated by f, id leaves it; no decreasing measure.
    let registry = ContractRegistry::standard();
    let g = loop_fb(seq(arr("f"), arr("id")));
    let result = analyze_termination(&g, &registry);
    assert!(!result.terminates, "fixed-point loop must NOT be guaranteed");
}

#[test]
fn test_termination_nested_loop_terminating() {
    // loop(seq(loop(inner_terminating), first(id)))
    let registry = ContractRegistry::standard();
    let g = loop_fb(seq(
        loop_fb(arr("inner_terminating")),
        first(arr("id")),
    ));
    let result = analyze_termination(&g, &registry);
    assert!(
        result.terminates,
        "nested loop with terminating inner must terminate"
    );
}

#[test]
fn test_termination_nested_loop_nonterminating() {
    // loop(seq(loop(first(id)), first(id)))
    let registry = ContractRegistry::standard();
    let g = loop_fb(seq(
        loop_fb(first(arr("id"))),
        first(arr("id")),
    ));
    let result = analyze_termination(&g, &registry);
    assert!(
        !result.terminates,
        "nested loop with non-terminating inner must NOT be guaranteed"
    );
}

#[test]
fn test_termination_map_no_loop() {
    // arr(map) -- no loop, direct termination.
    let registry = ContractRegistry::standard();
    let g = arr("map");
    let result = analyze_termination(&g, &registry);
    assert!(result.terminates, "map with no loop must terminate");
}

#[test]
fn test_termination_sort_via_loop() {
    // loop(seq(insert, remove_one)) -- C is a list, remove_one decrements |C|.
    let registry = ContractRegistry::standard();
    let g = loop_fb(seq(arr("insert"), arr("remove_one")));
    let result = analyze_termination(&g, &registry);
    assert!(result.terminates, "sort via loop must terminate");
    assert_eq!(result.measure, Measure::ListLength("c".to_string()));
}

// ============================================================================
// 6. SMT implication checker tests
// ============================================================================

#[test]
fn test_implies_reflexivity() {
    let p = Pred::Sorted("output".to_string());
    assert!(implies(&p, &p));
}

#[test]
fn test_implies_anything_implies_true() {
    let p = Pred::Sorted("output".to_string());
    assert!(implies(&p, &Pred::True));
}

#[test]
fn test_implies_false_implies_anything() {
    let p = Pred::False;
    let q = Pred::Sorted("output".to_string());
    assert!(implies(&p, &q));
}

#[test]
fn test_implies_length_eq_implies_length_le() {
    let leq = Pred::LengthEq("input".to_string(), "output".to_string());
    let lle = Pred::LengthLe("output".to_string(), "input".to_string());
    assert!(implies(&leq, &lle));
}

#[test]
fn test_implies_and_decomposition() {
    let alls = Pred::AllSatisfy("output".to_string(), "predicate".to_string());
    let sorted = Pred::Sorted("output".to_string());
    let combined = Pred::And(Box::new(alls.clone()), Box::new(sorted.clone()));
    // (AllSatisfy && Sorted) ==> AllSatisfy
    assert!(implies(&combined, &alls));
    // (AllSatisfy && Sorted) ==> Sorted
    assert!(implies(&combined, &sorted));
}

#[test]
fn test_toy_implies_reflexivity() {
    let p = Pred::Sorted("output".to_string());
    assert!(toy_implies(&p, &p));
}

#[test]
fn test_toy_implies_length_eq_implies_length_le() {
    let leq = Pred::LengthEq("input".to_string(), "output".to_string());
    let lle = Pred::LengthLe("output".to_string(), "input".to_string());
    assert!(toy_implies(&leq, &lle));
}

#[test]
fn test_implies_with_report() {
    let p = Pred::Sorted("output".to_string());
    let (ok, report) = implies_with_report(&p, &p);
    assert!(ok);
    // Report should mention either Z3 or toy
    assert!(!report.is_empty());
}

// ----------------------------------------------------------------------------
// toy_implies simplify fix: True ==> And(True, True) and related
// ----------------------------------------------------------------------------

#[test]
fn test_toy_implies_true_implies_and_true_true() {
    // This was the failing case before the simplify fix.
    // True ==> And(True, True) -- And(True, True) simplifies to True.
    assert!(
        toy_implies(&Pred::True, &Pred::And(Box::new(Pred::True), Box::new(Pred::True))),
        "True ==> And(True, True) must hold after simplify fix"
    );
}

#[test]
fn test_implies_true_implies_and_true_true() {
    // Full implies checker (may use Z3 or toy): True ==> And(True, True).
    assert!(
        implies(&Pred::True, &Pred::And(Box::new(Pred::True), Box::new(Pred::True))),
        "True ==> And(True, True) must hold"
    );
}

#[test]
fn test_toy_implies_true_implies_or_true_false() {
    // True ==> Or(True, False) -- Or(True, False) simplifies to True.
    assert!(
        toy_implies(&Pred::True, &Pred::Or(Box::new(Pred::True), Box::new(Pred::False))),
        "True ==> Or(True, False) must hold after simplify fix"
    );
}

#[test]
fn test_implies_true_implies_or_true_false() {
    assert!(
        implies(&Pred::True, &Pred::Or(Box::new(Pred::True), Box::new(Pred::False))),
        "True ==> Or(True, False) must hold"
    );
}

#[test]
fn test_toy_implies_and_true_true_implies_true() {
    // And(True, True) ==> True -- trivially true (anything implies True).
    assert!(
        toy_implies(&Pred::And(Box::new(Pred::True), Box::new(Pred::True)), &Pred::True),
        "And(True, True) ==> True must hold"
    );
}

#[test]
fn test_toy_implies_and_true_true_equivalence() {
    // After simplification, And(True, True) == True, so both directions hold.
    let tt = Pred::And(Box::new(Pred::True), Box::new(Pred::True));
    assert!(toy_implies(&Pred::True, &tt), "True ==> And(True, True)");
    assert!(toy_implies(&tt, &Pred::True), "And(True, True) ==> True");
}

#[test]
fn test_toy_implies_nested_simplify() {
    // True ==> And(And(True, True), Or(True, False))
    // And(True, True) -> True, Or(True, False) -> True, And(True, True) -> True
    let q = Pred::And(
        Box::new(Pred::And(Box::new(Pred::True), Box::new(Pred::True))),
        Box::new(Pred::Or(Box::new(Pred::True), Box::new(Pred::False))),
    );
    assert!(
        toy_implies(&Pred::True, &q),
        "True ==> And(And(True,True), Or(True,False)) must hold after simplify"
    );
}

// ----------------------------------------------------------------------------
// Length arithmetic implication tests
// ----------------------------------------------------------------------------

#[test]
fn test_toy_implies_length_plus_one_implies_length_le() {
    // LengthPlusOne(a, b) ==> LengthLe(b, a)  (|a| = |b|+1 ==> |b| <= |a|)
    let p = Pred::LengthPlusOne("output".to_string(), "input".to_string());
    let q = Pred::LengthLe("input".to_string(), "output".to_string());
    assert!(toy_implies(&p, &q));
}

#[test]
fn test_toy_implies_length_sub_one_implies_length_le() {
    // LengthSubOne(a, b) ==> LengthLe(a, b)  (|a| = |b|-1 ==> |a| <= |b|)
    let p = Pred::LengthSubOne("output".to_string(), "input".to_string());
    let q = Pred::LengthLe("output".to_string(), "input".to_string());
    assert!(toy_implies(&p, &q));
}

#[test]
fn test_implies_length_plus_one_implies_length_le() {
    let p = Pred::LengthPlusOne("output".to_string(), "input".to_string());
    let q = Pred::LengthLe("input".to_string(), "output".to_string());
    assert!(implies(&p, &q));
}

#[test]
fn test_implies_length_sub_one_implies_length_le() {
    let p = Pred::LengthSubOne("output".to_string(), "input".to_string());
    let q = Pred::LengthLe("output".to_string(), "input".to_string());
    assert!(implies(&p, &q));
}

#[test]
fn test_toy_implies_length_plus_one_reflexivity() {
    let p = Pred::LengthPlusOne("output".to_string(), "input".to_string());
    assert!(toy_implies(&p, &p));
}

#[test]
fn test_toy_implies_length_sub_one_reflexivity() {
    let p = Pred::LengthSubOne("output".to_string(), "input".to_string());
    assert!(toy_implies(&p, &p));
}

// ----------------------------------------------------------------------------
// Le and HeadOf implication tests
// ----------------------------------------------------------------------------

#[test]
fn test_toy_implies_le_reflexivity() {
    let p = Pred::Le("head".to_string(), "x".to_string());
    assert!(toy_implies(&p, &p));
}

#[test]
fn test_toy_implies_head_of_reflexivity() {
    let p = Pred::HeadOf("head".to_string(), "tail".to_string());
    assert!(toy_implies(&p, &p));
}

#[test]
fn test_implies_le_reflexivity() {
    let p = Pred::Le("head".to_string(), "x".to_string());
    assert!(implies(&p, &p));
}

#[test]
fn test_implies_head_of_reflexivity() {
    let p = Pred::HeadOf("head".to_string(), "tail".to_string());
    assert!(implies(&p, &p));
}

// ----------------------------------------------------------------------------
// Conditional contract implication tests
// ----------------------------------------------------------------------------

#[test]
fn test_implies_conditional_contract_sorted_cons() {
    // Sorted(tail) && Le(head, head_of(tail)) ==> Sorted(cons(head, tail))
    // This is a conditional contract implication. The toy checker cannot
    // prove this (Sorted(cons(...)) is a different atom from Sorted(tail)),
    // but it should not return true for a false one either. We verify that
    // the implication checker is sound: it returns false (unknown) rather
    // than incorrectly claiming validity.
    let p = Pred::And(
        Box::new(Pred::Sorted("tail".to_string())),
        Box::new(Pred::Le("head".to_string(), "head_of_tail".to_string())),
    );
    let q = Pred::Sorted("cons_head_tail".to_string());
    // The toy checker should return false (cannot prove), not true.
    // This is sound: we don't claim validity we can't prove.
    assert!(!toy_implies(&p, &q), "toy checker must not prove unprovable conditional contract");
}

#[test]
fn test_implies_length_arithmetic_transitivity() {
    // LengthPlusOne(a, b) && LengthSubOne(b, c) ==> LengthEq(a, c)
    // |a| = |b| + 1, |b| = |c| - 1 -> |a| = (|c| - 1) + 1 = |c|
    // The toy checker handles this via the And-decomposition arm:
    // And(LengthPlusOne(a,b), LengthSubOne(b,c)) ==> LengthEq(a,c)
    // requires either conjunct to imply LengthEq(a,c), which neither does
    // individually. So the toy checker returns false (sound but incomplete).
    // Z3 (native or subprocess) can prove this via integer arithmetic.
    let p = Pred::And(
        Box::new(Pred::LengthPlusOne("a".to_string(), "b".to_string())),
        Box::new(Pred::LengthSubOne("b".to_string(), "c".to_string())),
    );
    let q = Pred::LengthEq("a".to_string(), "c".to_string());
    // The toy checker cannot prove this (incomplete but sound).
    assert!(!toy_implies(&p, &q), "toy checker must not prove arithmetic transitivity");
    // The full implies checker may prove it if Z3 is available.
    // We just verify it doesn't panic.
    let _ = implies(&p, &q);
}

// ----------------------------------------------------------------------------
// Z3 native tests (gated behind native-z3 feature)
// ----------------------------------------------------------------------------

#[cfg(feature = "native-z3")]
#[test]
fn test_z3_native_true_implies_and_true_true() {
    // Z3 native should prove True ==> And(True, True) trivially.
    assert!(implies(&Pred::True, &Pred::And(Box::new(Pred::True), Box::new(Pred::True))));
}

#[cfg(feature = "native-z3")]
#[test]
fn test_z3_native_length_arithmetic_transitivity() {
    // LengthPlusOne(a, b) && LengthSubOne(b, c) ==> LengthEq(a, c)
    // Z3 can prove this via integer arithmetic: |a| = |b|+1, |b| = |c|-1
    // -> |a| = (|c|-1)+1 = |c|.
    let p = Pred::And(
        Box::new(Pred::LengthPlusOne("a".to_string(), "b".to_string())),
        Box::new(Pred::LengthSubOne("b".to_string(), "c".to_string())),
    );
    let q = Pred::LengthEq("a".to_string(), "c".to_string());
    assert!(implies(&p, &q), "Z3 native should prove length arithmetic transitivity");
}

#[cfg(feature = "native-z3")]
#[test]
fn test_z3_native_length_eq_implies_length_le() {
    let leq = Pred::LengthEq("input".to_string(), "output".to_string());
    let lle = Pred::LengthLe("input".to_string(), "output".to_string());
    assert!(implies(&leq, &lle));
}

#[cfg(feature = "native-z3")]
#[test]
fn test_z3_native_invalid_implication() {
    // LengthLe(a, b) does NOT imply LengthEq(a, b) -- Z3 should find a
    // counterexample (e.g. |a|=0, |b|=1).
    let lle = Pred::LengthLe("input".to_string(), "output".to_string());
    let leq = Pred::LengthEq("input".to_string(), "output".to_string());
    // Z3 should say this is invalid (SAT for P && !Q).
    // The full implies checker returns false only if both Z3 and toy say no.
    // Z3 says invalid, toy says false -> implies returns false.
    assert!(!implies(&lle, &leq), "LengthLe does not imply LengthEq");
}

#[cfg(feature = "native-z3")]
#[test]
fn test_z3_native_length_plus_one_arithmetic() {
    // LengthPlusOne(a, b) ==> LengthLe(b, a): |a| = |b|+1 ==> |b| <= |a|
    let p = Pred::LengthPlusOne("output".to_string(), "input".to_string());
    let q = Pred::LengthLe("input".to_string(), "output".to_string());
    assert!(implies(&p, &q), "Z3 native should prove |a|=|b|+1 ==> |b|<=|a|");
}

#[cfg(feature = "native-z3")]
#[test]
fn test_z3_native_length_sub_one_arithmetic() {
    // LengthSubOne(a, b) ==> LengthLe(a, b): |a| = |b|-1 ==> |a| <= |b|
    let p = Pred::LengthSubOne("output".to_string(), "input".to_string());
    let q = Pred::LengthLe("output".to_string(), "input".to_string());
    assert!(implies(&p, &q), "Z3 native should prove |a|=|b|-1 ==> |a|<=|b|");
}

// ============================================================================
// 7. Plugin integration tests
// ============================================================================

#[test]
fn test_refinement_core_plugin_metadata() {
    use handl_platform::{CapabilityId, Plugin};
    let plugin = crate::plugin::RefinementCore::new();
    let meta = plugin
        .metadata(&CapabilityId::from("refine"))
        .unwrap();
    assert_eq!(meta["command"], "refine");
    assert!(meta["description"].is_string());
}

#[test]
fn test_refinement_core_plugin_refine() {
    use handl_platform::{CapabilityId, CapabilityRequest, KitRegistry, Plugin, PluginContext, PluginRegistry};
    let plugin = crate::plugin::RefinementCore::new();
    let scheme = arr("sort");
    let payload = serde_json::to_value(&scheme).unwrap();
    let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
    let mut ctx = PluginContext {
        plugins: &mut plugins,
        kits: &mut kits,
    };
    let resp = plugin
        .run(
            &CapabilityRequest {
                capability: CapabilityId::from("refine"),
                payload,
            },
            &mut ctx,
        )
        .unwrap();
    assert_eq!(resp.0["refined"], serde_json::json!(true));
    assert_eq!(resp.0["guarantee"], "Sorted(output)");
}

#[test]
fn test_basic_refiner_plugin() {
    use handl_platform::{CapabilityId, CapabilityRequest, KitRegistry, Plugin, PluginContext, PluginRegistry};
    let plugin = crate::plugin::BasicRefiner::new();
    let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
    let mut ctx = PluginContext {
        plugins: &mut plugins,
        kits: &mut kits,
    };
    let resp = plugin
        .run(
            &CapabilityRequest {
                capability: CapabilityId::from("refine"),
                payload: serde_json::Value::Null,
            },
            &mut ctx,
        )
        .unwrap();
    assert_eq!(resp.0, serde_json::json!({"refined": true}));
}

#[test]
fn test_basic_refiner_rejects_unknown_capability() {
    use handl_platform::{CapabilityId, CapabilityRequest, KitRegistry, Plugin, PluginContext, PluginRegistry};
    let plugin = crate::plugin::BasicRefiner::new();
    let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
    let mut ctx = PluginContext {
        plugins: &mut plugins,
        kits: &mut kits,
    };
    let err = plugin
        .run(
            &CapabilityRequest {
                capability: CapabilityId::from("fmt"),
                payload: serde_json::Value::Null,
            },
            &mut ctx,
        )
        .unwrap_err();
    assert!(matches!(err, handl_platform::PlatformError::UnknownCapability(_)));
}

// ============================================================================
// 8. Contract registry tests
// ============================================================================

#[test]
fn test_contract_registry_standard() {
    let reg = ContractRegistry::standard();
    // sort should be registered
    let c = reg.lookup("sort", &ArrSource::Native);
    assert!(c.is_some());
    let c = c.unwrap();
    assert_eq!(c.guarantee, Pred::Sorted("output".to_string()));

    // consume_one should have decrements_length effect
    let c = reg.lookup("consume_one", &ArrSource::Native);
    assert!(c.is_some());
    let c = c.unwrap();
    assert_eq!(
        c.termination_effect,
        Some("decrements_length".to_string())
    );

    // Unknown arrow should return None
    assert!(reg.lookup("nonexistent", &ArrSource::Native).is_none());
}

#[test]
fn test_contract_registry_custom() {
    let mut reg = ContractRegistry::new();
    reg.register(
        "custom_op",
        ArrSource::Native,
        crate::types::Contract {
            input: RefTy::Plain(BaseTy::Int),
            output: RefTy::Plain(BaseTy::Int),
            precondition: Pred::True,
            guarantee: Pred::True,
            termination_effect: Some("none".to_string()),
        },
    );
    assert!(reg.contains("custom_op", &ArrSource::Native));
    assert!(!reg.contains("other_op", &ArrSource::Native));
}

// ============================================================================
// 9. Conditional contract tests (precondition field)
// ============================================================================

#[test]
fn test_contract_conditional_constructor() {
    use crate::types::Contract;
    // A conditional contract: guarantee Sorted(output) only when the input
    // list is already sorted.
    let c = Contract::conditional(
        RefTy::List(Box::new(RefTy::Plain(BaseTy::Var("a".to_string())))),
        RefTy::Refine(
            BaseTy::Var("a".to_string()),
            Pred::Sorted("output".to_string()),
        ),
        Pred::Sorted("input".to_string()),
        Pred::Sorted("output".to_string()),
        Some("none".to_string()),
    );
    assert_eq!(c.precondition, Pred::Sorted("input".to_string()));
    assert_eq!(c.guarantee, Pred::Sorted("output".to_string()));
}

#[test]
fn test_contract_new_constructor_unconditional() {
    use crate::types::Contract;
    // The `new` constructor produces an unconditional contract (precondition True).
    let c = Contract::new(
        RefTy::Plain(BaseTy::Int),
        RefTy::Plain(BaseTy::Int),
        Pred::True,
        None,
    );
    assert_eq!(c.precondition, Pred::True);
}

#[test]
fn test_conditional_contract_inference_optimistic() {
    // Register a conditional contract (non-trivial precondition) and verify
    // the inference optimistically propagates the guarantee, letting the
    // fixpoint iteration and WP computation determine whether the
    // precondition is actually met.
    let mut reg = ContractRegistry::new();
    reg.register(
        "cond_sort",
        ArrSource::Native,
        crate::types::Contract::conditional(
            RefTy::List(Box::new(RefTy::Plain(BaseTy::Var("a".to_string())))),
            RefTy::Refine(
                BaseTy::Var("a".to_string()),
                Pred::Sorted("output".to_string()),
            ),
            Pred::Sorted("input".to_string()), // non-trivial precondition
            Pred::Sorted("output".to_string()),
            Some("none".to_string()),
        ),
    );
    let g = arr("cond_sort");
    let result = infer_refinement(&g, &reg).unwrap();
    // Optimistic: the guarantee is propagated as-is.
    assert_eq!(result.guarantee, Pred::Sorted("output".to_string()));
}

#[test]
fn test_unconditional_contract_inference_uses_guarantee() {
    // An unconditional contract (precondition True) still uses its guarantee.
    let mut reg = ContractRegistry::new();
    reg.register(
        "uncond_sort",
        ArrSource::Native,
        crate::types::Contract::new(
            RefTy::List(Box::new(RefTy::Plain(BaseTy::Var("a".to_string())))),
            RefTy::Refine(
                BaseTy::Var("a".to_string()),
                Pred::Sorted("output".to_string()),
            ),
            Pred::Sorted("output".to_string()),
            Some("none".to_string()),
        ),
    );
    let g = arr("uncond_sort");
    let result = infer_refinement(&g, &reg).unwrap();
    assert_eq!(result.guarantee, Pred::Sorted("output".to_string()));
}

// ============================================================================
// 10. Minimal registry tests (3 atoms)
// ============================================================================

#[test]
fn test_contract_registry_minimal_has_three_atoms() {
    let reg = ContractRegistry::minimal();
    // 3 prim atoms + list atoms (cons, head, tail, is_empty) + id + cons alias
    assert!(reg.len() >= 3, "minimal registry must contain at least 3 atoms, got {}", reg.len());
    assert!(!reg.is_empty());
}

#[test]
fn test_contract_registry_minimal_atoms() {
    let reg = ContractRegistry::minimal();
    // prim.add : (Int * Int) -> Int
    let c = reg.lookup("prim.add", &ArrSource::Native);
    assert!(c.is_some(), "prim.add must be registered");
    let c = c.unwrap();
    assert_eq!(c.precondition, Pred::True);
    assert_eq!(c.guarantee, Pred::True);
    assert_eq!(c.termination_effect, None);

    // prim.compare : (a * b) -> Int
    let c = reg.lookup("prim.compare", &ArrSource::Native);
    assert!(c.is_some(), "prim.compare must be registered");
    let c = c.unwrap();
    assert_eq!(c.precondition, Pred::True);

    // prim.alloc : Int -> List a, guarantee LengthEq(output, 0)
    let c = reg.lookup("prim.alloc", &ArrSource::Native);
    assert!(c.is_some(), "prim.alloc must be registered");
    let c = c.unwrap();
    assert_eq!(
        c.guarantee,
        Pred::LengthEq("output".to_string(), "0".to_string())
    );

    // Higher-level ops from the legacy registry are NOT present.
    assert!(reg.lookup("sort", &ArrSource::Native).is_none());
    assert!(reg.lookup("insert", &ArrSource::Native).is_none());
    assert!(reg.lookup("map", &ArrSource::Native).is_none());
}

#[test]
fn test_contract_registry_minimal_inference_prim_alloc() {
    let reg = ContractRegistry::minimal();
    let g = arr("prim.alloc");
    let result = infer_refinement(&g, &reg).unwrap();
    // prim.alloc has an unconditional precondition, so its guarantee is used.
    assert_eq!(
        result.guarantee,
        Pred::LengthEq("output".to_string(), "0".to_string())
    );
}

// ============================================================================
// 11. Choice WP branching tests
// ============================================================================

#[test]
fn test_weakest_precondition_choice_branches_both_arms() {
    // wp(choice(sort, id), Sorted) = And(wp(sort, Sorted), wp(id, Sorted))
    //   = And(True, Sorted) = Sorted (after simplify)
    let registry = ContractRegistry::standard();
    let post = Pred::Sorted("output".to_string());
    let wp = weakest_precondition(&choice(arr("sort"), arr("id")), &registry, &post);
    // wp(sort, Sorted) = True (guarantee implies post)
    // wp(id, Sorted) = Sorted (identity)
    // And(True, Sorted) simplifies to Sorted.
    assert_eq!(wp.simplify(), post);
}

#[test]
fn test_weakest_precondition_choice_both_arms_contribute() {
    // wp(choice(map, sort), Sorted):
    //   wp(map, Sorted) = True  (map guarantee LengthEq doesn't imply Sorted -> conservative)
    //   wp(sort, Sorted) = True  (sort guarantee Sorted implies Sorted -> precondition True)
    //   = And(True, True) = True after simplify
    //
    // With the new WP rule, both arms return True (one conservatively, one
    // via precondition), so the conjunction simplifies to True.
    let registry = ContractRegistry::standard();
    let post = Pred::Sorted("output".to_string());
    let wp = weakest_precondition(&choice(arr("map"), arr("sort")), &registry, &post);
    let simplified = wp.simplify();
    // Both arms return True, so the result is True.
    assert_eq!(simplified, Pred::True);
}

#[test]
fn test_weakest_precondition_choice_id_id_is_post() {
    // wp(choice(id, id), I) = And(I, I). The toy simplify does not dedupe
    // identical conjuncts, so the result is And(post, post) -- both arms
    // contribute symmetrically.
    let registry = ContractRegistry::standard();
    let post = Pred::Sorted("output".to_string());
    let wp = weakest_precondition(&choice(arr("id"), arr("id")), &registry, &post);
    assert_eq!(wp, Pred::And(Box::new(post.clone()), Box::new(post)));
}

// ============================================================================
// 12. Fixpoint discovery with Choice branching
// ============================================================================

#[test]
fn test_fixpoint_choice_branching_converges() {
    // loop(choice(id, id)) -- both arms are identity, so the invariant is True.
    let registry = ContractRegistry::standard();
    let body = choice(arr("id"), arr("id"));
    let (inv, how, verified) = discover_invariant_fixpoint(&body, &registry);
    // wp(choice(id, id), True) = And(True, True), which simplifies to True,
    // so the fixpoint is True at iteration 0.
    // The toy checker now simplifies both sides before checking, so
    // `True ==> And(True, True)` is recognised (both simplify to True).
    assert_eq!(inv, Pred::True);
    assert!(how.contains("fixpoint"));
    assert!(verified, "True ==> And(True, True) should be verified after simplify fix");
}

#[test]
fn test_fixpoint_choice_with_guarantee_converges() {
    // loop(choice(sort, id)) -- sort has guarantee Sorted, id is identity.
    // wp(choice(sort, id), True) = And(wp(sort, True), wp(id, True))
    //   = And(True, True) = True -> fixpoint at 0.
    let registry = ContractRegistry::standard();
    let body = choice(arr("sort"), arr("id"));
    let (inv, _how, verified) = discover_invariant_fixpoint(&body, &registry);
    assert_eq!(inv, Pred::True);
    assert!(verified, "invariant should be verified after simplify fix");
}

#[test]
fn test_fixpoint_choice_real_iteration_trace() {
    // Verify the real fixpoint iteration branches through both arms and
    // produces a non-empty trace that converges.
    let registry = ContractRegistry::standard();
    let body = choice(arr("sort"), arr("id"));
    let (inv, how, trace, verified) =
        discover_invariant_fixpoint_real(&body, &registry, &Pred::True, 20);
    // Converges to True. The toy checker now simplifies both sides before
    // checking, so `True ==> And(True, True)` is recognised as valid.
    assert_eq!(inv, Pred::True);
    assert!(how.contains("converged"));
    assert!(!trace.is_empty());
    assert!(verified, "invariant should be verified after simplify fix");
}

#[test]
fn test_fixpoint_choice_in_seq_with_consume() {
    // loop(seq(choice(append_to_output, id), consume_one))
    // This mirrors the filter pattern: a conditional append followed by a
    // length-decrementing consume. The Choice WP must branch through both
    // arms and still converge.
    let registry = ContractRegistry::standard();
    let body = seq(choice(arr("append_to_output"), arr("id")), arr("consume_one"));
    let (inv, how, verified) = discover_invariant_fixpoint(&body, &registry);
    // All guarantees are True, so the fixpoint is True. The toy checker now
    // simplifies both sides, so `True ==> And(True, True)` is recognised.
    assert_eq!(inv, Pred::True);
    assert!(how.contains("fixpoint"));
    assert!(verified, "invariant should be verified after simplify fix");
}

// ============================================================================
// 7. Length-arithmetic strengthening (simplify_with_arithmetic)
// ============================================================================
//
// These tests exercise the arithmetic strengthening that lets the toy checker
// prove implications involving length transitivity and cancellation, which the
// pure structural checker cannot handle.

#[test]
fn test_implies_length_plus_one_and_sub_one_cancellation() {
    // LengthPlusOne(a,b) && LengthSubOne(b,c) ==> LengthEq(a,c)
    // |a| = |b| + 1, |b| = |c| - 1  ->  |a| = |c|
    let p = Pred::And(
        Box::new(Pred::LengthPlusOne("a".to_string(), "b".to_string())),
        Box::new(Pred::LengthSubOne("b".to_string(), "c".to_string())),
    );
    let q = Pred::LengthEq("a".to_string(), "c".to_string());
    assert!(implies(&p, &q), "LengthPlusOne(a,b) && LengthSubOne(b,c) ==> LengthEq(a,c)");
}

#[test]
fn test_implies_length_sub_one_and_plus_one_cancellation() {
    // LengthSubOne(a,b) && LengthPlusOne(b,c) ==> LengthEq(a,c)
    // |a| = |b| - 1, |b| = |c| + 1  ->  |a| = |c|
    let p = Pred::And(
        Box::new(Pred::LengthSubOne("a".to_string(), "b".to_string())),
        Box::new(Pred::LengthPlusOne("b".to_string(), "c".to_string())),
    );
    let q = Pred::LengthEq("a".to_string(), "c".to_string());
    assert!(implies(&p, &q), "LengthSubOne(a,b) && LengthPlusOne(b,c) ==> LengthEq(a,c)");
}

#[test]
fn test_implies_length_eq_transitivity() {
    // LengthEq(a,b) && LengthEq(b,c) ==> LengthEq(a,c)
    let p = Pred::And(
        Box::new(Pred::LengthEq("a".to_string(), "b".to_string())),
        Box::new(Pred::LengthEq("b".to_string(), "c".to_string())),
    );
    let q = Pred::LengthEq("a".to_string(), "c".to_string());
    assert!(implies(&p, &q), "LengthEq(a,b) && LengthEq(b,c) ==> LengthEq(a,c)");
}

#[test]
fn test_implies_length_eq_symmetry() {
    // LengthEq(a,b) ==> LengthEq(b,a)
    let p = Pred::LengthEq("a".to_string(), "b".to_string());
    let q = Pred::LengthEq("b".to_string(), "a".to_string());
    assert!(implies(&p, &q), "LengthEq(a,b) ==> LengthEq(b,a)");
}

#[test]
fn test_implies_length_eq_chain_three() {
    // LengthEq(a,b) && LengthEq(b,c) && LengthEq(c,d) ==> LengthEq(a,d)
    let p = Pred::And(
        Box::new(Pred::And(
            Box::new(Pred::LengthEq("a".to_string(), "b".to_string())),
            Box::new(Pred::LengthEq("b".to_string(), "c".to_string())),
        )),
        Box::new(Pred::LengthEq("c".to_string(), "d".to_string())),
    );
    let q = Pred::LengthEq("a".to_string(), "d".to_string());
    assert!(implies(&p, &q), "LengthEq chain of 3 ==> LengthEq(a,d)");
}

#[test]
fn test_implies_length_plus_one_implies_length_le_via_arithmetic() {
    // LengthPlusOne(a,b) ==> LengthLe(b,a)  (|a| = |b|+1 ==> |b| <= |a|)
    // Already handled by toy_implies directly, but verify arithmetic path too.
    let p = Pred::LengthPlusOne("a".to_string(), "b".to_string());
    let q = Pred::LengthLe("b".to_string(), "a".to_string());
    assert!(implies(&p, &q));
}

#[test]
fn test_implies_length_plus_one_transitivity_to_length_le() {
    // LengthPlusOne(a,b) && LengthPlusOne(b,c) ==> LengthLe(a,c)
    // |a| = |b|+1, |b| = |c|+1  ->  |a| = |c|+2  ->  |c| <= |a| and |a| > |c|
    // So LengthLe(c, a) holds, but LengthLe(a, c) does NOT.
    let p = Pred::And(
        Box::new(Pred::LengthPlusOne("a".to_string(), "b".to_string())),
        Box::new(Pred::LengthPlusOne("b".to_string(), "c".to_string())),
    );
    let q = Pred::LengthLe("c".to_string(), "a".to_string());
    assert!(implies(&p, &q), "two LengthPlusOne ==> LengthLe(c, a)");
}

#[test]
fn test_simplify_with_arithmetic_derives_length_eq() {
    // Direct unit test: simplify_with_arithmetic should derive LengthEq(a,c)
    // from LengthPlusOne(a,b) && LengthSubOne(b,c).
    let p = Pred::And(
        Box::new(Pred::LengthPlusOne("a".to_string(), "b".to_string())),
        Box::new(Pred::LengthSubOne("b".to_string(), "c".to_string())),
    );
    let strengthened = simplify_with_arithmetic(&p);
    // The strengthened predicate must imply LengthEq(a,c) via the toy checker.
    assert!(
        toy_implies(&strengthened, &Pred::LengthEq("a".to_string(), "c".to_string())),
        "simplify_with_arithmetic should derive LengthEq(a,c)"
    );
}

#[test]
fn test_simplify_with_arithmetic_no_false_derivations() {
    // Soundness: with no connecting facts, nothing should be derived that
    // would let us prove a false implication. LengthEq(a,b) alone must NOT
    // imply LengthEq(a,c) for an unrelated c.
    let p = Pred::LengthEq("a".to_string(), "b".to_string());
    let strengthened = simplify_with_arithmetic(&p);
    assert!(
        !toy_implies(&strengthened, &Pred::LengthEq("a".to_string(), "c".to_string())),
        "simplify_with_arithmetic must not derive spurious LengthEq(a,c)"
    );
}

#[test]
fn test_simplify_with_arithmetic_preserves_non_arithmetic() {
    // A predicate with no length facts should be returned unchanged (up to
    // simplification).
    let p = Pred::Sorted("output".to_string());
    let strengthened = simplify_with_arithmetic(&p);
    assert_eq!(strengthened, p);
}

#[test]
fn test_implies_length_sub_one_chain_to_length_le() {
    // LengthSubOne(a,b) && LengthSubOne(b,c) ==> LengthLe(a,c)
    // |a| = |b|-1, |b| = |c|-1  ->  |a| = |c|-2  ->  |a| <= |c|
    let p = Pred::And(
        Box::new(Pred::LengthSubOne("a".to_string(), "b".to_string())),
        Box::new(Pred::LengthSubOne("b".to_string(), "c".to_string())),
    );
    let q = Pred::LengthLe("a".to_string(), "c".to_string());
    assert!(implies(&p, &q), "two LengthSubOne ==> LengthLe(a, c)");
}

