//! End-to-end runtime tests for the Option B frontier operators
//! (`=>>`, `=>|`, `<@`, `@>`, `#`, `?>`), `id2`, and `where`.
//!
//! These tests drive the canonical kit through the data lexer, Pratt parser,
//! and tree-walk eval engine using raw `hnl` source strings.

use std::rc::Rc;

use handl_kit_data_canonical;
use handl_language::Source;
use hnl_engine_exec_tree_walk::TreeWalkEngine;
use hnl_engine_lex_pattern::DataLexerEngine;
use hnl_engine_parse_pratt::PrattEngine;
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
    Runtime::from_registry(kit.lang().clone(), &registry, &config).expect("frontier runtime binds")
}

#[test]
fn cell_vcomp_composes_end_to_end() {
    let rt = frontier_runtime();
    // `1 + 2` (=> 3) is fed as the incoming value to the second cell (`3`).
    let result = rt
        .run(&Source::new("1 + 2 =>> 3"))
        .expect("cell_vcomp runs");
    assert_eq!(result, json!(3));
}

#[test]
fn axis_flow_axis1_matches_cell_comp_v() {
    let rt = frontier_runtime();
    // `a >>>[1] b` lowers to `Seq{axis: 1}` — the same `∘₁` paste the
    // derived `cell_comp_v` (`=>>`) performs. `=>>` additionally projects
    // the composite's witness `value`, so the indexed composite is the
    // `Cell2` record whose witness is the `=>>` result.
    let indexed = rt
        .run(&Source::new("1 + 2 >>>[1] 3"))
        .expect("axis_flow >>>[1] runs");
    let derived = rt.run(&Source::new("1 + 2 =>> 3")).expect("=>> runs");
    assert_eq!(
        indexed,
        json!({
            "__tag__": "Cell2",
            "__src__": 3,
            "__tgt__": 3,
            "value": derived
        })
    );
}

#[test]
fn axis_flow_axis1_checks_boundary() {
    let rt = frontier_runtime();
    // `1 >>>[1] 2`: the first cell's target (`1`) does not meet the
    // second cell's source (`2`) — the globular boundary law rejects
    // the paste.
    let err = rt
        .run(&Source::new("1 >>>[1] 2"))
        .expect_err("boundary mismatch must fail");
    assert!(
        err.to_string().contains("boundary"),
        "error should name the boundary violation: {err}"
    );
}

#[test]
fn axis_flow_axis_above_dimension_errors() {
    let rt = frontier_runtime();
    // `1 >>>[2] 1`: the runtime cell shape is `Cell2` (dimension 2), so
    // axis 2 violates the `k < dim` law — the shared executor reports
    // the dimension error rather than silently degrading to `>>>`.
    let err = rt
        .run(&Source::new("1 >>>[2] 1"))
        .expect_err("axis 2 exceeds the 2-cell dimension");
    assert!(
        err.to_string().contains("dimension"),
        "error should name the dimension violation: {err}"
    );
}

#[test]
fn cell_hcomp_composes_end_to_end() {
    let rt = frontier_runtime();
    // Horizontal composition derives from two whiskers + vertical
    // composition (ADR-0104b). The returned 2-cell carries composed 1-cell
    // arrows as its source/target and the second cell's witness value.
    let result = rt.run(&Source::new("1 =>| 2")).expect("cell_hcomp runs");
    let arrow = json!({"__tag__": "Arrow", "first": 1, "second": 2});
    assert_eq!(
        result,
        json!({
            "__tag__": "Cell2",
            "__src__": arrow,
            "__tgt__": arrow,
            "value": 2
        })
    );
}

#[test]
fn left_whisker_composes_end_to_end() {
    let rt = frontier_runtime();
    let result = rt.run(&Source::new("1 <@ 2")).expect("left whisker runs");
    let arrow = json!({"__tag__": "Arrow", "first": 1, "second": 2});
    assert_eq!(
        result,
        json!({
            "__tag__": "Cell2",
            "__src__": arrow,
            "__tgt__": arrow,
            "value": 2
        })
    );
}

#[test]
fn right_whisker_composes_end_to_end() {
    let rt = frontier_runtime();
    // Surface `@>` is "cell @> arrow": the left operand is the cell, the
    // right operand is the 1-cell arrow whiskered onto the right side.
    let result = rt.run(&Source::new("1 @> 2")).expect("right whisker runs");
    let arrow = json!({"__tag__": "Arrow", "first": 1, "second": 2});
    assert_eq!(
        result,
        json!({
            "__tag__": "Cell2",
            "__src__": arrow,
            "__tgt__": arrow,
            "value": 1
        })
    );
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
fn transport_returns_value_along_path() {
    let rt = frontier_runtime();
    let result = rt.run(&Source::new("1 # 2")).expect("transport runs");
    assert_eq!(result, json!(2));
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
    // `Storable ?> 42` evaluates the body. The `Storable` grade is also
    // collected into the runtime effect row, but the effect row is not yet
    // exposed as a return value, so we assert the body value here.
    let result = rt.run(&Source::new("Storable ?> 42")).expect("cobind runs");
    assert_eq!(result, json!(42));
}

#[test]
fn graded_arrow_lexes_parses_runs() {
    let rt = frontier_runtime();
    // `add ~[Addable]~> Int` produces an `Arrow` record carrying the grade.
    // Effect collection is not yet exposed at runtime, so we assert the record.
    let result = rt
        .run(&Source::new("add ~[Addable]~> Int"))
        .expect("graded arrow runs");
    assert_eq!(
        result.get("__tag__").and_then(|v| v.as_str()),
        Some("Arrow")
    );
}
