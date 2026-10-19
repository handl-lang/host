//! Tests for the Pratt parser arrow graph and parse atoms.
//!
//! These tests verify that:
//! - Parse atoms are registered correctly in both `minimal()` and `from_axioms()`
//! - The Pratt parser `.hnl` arrow file parses to the correct `Term` IR
//! - Inference succeeds on the Pratt parser graph
//! - The inferred guarantee includes `LengthSubOne` (tokens are consumed)
//! - The inferred termination measure shows the token list decreases
//! - The Pratt parser can be loaded and inferred end-to-end from disk

use handl_language::{ArrSource, Term};
use hnl_engine::TermData;

use crate::arrow_loader::{infer_file, load_arrow_file};
use crate::arrow_parser::{ParsedFile, parse_arrow_file};
use crate::contracts::ContractRegistry;
use crate::inference::infer_refinement;
use crate::types::{Measure, Pred};

// Embed the actual .hnl stdlib files for testing.
/// The arrow-dialect [`Language`] for tests — canonical base from the
/// dev-dep data crate plus the `arrow_language_for` overrides (tests may
/// depend on `lang/` data; the library must not).
fn arrow_lang() -> &'static handl_language::Language {
    static LANG: std::sync::OnceLock<handl_language::Language> =
        std::sync::OnceLock::new();
    LANG.get_or_init(|| {
        crate::arrow_lang::arrow_language_for(
            &handl_kit_data_prelude::language_from_yaml()
                .expect("canonical language assembles"),
        )
    })
}

const PRATT_HNL: &str = include_str!("../tests/fixtures/parse-pratt/pratt.hnl");

// ============================================================================
// Hand-built Pratt parser graph (matching the .hnl file)
// ============================================================================

/// Convert `TermData` back to `Term` (bootstrap compatibility).
fn td(t: TermData) -> Term {
    t.to_term().expect("TermData -> Term conversion")
}

/// Build a native `op` `TermData` (no args, no literal).
fn op(name: &str) -> TermData {
    TermData::op(name, vec![], &ArrSource::Native, None)
}

/// Build the Pratt parser arrow graph as a Term:
///   loop(parse_atom >>> parse_ops)
/// where:
///   parse_atom  = arr("parse.consume") >>> arr("parse.atom_value")
///   parse_ops   = loop(arr("parse.peek") >>> (apply_op ||| id))
///   apply_op    = arr("parse.consume") >>> arr("parse.make_node")
///
/// The outer loop parses atoms (consuming one token each), the inner loop
/// applies operators (consuming one token per operator). The nested loop
/// structure lets the inference engine derive:
///   - LengthSubOne(output, input) -- tokens are consumed
///   - Nested(ListLength) -- termination via token list decrease
fn pratt_graph() -> Term {
    // apply_op = arr("parse.consume") >>> arr("parse.make_node")
    let apply_op = TermData::seq(op("parse.consume"), op("parse.make_node"));

    // parse_ops = loop(arr("parse.peek") >>> (apply_op ||| id))
    let parse_ops = TermData::loop_feedback(TermData::seq(
        op("parse.peek"),
        TermData::choice(apply_op, op("id")),
    ));

    // parse_atom = arr("parse.consume") >>> arr("parse.atom_value")
    let parse_atom = TermData::seq(op("parse.consume"), op("parse.atom_value"));

    // pratt = loop(parse_atom >>> parse_ops)
    td(TermData::loop_feedback(TermData::seq(
        parse_atom, parse_ops,
    )))
}

// ============================================================================
// 1. Parse atoms registered correctly
// ============================================================================

#[test]
fn test_parse_atoms_registered_minimal() {
    let registry = ContractRegistry::minimal();
    for op in [
        "parse.peek",
        "parse.consume",
        "parse.is_op",
        "parse.prec",
        "parse.make_node",
        "parse.atom_value",
    ] {
        assert!(
            registry.contains(op, &ArrSource::Native),
            "atom `{}` should be registered in minimal()",
            op
        );
    }
}

#[test]
fn test_parse_atoms_registered_from_axioms() {
    let registry = ContractRegistry::from_axioms();
    for op in [
        "parse.peek",
        "parse.consume",
        "parse.is_op",
        "parse.prec",
        "parse.make_node",
        "parse.atom_value",
    ] {
        assert!(
            registry.contains(op, &ArrSource::Native),
            "atom `{}` should be parsed by from_axioms()",
            op
        );
    }
}

#[test]
fn test_parse_consume_guarantee_length_sub_one() {
    // parse.consume should have a LengthSubOne(output, input) guarantee.
    let registry = ContractRegistry::minimal();
    let contract = registry
        .lookup("parse.consume", &ArrSource::Native)
        .expect("parse.consume registered");
    assert_eq!(
        contract.guarantee,
        Pred::LengthSubOne("output".to_string(), "input".to_string())
    );
}

