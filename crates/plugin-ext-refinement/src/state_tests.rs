//! Tests for state machine arrow graphs and state atoms.
//!
//! These tests verify that:
//! - State atoms are registered correctly in both `minimal()` and `from_axioms()`
//! - `state.dec` has a conditional contract with `Le(0, input)` precondition
//! - Counter state machine arrow file parses to the correct `Scheme` IR
//! - Counter inference succeeds and discovers `Le(0, ...)` (Count >= 0)
//! - Buffer state machine inference succeeds with length-related predicates
//! - Protocol state machine inference succeeds
//! - All state machines infer with zero annotations
//! - Termination analysis reports Constant (side-channel loops)

use handl_language::{ArrSource, Scheme};

use crate::arrow_loader::{infer_file, load_arrow_file};
use crate::arrow_parser::{ParsedFile, parse_arrow_file};
use crate::contracts::ContractRegistry;
use crate::inference::infer_refinement;
use crate::types::{Measure, Pred};

// Embed the actual .hnl stdlib files for testing.
const COUNTER_HNL: &str = include_str!("../../../handl-packages/state/counter.hnl");
const BUFFER_HNL: &str = include_str!("../../../handl-packages/state/buffer.hnl");
const PROTOCOL_HNL: &str = include_str!("../../../handl-packages/state/protocol.hnl");

// ============================================================================
// Hand-built state machine graphs (matching the .hnl files)
// ============================================================================

/// Build the counter state machine arrow graph:
///   loop(comp(choice(arr("state.inc"), arr("state.dec")), first(id)))
fn counter_graph() -> Scheme {
    use Scheme::*;

    let handle_event = Choice {
        l: Box::new(Scheme::arr_native("state.inc", vec![])),
        r: Box::new(Scheme::arr_native("state.dec", vec![])),
    };

    let body = Seq {
        fst: Box::new(handle_event),
        snd: Box::new(First {
            inner: Box::new(Scheme::arr_native("id", vec![])),
        }),
    };

    Loop {
        body: Box::new(body),
    }
}

/// Build the buffer state machine arrow graph:
///   loop(comp(choice(arr("state.push"), arr("state.pop")), first(id)))
fn buffer_graph() -> Scheme {
    use Scheme::*;

    let handle_event = Choice {
        l: Box::new(Scheme::arr_native("state.push", vec![])),
        r: Box::new(Scheme::arr_native("state.pop", vec![])),
    };

    let body = Seq {
        fst: Box::new(handle_event),
        snd: Box::new(First {
            inner: Box::new(Scheme::arr_native("id", vec![])),
        }),
    };

    Loop {
        body: Box::new(body),
    }
}

/// Build the protocol state machine arrow graph:
///   loop(comp(choice(arr("state.idle_to_active"), arr("state.active_to_done")), first(id)))
fn protocol_graph() -> Scheme {
    use Scheme::*;

    let handle_event = Choice {
        l: Box::new(Scheme::arr_native("state.idle_to_active", vec![])),
        r: Box::new(Scheme::arr_native("state.active_to_done", vec![])),
    };

    let body = Seq {
        fst: Box::new(handle_event),
        snd: Box::new(First {
            inner: Box::new(Scheme::arr_native("id", vec![])),
        }),
    };

    Loop {
        body: Box::new(body),
    }
}

// ============================================================================
// 1. State atoms registered correctly
// ============================================================================

#[test]
fn test_state_atoms_registered_minimal() {
    let registry = ContractRegistry::minimal();
    for op in [
        "state.inc",
        "state.dec",
        "state.push",
        "state.pop",
        "state.is_empty",
        "state.idle_to_active",
        "state.active_to_done",
    ] {
        assert!(
            registry.contains(op, &ArrSource::Native),
            "atom `{}` should be registered in minimal()",
            op
        );
    }
}

#[test]
fn test_state_atoms_registered_from_axioms() {
    let registry = ContractRegistry::from_axioms();
    for op in [
        "state.inc",
        "state.dec",
        "state.push",
        "state.pop",
        "state.is_empty",
        "state.idle_to_active",
        "state.active_to_done",
    ] {
        assert!(
            registry.contains(op, &ArrSource::Native),
            "atom `{}` should be parsed by from_axioms()",
            op
        );
    }
}

