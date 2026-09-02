//! Tests for the trait inference module (Phase 9 — TraitInference.lean port).
//!
//! These tests verify that the Rust `trait_inference` module correctly
//! mirrors the Lean4 `TraitInference.lean` formalization:
//! - `uses_first` recognizes graphs that use the `first` combinator.
//! - `uses_left` recognizes graphs that use the `left` (coproduct functor)
//!   combinator.
//! - `satisfies_law` correctly maps structural patterns to law satisfaction.
//! - `inferred_trait` returns the correct set of laws for a graph.
//! - Graphs using neither combinator are not recognized.

use handl_language::Scheme;

use crate::trait_inference::{
    infer_law_annotations, inferred_trait, law_verified, satisfies_law, uses_first, uses_left, Law,
};

// ============================================================================
// Helpers: build Scheme graphs matching the Lean4 graph constructors
// ============================================================================

/// `id` — the identity arrow.
fn id() -> Scheme {
    Scheme::arr_native("id", vec![])
}

/// `first(F)` — the product-functor combinator applied to `F`.
fn first(inner: Scheme) -> Scheme {
    Scheme::First {
        inner: Box::new(inner),
    }
}

/// `left(F)` = `choice(F, id)` — the coproduct-functor combinator. In the
/// Rust IR, `left` is represented as `Choice` with the right branch being
/// the identity (pass-through).
fn left(inner: Scheme) -> Scheme {
    Scheme::Choice {
        l: Box::new(inner),
        r: Box::new(id()),
    }
}

/// `comp(G, H)` — sequential composition.
fn comp(g: Scheme, h: Scheme) -> Scheme {
    Scheme::Seq {
        fst: Box::new(g),
        snd: Box::new(h),
    }
}

/// `loop(G)` — feedback loop.
fn loop_fb(body: Scheme) -> Scheme {
    Scheme::Loop {
        body: Box::new(body),
    }
}

/// A general `choice(G, H)` where neither branch is necessarily identity
/// (NOT a `left` — this is a genuine conditional).
fn choice(g: Scheme, h: Scheme) -> Scheme {
    Scheme::Choice {
        l: Box::new(g),
        r: Box::new(h),
    }
}

// ============================================================================
// `uses_first` tests (mirrors Lean4 `UsesFirst`)
// ============================================================================

#[test]
fn test_uses_first_direct() {
    // first(id) — uses first directly (the `here` case).
    let g = first(id());
    assert!(uses_first(&g));
}

#[test]
fn test_uses_first_in_left_of_comp() {
    // comp(first(id), id) — uses first in the left component.
    let g = comp(first(id()), id());
    assert!(uses_first(&g));
}

#[test]
fn test_uses_first_in_right_of_comp() {
    // comp(id, first(id)) — uses first in the right component.
    let g = comp(id(), first(id()));
    assert!(uses_first(&g));
}

#[test]
fn test_uses_first_nested_in_loop() {
    // loop(first(id)) — uses first inside a loop.
    let g = loop_fb(first(id()));
    assert!(uses_first(&g));
}

#[test]
fn test_uses_first_nested_in_choice() {
    // choice(first(id), id) — uses first in the left branch.
    let g = choice(first(id()), id());
    assert!(uses_first(&g));
}

#[test]
fn test_uses_first_deeply_nested() {
    // comp(loop(first(id)), first(comp(id, id))) — first appears in both
    // halves, at different depths.
    let g = comp(
        loop_fb(first(id())),
        first(comp(id(), id())),
    );
    assert!(uses_first(&g));
}

#[test]
fn test_uses_first_absent_in_plain_arr() {
    // id — no first.
    let g = id();
    assert!(!uses_first(&g));
}

#[test]
fn test_uses_first_absent_in_comp_without_first() {
    // comp(id, id) — no first.
    let g = comp(id(), id());
    assert!(!uses_first(&g));
}

#[test]
fn test_uses_first_absent_in_loop_without_first() {
    // loop(id) — no first.
    let g = loop_fb(id());
    assert!(!uses_first(&g));
}

#[test]
fn test_uses_first_absent_in_choice_without_first() {
    // choice(id, id) — no first (this is left(id), but not first).
    let g = choice(id(), id());
    assert!(!uses_first(&g));
}

// ============================================================================
// `uses_left` tests (mirrors Lean4 `UsesLeft`)
// ============================================================================

#[test]
fn test_uses_left_direct() {
    // left(id) = choice(id, id) — uses left directly.
    let g = left(id());
    assert!(uses_left(&g));
}

