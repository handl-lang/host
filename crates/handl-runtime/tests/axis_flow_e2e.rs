//! End-to-end tests for indexed composition `a >>>[k] b` (ADR-0120
//! Phase 5) across all three eval engines.
//!
//! Axis 0 shares the `>>>` path exactly; `k > 0` lowers to
//! `Seq{axis: k}` over dimension-2 operands — the same `∘ₖ` globular
//! paste the derived cell ops (`=>>`, `=>|`, whiskers) use. The
//! tree-walk interprets `Seq{axis}` directly; the bytecode and
//! Cranelift backends lower the paste to the shared `cell2_comp_axis`
//! native op, so all three engines agree.

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

#[test]
fn axis_flow_zero_matches_flow_on_all_engines() {
    for (name, make_rt) in all_runtimes() {
        let plain = make_rt()
            .run(&Source::new("do { inc :: x ~> x + 1; 41 >>> inc }"))
            .unwrap_or_else(|e| panic!("{name}: >>> failed: {e:?}"));
        let indexed = make_rt()
            .run(&Source::new("do { inc :: x ~> x + 1; 41 >>>[0] inc }"))
            .unwrap_or_else(|e| panic!("{name}: >>>[0] failed: {e:?}"));
        assert_eq!(indexed, plain, "{name}: >>>[0] must equal >>>");
    }
}

#[test]
fn axis_flow_axis1_pastes_on_all_engines() {
    // `1 + 2 >>>[1] 3` — both operands see the same incoming; the `∘₁`
    // paste composes the degenerate cells `3` and `3` (their boundaries
    // meet), yielding the `Cell2` composite — the same value `=>>`
    // builds before projecting its witness.
    for (name, make_rt) in all_runtimes() {
        let result = make_rt()
            .run(&Source::new("1 + 2 >>>[1] 3"))
            .unwrap_or_else(|e| panic!("{name}: >>>[1] failed: {e:?}"));
        assert_eq!(
            result,
            json!({"__tag__": "Cell2", "__src__": 3, "__tgt__": 3, "value": 3}),
            "{name}: axis-1 paste"
        );
    }
}

#[test]
fn axis_flow_axis1_boundary_mismatch_errors_on_all_engines() {
    // `1 >>>[1] 2`: `tgt(1) ≠ src(2)` — the globular boundary law rejects.
    for (name, make_rt) in all_runtimes() {
        let err = make_rt()
            .run(&Source::new("1 >>>[1] 2"))
            .expect_err("boundary mismatch must fail");
        assert!(
            err.to_string().contains("boundary"),
            "{name}: error should name the boundary violation: {err}"
        );
    }
}

#[test]
fn axis_flow_axis_above_dimension_errors_on_all_engines() {
    // `1 >>>[2] 1`: `Cell2` is dimension 2 — axis 2 violates `k < dim`.
    for (name, make_rt) in all_runtimes() {
        let err = make_rt()
            .run(&Source::new("1 >>>[2] 1"))
            .expect_err("axis 2 exceeds the 2-cell dimension");
        assert!(
            err.to_string().contains("dimension"),
            "{name}: error should name the dimension violation: {err}"
        );
    }
}
