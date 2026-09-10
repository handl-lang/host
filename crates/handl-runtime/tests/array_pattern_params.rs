//! Integration tests for array-pattern arrow parameters across all three
//! eval engines.
//!
//! Square-bracket parameter lists (`[x, y] ~> body`) are array-destructuring
//! patterns: the arrow consumes a single array argument and binds fixed
//! names by position, with an optional rest binding capturing the tail.
//! Round-bracket parameter lists (`(x, y) ~> body`) remain positional and
//! continue to flatten a single `Pair` argument.

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

fn run_with<F>(
    make_runtime: F,
    source: &str,
) -> Result<handl_language::Value, handl_language::LangError>
where
    F: FnOnce() -> Runtime,
{
    make_runtime().run(&Source::new(source))
}

fn all_runtimes() -> impl Iterator<Item = (&'static str, fn() -> Runtime)> {
    [
        ("tree-walk", tree_walk_runtime as fn() -> Runtime),
        ("bytecode", bytecode_runtime as fn() -> Runtime),
        ("cranelift", cranelift_runtime as fn() -> Runtime),
    ]
    .into_iter()
}

#[test]
fn array_pattern_two_params_adds() {
    let source = "do { f :: [x, y] ~> x + y; f(3, 4) }";
    for (name, make_rt) in all_runtimes() {
        let result =
            run_with(make_rt, source).unwrap_or_else(|e| panic!("{} failed: {:?}", name, e));
        assert_eq!(
            result,
            json!(7),
            "{}: [x, y] ~> x + y applied to (3, 4)",
            name
        );
    }
}

#[test]
fn array_pattern_rest_params() {
    let source = "do { f :: [x, ...rest] ~> rest; f(1, 2, 3) }";
    for (name, make_rt) in all_runtimes() {
        let result =
            run_with(make_rt, source).unwrap_or_else(|e| panic!("{} failed: {:?}", name, e));
        assert_eq!(
            result,
            json!([2, 3]),
            "{}: [x, ...rest] ~> rest applied to (1, 2, 3)",
            name
        );
    }
}

#[test]
fn positional_pair_params() {
    let source = "do { f :: (x, y) ~> x + y; f(3 *** 4) }";
    for (name, make_rt) in all_runtimes() {
        let result =
            run_with(make_rt, source).unwrap_or_else(|e| panic!("{} failed: {:?}", name, e));
        assert_eq!(
            result,
            json!(7),
            "{}: (x, y) ~> x + y applied to 3 *** 4",
            name
        );
    }
}

#[test]
fn positional_two_params_still_works() {
    let source = "do { f :: (x, y) ~> x + y; f(3, 4) }";
    for (name, make_rt) in all_runtimes() {
        let result =
            run_with(make_rt, source).unwrap_or_else(|e| panic!("{} failed: {:?}", name, e));
        assert_eq!(
            result,
            json!(7),
            "{}: (x, y) ~> x + y applied to (3, 4)",
            name
        );
    }
}