#[test]
fn test_uses_left_with_non_identity_inner() {
    // left(cons) = choice(cons, id) — uses left with a non-identity inner.
    let g = left(Scheme::arr_native("cons", vec![]));
    assert!(uses_left(&g));
}

#[test]
fn test_uses_left_in_left_of_comp() {
    // comp(left(id), id) — uses left in the left component.
    let g = comp(left(id()), id());
    assert!(uses_left(&g));
}

#[test]
fn test_uses_left_in_right_of_comp() {
    // comp(id, left(id)) — uses left in the right component.
    let g = comp(id(), left(id()));
    assert!(uses_left(&g));
}

#[test]
fn test_uses_left_nested_in_loop() {
    // loop(left(id)) — uses left inside a loop.
    let g = loop_fb(left(id()));
    assert!(uses_left(&g));
}

#[test]
fn test_uses_left_nested_in_first() {
    // first(left(id)) — uses left inside a first.
    let g = first(left(id()));
    assert!(uses_left(&g));
}

#[test]
fn test_uses_left_absent_in_plain_arr() {
    // id — no left.
    let g = id();
    assert!(!uses_left(&g));
}

#[test]
fn test_uses_left_absent_in_comp_without_left() {
    // comp(id, id) — no left.
    let g = comp(id(), id());
    assert!(!uses_left(&g));
}

#[test]
fn test_uses_left_absent_in_first_without_left() {
    // first(id) — no left (this is a product functor, not a coproduct functor).
    let g = first(id());
    assert!(!uses_left(&g));
}

#[test]
fn test_uses_left_absent_in_general_choice() {
    // choice(cons, cons) — a general choice (conditional), NOT a left,
    // because neither branch is an identity. This should NOT be recognized
    // as using left.
    let g = choice(
        Scheme::arr_native("cons", vec![]),
        Scheme::arr_native("cons", vec![]),
    );
    assert!(!uses_left(&g));
}

// ============================================================================
// `satisfies_law` tests (mirrors Lean4 `SatisfiesLaw`)
// ============================================================================

#[test]
fn test_satisfies_functor_identity_with_first() {
    // first(id) uses first → satisfies FunctorIdentity.
    let g = first(id());
    assert!(satisfies_law(&g, Law::FunctorIdentity));
}

#[test]
fn test_satisfies_functor_composition_with_first() {
    // first(id) uses first → satisfies FunctorComposition.
    let g = first(id());
    assert!(satisfies_law(&g, Law::FunctorComposition));
}

#[test]
fn test_satisfies_functor_laws_with_comp_first() {
    // comp(first(id), first(id)) uses first → satisfies both functor laws.
    let g = comp(first(id()), first(id()));
    assert!(satisfies_law(&g, Law::FunctorIdentity));
    assert!(satisfies_law(&g, Law::FunctorComposition));
}

#[test]
fn test_does_not_satisfy_coproduct_with_first() {
    // first(id) does not use left → does not satisfy CoproductIdentity.
    let g = first(id());
    assert!(!satisfies_law(&g, Law::CoproductIdentity));
    assert!(!satisfies_law(&g, Law::CoproductComposition));
}

#[test]
fn test_satisfies_coproduct_identity_with_left() {
    // left(id) uses left → satisfies CoproductIdentity.
    let g = left(id());
    assert!(satisfies_law(&g, Law::CoproductIdentity));
}

#[test]
fn test_satisfies_coproduct_composition_with_left() {
    // left(id) uses left → satisfies CoproductComposition.
    let g = left(id());
    assert!(satisfies_law(&g, Law::CoproductComposition));
}

#[test]
fn test_does_not_satisfy_functor_with_left() {
    // left(id) does not use first → does not satisfy FunctorIdentity.
    let g = left(id());
    assert!(!satisfies_law(&g, Law::FunctorIdentity));
    assert!(!satisfies_law(&g, Law::FunctorComposition));
}

#[test]
fn test_satisfies_both_with_first_and_left() {
    // comp(first(id), left(id)) uses both first and left → satisfies all
    // four laws.
    let g = comp(first(id()), left(id()));
    assert!(satisfies_law(&g, Law::FunctorIdentity));
    assert!(satisfies_law(&g, Law::FunctorComposition));
    assert!(satisfies_law(&g, Law::CoproductIdentity));
    assert!(satisfies_law(&g, Law::CoproductComposition));
}

