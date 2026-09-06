//! Eval conformance suite for the cranelift engine (ADR-0044 Phase 7).
//!
//! Runs the shared eval conformance corpus from `handl-test-utils` against
//! the `CraneliftEngine`, verifying it produces the expected `Value` for each
//! case. The "foreign" case is handled by constructing the engine with the
//! conformance host registry.
//!
//! Additionally, a direct cross-engine test runs the tree-walk, bytecode, and
//! cranelift engines for every conformance case and asserts their outputs are
//! identical — the three-way cross-engine conformance gate.

use std::rc::Rc;

use handl_language::Value;
use handl_platform::{EvalContext, Function, Module, ModuleLookup};
use handl_plugin_engine_exec::{EvalEngine, EvalRequest};
use handl_plugin_engine_exec_bytecode::BytecodeEngine;
use handl_plugin_engine_exec_cranelift::CraneliftEngine;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_module::ModuleRegistry;
use handl_test_utils::conformance::{eval_corpus, run_eval_conformance};

/// A host function that doubles its integer argument (matches the "double"
/// function expected by the "foreign" conformance case).
struct DoubleHostFn;

impl Function for DoubleHostFn {
    fn call(
        &self,
        args: &[Value],
        _ctx: &mut EvalContext,
    ) -> Result<Value, handl_platform::EvalError> {
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

/// The cranelift engine passes the full eval conformance suite (non-foreign
/// cases).
#[test]
fn cranelift_engine_passes_eval_conformance() {
    let engine = CraneliftEngine::new();
    run_eval_conformance(&engine).expect("eval conformance suite (non-foreign) must pass");
}

/// The cranelift engine passes the "foreign" conformance case with a host
/// registry.
#[test]
fn cranelift_engine_passes_foreign_conformance() {
    let engine = CraneliftEngine::with_modules(conformance_registry());
    let corpus = eval_corpus();
    let foreign_case = corpus
        .iter()
        .find(|c| c.name == "foreign")
        .expect("foreign case must exist in corpus");
    let req = EvalRequest::new(foreign_case.ast.clone());
    let result = engine.eval(&foreign_case.lang, &req).unwrap();
    assert_eq!(result, foreign_case.expected);
}

/// Three-way cross-engine conformance: for every non-foreign case, the
/// cranelift, bytecode, and tree-walk engines produce identical results.
#[test]
fn three_way_non_foreign_results_match() {
    let cranelift = CraneliftEngine::new();
    let bytecode = BytecodeEngine::new();
    let tree_walk = TreeWalkEngine::new();
    let corpus = eval_corpus();
    for case in &corpus {
        if case.name == "foreign" {
            continue;
        }
        let req = EvalRequest::new(case.ast.clone());
        let cl_result = cranelift.eval(&case.lang, &req);
        let bc_result = bytecode.eval(&case.lang, &req);
        let tw_result = tree_walk.eval(&case.lang, &req);
        assert_eq!(
            cl_result, bc_result,
            "case '{}': cranelift and bytecode diverged",
            case.name
        );
        assert_eq!(
            cl_result, tw_result,
            "case '{}': cranelift and tree-walk diverged",
            case.name
        );
        // Also verify all match the expected value.
        if let Ok(v) = &cl_result {
            assert_eq!(
                v, &case.expected,
                "case '{}': result does not match expected",
                case.name
            );
        }
    }
}

/// Three-way cross-engine conformance for the "foreign" case: all three
/// engines with the host registry produce identical results.
#[test]
fn three_way_foreign_results_match() {
    let reg = conformance_registry();
    let cranelift = CraneliftEngine::with_modules(Rc::clone(&reg));
    let bytecode = BytecodeEngine::with_modules(Rc::clone(&reg));
    let tree_walk = TreeWalkEngine::new().with_modules(Rc::clone(&reg));
    let corpus = eval_corpus();
    let foreign_case = corpus
        .iter()
        .find(|c| c.name == "foreign")
        .expect("foreign case must exist in corpus");
    let req = EvalRequest::new(foreign_case.ast.clone());
    let cl_result = cranelift.eval(&foreign_case.lang, &req);
    let bc_result = bytecode.eval(&foreign_case.lang, &req);
    let tw_result = tree_walk.eval(&foreign_case.lang, &req);
    assert_eq!(
        cl_result, bc_result,
        "foreign case: cranelift and bytecode diverged"
    );
    assert_eq!(
        cl_result, tw_result,
        "foreign case: cranelift and tree-walk diverged"
    );
    assert_eq!(cl_result.unwrap(), foreign_case.expected);
}
