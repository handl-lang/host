//! End-to-end tests for indexed composition `a >>>[k] b` (ADR-0120
//! Phase 5) across all three eval engines.
//!
//! Axis 0 shares the `>>>` path exactly; `k > 0` lowers to
//! `Seq{axis: k}` — the same `∘ₖ` globular paste the derived cell ops
//! (`=>>`, `=>|`, whiskers) use. The tree-walk interprets `Seq{axis}`
//! directly; the bytecode and Cranelift backends lower the paste to the
//! shared `cell_comp_axis` native op, so all three engines agree.
//!
//! Cells generalize past `Cell2`: a `{__dim__: n, __src__, __tgt__,
//! value}` record is an n-cell whose boundaries are `(n-1)`-cells, and
//! the paste's effective dimension is the greater of the declared
//! operand dimension and the evaluated values' `cell_dim` — so
//! `>>>[2]` runs on 3-cell values through the dimension-2 surface
//! scheme, while `1 >>>[2] 1` still fails the dimension law on
//! 1-cells.
//!
//! Two coverage modes per case:
//!
//! - `rt.run` — the shipped pipeline (`Runtime::eval` first lowers the
//!   AST to a `Term` and runs the pure term interpreter, falling back to
//!   the configured eval engine only on failure). This covers the term
//!   interpreter and the engine fallback path.
//! - `engine.eval` — the engine evaluates the parsed AST directly, with
//!   no term-interpreter shortcut. This is the true per-engine parity
//!   check.
//!
//! Test cells are written as `{__dim__: n, ...}` records rather than
//! `{__tag__: "Cell2", ...}` literals: `__tag__` is a reserved key that
//! engine-side object evaluation rejects (ADR-0055), while `__dim__`
//! records are the documented surface n-cell shape.

use handl_language::{Ast, Language, Source};
use handl_plugin_engine_exec::{EvalEngine, EvalRequest};
use handl_plugin_engine_exec_bytecode::BytecodeEngine;
use handl_plugin_engine_exec_cranelift::CraneliftEngine;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::Runtime;
use serde_json::{Value, json};

fn canonical_lang() -> Language {
    handl_kit_data_canonical::assemble()
        .expect("canonical kit assembles")
        .lang()
        .clone()
}

/// Lex+parse only — the eval engine is irrelevant to these stages.
fn parse(src: &str) -> Ast {
    let rt = Runtime::new(canonical_lang())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new());
    rt.parse(
        &rt.lex(&Source::new(src))
            .unwrap_or_else(|e| panic!("lex failed: {e:?}")),
    )
    .unwrap_or_else(|e| panic!("parse failed for {src:?}: {e:?}"))
}

fn runtime_for(engine: Box<dyn EvalEngine>) -> Runtime {
    Runtime::new(canonical_lang())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(engine)
}

fn all_engines() -> Vec<(&'static str, Box<dyn EvalEngine>)> {
    vec![
        ("tree-walk", Box::new(TreeWalkEngine::new())),
        ("bytecode", Box::new(BytecodeEngine::new())),
        ("cranelift", Box::new(CraneliftEngine::new())),
    ]
}

/// Run `src` through the shipped pipeline (`rt.run`) *and* through each
/// eval engine directly (`engine.eval` on the parsed AST). Every run of
/// a `src` marked `ok` must succeed with identical values; `err` runs
/// must fail with `err_substr` in the message.
fn check_all(src: &str, expected: Result<Value, &'static str>) {
    let lang = canonical_lang();
    let ast = parse(src);
    for (name, engine) in all_engines() {
        // Direct engine eval — the parity check with no term-interpreter
        // shortcut.
        match (engine.eval(&lang, &EvalRequest::new(ast.clone())), &expected) {
            (Ok(v), Ok(want)) => assert_eq!(&v, want, "{name}: direct eval value"),
            (Ok(v), Err(_)) => panic!("{name}: expected error, got value {v}"),
            (Err(e), Ok(_)) => panic!("{name}: direct eval failed: {e}"),
            (Err(e), Err(substr)) => assert!(
                e.to_string().contains(substr),
                "{name}: error should contain {substr:?}: {e}"
            ),
        }
        // Shipped pipeline — term interpreter first, engine on fallback.
        let rt = runtime_for(engine);
        match (rt.run(&Source::new(src)), &expected) {
            (Ok(v), Ok(want)) => assert_eq!(&v, want, "{name}: rt.run value"),
            (Ok(v), Err(_)) => panic!("{name}: rt.run expected error, got {v}"),
            (Err(e), Ok(_)) => panic!("{name}: rt.run failed: {e}"),
            (Err(e), Err(substr)) => assert!(
                e.to_string().contains(substr),
                "{name}: rt.run error should contain {substr:?}: {e}"
            ),
        }
    }
}