#[test]
fn test_parse_consume_termination_decrements_length() {
    // parse.consume should have a decrements_length termination effect.
    let registry = ContractRegistry::minimal();
    let contract = registry
        .lookup("parse.consume", &ArrSource::Native)
        .expect("parse.consume registered");
    assert_eq!(
        contract.termination_effect.as_deref(),
        Some("decrements_length")
    );
}

#[test]
fn test_from_axioms_matches_minimal_for_parse_consume() {
    // The from_axioms() registry must produce the same contract as minimal()
    // for the parse.consume atom.
    let reg_axioms = ContractRegistry::from_axioms();
    let reg_minimal = ContractRegistry::minimal();

    let c_axioms = reg_axioms
        .lookup("parse.consume", &ArrSource::Native)
        .expect("parse.consume in from_axioms");
    let c_minimal = reg_minimal
        .lookup("parse.consume", &ArrSource::Native)
        .expect("parse.consume in minimal");

    assert_eq!(c_axioms.guarantee, c_minimal.guarantee);
    assert_eq!(c_axioms.termination_effect, c_minimal.termination_effect);
}

// ============================================================================
// 2. Pratt parser arrow file parses to Term
// ============================================================================

#[test]
fn test_parse_pratt_hnl() {
    let parsed = parse_arrow_file(arrow_lang(), PRATT_HNL).expect("parse pratt.hnl");
    match parsed {
        ParsedFile::Single(scheme) => {
            // Should be: loop((parse.consume >>> parse.atom_value) >>>
            //                  loop(parse.peek >>> ((parse.consume >>> parse.make_node) ||| id)))
            assert!(
                matches!(scheme, Term::Loop { .. }),
                "top-level should be Loop"
            );
        }
        other => panic!("expected Single, got {:?}", other),
    }
}

#[test]
fn test_parsed_pratt_matches_handbuilt() {
    let parsed = parse_arrow_file(arrow_lang(), PRATT_HNL).expect("parse pratt.hnl");
    let handbuilt = pratt_graph();
    if let ParsedFile::Single(scheme) = parsed {
        assert_eq!(
            scheme, handbuilt,
            "parsed pratt.hnl should match hand-built graph"
        );
    } else {
        panic!("expected Single");
    }
}

// ============================================================================
// 3. Inference succeeds on the Pratt parser graph
// ============================================================================

#[test]
fn test_pratt_inference_succeeds() {
    let graph = pratt_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry);
    assert!(
        result.is_ok(),
        "pratt inference should succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_pratt_inference_from_hnl() {
    let registry = ContractRegistry::minimal();
    let parsed = parse_arrow_file(arrow_lang(), PRATT_HNL).expect("parse pratt.hnl");
    let handbuilt = pratt_graph();

    if let ParsedFile::Single(scheme) = parsed {
        let parsed_result = infer_refinement(&scheme, &registry).expect("infer parsed");
        let handbuilt_result = infer_refinement(&handbuilt, &registry).expect("infer handbuilt");

        // Same guarantee
        assert_eq!(
            parsed_result.guarantee, handbuilt_result.guarantee,
            "parsed and hand-built pratt should have the same guarantee"
        );
    } else {
        panic!("expected Single");
    }
}

// ============================================================================
// 4. Guarantee includes LengthSubOne (tokens consumed)
// ============================================================================

#[test]
fn test_pratt_guarantee_contains_length_sub_one() {
    let graph = pratt_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry).expect("pratt inference");

    println!("pratt inferred guarantee: {}", result.guarantee.pretty());
    let pretty = result.guarantee.pretty();
    assert!(
        pretty.contains("LengthSubOne") || (pretty.contains("|output|") && pretty.contains("- 1")),
        "pratt guarantee should contain LengthSubOne (tokens consumed): {}",
        pretty
    );
}

#[test]
fn test_pratt_guarantee_non_trivial() {
    let graph = pratt_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry).expect("pratt inference");

    assert_ne!(
        result.guarantee,
        Pred::True,
        "pratt guarantee should be non-trivial (not just True)"
    );
}

// ============================================================================
// 5. Termination: token list decreases
// ============================================================================

#[test]
fn test_pratt_termination_list_length() {
    let graph = pratt_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry).expect("pratt inference");

    let term = result.termination.as_ref();
    assert!(term.is_some(), "pratt should have a termination measure");
    let term = term.unwrap();
    println!("pratt termination: {:?}", term);

    // The termination should NOT be Constant (the token list does decrease).
    assert_ne!(
        *term,
        Measure::Constant,
        "pratt termination must NOT be Constant -- tokens are consumed"
    );
    assert_ne!(
        *term,
        Measure::None,
        "pratt termination must NOT be None -- a measure was found"
    );
}

