//! Verify that the canonical query/command/event package roots parse
//! successfully under the canonical runtime (the same engine used by
//! `hnl typecheck` / `hnl fmt`).

use handl_kit_data_canonical;
use handl_language::Source;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::{EngineRegistry, Runtime};
use std::rc::Rc;

fn full_runtime() -> Runtime {
    let kit = handl_kit_data_canonical::assemble().expect("full kit assembles");
    let mut reg = EngineRegistry::new();
    reg.register_lex("data", Box::new(DataLexerEngine::new()));
    reg.register_parse("pratt", PrattEngine::new());
    reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    let registry = Rc::new(reg);
    let mut config = kit.runtime_config().clone();
    config.typecheck.clear();
    Runtime::from_registry(kit.lang().clone(), &registry, &config).expect("runtime binds engines")
}

fn parse_package(name: &str, path: &str) {
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {name}: {e}"));
    let rt = full_runtime();
    let toks = rt
        .lex(&Source::new(&src))
        .unwrap_or_else(|e| panic!("lex {name}: {e}"));
    let _ast = rt
        .parse(&toks)
        .unwrap_or_else(|e| panic!("parse {name}: {e}"));
}

#[test]
fn query_package_parses() {
    parse_package("query", "../../handl-packages/query/index.hnl");
}

#[test]
fn command_package_parses() {
    parse_package("command", "../../handl-packages/command/index.hnl");
}

#[test]
fn event_package_parses() {
    parse_package("event", "../../handl-packages/event/index.hnl");
}