// ============================================================================
// 2. state.dec has conditional contract with Le(0, input) precondition
// ============================================================================

#[test]
fn test_state_dec_conditional_contract() {
    let registry = ContractRegistry::minimal();
    let contract = registry
        .lookup("state.dec", &ArrSource::Native)
        .expect("state.dec registered");

    // The precondition must be Le(0, input) -- not True.
    assert_ne!(
        contract.precondition,
        Pred::True,
        "state.dec should have a non-trivial precondition"
    );
    assert_eq!(
        contract.precondition,
        Pred::Le("0".to_string(), "input".to_string()),
        "state.dec precondition should be Le(0, input)"
    );

    // The guarantee must be Le(0, output) -- Count stays >= 0 after decrement.
    assert_eq!(
        contract.guarantee,
        Pred::Le("0".to_string(), "output".to_string()),
        "state.dec guarantee should be Le(0, output)"
    );
}

#[test]
fn test_state_dec_from_axioms_matches_minimal() {
    let reg_axioms = ContractRegistry::from_axioms();
    let reg_minimal = ContractRegistry::minimal();

    let c_axioms = reg_axioms
        .lookup("state.dec", &ArrSource::Native)
        .expect("state.dec in from_axioms");
    let c_minimal = reg_minimal
        .lookup("state.dec", &ArrSource::Native)
        .expect("state.dec in minimal");

    assert_eq!(c_axioms.precondition, c_minimal.precondition);
    assert_eq!(c_axioms.guarantee, c_minimal.guarantee);
    assert_eq!(c_axioms.termination_effect, c_minimal.termination_effect);
}

#[test]
fn test_state_inc_guarantee_le_input_output() {
    let registry = ContractRegistry::minimal();
    let contract = registry
        .lookup("state.inc", &ArrSource::Native)
        .expect("state.inc registered");
    assert_eq!(
        contract.guarantee,
        Pred::Le("input".to_string(), "output".to_string()),
        "state.inc guarantee should be Le(input, output)"
    );
}

#[test]
fn test_state_pop_conditional_contract() {
    let registry = ContractRegistry::minimal();
    let contract = registry
        .lookup("state.pop", &ArrSource::Native)
        .expect("state.pop registered");

    // The precondition must be Le(0, input) -- buffer non-empty.
    assert_ne!(
        contract.precondition,
        Pred::True,
        "state.pop should have a non-trivial precondition"
    );
    assert_eq!(
        contract.precondition,
        Pred::Le("0".to_string(), "input".to_string()),
        "state.pop precondition should be Le(0, input)"
    );

    // The guarantee must contain LengthSubOne and Le(0, output).
    let pretty = contract.guarantee.pretty();
    assert!(
        pretty.contains("LengthSubOne") || (pretty.contains("|output|") && pretty.contains("- 1")),
        "state.pop guarantee should contain LengthSubOne: {}",
        pretty
    );
    assert!(
        pretty.contains("0 <= output"),
        "state.pop guarantee should contain Le(0, output): {}",
        pretty
    );
}

// ============================================================================
// 3. Counter state machine arrow file parses to Scheme
// ============================================================================

#[test]
fn test_parse_counter_hnl() {
    let parsed = parse_arrow_file(COUNTER_HNL).expect("parse counter.hnl");
    match parsed {
        ParsedFile::Single(scheme) => {
            assert!(
                matches!(scheme, Scheme::Loop { .. }),
                "top-level should be Loop"
            );
        }
        other => panic!("expected Single, got {:?}", other),
    }
}

#[test]
fn test_parsed_counter_matches_handbuilt() {
    let parsed = parse_arrow_file(COUNTER_HNL).expect("parse counter.hnl");
    let handbuilt = counter_graph();
    if let ParsedFile::Single(scheme) = parsed {
        assert_eq!(
            scheme, handbuilt,
            "parsed counter.hnl should match hand-built graph"
        );
    } else {
        panic!("expected Single");
    }
}

// ============================================================================
// 4. Counter inference succeeds and discovers Le(0, ...)
// ============================================================================