#[test]
fn axis_flow_zero_matches_flow_on_all_engines() {
    for (name, engine) in all_engines() {
        let rt = runtime_for(engine);
        let plain = rt
            .run(&Source::new("do { inc :: x ~> x + 1; 41 >>> inc }"))
            .unwrap_or_else(|e| panic!("{name}: >>> failed: {e:?}"));
        let indexed = rt
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
    check_all(
        "1 + 2 >>>[1] 3",
        Ok(json!({"__tag__": "Cell2", "__src__": 3, "__tgt__": 3, "value": 3})),
    );
}

#[test]
fn axis_flow_axis1_boundary_mismatch_errors_on_all_engines() {
    // `1 >>>[1] 2`: `tgt(1) ≠ src(2)` — the globular boundary law rejects.
    check_all("1 >>>[1] 2", Err("boundary"));
}

#[test]
fn axis_flow_axis_above_dimension_errors_on_all_engines() {
    // `1 >>>[2] 1`: plain values are 1-cells — axis 2 violates `k < dim`.
    check_all("1 >>>[2] 1", Err("dimension"));
}

/// A `{__dim__: n, __src__, __tgt__, value}` record is an n-cell: its
/// `__src__`/`__tgt__` are the (n-1)-cell boundaries. `a >>>[2] b`
/// pastes two 3-cells along the 2-boundary — `tgt_2(a) = src_2(b)` —
/// producing the `Cell{__dim__: 3}` composite. The compiled engines
/// reach it through `cell_comp_axis:2`, whose effective dimension lifts
/// to the operands' `cell_dim`.
#[test]
fn axis_flow_axis2_pastes_3cells_on_all_engines() {
    let c2 = |src: i64, tgt: i64, w: i64| {
        format!("{{__dim__: 2, __src__: {src}, __tgt__: {tgt}, value: {w}}}")
    };
    let c3 = |s: String, t: String, w: i64| {
        format!("{{__dim__: 3, __src__: {s}, __tgt__: {t}, value: {w}}}")
    };
    let shared = c2(1, 2, 5);
    let a = c3(c2(0, 1, 8), shared.clone(), 100);
    let b = c3(shared.clone(), c2(2, 3, 9), 200);
    // Axis-2 paste keeps the outer (n-1)-boundaries verbatim: src from
    // `a`, tgt from `b`; the witness is the non-identity operand's.
    let expected = json!({
        "__tag__": "Cell",
        "__dim__": 3,
        "__src__": {"__dim__": 2, "__src__": 0, "__tgt__": 1, "value": 8},
        "__tgt__": {"__dim__": 2, "__src__": 2, "__tgt__": 3, "value": 9},
        "value": 200,
    });
    check_all(
        &format!("do {{ a = {a}; b = {b}; a >>>[2] b }}"),
        Ok(expected),
    );
}

/// `>>>[2]` enforces `tgt_2(first) = src_2(second)` on 3-cells.
#[test]
fn axis_flow_axis2_boundary_mismatch_errors_on_all_engines() {
    let a = "{__dim__: 3, __src__: 10, __tgt__: 20, value: 1}";
    let b = "{__dim__: 3, __src__: 30, __tgt__: 40, value: 2}";
    check_all(
        &format!("do {{ a = {a}; b = {b}; a >>>[2] b }}"),
        Err("boundary"),
    );
}

/// `a >>>[1] b` on 3-cells pastes along the 1-boundary: `tgt_1(a)` and
/// `src_1(b)` are whole 2-cells that must be equal, and the composite's
/// other (n-1)-boundaries are the `∘₁` of the operands' boundaries —
/// recursively recomposed, so they come out in the canonical `Cell2`
/// record shape.
#[test]
fn axis_flow_axis1_on_3cells_composes_boundaries_pointwise() {
    let c2 = |src: i64, tgt: i64, w: i64| {
        format!("{{__dim__: 2, __src__: {src}, __tgt__: {tgt}, value: {w}}}")
    };
    let shared = c2(1, 2, 5);
    let a = format!(
        "{{__dim__: 3, __src__: {}, __tgt__: {shared}, value: 100}}",
        c2(0, 1, 8)
    );
    let b = format!(
        "{{__dim__: 3, __src__: {shared}, __tgt__: {}, value: 200}}",
        c2(2, 9, 4)
    );
    // src: `{0,1,8} ∘₁ {1,2,5}` = `Cell2{0,2,5}`;
    // tgt: `{1,2,5} ∘₁ {2,9,4}` = `Cell2{1,9,4}`. The composite's witness
    // is the non-identity operand's `value` (the `Cell2` convention),
    // and the recursively recomposed boundaries land in the canonical
    // `Cell2` record shape.
    let expected = json!({
        "__tag__": "Cell",
        "__dim__": 3,
        "__src__": {"__tag__": "Cell2", "__src__": 0, "__tgt__": 2, "value": 5},
        "__tgt__": {"__tag__": "Cell2", "__src__": 1, "__tgt__": 9, "value": 4},
        "value": 200,
    });
    check_all(
        &format!("do {{ a = {a}; b = {b}; a >>>[1] b }}"),
        Ok(expected),
    );
}

/// A lower-dimensional operand lifts to the paste height as the
/// iterated identity: `c2 >>>[2] c3` promotes the 2-cell to
/// `Cell3{c2, c2, c2}` — so `tgt_2` of the lifted side is `c2` itself.
#[test]
fn axis_flow_axis2_promotes_lower_dimensional_operands() {
    let c2 = "{__dim__: 2, __src__: 1, __tgt__: 2, value: 5}";
    let b = format!(
        "{{__dim__: 3, __src__: {c2}, __tgt__: {{__dim__: 2, __src__: 2, __tgt__: 3, value: 9}}, value: 7}}"
    );
    let expected = json!({
        "__tag__": "Cell",
        "__dim__": 3,
        "__src__": {"__dim__": 2, "__src__": 1, "__tgt__": 2, "value": 5},
        "__tgt__": {"__dim__": 2, "__src__": 2, "__tgt__": 3, "value": 9},
        "value": 7,
    });
    check_all(
        &format!("do {{ c = {c2}; b = {b}; c >>>[2] b }}"),
        Ok(expected),
    );
}

/// `f ~[2]~> g` evaluates to a `dim_arrow` record — an n-cell whose
/// boundaries are `inputs`/`output` — so `alpha >>>[1] beta` composes
/// when `output(alpha) = inputs(beta)`.
#[test]
fn axis_flow_axis1_composes_dim_arrow_values() {
    for (name, engine) in all_engines() {
        let rt = runtime_for(engine);
        let result = rt
            .run(&Source::new(
                "do { f = x ~> x + 1; g = x ~> x + 2; h = x ~> x + 3; \
                 alpha = f ~[2]~> g; beta = g ~[2]~> h; alpha >>>[1] beta }",
            ))
            .unwrap_or_else(|e| panic!("{name}: >>>[1] on dim_arrow values failed: {e:?}"));
        assert_eq!(
            result["__tag__"],
            json!("Cell2"),
            "{name}: dim_arrow paste produces a 2-cell"
        );
        assert_eq!(
            result["value"]["__tag__"],
            json!("Arrow"),
            "{name}: witness is the second cell"
        );
    }
}

/// `compose(diagram)` — the n-ary generalization of `Seq{axis}`
/// (ADR-0120): `[c0, {axis: k, cell: c1}, ...]` folds the steps as
/// binary globular pastes. `arr compose` lifts the op to an arrow.
///
/// `arr <op>` payload dispatch is a tree-walk/term-interpreter feature:
/// the compiled backends reject the `$payload` op name at compile time
/// (a clean "unknown native op" error, not a miscompile), so the direct
/// `engine.eval` coverage for `compose` runs on tree-walk while
/// `rt.run` covers all runtimes through the term interpreter.
#[test]
fn compose_runs_pasting_diagram_on_all_engines() {
    let c2 = |src: i64, tgt: i64, w: i64| {
        format!("{{__dim__: 2, __src__: {src}, __tgt__: {tgt}, value: {w}}}")
    };
    let src = format!(
        "do {{ d = [{}, {{axis: 1, cell: {}}}, {{axis: 1, cell: {}}}]; d >>> arr compose }}",
        c2(1, 2, 10),
        c2(2, 3, 20),
        c2(3, 4, 30)
    );
    let expected = json!({"__tag__": "Cell2", "__src__": 1, "__tgt__": 4, "value": 30});
    for (name, engine) in all_engines() {
        let rt = runtime_for(engine);
        let result = rt
            .run(&Source::new(&src))
            .unwrap_or_else(|e| panic!("{name}: compose failed: {e:?}"));
        assert_eq!(result, expected, "{name}: compose folds the pasting chain");
    }
    // Direct engine eval: tree-walk supports `arr <op>` dispatch.
    let ast = parse(&src);
    let v = TreeWalkEngine::new()
        .eval(&canonical_lang(), &EvalRequest::new(ast))
        .unwrap_or_else(|e| panic!("tree-walk direct compose failed: {e}"));
    assert_eq!(v, expected, "tree-walk: direct compose value");
}

/// `compose` enforces the globular boundary law at each step and
/// reports the failing step.
#[test]
fn compose_step_boundary_mismatch_errors_on_all_engines() {
    let src = "do { d = [{__dim__: 2, __src__: 1, __tgt__: 2, value: 10}, \
               {axis: 1, cell: {__dim__: 2, __src__: 9, __tgt__: 3, value: 20}}]; \
               d >>> arr compose }";
    for (name, engine) in all_engines() {
        let rt = runtime_for(engine);
        let err = rt
            .run(&Source::new(src))
            .expect_err("compose boundary mismatch must fail");
        // The term interpreter's boundary error falls back to the
        // configured engine: tree-walk re-runs `arr compose` and reports
        // the same boundary violation; the compiled backends cannot
        // compile `arr <op>` (`$payload` op-name resolution is a known
        // gap) and surface that clean compile error instead.
        if name == "tree-walk" {
            assert!(
                err.to_string().contains("boundary"),
                "{name}: error should name the boundary violation: {err}"
            );
        } else {
            assert!(
                err.to_string().contains("$payload"),
                "{name}: expected the arr-dispatch gap error: {err}"
            );
        }
    }
    let ast = parse(src);
    let err = TreeWalkEngine::new()
        .eval(&canonical_lang(), &EvalRequest::new(ast))
        .expect_err("tree-walk direct compose mismatch must fail");
    assert!(
        err.to_string().contains("boundary"),
        "tree-walk: error should name the boundary violation: {err}"
    );
}

/// `compose` pastes 3-cell steps at axis 2 — the diagram carries its
/// own dimensions.
#[test]
fn compose_pastes_at_axis2_on_all_engines() {
    let c2 = |src: i64, tgt: i64, w: i64| {
        format!("{{__dim__: 2, __src__: {src}, __tgt__: {tgt}, value: {w}}}")
    };
    let shared = c2(1, 2, 5);
    let a = format!(
        "{{__dim__: 3, __src__: {}, __tgt__: {shared}, value: 100}}",
        c2(0, 1, 8)
    );
    let b = format!(
        "{{__dim__: 3, __src__: {shared}, __tgt__: {}, value: 200}}",
        c2(2, 3, 9)
    );
    let src = format!("do {{ d = [{a}, {{axis: 2, cell: {b}}}]; d >>> arr compose }}");
    for (name, engine) in all_engines() {
        let rt = runtime_for(engine);
        let result = rt
            .run(&Source::new(&src))
            .unwrap_or_else(|e| panic!("{name}: compose at axis 2 failed: {e:?}"));
        assert_eq!(result["__dim__"], json!(3), "{name}: composite dim");
        assert_eq!(result["value"], json!(200), "{name}: composite witness");
    }
}
