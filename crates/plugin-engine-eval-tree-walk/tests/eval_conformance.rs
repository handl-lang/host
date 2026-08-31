//! Eval conformance suite for the tree-walk engine.
//!
//! Runs the shared eval conformance corpus from `handl-test-utils` against
//! the `TreeWalkEngine`, verifying it correctly interprets `Scheme` data and
//! produces the expected `Value` for each case. The "foreign" case is
//! handled by constructing the engine with the conformance host registry.

use std::rc::Rc;

use handl_language::Value;
use handl_platform::{EvalContext, Function, Module, ModuleLookup};
use handl_plugin_engine_eval::{EvalEngine, EvalRequest};
use handl_plugin_engine_eval_tree_walk::TreeWalkEngine;
use handl_plugin_module::ModuleRegistry;
use handl_test_utils::conformance::{eval_corpus, run_eval_conformance};

/// A host function that doubles its integer argument (matches the "double"
/// function expected by the "foreign" conformance case).
struct DoubleHostFn;

impl Function for DoubleHostFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, handl_platform::EvalError> {
        let n = args[0].as_i64().unwrap();
        Ok(Value::from(n * 2))
    }
}

/// A module with "double" registered.
struct ConformanceModule;

impl Module for ConformanceModule {
    fn name(&self) -> &str {
        "conformance"
    }
    fn function(&self, name: &str) -> Option<&dyn Function> {
        if name == "double" {
            Some(&DoubleHostFn)
        } else {
            None
        }
    }
}

/// Build a module registry with the `double` function registered.
fn conformance_registry() -> Rc<dyn ModuleLookup> {
    Rc::new(ModuleRegistry::new().with(ConformanceModule))
}

#[test]
fn tree_walk_engine_passes_eval_conformance() {
    // Run all non-foreign cases without a host.
    let engine = TreeWalkEngine::new();
    run_eval_conformance(&engine).expect("eval conformance suite (non-foreign) must pass");
}

#[test]
fn tree_walk_engine_passes_foreign_conformance() {
    // Run the "foreign" case with the module registry.
    let engine = TreeWalkEngine::new().with_modules(conformance_registry());
    let corpus = eval_corpus();
    let foreign_case = corpus
        .iter()
        .find(|c| c.name == "foreign")
        .expect("foreign case must exist in corpus");
    let req = EvalRequest::new(foreign_case.ast.clone());
    let result = engine.eval(&foreign_case.lang, &req).unwrap();
    assert_eq!(result, foreign_case.expected);
}
