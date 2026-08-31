//! Eval conformance suite for the tree-walk engine.
//!
//! Runs the shared eval conformance corpus from `handl-test-utils` against
//! the `TreeWalkEngine`, verifying it correctly interprets `Scheme` data and
//! produces the expected `Value` for each case. The "foreign" case is
//! handled by constructing the engine with the conformance host registry.

use std::rc::Rc;

use handl_language::{LangError, Value};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest, HostFn, HostFunctions};
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

#[test]
fn tree_walk_engine_passes_eval_conformance() {
    // Run all non-foreign cases without a host.
    let engine = TreeWalkEngine::new();
    run_eval_conformance(&engine).expect("eval conformance suite (non-foreign) must pass");
}

#[test]
fn tree_walk_engine_passes_foreign_conformance() {
    // Run the "foreign" case with the host registry.
    let host = ConformanceHost;
    let engine = TreeWalkEngine::new().with_host(Rc::new(host));
    let corpus = eval_corpus();
    let foreign_case = corpus
        .iter()
        .find(|c| c.name == "foreign")
        .expect("foreign case must exist in corpus");
    let req = EvalRequest::new(foreign_case.ast.clone());
    let result = engine.eval(&foreign_case.lang, &req).unwrap();
    assert_eq!(result, foreign_case.expected);
}
