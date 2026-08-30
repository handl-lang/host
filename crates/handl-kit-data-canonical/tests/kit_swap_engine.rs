//! Integration test: swap the eval engine selection in the canonical kit and
//! verify `Runtime::from_config` returns a clear config error for an engine
//! not yet built in Phase 5 (ADR-0044 §7 "Engine selection as data").
//!
//! This proves engine selection is data-driven and swappable: the same
//! `Language` and manifest, with `eval = "bytecode"` instead of
//! `eval = "tree-walk"`, produces a clear `LangError::Engine` rather than
//! silently binding a wrong engine. The bytecode engine arrives in Phase 6.

use handl_kit_data::{DataKit, DataKitManifest, FragmentSourceEntry};
use handl_kit_data_canonical;
use handl_language::LangError;
use handl_runtime::RuntimeConfig;

fn manifest_with_eval(eval: &str) -> DataKitManifest {
    DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("identifier", "identifier"))
        .with_fragment(FragmentSourceEntry::emitter("value", "value"))
        .with_fragment(FragmentSourceEntry::emitter("arith", "arith"))
        .with_fragment(FragmentSourceEntry::emitter("let", "let"))
        .with_fragment(FragmentSourceEntry::emitter("call", "call"))
        .with_fragment(FragmentSourceEntry::emitter("branch", "branch"))
        .with_fragment(FragmentSourceEntry::emitter("function", "function"))
        .with_runtime(
            RuntimeConfig::new()
                .with_parse("pratt")
                .with_eval(eval),
        )
}

#[test]
fn swap_eval_to_bytecode_errors_clearly() {
    let reg = handl_kit_data_canonical::registry();
    let manifest = manifest_with_eval("bytecode");
    let kit = DataKit::assemble(&manifest, &reg).expect("assembly should succeed");
    // Assembly succeeds — the Language is built regardless of engine choice.
    assert_eq!(kit.lang().tokens.len(), 15);
    // Building the runtime fails: bytecode is not yet built in Phase 5.
    let err = kit.into_runtime(None).unwrap_err();
    assert_eq!(
        err,
        LangError::Engine("unknown eval engine: bytecode".into())
    );
}

#[test]
fn swap_eval_to_tree_walk_succeeds() {
    let reg = handl_kit_data_canonical::registry();
    let manifest = manifest_with_eval("tree-walk");
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    let rt = kit.into_runtime(None).expect("tree-walk should bind");
    assert_eq!(rt.lang().tokens.len(), 15);
}

#[test]
fn swap_parse_to_chumsky_errors_clearly() {
    let reg = handl_kit_data_canonical::registry();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("identifier", "identifier"))
        .with_runtime(
            RuntimeConfig::new()
                .with_parse("chumsky")
                .with_eval("tree-walk"),
        );
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    let err = kit.into_runtime(None).unwrap_err();
    assert_eq!(
        err,
        LangError::Engine("unknown parse engine: chumsky".into())
    );
}

#[test]
fn no_eval_engine_leaves_eval_unbound() {
    // A manifest with no eval engine selection assembles fine; the runtime
    // builds but eval dispatch returns NotImplemented (no eval engine bound).
    let reg = handl_kit_data_canonical::registry();
    let manifest = DataKitManifest::new()
        .with_fragment(FragmentSourceEntry::emitter("identifier", "identifier"))
        .with_runtime(RuntimeConfig::new().with_parse("pratt"));
    let kit = DataKit::assemble(&manifest, &reg).unwrap();
    let rt = kit.into_runtime(None).expect("runtime with no eval should build");
    // eval is unbound -> NotImplemented on eval.
    let ast = handl_language::Ast::new();
    assert_eq!(rt.eval(&ast).unwrap_err(), LangError::NotImplemented);
}
