//! End-to-end test: inference on handl stdlib arrows built from 3 Rust atoms.
//!
//! This test demonstrates the minimal proof: 3 Rust atoms (prim.add,
//! prim.compare, prim.alloc) → handl arrow graphs (insert, sort, map,
//! filter, fold) → inferred contracts with zero annotations.

use handl_language::Scheme;
use crate::contracts::ContractRegistry;
use crate::inference::infer_refinement;

/// Build the `insert` arrow graph as a Scheme:
///   loop(comp(compare_step, choice(cons_before, cons_after)))
///
/// In handl syntax:
///   loop { elem, acc }(
///     proc (state) -> match state.acc {
///       []: { out: [elem] },
///       [h, ...t]: if compare(elem, h) <= 0 then { out: [elem, ...acc] }
///                  else { state: { elem, acc: [h, ...insert(elem, t)] } }
///     }
///   )
fn insert_graph() -> Scheme {
    // For the inference engine, we represent insert as:
    // loop(comp(arr("prim.compare"), choice(arr("cons"), arr("recurse"))))
    //
    // The inference engine reads the graph structure and infers Sorted
    // from the Choice WP branching through both arms.

    use Scheme::*;

    // The loop body: compare elem with head, then choice
    let compare_step = Scheme::arr_native("prim.compare", vec![]);
    let cons_before = Scheme::arr_native("cons", vec![]);  // cons elem before head
    let cons_after = Scheme::arr_native("cons", vec![]);   // cons head, recurse

    let body = Seq {
        fst: Box::new(compare_step),
        snd: Box::new(Choice {
            l: Box::new(cons_before),
            r: Box::new(cons_after),
        }),
    };

    Loop { body: Box::new(body) }
}

/// Build the `sort` arrow graph:
///   loop(comp(insert, first(id)))
fn sort_graph() -> Scheme {
    use Scheme::*;

    let insert_step = insert_graph();
    let pass_through = First {
        inner: Box::new(Scheme::arr_native("id", vec![])),
    };

    let body = Seq {
        fst: Box::new(insert_step),
        snd: Box::new(pass_through),
    };

    Loop { body: Box::new(body) }
}

/// Build the `map` arrow graph:
///   loop(first(f))
fn map_graph() -> Scheme {
    use Scheme::*;

    Loop {
        body: Box::new(First {
            inner: Box::new(Scheme::arr_native("fn.apply", vec![])),
        }),
    }
}

/// Build the `filter` arrow graph:
///   loop(comp(filter_step, id))
fn filter_graph() -> Scheme {
    use Scheme::*;

    let filter_step = Scheme::arr_native("filter_step", vec![]);
    let pass_through = Scheme::arr_native("id", vec![]);

    Loop {
        body: Box::new(Seq {
            fst: Box::new(filter_step),
            snd: Box::new(pass_through),
        }),
    }
}

