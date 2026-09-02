//! Tests for the handl contract file loader (`loader.rs`).
//!
//! These verify that:
//! - A simple axiom file parses into the correct `Contract`.
//! - Preconditions produce conditional contracts.
//! - Termination effects are parsed correctly.
//! - The bundled standard axiom set loads with the correct count.
//! - A round-trip (parse → registry → `infer_refinement`) works and matches
//!   `minimal()` for inference.
//! - Error cases (missing guarantee, malformed type, unknown clause, etc.) are
//!   reported.

use handl_language::{ArrSource, Scheme};

use crate::contracts::ContractRegistry;
use crate::inference::infer_refinement;
use crate::loader::{load_standard_axioms, parse_contracts};
use crate::types::{BaseTy, Pred, RefTy};

// ============================================================================
// Helpers
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

/// The total number of axioms across the bundled `prim.hnl`, `list.hnl`,
/// `parse.hnl`, and `state.hnl`.
const STANDARD_AXIOM_COUNT: usize = 39;

// ============================================================================
// 1. Parse a simple axiom file → correct Contract
// ============================================================================

#[test]
fn test_parse_simple_axiom() {
    let src = "axiom prim.add : (Int × Int) → Int\n  guarantee: True\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg
        .lookup("prim.add", &ArrSource::Native)
        .expect("prim.add registered");
    assert_eq!(c.input, RefTy::Prod(
        Box::new(RefTy::Plain(BaseTy::Int)),
        Box::new(RefTy::Plain(BaseTy::Int)),
    ));
    assert_eq!(c.output, RefTy::Plain(BaseTy::Int));
    assert_eq!(c.precondition, Pred::True);
    assert_eq!(c.guarantee, Pred::True);
    assert_eq!(c.termination_effect, None);
}

#[test]
fn test_parse_axiom_with_list_type() {
    let src = "axiom list.head : List α → α\n  guarantee: True\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg
        .lookup("list.head", &ArrSource::Native)
        .expect("list.head registered");
    assert_eq!(
        c.input,
        RefTy::List(Box::new(RefTy::Plain(BaseTy::Var("α".to_string()))))
    );
    assert_eq!(c.output, RefTy::Plain(BaseTy::Var("α".to_string())));
}

#[test]
fn test_parse_axiom_with_lengtheq_guarantee() {
    let src =
        "axiom prim.alloc : Int → List α\n  guarantee: LengthEq(output, 0)\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg
        .lookup("prim.alloc", &ArrSource::Native)
        .expect("prim.alloc registered");
    assert_eq!(
        c.guarantee,
        Pred::LengthEq("output".to_string(), "0".to_string())
    );
}

#[test]
fn test_parse_axiom_with_product_input() {
    let src = "axiom cons : (α × List α) → List α\n  guarantee: True\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg
        .lookup("cons", &ArrSource::Native)
        .expect("cons registered");
    assert_eq!(
        c.input,
        RefTy::Prod(
            Box::new(RefTy::Plain(BaseTy::Var("α".to_string()))),
            Box::new(RefTy::List(Box::new(RefTy::Plain(BaseTy::Var(
                "α".to_string()
            )))))
        )
    );
    assert_eq!(
        c.output,
        RefTy::List(Box::new(RefTy::Plain(BaseTy::Var("α".to_string()))))
    );
}

#[test]
fn test_parse_comments_and_blank_lines_ignored() {
    let src = "// a comment\n\naxiom id : α → α\n  // inline comment\n  guarantee: True\n\n// trailing comment\n";
    let reg = parse_contracts(src).expect("must parse");
    assert!(reg.contains("id", &ArrSource::Native));
    assert_eq!(reg.len(), 1);
}

#[test]
fn test_parse_multiple_axioms() {
    let src = "\
axiom prim.add : (Int × Int) → Int
  guarantee: True

axiom prim.compare : (α × β) → Int
  guarantee: True

axiom prim.alloc : Int → List α
  guarantee: LengthEq(output, 0)
";
    let reg = parse_contracts(src).expect("must parse");
    assert_eq!(reg.len(), 3);
    assert!(reg.contains("prim.add", &ArrSource::Native));
    assert!(reg.contains("prim.compare", &ArrSource::Native));
    assert!(reg.contains("prim.alloc", &ArrSource::Native));
}

// ============================================================================
// 2. Parse with precondition → conditional contract
// ============================================================================

