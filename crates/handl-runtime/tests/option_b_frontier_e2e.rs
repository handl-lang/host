//! End-to-end runtime tests for the Option B frontier operators
//! (`=>>`, `=>|`, `<@`, `@>`, `#`, `?>`), `id2`, and `where`.
//!
//! These tests drive the canonical kit through the data lexer, Pratt parser,
//! and tree-walk eval engine using raw `hnl` source strings.

use std::rc::Rc;

use handl_kit_data_canonical;
use handl_language::{LangError, Source};
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::{EngineRegistry, Runtime};
use serde_json::json;

fn frontier_runtime() -> Runtime {
    let kit = handl_kit_data_canonical::assemble().expect("canonical kit assembles");

    let mut reg = EngineRegistry::new();
    reg.register_lex("data", Box::new(DataLexerEngine::new()));
    reg.register_parse("pratt", PrattEngine::new());
    reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    let registry = Rc::new(reg);

    let mut config = kit.runtime_config().clone();
    config.typecheck.clear();
    Runtime::from_registry(kit.lang().clone(), &registry, &config)
        .expect("frontier runtime binds")
}

#[test]
fn cell_vcomp_composes_end_to_end() {
    let rt = frontier_runtime();
    // `1 + 2` (=> 3) is fed as the incoming value to the second cell (`3`).
    let result = rt.run(&Source::new("1 + 2 =>> 3")).expect("cell_vcomp runs");
    assert_eq!(result, json!(3));
}

#[test]
fn witness_end_to_end() {
    let rt = frontier_runtime();
    // `v as G` parses as a witness; the grade is metadata and the proof is
    // the value returned.
    let result = rt.run(&Source::new("NonZero as 1")).expect("witness runs");
    assert_eq!(result, json!(1));
}

#[test]
fn transport_lexes_parses_and_errors_not_implemented() {
    let rt = frontier_runtime();
    let err = rt.run(&Source::new("p # v")).unwrap_err();
    assert_eq!(err, LangError::NotImplemented);
}

#[test]
fn refine_with_predicate_runs_end_to_end() {
    let rt = frontier_runtime();
    // The arrow's incoming (5) is refined by the `where` predicate. The
    // predicate is a function; a truthy predicate lets the incoming value pass.
    let result = rt
        .run(&Source::new("5 where (y) ~> y != 0"))
        .expect("refine with function runs");
    assert_eq!(result, json!(5));
}

#[test]
fn cell_id_end_to_end() {
    let rt = frontier_runtime();
    let result = rt.run(&Source::new("id2 (1 + 2)")).expect("cell_id runs");
    assert_eq!(result, json!(3));
}

#[test]
fn cobind_end_to_end() {
    let rt = frontier_runtime();
    // `Storable ?> 42` ignores the grade and evaluates the body.
    let result = rt.run(&Source::new("Storable ?> 42")).expect("cobind runs");
    assert_eq!(result, json!(42));
}