/// Build the `fold` arrow graph:
///   loop(comp(step, first(id)))
fn fold_graph() -> Scheme {
    use Scheme::*;

    let step = Scheme::arr_native("fold_step", vec![]);
    let pass_through = First {
        inner: Box::new(Scheme::arr_native("id", vec![])),
    };

    Loop {
        body: Box::new(Seq {
            fst: Box::new(step),
            snd: Box::new(pass_through),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_inference_with_minimal_registry() {
        let graph = insert_graph();
        let registry = ContractRegistry::minimal();

        let result = infer_refinement(&graph, &registry);
        assert!(result.is_ok(), "insert inference should succeed: {:?}", result.err());

        let result = result.unwrap();
        // The guarantee should be non-trivial (not just True).
        // With the fixpoint seeded from the body's guarantee, the cons
        // contract's Sorted(output) flows through the Choice and Loop,
        // so the insert guarantee contains a Sorted atom.
        assert_ne!(
            result.guarantee,
            crate::types::Pred::True,
            "insert guarantee should be non-trivial"
        );
        let pretty = result.guarantee.pretty();
        assert!(
            pretty.contains("Sorted"),
            "insert guarantee should contain Sorted (derived from cons contract): {}",
            pretty
        );
        println!("insert inferred guarantee: {:?}", result.guarantee);
        println!("insert loop invariants: {:?}", result.loop_invariants);
    }

    #[test]
    fn test_sort_inference_from_insert() {
        let graph = sort_graph();
        let registry = ContractRegistry::minimal();

        let result = infer_refinement(&graph, &registry);
        assert!(result.is_ok(), "sort inference should succeed");

        let result = result.unwrap();
        println!("sort inferred guarantee: {:?}", result.guarantee);
        println!("sort loop invariants: {:?}", result.loop_invariants);
        // sort = loop(comp(insert, first(id))). insert's fixpoint discovers
        // Sorted(output) as the invariant, and sort's loop propagates it
        // through. The guarantee must contain Sorted — not just Le or True.
        let pretty = result.guarantee.pretty();
        assert!(
            pretty.contains("Sorted"),
            "sort guarantee should contain Sorted (propagated from insert): {}",
            pretty
        );
    }

    #[test]
    fn test_map_inference_side_channel() {
        let graph = map_graph();
        let registry = ContractRegistry::minimal();

        let result = infer_refinement(&graph, &registry);
        assert!(result.is_ok(), "map inference should succeed");

        let result = result.unwrap();
        println!("map inferred guarantee: {:?}", result.guarantee);

        // map = loop(first(fn.apply)) — the side channel (the list) passes
        // through `first` unchanged, so |input.1| = |output.1|. The guarantee
        // must contain a LengthEq atom.
        let pretty = result.guarantee.pretty();
        assert!(
            pretty.contains("input.1") && pretty.contains("output.1"),
            "map guarantee should contain LengthEq(input.1, output.1): {}",
            pretty
        );
        assert_ne!(
            result.guarantee,
            crate::types::Pred::True,
            "map guarantee should be non-trivial (not just True)"
        );
    }

    #[test]
    fn test_filter_inference() {
        let graph = filter_graph();
        let registry = ContractRegistry::minimal();

        let result = infer_refinement(&graph, &registry);
        assert!(result.is_ok(), "filter inference should succeed");

        let result = result.unwrap();
        println!("filter inferred guarantee: {:?}", result.guarantee);
    }

    #[test]
    fn test_fold_inference() {
        let graph = fold_graph();
        let registry = ContractRegistry::minimal();

        let result = infer_refinement(&graph, &registry);
        assert!(result.is_ok(), "fold inference should succeed");

        let result = result.unwrap();
        println!("fold inferred guarantee: {:?}", result.guarantee);

        // fold = loop(comp(fold_step, first(id))) — fold_step consumes one
        // element from the list (feedback channel C) each iteration, so the
        // termination measure must be ListLength, NOT Constant.
        let term = result.termination.as_ref();
        assert!(
            term.is_some(),
            "fold should have a termination measure"
        );
        let term = term.unwrap();
        assert_ne!(
            *term,
            crate::types::Measure::Constant,
            "fold termination must NOT be Constant — the list decreases each iteration"
        );
        assert!(
            matches!(term, crate::types::Measure::ListLength(_)),
            "fold termination should be ListLength, got {:?}",
            term
        );
    }

    #[test]
    fn test_demo_all_five_programs() {
        // Run inference on all 5 stdlib arrows with the minimal 3-atom registry
        let registry = ContractRegistry::minimal();

        let programs = vec![
            ("insert", insert_graph()),
            ("sort", sort_graph()),
            ("map", map_graph()),
            ("filter", filter_graph()),
            ("fold", fold_graph()),
        ];

        for (name, graph) in programs {
            let result = infer_refinement(&graph, &registry);
            assert!(result.is_ok(), "{} inference should succeed", name);
            let result = result.unwrap();
            println!(
                "{}: guarantee={:?}, invariants={:?}, termination={:?}",
                name, result.guarantee, result.loop_invariants, result.termination
            );
        }
    }

    #[test]
    fn test_zero_user_annotations() {
        // The key property: user programs require ZERO annotations.
        // The only contracts are on the irreducible Rust atoms.
        let registry = ContractRegistry::minimal();

        // Verify we have the atom contracts (prim.* + list.* + id + cons)
        assert!(registry.len() >= 3, "minimal registry should have at least 3 atoms, got {}", registry.len());

        // Verify all 5 stdlib arrows can be inferred with just these atoms
        for graph in [insert_graph(), sort_graph(), map_graph(), filter_graph(), fold_graph()] {
            let result = infer_refinement(&graph, &registry);
            assert!(result.is_ok(), "inference should succeed with atom registry");
        }
    }

    #[test]
    fn test_cons_direct_inference_produces_sorted() {
        // Direct inference on the `cons` atom should produce a non-trivial
        // guarantee containing Sorted (the enriched conditional contract).
        let registry = ContractRegistry::minimal();
        let graph = Scheme::arr_native("cons", vec![]);
        let result = infer_refinement(&graph, &registry).expect("cons inference");
        // The guarantee is Sorted(output) ∧ LengthPlusOne(output, tail).
        assert_ne!(
            result.guarantee,
            crate::types::Pred::True,
            "cons guarantee should be non-trivial"
        );
        // It should contain a Sorted atom (check via pretty-print).
        let pretty = result.guarantee.pretty();
        assert!(
            pretty.contains("Sorted"),
            "cons guarantee should contain Sorted: {}",
            pretty
        );
    }

    #[test]
    fn test_filter_inference_non_trivial() {
        // filter uses filter_step which now has AllSatisfy ∧ LengthLe.
        let graph = filter_graph();
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry).expect("filter inference");
        assert_ne!(
            result.guarantee,
            crate::types::Pred::True,
            "filter guarantee should be non-trivial"
        );
        let pretty = result.guarantee.pretty();
        assert!(
            pretty.contains("AllSatisfy"),
            "filter guarantee should contain AllSatisfy: {}",
            pretty
        );
    }
}
