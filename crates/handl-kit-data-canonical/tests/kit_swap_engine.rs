//! Integration test: swap the eval engine selection in the canonical kit and
//! verify `Runtime::from_config` resolves the requested engine (ADR-0044 §7
//! "Engine selection as data").
//!
//! This proves engine selection is data-driven and swappable: the same
//! `Language` and manifest, with `eval = "bytecode"` instead of
//! `eval = "tree-walk"`, binds the bytecode engine and produces a working
//! runtime.

use handl_kit_data::{DataKit, DataKitManifest, FragmentSourceEntry};
use handl_kit_data_canonical;
use handl_language::LangError;
use handl_plugin_engine_exec_bytecode::BytecodeEngine;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::{EngineRegistry, RuntimeConfig};
use std::rc::Rc;

/// Build a registry with pratt parse + tree-walk/bytecode eval (the engines
/// needed by the swap tests).
fn test_registry() -> Rc<EngineRegistry> {
    let mut reg = EngineRegistry::new();
    reg.register_parse("pratt", PrattEngine::new());
    reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    reg.register_eval("bytecode", Box::new(BytecodeEngine::new()));
    Rc::new(reg)
}

fn manifest_with_eval(eval: &str) -> DataKitManifest {
    DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter(
            "canonical-yaml",
            "canonical-yaml",
        ))
        .with_runtime(RuntimeConfig::new().with_parse("pratt").with_eval(eval))
}

#[test]
fn swap_eval_to_bytecode_succeeds() {
    let reg = handl_kit_data_canonical::registry();
    let registry = test_registry();
    let manifest = manifest_with_eval("bytecode");
    let kit = DataKit::assemble(&manifest, &reg).expect("assembly should succeed");
    // Assembly succeeds — the Language is built regardless of engine choice.
    assert!(kit.lang().tokens.len() > 0);
    // Building the runtime succeeds: bytecode is now available.
    let rt = kit
        .into_runtime(&registry, None)
        .expect("bytecode should bind");
    assert!(rt.lang().tokens.len() > 0);
    // The runtime works: evaluating an empty Ast returns Null.
    let result = rt.eval(&handl_language::Ast::new()).unwrap();
    assert_eq!(result, handl_language::Value::Null);
}

#[test]
fn swap_eval_to_tree_walk_succeeds() {
    let reg = handl_kit_data_canonical::registry();
    let registry = test_registry();
    let manifest = manifest_with_eval("tree-walk");
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    let rt = kit
        .into_runtime(&registry, None)
        .expect("tree-walk should bind");
    assert!(rt.lang().tokens.len() > 0);
}

#[test]
fn swap_parse_to_chumsky_errors_clearly() {
    let reg = handl_kit_data_canonical::registry();
    let registry = test_registry();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter(
            "canonical-yaml",
            "canonical-yaml",
        ))
        .with_runtime(
            RuntimeConfig::new()
                .with_parse("chumsky")
                .with_eval("tree-walk"),
        );
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    let err = kit.into_runtime(&registry, None).unwrap_err();
    assert_eq!(err, LangError::engine("unknown parse engine: chumsky"));
}

#[test]
fn no_eval_engine_leaves_eval_unbound() {
    // A manifest with no eval engine selection assembles fine; the runtime
    // builds but eval dispatch returns NotImplemented (no eval engine bound).
    let reg = handl_kit_data_canonical::registry();
    let registry = test_registry();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter(
            "canonical-yaml",
            "canonical-yaml",
        ))
        .with_runtime(RuntimeConfig::new().with_parse("pratt"));
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    let rt = kit
        .into_runtime(&registry, None)
        .expect("runtime with no eval should build");
    // eval is unbound -> NotImplemented on eval.
    let ast = handl_language::Ast::new();
    assert_eq!(rt.eval(&ast).unwrap_err(), LangError::NotImplemented);
}
