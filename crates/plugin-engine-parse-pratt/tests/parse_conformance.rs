//! Parse conformance suite for the Pratt engine.
//!
//! Runs the shared parse conformance corpus from `handl-test-utils` against
//! the `PrattEngine`, verifying it correctly reads `Language.grammar` data
//! and produces the expected `Ast` for each case.

use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_test_utils::conformance::run_parse_conformance;

#[test]
fn pratt_engine_passes_parse_conformance() {
    let engine = PrattEngine::new();
    run_parse_conformance(&engine).expect("parse conformance suite must pass");
}
