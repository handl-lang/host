//! Tests for backtracking parser with Result-type modeling (ADR-0071).
//!
//! These tests verify that:
//! - Backtracking parser atoms are registered in both `minimal()` and `from_axioms()`
//! - The backtrack.hnl arrow file parses to the correct Scheme IR
//! - Inference succeeds on the backtracking parser graph
//! - The inferred guarantee includes IsSuccess ∨ IsError (the parser always
//!   produces a valid Result)
//! - The Choice branching mechanism conjoins both arms' guarantees correctly
//! - End-to-end load and infer from disk works

use handl_language::{ArrSource, Scheme};

use crate::arrow_loader::{infer_file, load_arrow_file};
use crate::arrow_parser::parse_arrow_file;
use crate::contracts::ContractRegistry;
use crate::inference::infer_refinement;
use crate::types::Pred;

// Embed the actual .hnl stdlib files for testing.
const BACKTRACK_HNL: &str =
    include_str!("../../../kit-stdlib/data/arrows/parse/backtrack.hnl");

// ============================================================================
// Hand-built backtracking parser graph (matching the .hnl file)
// ============================================================================

/// Build the backtracking parser arrow graph as a Scheme:
///   backtrack = parse.try_atom >>> (parse.extract_success ||| (parse.extract_error >>> parse.try_op))
///
/// - parse.try_atom : TokenList → Result (IsSuccess ∨ IsError)
/// - parse.extract_success ||| (parse.extract_error >>> parse.try_op):
///   - inl(success) → extract_success → AST
///   - inr(error)   → extract_error → parse.try_op → Result
///
/// The inference engine conjoins both Choice arms' guarantees.
fn backtrack_graph() -> Scheme {
    // extract_error → parse.try_op
    let retry = Scheme::Seq {
        fst: Box::new(Scheme::arr_native("parse.extract_error", vec![])),
        snd: Box::new(Scheme::arr_native("parse.try_op", vec![])),
    };

    // extract_success ||| retry
    let branch = Scheme::Choice {
        l: Box::new(Scheme::arr_native("parse.extract_success", vec![])),
        r: Box::new(retry),
    };

    // parse.try_atom >>> branch
    Scheme::Seq {
        fst: Box::new(Scheme::arr_native("parse.try_atom", vec![])),
        snd: Box::new(branch),
    }
}

// ============================================================================
// 1. Atoms registered correctly
// ============================================================================

#[test]
fn test_backtrack_atoms_registered_minimal() {
    let registry = ContractRegistry::minimal();
    for name in &[
        "parse.try_atom",
        "parse.try_op",
        "parse.extract_success",
        "parse.extract_error",
        "parse.succeed",
        "parse.fail",
    ] {
        assert!(
            registry.lookup(name, &ArrSource::Native).is_some(),
            "{} should be registered in minimal()",
            name
        );
    }
}

#[test]
fn test_backtrack_atoms_registered_from_axioms() {
    let registry = ContractRegistry::from_axioms();
    for name in &[
        "parse.try_atom",
        "parse.try_op",
        "parse.extract_success",
        "parse.extract_error",
        "parse.succeed",
        "parse.fail",
    ] {
        assert!(
            registry.lookup(name, &ArrSource::Native).is_some(),
            "{} should be registered in from_axioms()",
            name
        );
    }
}

// ============================================================================
// 2. Atom contracts are correct
// ============================================================================

#[test]
fn test_try_atom_guarantee_is_success_or_error() {
    let registry = ContractRegistry::minimal();
    let contract = registry.lookup("parse.try_atom", &ArrSource::Native).unwrap();
    // Guarantee should be Or(IsSuccess(output), IsError(output))
    match &contract.guarantee {
        Pred::Or(l, r) => {
            assert!(
                matches!(l.as_ref(), Pred::IsSuccess(v) if v == "output"),
                "Left should be IsSuccess(output), got: {:?}",
                l
            );
            assert!(
                matches!(r.as_ref(), Pred::IsError(v) if v == "output"),
                "Right should be IsError(output), got: {:?}",
                r
            );
        }
        other => panic!("try_atom guarantee should be Or, got: {:?}", other),
    }
}

#[test]
fn test_succeed_guarantee_is_success() {
    let registry = ContractRegistry::minimal();
    let contract = registry.lookup("parse.succeed", &ArrSource::Native).unwrap();
    assert_eq!(
        contract.guarantee,
        Pred::IsSuccess("output".to_string()),
        "succeed guarantee should be IsSuccess(output)"
    );
}

#[test]
fn test_fail_guarantee_is_error() {
    let registry = ContractRegistry::minimal();
    let contract = registry.lookup("parse.fail", &ArrSource::Native).unwrap();
    assert_eq!(
        contract.guarantee,
        Pred::IsError("output".to_string()),
        "fail guarantee should be IsError(output)"
    );
}

// ============================================================================
// 3. .hnl file parses to correct Scheme
// ============================================================================

#[test]
fn test_backtrack_hnl_parses() {
    let parsed = parse_arrow_file(BACKTRACK_HNL).unwrap();
    let expected = backtrack_graph();
    match parsed {
        crate::arrow_parser::ParsedFile::Single(scheme) => {
            assert_eq!(
                scheme, expected,
                "backtrack.hnl should parse to the expected Scheme graph"
            );
        }
        other => panic!("expected ParsedFile::Single, got: {:?}", other),
    }
}

// ============================================================================
// 4. Inference succeeds and produces IsSuccess ∨ IsError
// ============================================================================