#[test]
fn test_parse_axiom_with_precondition() {
    let src = "\
axiom remove_one : List α → List α
  requires: LengthLe(input, 10)
  guarantee: LengthLe(output, input)
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg
        .lookup("remove_one", &ArrSource::Native)
        .expect("remove_one registered");
    assert_eq!(
        c.precondition,
        Pred::LengthLe("input".to_string(), "10".to_string())
    );
    assert_eq!(
        c.guarantee,
        Pred::LengthLe("output".to_string(), "input".to_string())
    );
}

#[test]
fn test_parse_axiom_precondition_defaults_to_true() {
    // No `requires:` clause → precondition should default to True.
    let src = "axiom id : α → α\n  guarantee: True\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("id", &ArrSource::Native).unwrap();
    assert_eq!(c.precondition, Pred::True);
}

#[test]
fn test_parse_axiom_with_conjunctive_guarantee() {
    let src = "\
axiom filter : List α → List α
  guarantee: AllSatisfy(output, predicate) ∧ LengthLe(output, input)
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("filter", &ArrSource::Native).unwrap();
    let expected = Pred::And(
        Box::new(Pred::AllSatisfy("output".to_string(), "predicate".to_string())),
        Box::new(Pred::LengthLe("output".to_string(), "input".to_string())),
    );
    assert_eq!(c.guarantee, expected);
}

#[test]
fn test_parse_axiom_with_implication_guarantee() {
    let src = "\
axiom cond : List α → List α
  guarantee: Sorted(input) ⟹ Sorted(output)
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("cond", &ArrSource::Native).unwrap();
    assert_eq!(
        c.guarantee,
        Pred::Implies(
            Box::new(Pred::Sorted("input".to_string())),
            Box::new(Pred::Sorted("output".to_string())),
        )
    );
}

// ============================================================================
// 3. Parse with termination effect → correct effect
// ============================================================================

#[test]
fn test_parse_axiom_with_termination_decrements_length() {
    let src = "\
axiom consume_one : List α → List α
  guarantee: True
  termination: decrements_length
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("consume_one", &ArrSource::Native).unwrap();
    assert_eq!(
        c.termination_effect.as_deref(),
        Some("decrements_length")
    );
}

#[test]
fn test_parse_axiom_with_termination_decrements_counter() {
    let src = "\
axiom decrement : Int → Int
  guarantee: True
  termination: decrements_counter
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("decrement", &ArrSource::Native).unwrap();
    assert_eq!(
        c.termination_effect.as_deref(),
        Some("decrements_counter")
    );
}

#[test]
fn test_parse_axiom_with_termination_constant() {
    let src = "\
axiom id : α → α
  guarantee: True
  termination: constant
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("id", &ArrSource::Native).unwrap();
    assert_eq!(c.termination_effect.as_deref(), Some("constant"));
}

#[test]
fn test_parse_axiom_with_termination_none() {
    let src = "\
axiom sort : List α → List α
  guarantee: Sorted(output)
  termination: none
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("sort", &ArrSource::Native).unwrap();
    assert_eq!(c.termination_effect.as_deref(), Some("none"));
    assert_eq!(c.guarantee, Pred::Sorted("output".to_string()));
}

#[test]
fn test_parse_axiom_termination_defaults_to_none() {
    // No `termination:` clause → termination_effect should be None.
    let src = "axiom id : α → α\n  guarantee: True\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("id", &ArrSource::Native).unwrap();
    assert_eq!(c.termination_effect, None);
}

// ============================================================================
// 4. Parse all standard axioms → registry has correct count
// ============================================================================

#[test]
fn test_load_standard_axioms_count() {
    let reg = load_standard_axioms();
    assert_eq!(
        reg.len(),
        STANDARD_AXIOM_COUNT,
        "standard axiom set must contain exactly {} axioms",
        STANDARD_AXIOM_COUNT
    );
    assert!(!reg.is_empty());
}

#[test]
fn test_load_standard_axioms_has_prim_atoms() {
    let reg = load_standard_axioms();
    assert!(reg.contains("prim.add", &ArrSource::Native));
    assert!(reg.contains("prim.compare", &ArrSource::Native));
    assert!(reg.contains("prim.alloc", &ArrSource::Native));
}

