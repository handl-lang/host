//! Eval conformance suite for the bytecode engine (ADR-0044 Phase 6).
//!
//! Runs the shared eval conformance corpus from `handl-test-utils` against
//! the `BytecodeEngine`, verifying it produces the expected `Value` for each
//! case. The "foreign" case is handled by constructing the engine with the
//! conformance host registry.
//!
//! Additionally, a direct cross-engine test runs BOTH the tree-walk and the
//! bytecode engine for every conformance case and asserts their outputs are
//! identical — the cross-engine conformance gate.

use std::rc::Rc;

use handl_language::{LangError, Value};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest, HostFn, HostFunctions};
use handl_plugin_engine_eval_bytecode::BytecodeEngine;
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use handl_test_utils::conformance::{eval_corpus, run_eval_conformance};

/// A host function that doubles its integer argument (matches the "double"
/// function expected by the "foreign" conformance case).
struct DoubleHostFn;

impl HostFn for DoubleHostFn {
    fn call(&self, args: &[Value]) -> Result<Value, LangError> {
        let n = args[0].as_i64().unwrap();
        Ok(Value::from(n * 2))
    }
}

/// A host function registry with "double" registered.
struct ConformanceHost;

impl HostFunctions for ConformanceHost {
    fn lookup(&self, name: &str) -> Option<&dyn HostFn> {
        if name == "double" {
            Some(&DoubleHostFn)
        } else {
            None
        }
    }
}

/// The bytecode engine passes the full eval conformance suite (non-foreign
/// cases).
#[test]
fn bytecode_engine_passes_eval_conformance() {
    let engine = BytecodeEngine::new();
    run_eval_conformance(&engine).expect("eval conformance suite (non-foreign) must pass");
}

/// The bytecode engine passes the "foreign" conformance case with a host
/// registry.
#[test]
fn bytecode_engine_passes_foreign_conformance() {
    let host = ConformanceHost;
    let engine = BytecodeEngine::with_host(Rc::new(host));
    let corpus = eval_corpus();
    let foreign_case = corpus
        .iter()
        .find(|c| c.name == "foreign")
        .expect("foreign case must exist in corpus");
    let req = EvalRequest::new(foreign_case.ast.clone());
    let result = engine.eval(&foreign_case.lang, &req).unwrap();
    assert_eq!(result, foreign_case.expected);
}

/// Cross-engine conformance: for every non-foreign case, the bytecode engine
/// and the tree-walk engine produce identical results.
#[test]
fn cross_engine_non_foreign_results_match() {
    let bytecode = BytecodeEngine::new();
    let tree_walk = TreeWalkEngine::new();
    let corpus = eval_corpus();
    for case in &corpus {
        if case.name == "foreign" {
            continue;
        }
        let req = EvalRequest::new(case.ast.clone());
        let bc_result = bytecode.eval(&case.lang, &req);
        let tw_result = tree_walk.eval(&case.lang, &req);
        assert_eq!(
            bc_result, tw_result,
            "case '{}': bytecode and tree-walk diverged",
            case.name
        );
        // Also verify both match the expected value.
        if let Ok(v) = &bc_result {
            assert_eq!(
                v, &case.expected,
                "case '{}': result does not match expected",
                case.name
            );
        }
    }
}

/// Cross-engine conformance for the "foreign" case: both engines with the
/// host registry produce identical results.
#[test]
fn cross_engine_foreign_results_match() {
    let host = Rc::new(ConformanceHost) as Rc<dyn HostFunctions>;
    let bytecode = BytecodeEngine::with_host(Rc::clone(&host));
    let tree_walk = TreeWalkEngine::new().with_host(Rc::clone(&host));
    let corpus = eval_corpus();
    let foreign_case = corpus
        .iter()
        .find(|c| c.name == "foreign")
        .expect("foreign case must exist in corpus");
    let req = EvalRequest::new(foreign_case.ast.clone());
    let bc_result = bytecode.eval(&foreign_case.lang, &req);
    let tw_result = tree_walk.eval(&foreign_case.lang, &req);
    assert_eq!(
        bc_result, tw_result,
        "foreign case: bytecode and tree-walk diverged"
    );
    assert_eq!(bc_result.unwrap(), foreign_case.expected);
}