#[test]
fn test_counter_inference_succeeds() {
    let graph = counter_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry);
    assert!(
        result.is_ok(),
        "counter inference should succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_counter_inference_from_hnl() {
    let registry = ContractRegistry::minimal();
    let parsed = parse_arrow_file(COUNTER_HNL).expect("parse counter.hnl");
    let handbuilt = counter_graph();

    if let ParsedFile::Single(scheme) = parsed {
        let parsed_result = infer_refinement(&scheme, &registry).expect("infer parsed");
        let handbuilt_result = infer_refinement(&handbuilt, &registry).expect("infer handbuilt");

        assert_eq!(
            parsed_result.guarantee, handbuilt_result.guarantee,
            "parsed and hand-built counter should have the same guarantee"
        );
    } else {
        panic!("expected Single");
    }
}

#[test]
fn test_counter_guarantee_contains_le_zero() {
    // The key test: can the fixpoint discover Le(0, ...) (Count >= 0)?
    //
    // The counter state machine is:
    //   loop(comp(choice(arr("state.inc"), arr("state.dec")), first(id)))
    //
    // state.dec has guarantee Le(0, output), which flows into the body's
    // guarantee via the Choice combinator (which conjoins both branches'
    // guarantees). The fixpoint is seeded with the body's guarantee, so
    // Le(0, output) appears in the invariant.
    //
    // The invariant contains Le(0, output) -- representing Count >= 0 on
    // the state (feedback) channel. In a loop, the output of one iteration
    // becomes the input of the next, so Le(0, output) means the count is
    // always >= 0.
    let graph = counter_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry).expect("counter inference");

    println!("counter inferred guarantee: {}", result.guarantee.pretty());
    println!("counter loop invariants: {:?}", result.loop_invariants);

    let pretty = result.guarantee.pretty();
    assert!(
        pretty.contains("0 <= output"),
        "counter guarantee should contain Le(0, output) (Count >= 0): {}",
        pretty
    );
}

#[test]
fn test_counter_guarantee_non_trivial() {
    let graph = counter_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry).expect("counter inference");

    assert_ne!(
        result.guarantee,
        Pred::True,
        "counter guarantee should be non-trivial (not just True)"
    );
}

#[test]
fn test_counter_loop_invariant_contains_le_zero() {
    // The loop invariant (not just the guarantee) should contain Le(0, ...).
    let graph = counter_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry).expect("counter inference");

    assert!(
        !result.loop_invariants.is_empty(),
        "counter should have at least one loop invariant"
    );

    let inv = &result.loop_invariants[0];
    println!(
        "counter loop invariant: {} (via {})",
        inv.invariant.pretty(),
        inv.how_discovered
    );

    let pretty = inv.invariant.pretty();
    assert!(
        pretty.contains("0 <= output"),
        "counter loop invariant should contain Le(0, output) (Count >= 0): {}",
        pretty
    );
}

// ============================================================================
// 5. Buffer state machine inference
// ============================================================================

#[test]
fn test_buffer_inference_succeeds() {
    let graph = buffer_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry);
    assert!(
        result.is_ok(),
        "buffer inference should succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_buffer_guarantee_contains_length_predicate() {
    let graph = buffer_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry).expect("buffer inference");

    println!("buffer inferred guarantee: {}", result.guarantee.pretty());
    println!("buffer loop invariants: {:?}", result.loop_invariants);

    let pretty = result.guarantee.pretty();
    // The buffer's body guarantee contains LengthPlusOne (from push) and
    // LengthSubOne (from pop), conjoined by the Choice combinator.
    let has_length_pred = pretty.contains("LengthPlusOne")
        || pretty.contains("LengthSubOne")
        || (pretty.contains("+ 1") || pretty.contains("- 1"));
    assert!(
        has_length_pred,
        "buffer guarantee should contain a length-related predicate: {}",
        pretty
    );
}

#[test]
fn test_buffer_guarantee_contains_le_zero() {
    // state.pop has guarantee Le(0, output), which flows into the body's
    // guarantee via the Choice combinator.
    let graph = buffer_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry).expect("buffer inference");

    let pretty = result.guarantee.pretty();
    assert!(
        pretty.contains("0 <= output"),
        "buffer guarantee should contain Le(0, output) (buffer >= 0): {}",
        pretty
    );
}

