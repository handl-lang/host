//! End-to-end test: inference on handl stdlib arrows built from 3 Rust atoms.
//!
//! This test demonstrates the minimal proof: 3 Rust atoms (prim.add,
//! prim.compare, prim.alloc) -> handl arrow graphs (insert, sort, map,
//! filter, fold) -> inferred contracts with zero annotations.

use crate::contracts::ContractRegistry;
use crate::inference::infer_refinement;
use handl_language::{ArrSource, Term};
use plugin_engine::TermData;

/// Convert `TermData` back to `Term` (bootstrap compatibility).
fn td(t: TermData) -> Term {
    t.to_term().expect("TermData -> Term conversion")
}

/// Build a native `op` `TermData` (no args, no literal).
fn op(name: &str) -> TermData {
    TermData::op(name, vec![], &ArrSource::Native, None)
}

/// Build the `insert` arrow graph as a Term:
///   loop(comp(compare_step, choice(cons_before, cons_after)))
///
/// In handl syntax:
///   loop { elem, acc }(
///     state ~> match state.acc {
///       []: { out: [elem] },
///       [h, ...t]: if compare(elem, h) <= 0 then { out: [elem, ...acc] }
///                  else { state: { elem, acc: [h, ...insert(elem, t)] } }
///     }
///   )
fn insert_graph() -> Term {
    // For the inference engine, we represent insert as:
    // loop(comp(arr("prim.compare"), choice(arr("cons"), arr("recurse"))))
    //
    // The inference engine reads the graph structure and infers Sorted
    // from the Choice WP branching through both arms.

    // The loop body: compare elem with head, then choice
    let compare_step = op("prim.compare");
    let cons_before = op("cons"); // cons elem before head
    let cons_after = op("cons"); // cons head, recurse

    td(TermData::loop_feedback(TermData::seq(
        compare_step,
        TermData::choice(cons_before, cons_after),
    )))
}

/// Build the `sort` arrow graph:
///   loop(comp(insert, first(id)))
fn sort_graph() -> Term {
    let insert_step = TermData::from_term(&insert_graph());
    let pass_through = TermData::first(op("id"));

    td(TermData::loop_feedback(TermData::seq(
        insert_step,
        pass_through,
    )))
}

/// Build the `map` arrow graph:
///   loop(first(f))
fn map_graph() -> Term {
    td(TermData::loop_feedback(TermData::first(op("fn.apply"))))
}

/// Build the `filter` arrow graph:
///   loop(comp(filter_step, id))
fn filter_graph() -> Term {
    let filter_step = op("filter_step");
    let pass_through = op("id");

    td(TermData::loop_feedback(TermData::seq(
        filter_step,
        pass_through,
    )))
}