#[test]
fn test_load_standard_axioms_has_list_atoms() {
    let reg = load_standard_axioms();
    assert!(reg.contains("list.cons", &ArrSource::Native));
    assert!(reg.contains("list.head", &ArrSource::Native));
    assert!(reg.contains("list.tail", &ArrSource::Native));
    assert!(reg.contains("list.is_empty", &ArrSource::Native));
    assert!(reg.contains("cons", &ArrSource::Native));
    assert!(reg.contains("id", &ArrSource::Native));
    assert!(reg.contains("fn.apply", &ArrSource::Native));
    assert!(reg.contains("filter_step", &ArrSource::Native));
    assert!(reg.contains("fold_step", &ArrSource::Native));
}

#[test]
fn test_load_standard_axioms_prim_alloc_guarantee() {
    let reg = load_standard_axioms();
    let c = reg
        .lookup("prim.alloc", &ArrSource::Native)
        .expect("prim.alloc present");
    assert_eq!(
        c.guarantee,
        Pred::LengthEq("output".to_string(), "0".to_string())
    );
    assert_eq!(c.precondition, Pred::True);
}

// ============================================================================
// 4b. Enriched axioms: conditional contracts with real semantic content
// ============================================================================

#[test]
fn test_load_standard_axioms_prim_compare_guarantee() {
    let reg = load_standard_axioms();
    let c = reg
        .lookup("prim.compare", &ArrSource::Native)
        .expect("prim.compare present");
    // Enriched: prim.compare guarantees Le(output, 1).
    assert_eq!(
        c.guarantee,
        Pred::Le("output".to_string(), "1".to_string())
    );
    assert_eq!(c.precondition, Pred::True);
}

#[test]
fn test_load_standard_axioms_cons_conditional_contract() {
    let reg = load_standard_axioms();
    let c = reg
        .lookup("cons", &ArrSource::Native)
        .expect("cons present");
    // Enriched: cons has a conditional contract.
    // requires: Sorted(tail) ∧ Le(head, head_of_tail)
    // guarantee: Sorted(output) ∧ LengthPlusOne(output, tail) ∧ IsPerm(output, output)
    // (Parser is left-associative: A ∧ B ∧ C → And(And(A, B), C))
    assert_eq!(
        c.precondition,
        Pred::And(
            Box::new(Pred::Sorted("tail".to_string())),
            Box::new(Pred::Le("head".to_string(), "head_of_tail".to_string())),
        )
    );
    assert_eq!(
        c.guarantee,
        Pred::And(
            Box::new(Pred::And(
                Box::new(Pred::Sorted("output".to_string())),
                Box::new(Pred::LengthPlusOne("output".to_string(), "tail".to_string())),
            )),
            Box::new(Pred::IsPerm("output".to_string(), "output".to_string())),
        )
    );
}

#[test]
fn test_load_standard_axioms_list_cons_conditional_contract() {
    let reg = load_standard_axioms();
    let c = reg
        .lookup("list.cons", &ArrSource::Native)
        .expect("list.cons present");
    // Same conditional contract as cons.
    // (Parser is left-associative: A ∧ B ∧ C → And(And(A, B), C))
    assert_eq!(
        c.precondition,
        Pred::And(
            Box::new(Pred::Sorted("tail".to_string())),
            Box::new(Pred::Le("head".to_string(), "head_of_tail".to_string())),
        )
    );
    assert_eq!(
        c.guarantee,
        Pred::And(
            Box::new(Pred::And(
                Box::new(Pred::Sorted("output".to_string())),
                Box::new(Pred::LengthPlusOne("output".to_string(), "tail".to_string())),
            )),
            Box::new(Pred::IsPerm("output".to_string(), "output".to_string())),
        )
    );
}

#[test]
fn test_load_standard_axioms_list_tail_conditional_contract() {
    let reg = load_standard_axioms();
    let c = reg
        .lookup("list.tail", &ArrSource::Native)
        .expect("list.tail present");
    // Enriched: list.tail preserves sortedness and decrements length.
    // requires: Sorted(input)
    // guarantee: Sorted(output) ∧ LengthSubOne(output, input) ∧ IsPerm(output, output)
    // (Parser is left-associative: A ∧ B ∧ C → And(And(A, B), C))
    assert_eq!(c.precondition, Pred::Sorted("input".to_string()));
    assert_eq!(
        c.guarantee,
        Pred::And(
            Box::new(Pred::And(
                Box::new(Pred::Sorted("output".to_string())),
                Box::new(Pred::LengthSubOne("output".to_string(), "input".to_string())),
            )),
            Box::new(Pred::IsPerm("output".to_string(), "output".to_string())),
        )
    );
}

