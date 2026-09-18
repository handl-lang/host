//! Integration tests for `true`/`false` as imported value globals across all
//! three eval engines.
//!
//! `true` and `false` are no longer lexer literals: they are ordinary
//! identifiers bound to Bool values exported by the `bool` package
//! (`handl-packages/utils/bool.hnl`) and seeded into every runtime from
//! `Language::globals` (`data/globals.yaml`). In pattern position a global
//! name compares against its bound value rather than binding unconditionally.

use handl_language::Source;
use handl_plugin_engine_exec_bytecode::BytecodeEngine;
use handl_plugin_engine_exec_cranelift::CraneliftEngine;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::Runtime;
use serde_json::json;

fn canonical_lang() -> handl_language::Language {
    handl_kit_data_canonical::assemble()
        .expect("canonical kit assembles")
        .lang()
        .clone()
}

fn tree_walk_runtime() -> Runtime {
    Runtime::new(canonical_lang())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(Box::new(TreeWalkEngine::new()))
}

fn bytecode_runtime() -> Runtime {
    Runtime::new(canonical_lang())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(Box::new(BytecodeEngine::new()))
}

fn cranelift_runtime() -> Runtime {
    Runtime::new(canonical_lang())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(Box::new(CraneliftEngine::new()))
}

fn all_runtimes() -> impl Iterator<Item = (&'static str, fn() -> Runtime)> {
    [
        ("tree-walk", tree_walk_runtime as fn() -> Runtime),
        ("bytecode", bytecode_runtime as fn() -> Runtime),
        ("cranelift", cranelift_runtime as fn() -> Runtime),
    ]
    .into_iter()
}

fn run_all(source: &str, expected: serde_json::Value) {
    for (name, make_rt) in all_runtimes() {
        let result = make_rt()
            .run(&Source::new(source))
            .unwrap_or_else(|e| panic!("{name} failed on {source:?}: {e:?}"));
        assert_eq!(result, expected, "{name}: {source:?}");
    }
}

#[test]
fn true_and_false_are_bool_globals() {
    run_all("true", json!(true));
    run_all("false", json!(false));
}

#[test]
fn bool_globals_drive_conditions_and_negation() {
    run_all("if true { 1 } else { 0 }", json!(1));
    run_all("if false { 1 } else { 0 }", json!(0));
    run_all("!false", json!(true));
    run_all("!true", json!(false));
}

#[test]
fn bool_globals_in_bindings_and_equality() {
    run_all("do { x := true; x == true }", json!(true));
    run_all("do { x := false; x == false }", json!(true));
    run_all("do { x := true; x == false }", json!(false));
}

#[test]
fn bool_globals_compare_in_match_patterns() {
    // `true`/`false` in pattern position must compare against the bound
    // global value — they are not unconditional binders.
    run_all(
        "match(true) { true: \"yes\", false: \"no\", _: \"?\" }",
        json!("yes"),
    );
    run_all(
        "match(false) { true: \"yes\", false: \"no\", _: \"?\" }",
        json!("no"),
    );
    run_all("match(1) { true: \"yes\", _: \"?\" }", json!("?"));
}

#[test]
fn bool_globals_shadowed_by_local_bindings() {
    run_all("do { true := 7; true }", json!(7));
}

#[test]
fn bool_values_as_record_fields_and_keys() {
    run_all("do { r := { t: true, f: false }; r.t }", json!(true));
    run_all(
        "do { r := { \"true\": 1, \"false\": 0 }; r[\"true\"] }",
        json!(1),
    );
}
