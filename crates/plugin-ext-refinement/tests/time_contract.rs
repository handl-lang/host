//! Verifies that `handl-packages/time/time.hnl` (the time abstraction
//! contract) parses cleanly with `parse_contracts` and registers all 3
//! axioms.

use std::path::PathBuf;

use handl_plugin_ext_refinement::parse_contracts;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn time_contract_parses_all_3_axioms() {
    let path = workspace_root().join("handl-packages/time/time.hnl");
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