#[test]
fn test_load_standard_axioms_filter_step_guarantee() {
    let reg = load_standard_axioms();
    let c = reg
        .lookup("filter_step", &ArrSource::Native)
        .expect("filter_step present");
    // Enriched: filter_step guarantees AllSatisfy ∧ LengthLe.
    assert_eq!(
        c.guarantee,
        Pred::And(
            Box::new(Pred::AllSatisfy("output".to_string(), "predicate".to_string())),
            Box::new(Pred::LengthLe("output".to_string(), "input".to_string())),
        )
    );
    assert_eq!(c.precondition, Pred::True);
}

// ============================================================================
// 4c. New predicate parsing
// ============================================================================

#[test]
fn test_parse_le_predicate() {
    let src = "axiom cmp : (Int × Int) → Int\n  guarantee: Le(output, 1)\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("cmp", &ArrSource::Native).unwrap();
    assert_eq!(
        c.guarantee,
        Pred::Le("output".to_string(), "1".to_string())
    );
}

#[test]
fn test_parse_head_of_predicate() {
    let src = "axiom hd : List α → α\n  guarantee: HeadOf(output, input)\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("hd", &ArrSource::Native).unwrap();
    assert_eq!(
        c.guarantee,
        Pred::HeadOf("output".to_string(), "input".to_string())
    );
}

#[test]
fn test_parse_length_plus_one_predicate() {
    let src = "axiom cons : (α × List α) → List α\n  guarantee: LengthPlusOne(output, tail)\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("cons", &ArrSource::Native).unwrap();
    assert_eq!(
        c.guarantee,
        Pred::LengthPlusOne("output".to_string(), "tail".to_string())
    );
}

#[test]
fn test_parse_length_sub_one_predicate() {
    let src = "axiom tl : List α → List α\n  guarantee: LengthSubOne(output, input)\n";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("tl", &ArrSource::Native).unwrap();
    assert_eq!(
        c.guarantee,
        Pred::LengthSubOne("output".to_string(), "input".to_string())
    );
}

#[test]
fn test_parse_conditional_cons_contract() {
    let src = "\
axiom cons : (α × List α) → List α
  requires: Sorted(tail) ∧ Le(head, head_of_tail)
  guarantee: Sorted(output) ∧ LengthPlusOne(output, tail)
";
    let reg = parse_contracts(src).expect("must parse");
    let c = reg.lookup("cons", &ArrSource::Native).unwrap();
    assert_eq!(
        c.precondition,
        Pred::And(
            Box::new(Pred::Sorted("tail".to_string())),
            Box::new(Pred::Le("head".to_string(), "head_of_tail".to_string())),
        )
    );
    assert_eq!(
        c.guarantee,
        Pred::And(
            Box::new(Pred::Sorted("output".to_string())),
            Box::new(Pred::LengthPlusOne("output".to_string(), "tail".to_string())),
        )
    );
}

#[test]
fn test_from_axioms_matches_load_standard_axioms() {
    // ContractRegistry::from_axioms() must delegate to load_standard_axioms().
    let via_from = ContractRegistry::from_axioms();
    let via_load = load_standard_axioms();
    assert_eq!(via_from.len(), via_load.len());
}

// ============================================================================
// 5. Round-trip: parse → registry → infer_refinement works
// ============================================================================

#[test]
fn test_round_trip_inference_prim_alloc() {
    let reg = load_standard_axioms();
    let g = arr("prim.alloc");
    let result = infer_refinement(&g, &reg).expect("inference must succeed");
    // prim.alloc has an unconditional precondition, so its guarantee is used.
    assert_eq!(
        result.guarantee,
        Pred::LengthEq("output".to_string(), "0".to_string())
    );
}

#[test]
fn test_round_trip_inference_prim_add() {
    let reg = load_standard_axioms();
    let g = arr("prim.add");
    let result = infer_refinement(&g, &reg).expect("inference must succeed");
    assert_eq!(result.guarantee, Pred::True);
}

#[test]
fn test_round_trip_inference_list_head() {
    let reg = load_standard_axioms();
    let g = arr("list.head");
    let result = infer_refinement(&g, &reg).expect("inference must succeed");
    assert_eq!(result.guarantee, Pred::True);
    assert_eq!(
        result.input,
        RefTy::List(Box::new(RefTy::Plain(BaseTy::Var("α".to_string()))))
    );
    assert_eq!(result.output, RefTy::Plain(BaseTy::Var("α".to_string())));
}