#[test]
fn test_backtrack_inference_succeeds() {
    let registry = ContractRegistry::minimal();
    let graph = backtrack_graph();
    let result = infer_refinement(&graph, &registry).unwrap();
    let pretty = result.guarantee.pretty();
    println!("backtrack inferred guarantee: {}", pretty);

    // The guarantee should contain IsSuccess (from the Choice conjoining
    // both arms' guarantees). The try_atom arm contributes
    // Or(IsSuccess, IsError), and the Choice conjoins with the retry arm.
    // The exact form depends on simplification, but IsSuccess should appear.
    assert!(
        pretty.contains("IsSuccess"),
        "backtrack guarantee should contain IsSuccess: {}",
        pretty
    );
}

#[test]
fn test_backtrack_inference_contains_result_property() {
    let registry = ContractRegistry::minimal();
    let graph = backtrack_graph();
    let result = infer_refinement(&graph, &registry).unwrap();
    let pretty = result.guarantee.pretty();
    println!("backtrack full guarantee: {}", pretty);

    // The overall guarantee should reference either IsSuccess or IsError,
    // proving that the Result-type property flows through Choice.
    assert!(
        pretty.contains("IsSuccess") || pretty.contains("IsError"),
        "backtrack guarantee should reference Result predicates: {}",
        pretty
    );
}

// ============================================================================
// 5. Choice conjoins both arms' guarantees
// ============================================================================

#[test]
fn test_choice_conjoins_backtrack_arms() {
    let registry = ContractRegistry::minimal();

    // Just the choice part: extract_success ||| (extract_error >>> parse.try_op)
    let choice_part = Scheme::Choice {
        l: Box::new(Scheme::arr_native("parse.extract_success", vec![])),
        r: Box::new(Scheme::Seq {
            fst: Box::new(Scheme::arr_native("parse.extract_error", vec![])),
            snd: Box::new(Scheme::arr_native("parse.try_op", vec![])),
        }),
    };

    let result = infer_refinement(&choice_part, &registry).unwrap();
    let pretty = result.guarantee.pretty();
    println!("choice part guarantee: {}", pretty);

    // extract_success has guarantee True, try_op has Or(IsSuccess, IsError).
    // Choice conjoins: True ∧ Or(IsSuccess, IsError) = Or(IsSuccess, IsError).
    assert!(
        pretty.contains("IsSuccess"),
        "choice guarantee should contain IsSuccess from try_op: {}",
        pretty
    );
}

// ============================================================================
// 6. End-to-end: load and infer from disk
// ============================================================================

#[test]
fn test_backtrack_e2e_load_and_infer() {
    let registry = ContractRegistry::from_axioms();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = format!("{}/../../kit-stdlib/data/arrows/parse/backtrack.hnl", manifest_dir);
    let graph = load_arrow_file(&path).unwrap();
    let result = infer_refinement(&graph, &registry).unwrap();
    let pretty = result.guarantee.pretty();
    println!("backtrack e2e guarantee: {}", pretty);
    assert!(
        pretty.contains("IsSuccess") || pretty.contains("IsError"),
        "e2e backtrack guarantee should reference Result predicates: {}",
        pretty
    );
}

#[test]
fn test_backtrack_e2e_infer_file() {
    let registry = ContractRegistry::from_axioms();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = format!("{}/../../kit-stdlib/data/arrows/parse/backtrack.hnl", manifest_dir);
    let result = infer_file(&path, &registry).unwrap();
    let pretty = result.guarantee.pretty();
    println!("backtrack e2e (infer_file) guarantee: {}", pretty);
    assert!(
        pretty.contains("IsSuccess") || pretty.contains("IsError"),
        "e2e infer_file guarantee should reference Result predicates: {}",
        pretty
    );
}

// ============================================================================
// 7. IsSuccess/IsError predicate properties
// ============================================================================

#[test]
fn test_is_success_pretty() {
    let p = Pred::IsSuccess("output".to_string());
    assert_eq!(p.pretty(), "IsSuccess(output)");
}

#[test]
fn test_is_error_pretty() {
    let p = Pred::IsError("output".to_string());
    assert_eq!(p.pretty(), "IsError(output)");
}

#[test]
fn test_is_success_reflexivity_toy_implies() {
    let p = Pred::IsSuccess("output".to_string());
    let q = Pred::IsSuccess("output".to_string());
    assert!(
        crate::smt::toy_implies(&p, &q),
        "toy_implies must prove IsSuccess reflexivity"
    );
}

#[test]
fn test_is_error_reflexivity_toy_implies() {
    let p = Pred::IsError("output".to_string());
    let q = Pred::IsError("output".to_string());
    assert!(
        crate::smt::toy_implies(&p, &q),
        "toy_implies must prove IsError reflexivity"
    );
}

#[test]
fn test_is_success_implies_or_with_success() {
    // IsSuccess(x) ⟹ IsSuccess(x) ∨ IsError(x)
    let p = Pred::IsSuccess("output".to_string());
    let q = Pred::Or(
        Box::new(Pred::IsSuccess("output".to_string())),
        Box::new(Pred::IsError("output".to_string())),
    );
    assert!(
        crate::smt::toy_implies(&p, &q),
        "IsSuccess ⟹ Or(IsSuccess, IsError) should hold"
    );
}

#[test]
fn test_is_error_implies_or_with_error() {
    // IsError(x) ⟹ IsSuccess(x) ∨ IsError(x)
    let p = Pred::IsError("output".to_string());
    let q = Pred::Or(
        Box::new(Pred::IsSuccess("output".to_string())),
        Box::new(Pred::IsError("output".to_string())),
    );
    assert!(
        crate::smt::toy_implies(&p, &q),
        "IsError ⟹ Or(IsSuccess, IsError) should hold"
    );
}
