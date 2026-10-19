//! Regression: a `:=` binding whose RHS references its own name keeps
//! working on the AST-fallback path.
//!
//! The arrow lowerer declines self-recursive definitions by design (the
//! calculus has no recursion), so these run through the env engines.
//! There the arrow's `binds` gets a serializable self-ref marker under
//! the bound name (`handl_value::{self_ref_marker, with_self_ref_bind}`)
//! and `apply_arrow_body` seeds the marker name with the applied arrow —
//! previously the name was unbound at mint time, never reached `binds`,
//! and the recursive call fell through to `call_host` ("function 'alen'
//! not found").
//!
//! Companion: a *mention* of the bound name that is not a `var`
//! reference — the `x := "x"` bare-name import lowering — must not trip
//! the decline: `decline_self_reference` counts `var` (leaf-role) nodes
//! only. That is exercised by `bare_module_import.rs`; here the literal
//! `s := "s"` case pins the lowerer side directly.

use handl_language::Source;
use hnl_engine_exec_bytecode::BytecodeEngine;
use hnl_engine_exec_cranelift::CraneliftEngine;
use hnl_engine_exec_tree_walk::TreeWalkEngine;
use hnl_engine_lex_pattern::DataLexerEngine;
use hnl_engine_parse_pratt::PrattEngine;
use handl_runtime::Runtime;
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

fn run_all(source: &str, expected: serde_json::Value) {
    for (name, make_rt) in [
        ("tree-walk", tree_walk_runtime as fn() -> Runtime),
        ("bytecode", bytecode_runtime as fn() -> Runtime),
        ("cranelift", cranelift_runtime as fn() -> Runtime),
    ] {
        let result = make_rt()
            .run(&Source::new(source))
            .unwrap_or_else(|e| panic!("{name} failed on {source:?}: {e:?}"));
        assert_eq!(result, expected, "{name}: {source:?}");
    }
}

#[test]
fn self_recursive_match_reaches_base_case() {
    run_all(
        "do { alen := (xs) ~> match xs { []: 0, [_, ...t]: 1 + alen(t) }; alen([1, 2, 3]) }",
        json!(3),
    );
}

#[test]
fn self_recursion_at_depth() {
    run_all(
        "do { fib := (n) ~> if n < 2 { n } else { fib(n - 1) + fib(n - 2) }; fib(10) }",
        json!(55),
    );
}

#[test]
fn name_spelled_in_a_string_literal_is_not_self_reference() {
    // `s := "s"` — the bound name appears only as literal text (the same
    // shape the `x := "x"` bare-name import lowering produces). It is a
    // mention, not a `var` reference, so the lowerer must not decline it
    // as self-recursive; either way the program evaluates to "s".
    run_all("do { s := \"s\"; s }", json!("s"));
}