#[test]
fn test_from_axioms_matches_minimal_for_inference() {
    // The from_axioms() registry must produce the same inference results as
    // minimal() for the prim.alloc atom (the rule's requirement #4).
    let reg_axioms = ContractRegistry::from_axioms();
    let reg_minimal = ContractRegistry::minimal();

    let g = arr("prim.alloc");
    let res_axioms = infer_refinement(&g, &reg_axioms).expect("axioms inference");
    let res_minimal = infer_refinement(&g, &reg_minimal).expect("minimal inference");
    assert_eq!(res_axioms.guarantee, res_minimal.guarantee);
    assert_eq!(res_axioms.input, res_minimal.input);
    assert_eq!(res_axioms.output, res_minimal.output);
}

#[test]
fn test_from_axioms_matches_minimal_for_prim_add() {
    let reg_axioms = ContractRegistry::from_axioms();
    let reg_minimal = ContractRegistry::minimal();

    let g = arr("prim.add");
    let res_axioms = infer_refinement(&g, &reg_axioms).expect("axioms inference");
    let res_minimal = infer_refinement(&g, &reg_minimal).expect("minimal inference");
    assert_eq!(res_axioms.guarantee, res_minimal.guarantee);
}

#[test]
fn test_from_axioms_and_minimal_have_same_count() {
    let reg_axioms = ContractRegistry::from_axioms();
    let reg_minimal = ContractRegistry::minimal();
    assert_eq!(reg_axioms.len(), reg_minimal.len());
}

// ============================================================================
// 6. Error cases
// ============================================================================

#[test]
fn test_error_missing_guarantee() {
    let src = "axiom id : α → α\n";
    let err = parse_contracts(src).expect_err("must error on missing guarantee");
    assert!(
        err.contains("guarantee"),
        "error should mention guarantee: {}",
        err
    );
}

#[test]
fn test_error_missing_arrow() {
    let src = "axiom id : α\n  guarantee: True\n";
    let err = parse_contracts(src).expect_err("must error on missing arrow");
    assert!(
        err.contains("→") || err.contains("arrow"),
        "error should mention the arrow: {}",
        err
    );
}

#[test]
fn test_error_missing_colon() {
    let src = "axiom id α → α\n  guarantee: True\n";
    let err = parse_contracts(src).expect_err("must error on missing colon");
    assert!(
        err.contains(":") || err.contains("colon"),
        "error should mention the colon: {}",
        err
    );
}

#[test]
fn test_error_malformed_type() {
    // `×` without a closing `)`.
    let src = "axiom bad : (Int × Int → Int\n  guarantee: True\n";
    let err = parse_contracts(src).expect_err("must error on malformed type");
    assert!(!err.is_empty(), "error message must be non-empty: {}", err);
}

#[test]
fn test_error_unknown_clause() {
    let src = "axiom id : α → α\n  guarantee: True\n  bogus: something\n";
    let err = parse_contracts(src).expect_err("must error on unknown clause");
    assert!(
        err.contains("bogus") || err.contains("unknown"),
        "error should mention the unknown clause: {}",
        err
    );
}

#[test]
fn test_error_unknown_termination_effect() {
    let src = "axiom id : α → α\n  guarantee: True\n  termination: explodes\n";
    let err = parse_contracts(src).expect_err("must error on bad termination");
    assert!(
        err.contains("termination") || err.contains("explodes"),
        "error should mention the bad effect: {}",
        err
    );
}

#[test]
fn test_error_unknown_predicate() {
    let src = "axiom id : α → α\n  guarantee: Mystery(output)\n";
    let err = parse_contracts(src).expect_err("must error on unknown predicate");
    assert!(
        err.contains("Mystery") || err.contains("unknown predicate"),
        "error should mention the unknown predicate: {}",
        err
    );
}

#[test]
fn test_error_top_level_non_axiom_line() {
    let src = "hello world\n";
    let err = parse_contracts(src).expect_err("must error on non-axiom top-level line");
    assert!(
        err.contains("axiom") || err.contains("unexpected"),
        "error should mention the unexpected line: {}",
        err
    );
}

#[test]
fn test_error_empty_guarantee_value() {
    // `guarantee:` with no value — the predicate parser should reject empty input.
    let src = "axiom id : α → α\n  guarantee:\n";
    let err = parse_contracts(src).expect_err("must error on empty guarantee");
    assert!(!err.is_empty(), "error message must be non-empty: {}", err);
}