/// Build the `fold` arrow graph:
///   loop(fold_step) — returns both channels (element stream + accumulator)
fn fold_graph() -> Term {
    let step = op("fold_step");

    td(TermData::loop_feedback(step))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_inference_with_minimal_registry() {
        let graph = insert_graph();
        let registry = ContractRegistry::minimal();

        let result = infer_refinement(&graph, &registry);
        assert!(
            result.is_ok(),
            "insert inference should succeed: {:?}",
            result.err()
        );

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
        // through. The guarantee must contain Sorted -- not just Le or True.
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

        // map = loop(first(fn.apply)) -- the side channel (the list) passes
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

        // fold = loop(fold_step) -- fold_step consumes one element from the
        // list (feedback channel C) each iteration, so the termination
        // measure must be ListLength, NOT Constant.
        let term = result.termination.as_ref();
        assert!(term.is_some(), "fold should have a termination measure");
        let term = term.unwrap();
        assert_ne!(
            *term,
            crate::types::Measure::Constant,
            "fold termination must NOT be Constant -- the list decreases each iteration"
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
        assert!(
            registry.len() >= 3,
            "minimal registry should have at least 3 atoms, got {}",
            registry.len()
        );

        // Verify all 5 stdlib arrows can be inferred with just these atoms
        for graph in [
            insert_graph(),
            sort_graph(),
            map_graph(),
            filter_graph(),
            fold_graph(),
        ] {
            let result = infer_refinement(&graph, &registry);
            assert!(
                result.is_ok(),
                "inference should succeed with atom registry"
            );
        }
    }

    #[test]
    fn test_cons_direct_inference_produces_sorted() {
        // Direct inference on the `cons` atom should produce a non-trivial
        // guarantee containing Sorted (the enriched conditional contract).
        let registry = ContractRegistry::minimal();
        let graph = td(op("cons"));
        let result = infer_refinement(&graph, &registry).expect("cons inference");
        // The guarantee is Sorted(output) && LengthPlusOne(output, tail).
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
        // filter uses filter_step which now has AllSatisfy && LengthLe.
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

    // ========================================================================
    // IsPerm predicate tests
    // ========================================================================

    #[test]
    fn test_isperm_predicate_parses() {
        // IsPerm(a, b) should parse correctly from a contract file.
        let src = "axiom perm_test : List a -> List a\n  guarantee: IsPerm(output, input)\n";
        let reg = crate::loader::parse_contracts(src).expect("must parse");
        let c = reg
            .lookup("perm_test", &handl_language::ArrSource::Native)
            .expect("perm_test registered");
        assert_eq!(
            c.guarantee,
            crate::types::Pred::IsPerm("output".to_string(), "input".to_string())
        );
    }

    #[test]
    fn test_isperm_reflexivity_toy_implies() {
        // IsPerm(a, b) ==> IsPerm(a, b) -- reflexivity must hold in the toy checker.
        let p = crate::types::Pred::IsPerm("input".to_string(), "output".to_string());
        let q = crate::types::Pred::IsPerm("input".to_string(), "output".to_string());
        assert!(
            crate::smt::toy_implies(&p, &q),
            "toy_implies must prove IsPerm reflexivity"
        );
    }

    #[test]
    fn test_isperm_pretty_print() {
        // IsPerm(a, b) should pretty-print as Perm(a, b).
        let p = crate::types::Pred::IsPerm("input".to_string(), "output".to_string());
        let pretty = p.pretty();
        assert!(
            pretty.contains("Perm(input, output)"),
            "IsPerm pretty-print should be Perm(input, output), got: {}",
            pretty
        );
    }

    #[test]
    fn test_cons_guarantee_contains_isperm() {
        // Direct inference on the `cons` atom should now include IsPerm in
        // its guarantee (alongside Sorted and LengthPlusOne).
        let registry = ContractRegistry::minimal();
        let graph = td(op("cons"));
        let result = infer_refinement(&graph, &registry).expect("cons inference");
        let pretty = result.guarantee.pretty();
        assert!(
            pretty.contains("Perm"),
            "cons guarantee should contain IsPerm (Perm): {}",
            pretty
        );
    }

    #[test]
    fn test_sort_guarantee_contains_isperm() {
        // sort = loop(comp(insert, first(id))). The cons atoms inside insert
        // carry IsPerm(output, output) in their guarantee, and the fixpoint
        // pattern recognizer detects the "output-building loop" pattern
        // (body contains cons) and seeds IsPerm(input, output) as a candidate
        // invariant. This matches the Agda example
        // `S ys. Sorted ys * Permutation xs ys`: sort derives
        // `Sorted(output) && IsPerm(input, output)`.
        let graph = sort_graph();
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry).expect("sort inference");
        let pretty = result.guarantee.pretty();
        println!("sort inferred guarantee (with IsPerm): {}", pretty);
        assert!(
            pretty.contains("Sorted"),
            "sort guarantee should contain Sorted: {}",
            pretty
        );
        // The guarantee must contain IsPerm with non-trivial arguments
        // (input, output) -- not just the trivial IsPerm(output, output).
        assert!(
            pretty.contains("Perm(input, output)"),
            "sort guarantee should contain IsPerm(input, output) (non-trivial): {}",
            pretty
        );
        // Ensure the trivial form is not the ONLY IsPerm present.
        // The guarantee should have IsPerm(input, output), which is
        // semantically meaningful (output is a permutation of the input).
    }

    #[test]
    fn test_isperm_pattern_recognizer_cons_loop() {
        // The pattern recognizer should add IsPerm(input, output) to any
        // loop whose body contains cons (the list constructor). This test
        // verifies the recognizer fires for a simple cons-containing loop.
        let body = TermData::seq(
            op("list.tail"),
            TermData::choice(op("cons"), op("id")),
        );
        let graph = td(TermData::loop_feedback(body));
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry).expect("loop inference");
        let pretty = result.guarantee.pretty();
        println!("pattern recognizer test guarantee: {}", pretty);
        assert!(
            pretty.contains("Perm(input, output)"),
            "cons-containing loop should get IsPerm(input, output) from pattern recognizer: {}",
            pretty
        );
    }

    // ========================================================================
    // Scaled stdlib: new atoms + new arrows (zero annotations)
    // ========================================================================

    /// vec.push = arr("list.cons")
    fn vec_push_graph() -> Term {
        td(op("list.cons"))
    }

    /// vec.pop = arr("list.head")
    fn vec_pop_graph() -> Term {
        td(op("list.head"))
    }

    /// vec.get = comp(arr("list.nth"), first(id))
    fn vec_get_graph() -> Term {
        td(TermData::seq(
            op("list.nth"),
            TermData::first(op("id")),
        ))
    }

    /// queue.enqueue = arr("list.append")
    fn queue_enqueue_graph() -> Term {
        td(op("list.append"))
    }

    /// queue.dequeue = arr("list.head")
    fn queue_dequeue_graph() -> Term {
        td(op("list.head"))
    }

    /// take = loop(comp(arr("list.head"), choice(arr("cons"), arr("id"))))
    fn take_graph() -> Term {
        td(TermData::loop_feedback(TermData::seq(
            op("list.head"),
            TermData::choice(op("cons"), op("id")),
        )))
    }

    /// drop = loop(comp(arr("list.tail"), first(id)))
    fn drop_graph() -> Term {
        td(TermData::loop_feedback(TermData::seq(
            op("list.tail"),
            TermData::first(op("id")),
        )))
    }

    /// reduce = loop(comp(arr("fold_step"), first(id)))
    fn reduce_graph() -> Term {
        td(TermData::loop_feedback(TermData::seq(
            op("fold_step"),
            TermData::first(op("id")),
        )))
    }

    /// scan = loop(comp(arr("fold_step"), first(arr("list.cons"))))
    fn scan_graph() -> Term {
        td(TermData::loop_feedback(TermData::seq(
            op("fold_step"),
            TermData::first(op("list.cons")),
        )))
    }

    /// zip = loop(comp(first(arr("list.head")),
    ///                  comp(first(arr("list.tail")), arr("list.cons"))))
    fn zip_graph() -> Term {
        td(TermData::loop_feedback(TermData::seq(
            TermData::first(op("list.head")),
            TermData::seq(
                TermData::first(op("list.tail")),
                op("list.cons"),
            ),
        )))
    }

    /// reverse = arr("list.reverse") -- direct atom with non-trivial guarantee.
    fn reverse_graph() -> Term {
        td(op("list.reverse"))
    }

    #[test]
    fn test_new_atoms_registered() {
        // All 8 new atoms must be present in the minimal registry.
        let registry = ContractRegistry::minimal();
        use handl_language::ArrSource;
        for op in [
            "prim.sub",
            "prim.mul",
            "prim.le",
            "prim.eq",
            "list.length",
            "list.append",
            "list.reverse",
            "list.nth",
        ] {
            assert!(
                registry.contains(op, &ArrSource::Native),
                "atom `{}` should be registered in minimal()",
                op
            );
        }
    }

    #[test]
    fn test_new_atoms_from_axioms() {
        // The loader (from_axioms) must also parse the new atoms from the
        // bundled .hnl contract files, keeping minimal() and from_axioms()
        // in sync.
        let registry = ContractRegistry::from_axioms();
        use handl_language::ArrSource;
        for op in [
            "prim.sub",
            "prim.mul",
            "prim.le",
            "prim.eq",
            "list.length",
            "list.append",
            "list.reverse",
            "list.nth",
        ] {
            assert!(
                registry.contains(op, &ArrSource::Native),
                "atom `{}` should be parsed by from_axioms()",
                op
            );
        }
    }

    #[test]
    fn test_vec_push_inference() {
        let graph = vec_push_graph();
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry);
        assert!(
            result.is_ok(),
            "vec.push inference should succeed: {:?}",
            result.err()
        );
        let result = result.unwrap();
        // list.cons has a conditional Sorted guarantee -- non-trivial.
        assert_ne!(
            result.guarantee,
            crate::types::Pred::True,
            "vec.push guarantee should be non-trivial (from list.cons)"
        );
        let pretty = result.guarantee.pretty();
        assert!(
            pretty.contains("Sorted"),
            "vec.push guarantee should contain Sorted: {}",
            pretty
        );
    }

    #[test]
    fn test_vec_pop_inference() {
        let graph = vec_pop_graph();
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry);
        assert!(
            result.is_ok(),
            "vec.pop inference should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_vec_get_inference() {
        // vec.get = comp(arr("list.nth"), first(id))
        let graph = vec_get_graph();
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry);
        assert!(
            result.is_ok(),
            "vec.get inference should succeed: {:?}",
            result.err()
        );
        let result = result.unwrap();
        // first(id) preserves the side channel, so LengthEq(input.1, output.1).
        let pretty = result.guarantee.pretty();
        assert!(
            pretty.contains("input.1") && pretty.contains("output.1"),
            "vec.get guarantee should contain LengthEq(input.1, output.1): {}",
            pretty
        );
    }

    #[test]
    fn test_queue_enqueue_inference() {
        let graph = queue_enqueue_graph();
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry);
        assert!(
            result.is_ok(),
            "queue.enqueue inference should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_queue_dequeue_inference() {
        let graph = queue_dequeue_graph();
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry);
        assert!(
            result.is_ok(),
            "queue.dequeue inference should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_take_inference() {
        // take = loop(comp(arr("list.head"), choice(arr("cons"), arr("id"))))
        let graph = take_graph();
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry);
        assert!(
            result.is_ok(),
            "take inference should succeed: {:?}",
            result.err()
        );
        let result = result.unwrap();
        // The Choice arm uses cons which carries Sorted; the loop fixpoint
        // should propagate a non-trivial guarantee.
        println!("take inferred guarantee: {:?}", result.guarantee);
    }

    #[test]
    fn test_drop_inference() {
        // drop = loop(comp(arr("list.tail"), first(id)))
        let graph = drop_graph();
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry);
        assert!(
            result.is_ok(),
            "drop inference should succeed: {:?}",
            result.err()
        );
        let result = result.unwrap();
        // first(id) preserves the side channel: LengthEq(input.1, output.1).
        let pretty = result.guarantee.pretty();
        assert!(
            pretty.contains("input.1") && pretty.contains("output.1"),
            "drop guarantee should contain LengthEq(input.1, output.1): {}",
            pretty
        );
    }

    #[test]
    fn test_reduce_inference() {
        // reduce = loop(comp(arr("fold_step"), first(id)))
        let graph = reduce_graph();
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry);
        assert!(
            result.is_ok(),
            "reduce inference should succeed: {:?}",
            result.err()
        );
        let result = result.unwrap();
        // reduce is structurally identical to fold, so it must terminate with
        // a ListLength measure (fold_step decrements the list).
        let term = result.termination.as_ref();
        assert!(term.is_some(), "reduce should have a termination measure");
        let term = term.unwrap();
        assert!(
            matches!(term, crate::types::Measure::ListLength(_)),
            "reduce termination should be ListLength, got {:?}",
            term
        );
    }

    #[test]
    fn test_scan_inference() {
        // scan = loop(comp(arr("fold_step"), first(arr("list.cons"))))
        let graph = scan_graph();
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry);
        assert!(
            result.is_ok(),
            "scan inference should succeed: {:?}",
            result.err()
        );
        let result = result.unwrap();
        // first(arr("list.cons")) carries the cons Sorted guarantee AND the
        // side-channel LengthEq, so the guarantee should be non-trivial.
        println!("scan inferred guarantee: {:?}", result.guarantee);
        let pretty = result.guarantee.pretty();
        assert!(
            pretty.contains("input.1") && pretty.contains("output.1"),
            "scan guarantee should contain LengthEq(input.1, output.1): {}",
            pretty
        );
    }

    #[test]
    fn test_zip_inference() {
        // zip = loop(comp(first(arr("list.head")),
        //                 comp(first(arr("list.tail")), arr("list.cons"))))
        let graph = zip_graph();
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry);
        assert!(
            result.is_ok(),
            "zip inference should succeed: {:?}",
            result.err()
        );
        let result = result.unwrap();
        println!("zip inferred guarantee: {:?}", result.guarantee);
    }

    #[test]
    fn test_reverse_inference_non_trivial() {
        // list.reverse has a structural LengthEq(output, input) guarantee.
        let graph = reverse_graph();
        let registry = ContractRegistry::minimal();
        let result = infer_refinement(&graph, &registry).expect("reverse inference");
        assert_ne!(
            result.guarantee,
            crate::types::Pred::True,
            "reverse guarantee should be non-trivial"
        );
        let pretty = result.guarantee.pretty();
        // LengthEq pretty-prints as |a| = |b|.
        assert!(
            pretty.contains("|output|") && pretty.contains("|input|"),
            "reverse guarantee should contain LengthEq (|output| = |input|): {}",
            pretty
        );
    }

    #[test]
    fn test_all_new_arrows_infer_with_zero_annotations() {
        // The key scaling property: every new arrow infers successfully with
        // only the atom contracts -- zero user annotations.
        let registry = ContractRegistry::minimal();
        let programs = vec![
            ("vec.push", vec_push_graph()),
            ("vec.pop", vec_pop_graph()),
            ("vec.get", vec_get_graph()),
            ("queue.enqueue", queue_enqueue_graph()),
            ("queue.dequeue", queue_dequeue_graph()),
            ("take", take_graph()),
            ("drop", drop_graph()),
            ("reduce", reduce_graph()),
            ("scan", scan_graph()),
            ("zip", zip_graph()),
            ("reverse", reverse_graph()),
        ];
        for (name, graph) in programs {
            let result = infer_refinement(&graph, &registry);
            assert!(
                result.is_ok(),
                "{} inference should succeed with zero annotations: {:?}",
                name,
                result.err()
            );
        }
    }

    #[test]
    fn test_scaled_stdlib_all_arrows() {
        // Run inference on ALL arrows (original 5 + new 11) to prove the
        // model scales beyond 5 examples.
        let registry = ContractRegistry::minimal();
        let programs = vec![
            ("insert", insert_graph()),
            ("sort", sort_graph()),
            ("map", map_graph()),
            ("filter", filter_graph()),
            ("fold", fold_graph()),
            ("vec.push", vec_push_graph()),
            ("vec.pop", vec_pop_graph()),
            ("vec.get", vec_get_graph()),
            ("queue.enqueue", queue_enqueue_graph()),
            ("queue.dequeue", queue_dequeue_graph()),
            ("take", take_graph()),
            ("drop", drop_graph()),
            ("reduce", reduce_graph()),
            ("scan", scan_graph()),
            ("zip", zip_graph()),
            ("reverse", reverse_graph()),
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
}
