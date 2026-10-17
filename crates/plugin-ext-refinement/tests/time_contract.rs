//! Verifies that the `time` abstraction contract parses cleanly with
//! `parse_contracts` and registers all 3 axioms.
//!
//! The contract lives in the `lib-time` repo (ADR-0128); this crate keeps a
//! **fixture copy** rather than reaching across the repo boundary, the same
//! shape as the other vendored `.hnl` fixtures here.

use std::path::PathBuf;

use handl_plugin_ext_refinement::parse_contracts;

#[test]
fn time_contract_parses_all_3_axioms() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/time/time.hnl");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
    let reg = parse_contracts(&src).expect("time.hnl must parse");
    assert_eq!(reg.len(), 3, "time contract must declare exactly 3 axioms");

    let names: Vec<String> = reg.iter().map(|(n, _, _)| n.clone()).collect();
    for required in ["time.now_ms", "time.instant_ns", "time.elapsed_ns"] {
        assert!(
            names.iter().any(|n| n == required),
            "missing axiom `{required}` in time contract"
        );
    }
}