#[test]
fn test_buffer_guarantee_non_trivial() {
    let graph = buffer_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry).expect("buffer inference");

    assert_ne!(
        result.guarantee,
        Pred::True,
        "buffer guarantee should be non-trivial (not just True)"
    );
}

// ============================================================================
// 6. Protocol state machine inference
// ============================================================================

#[test]
fn test_protocol_inference_succeeds() {
    let graph = protocol_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry);
    assert!(
        result.is_ok(),
        "protocol inference should succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_protocol_parsed_matches_handbuilt() {
    let parsed = parse_arrow_file(PROTOCOL_HNL).expect("parse protocol.hnl");
    let handbuilt = protocol_graph();
    if let ParsedFile::Single(scheme) = parsed {
        assert_eq!(
            scheme, handbuilt,
            "parsed protocol.hnl should match hand-built graph"
        );
    } else {
        panic!("expected Single");
    }
}

#[test]
fn test_buffer_parsed_matches_handbuilt() {
    let parsed = parse_arrow_file(BUFFER_HNL).expect("parse buffer.hnl");
    let handbuilt = buffer_graph();
    if let ParsedFile::Single(scheme) = parsed {
        assert_eq!(
            scheme, handbuilt,
            "parsed buffer.hnl should match hand-built graph"
        );
    } else {
        panic!("expected Single");
    }
}

// ============================================================================
// 7. All state machines infer with zero annotations
// ============================================================================

#[test]
fn test_all_state_machines_zero_annotations() {
    // The key property: all state machines require ZERO user annotations.
    // The only contracts are on the state atoms (and existing list/prim atoms).
    let registry = ContractRegistry::minimal();

    let programs = vec![
        ("counter", counter_graph()),
        ("buffer", buffer_graph()),
        ("protocol", protocol_graph()),
    ];

    for (name, graph) in programs {
        let result = infer_refinement(&graph, &registry);
        assert!(
            result.is_ok(),
            "{} inference should succeed with zero annotations: {:?}",
            name,
            result.err()
        );
        let result = result.unwrap();
        println!(
            "{}: guarantee={}, invariants={:?}, termination={:?}",
            name,
            result.guarantee.pretty(),
            result.loop_invariants,
            result.termination
        );
    }
}

// ============================================================================
// 8. Termination analysis (side-channel loops -> Constant)
// ============================================================================

#[test]
fn test_counter_termination_constant() {
    // The counter state machine is loop(comp(choice(inc, dec), first(id))).
    // first(id) leaves the feedback channel C unchanged (side-channel),
    // and all state atoms have termination: constant. So the termination
    // measure should be Constant.
    let graph = counter_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry).expect("counter inference");

    let term = result.termination.as_ref();
    assert!(term.is_some(), "counter should have a termination measure");
    let term = term.unwrap();
    println!("counter termination: {:?}", term);
    // Side-channel loop: C unchanged -> Constant measure.
    assert_eq!(
        *term,
        Measure::Constant,
        "counter termination should be Constant (side-channel loop), got {:?}",
        term
    );
}

#[test]
fn test_buffer_termination_constant() {
    let graph = buffer_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry).expect("buffer inference");

    let term = result.termination.as_ref().expect("termination measure");
    println!("buffer termination: {:?}", term);
    assert_eq!(
        *term,
        Measure::Constant,
        "buffer termination should be Constant (side-channel loop), got {:?}",
        term
    );
}

#[test]
fn test_protocol_termination_constant() {
    let graph = protocol_graph();
    let registry = ContractRegistry::minimal();
    let result = infer_refinement(&graph, &registry).expect("protocol inference");

    let term = result.termination.as_ref().expect("termination measure");
    println!("protocol termination: {:?}", term);
    assert_eq!(
        *term,
        Measure::Constant,
        "protocol termination should be Constant (side-channel loop), got {:?}",
        term
    );
}

// ============================================================================
// 9. End-to-end: load and infer from disk
// ============================================================================

#[test]
fn test_load_counter_from_disk() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let arrows_dir = format!("{}/../../handl-packages", manifest_dir);
    let counter_path = format!("{}/state/counter.hnl", arrows_dir);

    let scheme = load_arrow_file(&counter_path).expect("load counter from disk");
    assert!(
        matches!(scheme, Scheme::Loop { .. }),
        "counter should be a Loop"
    );
}