#[test]
fn test_satisfies_no_law_with_neither() {
    // id uses neither first nor left → satisfies no laws.
    let g = id();
    assert!(!satisfies_law(&g, Law::FunctorIdentity));
    assert!(!satisfies_law(&g, Law::FunctorComposition));
    assert!(!satisfies_law(&g, Law::CoproductIdentity));
    assert!(!satisfies_law(&g, Law::CoproductComposition));
}

#[test]
fn test_satisfies_no_law_with_comp_id() {
    // comp(id, id) uses neither → satisfies no laws.
    let g = comp(id(), id());
    for law in Law::all() {
        assert!(!satisfies_law(&g, *law), "should not satisfy {:?}", law);
    }
}

#[test]
fn test_satisfies_no_law_with_loop_id() {
    // loop(id) uses neither → satisfies no laws.
    let g = loop_fb(id());
    for law in Law::all() {
        assert!(!satisfies_law(&g, *law), "should not satisfy {:?}", law);
    }
}

// ============================================================================
// `inferred_trait` tests (mirrors Lean4 `inferred_trait`)
// ============================================================================

#[test]
fn test_inferred_trait_first() {
    // first(id) → functor laws only.
    let g = first(id());
    let laws = inferred_trait(&g);
    assert_eq!(laws.len(), 2);
    assert!(laws.contains(&Law::FunctorIdentity));
    assert!(laws.contains(&Law::FunctorComposition));
    assert!(!laws.contains(&Law::CoproductIdentity));
    assert!(!laws.contains(&Law::CoproductComposition));
}

#[test]
fn test_inferred_trait_left() {
    // left(id) → coproduct functor laws only.
    let g = left(id());
    let laws = inferred_trait(&g);
    assert_eq!(laws.len(), 2);
    assert!(laws.contains(&Law::CoproductIdentity));
    assert!(laws.contains(&Law::CoproductComposition));
    assert!(!laws.contains(&Law::FunctorIdentity));
    assert!(!laws.contains(&Law::FunctorComposition));
}

#[test]
fn test_inferred_trait_both() {
    // comp(first(id), left(id)) → all four laws.
    let g = comp(first(id()), left(id()));
    let laws = inferred_trait(&g);
    assert_eq!(laws.len(), 4);
    for law in Law::all() {
        assert!(laws.contains(law), "should contain {:?}", law);
    }
}

#[test]
fn test_inferred_trait_neither() {
    // id → no laws.
    let g = id();
    let laws = inferred_trait(&g);
    assert!(laws.is_empty());
}

#[test]
fn test_inferred_trait_comp_id_id() {
    // comp(id, id) → no laws.
    let g = comp(id(), id());
    let laws = inferred_trait(&g);
    assert!(laws.is_empty());
}

#[test]
fn test_inferred_trait_loop_first() {
    // loop(first(id)) → functor laws (first detected inside loop).
    let g = loop_fb(first(id()));
    let laws = inferred_trait(&g);
    assert_eq!(laws.len(), 2);
    assert!(laws.contains(&Law::FunctorIdentity));
    assert!(laws.contains(&Law::FunctorComposition));
}

#[test]
fn test_inferred_trait_real_map_graph() {
    // The real `map` graph: loop(first(fn.apply)).
    // This uses first → functor laws.
    let g = loop_fb(first(Scheme::arr_native("fn.apply", vec![])));
    let laws = inferred_trait(&g);
    assert!(laws.contains(&Law::FunctorIdentity));
    assert!(laws.contains(&Law::FunctorComposition));
}

#[test]
fn test_inferred_trait_real_sort_graph() {
    // The real `sort` graph: loop(comp(insert_loop, first(id))).
    // The `first(id)` in the second position uses first → functor laws.
    let insert_loop = loop_fb(choice(
        Scheme::arr_native("cons", vec![]),
        Scheme::arr_native("cons", vec![]),
    ));
    let g = loop_fb(comp(insert_loop, first(id())));
    let laws = inferred_trait(&g);
    assert!(laws.contains(&Law::FunctorIdentity));
    assert!(laws.contains(&Law::FunctorComposition));
}

// ============================================================================
// `Law` type tests
// ============================================================================

#[test]
fn test_law_all_returns_four_laws() {
    assert_eq!(Law::all().len(), 4);
}

#[test]
fn test_law_name() {
    assert_eq!(Law::FunctorIdentity.name(), "functorId");
    assert_eq!(Law::FunctorComposition.name(), "functorComp");
    assert_eq!(Law::CoproductIdentity.name(), "coproductFunctorId");
    assert_eq!(Law::CoproductComposition.name(), "coproductFunctorComp");
}

