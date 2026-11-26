//! Integration tests for object-pattern arrow parameters across all three
//! eval engines.
//!
//! Brace parameter lists (`({x, y}) ~> body`, `({x: a, y: b}) ~> body`) are
//! object-destructuring patterns: the arrow consumes a single object
//! argument and binds fields by key. The `{x, y}` shorthand parses the
//! `var` itself as the pattern child (no `entry` wrapper) — field `x`
//! binds var `x`, matching `Language::param_bound_names` and the
//! typechecker's `bind_pattern_node` (ADR-0081).

use handl_language::Source;
use handl_runtime::Runtime;
use hnl_exec_bytecode::BytecodeEngine;
use hnl_exec_cranelift::CraneliftEngine;
use hnl_exec_tree_walk::TreeWalkEngine;
use hnl_tool_lex_pattern::DataLexerEngine;
use hnl_tool_parse_pratt::PrattEngine;
use serde_json::json;

fn canonical_lang() -> handl_language::Language {
    handl_kit_data_prelude::assemble()
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
fn object_pattern_shorthand_binds_fields() {
    // `({x, y}) ~> x + y` applied to `{x: 1, y: 2}` → 3. The shorthand
    // children are bare `var` leaves; each binds the same-named field.
    let source = "do { f := ({x, y}) ~> x + y; f({x: 1, y: 2}) }";
    for (name, make_rt) in all_runtimes() {
        let result =
            run_with(make_rt, source).unwrap_or_else(|e| panic!("{} failed: {:?}", name, e));
        assert_eq!(
            result,
            json!(3),
            "{}: ({{x, y}}) ~> x + y applied to {{x: 1, y: 2}}",
            name
        );
    }
}

#[test]
fn object_pattern_long_form_binds_fields() {
    // `({x: a, y: b}) ~> a + b` — `entry` children rename the binders.
    let source = "do { f := ({x: a, y: b}) ~> a + b; f({x: 1, y: 2}) }";
    for (name, make_rt) in all_runtimes() {
        let result =
            run_with(make_rt, source).unwrap_or_else(|e| panic!("{} failed: {:?}", name, e));
        assert_eq!(
            result,
            json!(3),
            "{}: ({{x: a, y: b}}) ~> a + b applied to {{x: 1, y: 2}}",
            name
        );
    }
}

#[test]
fn object_pattern_mixed_shorthand_and_entry() {
    // `({x, y: b}) ~> x + b` — shorthand and `entry` children compose.
    let source = "do { f := ({x, y: b}) ~> x + b; f({x: 1, y: 2}) }";
    for (name, make_rt) in all_runtimes() {
        let result =
            run_with(make_rt, source).unwrap_or_else(|e| panic!("{} failed: {:?}", name, e));
        assert_eq!(
            result,
            json!(3),
            "{}: ({{x, y: b}}) ~> x + b applied to {{x: 1, y: 2}}",
            name
        );
    }
}

#[test]
fn object_pattern_missing_field_fails_match() {
    // A shorthand field absent from the scrutinee is a match failure.
    let source = "do { f := ({x, y}) ~> x + y; f({x: 1}) }";
    for (name, make_rt) in all_runtimes() {
        assert!(
            run_with(make_rt, source).is_err(),
            "{}: ({{x, y}}) ~> x + y applied to {{x: 1}} must fail",
            name
        );
    }
}