#[test]
fn test_pratt_termination_nested_or_list_length() {
    // The outer loop's termination depends on the inner loop (Nested),
    // and the inner loop terminates via ListLength (parse.consume decrements
    // the token list). So the measure should be either ListLength or
    // Nested(ListLength).
    let graph = pratt_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry).expect("pratt inference");

    let term = result.termination.as_ref().expect("termination measure");
    println!("pratt termination measure: {:?}", term);

    let is_list_length = matches!(term, Measure::ListLength(_));
    let is_nested_list_length = matches!(
        term,
        Measure::Nested(inner) if matches!(inner.as_ref(), Measure::ListLength(_))
    );
    assert!(
        is_list_length || is_nested_list_length,
        "pratt termination should be ListLength or Nested(ListLength), got {:?}",
        term
    );
}

// ============================================================================
// 6. End-to-end: load and infer from disk
// ============================================================================

#[test]
fn test_load_pratt_from_disk() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let arrows_dir = format!("{}/tests/fixtures", manifest_dir);
    let pratt_path = format!("{}/parse-pratt/pratt.hnl", arrows_dir);

    let scheme = load_arrow_file(arrow_lang(), &pratt_path).expect("load pratt from disk");
    assert!(
        matches!(scheme, Term::Loop { .. }),
        "pratt should be a Loop"
    );
}

#[test]
fn test_infer_pratt_from_disk() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let arrows_dir = format!("{}/tests/fixtures", manifest_dir);
    let pratt_path = format!("{}/parse-pratt/pratt.hnl", arrows_dir);
    let registry = ContractRegistry::minimal();

    let result = infer_file(arrow_lang(), &pratt_path, &registry).expect("infer pratt from disk");

    let pretty = result.guarantee.pretty();
    println!("pratt (from disk) guarantee: {}", pretty);
    assert!(
        pretty.contains("LengthSubOne") || (pretty.contains("|output|") && pretty.contains("- 1")),
        "pratt from disk should have LengthSubOne: {}",
        pretty
    );

    let term = result.termination.as_ref().expect("termination");
    println!("pratt (from disk) termination: {:?}", term);
    assert_ne!(
        *term,
        Measure::Constant,
        "pratt from disk termination must NOT be Constant"
    );
}

// ============================================================================
// 7. Inner loop (parse_ops) inference in isolation
// ============================================================================

#[test]
fn test_parse_ops_inner_loop_inference() {
    // Test the inner loop in isolation:
    //   parse_ops = loop(arr("parse.peek") >>> (apply_op ||| id))
    //   apply_op  = arr("parse.consume") >>> arr("parse.make_node")
    let apply_op = TermData::seq(op("parse.consume"), op("parse.make_node"));

    let parse_ops = td(TermData::loop_feedback(TermData::seq(
        op("parse.peek"),
        TermData::choice(apply_op, op("id")),
    )));

    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&parse_ops, &registry);
    assert!(
        result.is_ok(),
        "parse_ops inference should succeed: {:?}",
        result.err()
    );

    let result = result.unwrap();
    println!(
        "parse_ops inferred guarantee: {}",
        result.guarantee.pretty()
    );

    // The inner loop should also derive LengthSubOne from parse.consume.
    let pretty = result.guarantee.pretty();
    assert!(
        pretty.contains("LengthSubOne") || (pretty.contains("|output|") && pretty.contains("- 1")),
        "parse_ops guarantee should contain LengthSubOne: {}",
        pretty
    );

    // The inner loop should terminate with ListLength.
    let term = result.termination.as_ref().expect("termination");
    assert!(
        matches!(term, Measure::ListLength(_)),
        "parse_ops termination should be ListLength, got {:?}",
        term
    );
}

// ============================================================================
// 8. parse.consume direct inference
// ============================================================================

#[test]
fn test_parse_consume_direct_inference() {
    // Direct inference on the parse.consume atom should produce LengthSubOne.
    let registry = ContractRegistry::minimal();
    let graph = td(op("parse.consume"));
    let result = infer_refinement(&graph, &registry).expect("parse.consume inference");

    assert_eq!(
        result.guarantee,
        Pred::LengthSubOne("output".to_string(), "input".to_string())
    );
}

// ============================================================================
// 9. Zero annotations: Pratt parser infers with only atom contracts
// ============================================================================

#[test]
fn test_pratt_zero_user_annotations() {
    // The key property: the Pratt parser requires ZERO user annotations.
    // The only contracts are on the parse atoms (and the existing list/prim atoms).
    let registry = ContractRegistry::minimal();
    let graph = pratt_graph();
    let result = infer_refinement(&graph, &registry);
    assert!(
        result.is_ok(),
        "pratt inference should succeed with zero annotations: {:?}",
        result.err()
    );
}