#[test]
fn test_law_is_functor_law() {
    assert!(Law::FunctorIdentity.is_functor_law());
    assert!(Law::FunctorComposition.is_functor_law());
    assert!(!Law::CoproductIdentity.is_functor_law());
    assert!(!Law::CoproductComposition.is_functor_law());
}

#[test]
fn test_law_is_coproduct_law() {
    assert!(!Law::FunctorIdentity.is_coproduct_law());
    assert!(!Law::FunctorComposition.is_coproduct_law());
    assert!(Law::CoproductIdentity.is_coproduct_law());
    assert!(Law::CoproductComposition.is_coproduct_law());
}

#[test]
fn test_law_display() {
    assert_eq!(format!("{}", Law::FunctorIdentity), "functorId");
    assert_eq!(format!("{}", Law::CoproductIdentity), "coproductFunctorId");
}

#[test]
fn test_law_equation() {
    assert!(Law::FunctorIdentity.equation().contains("first"));
    assert!(Law::CoproductIdentity.equation().contains("left"));
}

// ============================================================================
// `law_verified` tests
// ============================================================================

#[test]
fn test_all_laws_verified() {
    // All four laws are proven verified in the Lean4 formalization.
    for law in Law::all() {
        assert!(
            law_verified(*law),
            "law {:?} should be verified",
            law
        );
    }
}

// ============================================================================
// `infer_law_annotations` tests (contract strengthening wiring)
// ============================================================================

#[test]
fn test_infer_law_annotations_first() {
    let g = first(id());
    let annotations = infer_law_annotations(&g);
    assert_eq!(annotations.len(), 2);
    assert!(annotations.iter().any(|a| a.law == Law::FunctorIdentity));
    assert!(annotations.iter().any(|a| a.law == Law::FunctorComposition));
    // Each annotation has a non-empty strengthening description.
    for a in &annotations {
        assert!(!a.strengthening.is_empty());
    }
}

#[test]
fn test_infer_law_annotations_left() {
    let g = left(id());
    let annotations = infer_law_annotations(&g);
    assert_eq!(annotations.len(), 2);
    assert!(annotations.iter().any(|a| a.law == Law::CoproductIdentity));
    assert!(
        annotations
            .iter()
            .any(|a| a.law == Law::CoproductComposition)
    );
}

#[test]
fn test_infer_law_annotations_neither() {
    let g = id();
    let annotations = infer_law_annotations(&g);
    assert!(annotations.is_empty());
}

#[test]
fn test_infer_law_annotations_both() {
    let g = comp(first(id()), left(id()));
    let annotations = infer_law_annotations(&g);
    assert_eq!(annotations.len(), 4);
}

#[test]
fn test_law_annotation_strengthening_mentions_combinator() {
    let g = first(id());
    let annotations = infer_law_annotations(&g);
    for a in &annotations {
        // Functor law annotations should mention "first".
        assert!(
            a.strengthening.contains("first"),
            "strengthening should mention 'first': {}",
            a.strengthening
        );
    }

    let g = left(id());
    let annotations = infer_law_annotations(&g);
    for a in &annotations {
        // Coproduct law annotations should mention "left".
        assert!(
            a.strengthening.contains("left"),
            "strengthening should mention 'left': {}",
            a.strengthening
        );
    }
}

// ============================================================================
// Integration: inferred_trait on real algorithm graphs
// ============================================================================

#[test]
fn test_inferred_trait_filter_graph_no_functor_law() {
    // The real `filter` graph: loop(comp(filter_step, id)).
    // This does NOT use first (it uses comp(filter_step, id), not
    // comp(filter_step, first(id))). So it should not satisfy functor laws.
    let g = loop_fb(comp(
        Scheme::arr_native("filter_step", vec![]),
        id(),
    ));
    let laws = inferred_trait(&g);
    // filter does not use first or left → no laws.
    assert!(
        !laws.contains(&Law::FunctorIdentity),
        "filter graph should not satisfy functor identity (no first)"
    );
}

#[test]
fn test_inferred_trait_fold_graph_uses_first() {
    // The real `fold` graph: loop(comp(fold_step, first(id))).
    // The `first(id)` uses first → functor laws.
    let g = loop_fb(comp(
        Scheme::arr_native("fold_step", vec![]),
        first(id()),
    ));
    let laws = inferred_trait(&g);
    assert!(laws.contains(&Law::FunctorIdentity));
    assert!(laws.contains(&Law::FunctorComposition));
}