#[test]
fn test_infer_counter_from_disk() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let arrows_dir = format!("{}/../../handl-packages", manifest_dir);
    let counter_path = format!("{}/state/counter.hnl", arrows_dir);
    let registry = ContractRegistry::minimal();

    let result = infer_file(&counter_path, &registry).expect("infer counter from disk");

    let pretty = result.guarantee.pretty();
    println!("counter (from disk) guarantee: {}", pretty);
    assert!(
        pretty.contains("0 <= output"),
        "counter from disk should contain Le(0, output): {}",
        pretty
    );
}

#[test]
fn test_infer_buffer_from_disk() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let arrows_dir = format!("{}/../../handl-packages", manifest_dir);
    let buffer_path = format!("{}/state/buffer.hnl", arrows_dir);
    let registry = ContractRegistry::minimal();

    let result = infer_file(&buffer_path, &registry).expect("infer buffer from disk");

    let pretty = result.guarantee.pretty();
    println!("buffer (from disk) guarantee: {}", pretty);
    let has_length_pred = pretty.contains("LengthPlusOne")
        || pretty.contains("LengthSubOne")
        || (pretty.contains("+ 1") || pretty.contains("- 1"));
    assert!(
        has_length_pred,
        "buffer from disk should contain a length-related predicate: {}",
        pretty
    );
}

#[test]
fn test_infer_protocol_from_disk() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let arrows_dir = format!("{}/../../handl-packages", manifest_dir);
    let protocol_path = format!("{}/state/protocol.hnl", arrows_dir);
    let registry = ContractRegistry::minimal();

    let result = infer_file(&protocol_path, &registry).expect("infer protocol from disk");
    println!(
        "protocol (from disk) guarantee: {}",
        result.guarantee.pretty()
    );
}

// ============================================================================
// 10. Direct atom inference
// ============================================================================

#[test]
fn test_state_inc_direct_inference() {
    let registry = ContractRegistry::minimal();
    let graph = Scheme::arr_native("state.inc", vec![]);
    let result = infer_refinement(&graph, &registry).expect("state.inc inference");
    assert_eq!(
        result.guarantee,
        Pred::Le("input".to_string(), "output".to_string())
    );
}

#[test]
fn test_state_dec_direct_inference() {
    let registry = ContractRegistry::minimal();
    let graph = Scheme::arr_native("state.dec", vec![]);
    let result = infer_refinement(&graph, &registry).expect("state.dec inference");
    assert_eq!(
        result.guarantee,
        Pred::Le("0".to_string(), "output".to_string())
    );
}

#[test]
fn test_state_push_direct_inference() {
    let registry = ContractRegistry::minimal();
    let graph = Scheme::arr_native("state.push", vec![]);
    let result = infer_refinement(&graph, &registry).expect("state.push inference");
    assert_eq!(
        result.guarantee,
        Pred::LengthPlusOne("output".to_string(), "input".to_string())
    );
}

// ============================================================================
// 11. from_axioms parity: state contracts parse correctly from .hnl files
// ============================================================================

#[test]
fn test_from_axioms_state_dec_guarantee() {
    let registry = ContractRegistry::from_axioms();
    let contract = registry
        .lookup("state.dec", &ArrSource::Native)
        .expect("state.dec in from_axioms");
    assert_eq!(
        contract.guarantee,
        Pred::Le("0".to_string(), "output".to_string()),
        "from_axioms state.dec guarantee should be Le(0, output)"
    );
}

#[test]
fn test_from_axioms_state_pop_guarantee() {
    let registry = ContractRegistry::from_axioms();
    let contract = registry
        .lookup("state.pop", &ArrSource::Native)
        .expect("state.pop in from_axioms");
    let pretty = contract.guarantee.pretty();
    assert!(
        pretty.contains("LengthSubOne") || (pretty.contains("|output|") && pretty.contains("- 1")),
        "from_axioms state.pop guarantee should contain LengthSubOne: {}",
        pretty
    );
    assert!(
        pretty.contains("0 <= output"),
        "from_axioms state.pop guarantee should contain Le(0, output): {}",
        pretty
    );
}
