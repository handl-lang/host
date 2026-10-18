//! Arrow-lowered `>>>` (`flow`) — the `x >>> f` composition must resolve
//! a slot/param-held arrow through the input-stack machinery rather than
//! degrading to a bare projection (`Seq[x, elem k]` reads slot `k` of
//! `x`'s *output*), and an `op`-decl binding used in `>>>` inside a
//! stacked block must keep its unrebased lazy reference rather than the
//! `elem(0)`-rebased read.
//!
//! The lowered `Term` must interpret to the same value the bound eval
//! engine produces — a decline or an interpreter error fails the test
//! (no silent AST fallback).

use handl_kit_arrow_lower::lower_ast;
use handl_language::{AstRef, Source, Value};
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::Runtime;
use plugin_engine::term_interpreter::interpret;
use std::collections::HashMap;

fn canonical_lang() -> handl_language::Language {
    handl_kit_data_canonical::assemble()
        .expect("canonical kit assembles")
        .lang()
        .clone()
}

fn runtime() -> Runtime {
    Runtime::new(canonical_lang())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(Box::new(TreeWalkEngine::new()))
}

/// `src` must lower *and* interpret to the same value the bound eval
/// engine produces — no `lower_ast` decline, no interpreter fallback.
fn assert_lowered(src: &str) {
    let rt = runtime();
    let toks = rt.lex(&Source::new(src)).expect("lex must succeed");
    let ast = rt.parse(&toks).expect("parse+desugar must succeed");
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, rt.lang(), root)
        .unwrap_or_else(|e| panic!("lower_ast declined {src:?}: {e}"));
    let got = interpret(&term, Value::Null)
        .unwrap_or_else(|e| panic!("term interpreter failed on {src:?}: {e}"));
    let want = rt
        .eval_with_globals(&ast, &HashMap::new())
        .unwrap_or_else(|e| panic!("engine eval failed on {src:?}: {e}"));
    assert_eq!(got, want, "{src:?}");
}

// ---------------------------------------------------------------------------
// `x >>> f` with a do-block-bound, param-held arrow inside a nested
// function: `f` rides the input stack / `__bound__`, and `x >>> f`
// dispatches through `Op{flow}` (compose two arrow values or apply).
// ---------------------------------------------------------------------------

/// The reported shape: an inner `(p) ~> (x >>> f)(p)` where `f` was
/// bound to a parameter in the enclosing do-block. `f := g` stacks the
/// param value, the nested function captures it into `__bound__`, and
/// `x >>> f` composes `x` then `f` — applied to `p` it is `f(x(p))`.
#[test]
fn flow_param_alias_nested_capture() {
    assert_lowered(
        "do {\n  outer := (g, x) ~> do {\n    f := g\n    (p) ~> (x >>> f)(p)\n  }\n  outer((n) ~> n + 1, (m) ~> m * 2)(10)\n}",
    );
}

/// Two param-held arrows composed at the top level — `f >>> g` yields
/// the composite arrow and applying it runs `g(f(arg))`.
#[test]
fn flow_two_param_arrows_compose() {
    assert_lowered(
        "do {\n  comp := (f, g) ~> f >>> g\n  comp((n) ~> n + 1, (m) ~> m * 2)(10)\n}",
    );
}

/// `x >>> f` where `f` is a slot-held bound-arrow record (`mk(1)`): the
/// flow applies the held arrow to the lhs value — `f(10)` = `10 + 1`.
#[test]
fn flow_slot_arrow_applies() {
    assert_lowered(
        "do {\n  mk := (k) ~> (n) ~> n + k\n  f := mk(1)\n  10 >>> f\n}",
    );
}

/// `x >>>[0] f` — axis-0 composition routes through `flow` like `>>>`:
/// the param-held `f` applies to `x`'s output.
#[test]
fn axis_flow_zero_param_arrow_applies() {
    assert_lowered(
        "do {\n  comp := (f, g) ~> f >>>[0] g\n  comp((n) ~> n + 1, (m) ~> m * 2)(10)\n}",
    );
}

/// `f := g` inside a nested function: the param alias is a slot, so the
/// inner arrow captures it into `__bound__` — `f(p)` applies `g`.
#[test]
fn param_alias_captured_into_nested_arrow() {
    assert_lowered(
        "do {\n  apply := (g) ~> do {\n    f := g\n    (p) ~> f(p)\n  }\n  apply((n) ~> n + 1)(5)\n}",
    );
}

/// The plain pipeline still composes body arrows directly: `10 >>> f`
/// with `f` a function literal binds through `Seq` unchanged.
#[test]
fn flow_literal_rhs_still_composes() {
    assert_lowered("do {\n  f := (n) ~> n + 1\n  10 >>> f\n}");
}

// ---------------------------------------------------------------------------
// `op`-decl bindings in `>>>` inside stacked blocks: the flow operand
// must use the binding's unrebased `quote` (`Op{inc}`), not the
// `elem(0)`-rebased slot read — `Seq[5, Seq[elem(0), Op{inc}]]`
// projected index 0 of the lhs output instead of applying `inc`.
// ---------------------------------------------------------------------------

#[test]
fn flow_op_decl_in_stacked_block_uses_quote() {
    let rt = runtime();
    let src = "do {\n  op \"inc\" : Num ~> Num\n  v := 1 + 1\n  5 >>> inc\n}";
    let toks = rt.lex(&Source::new(src)).expect("lex must succeed");
    let ast = rt.parse(&toks).expect("parse+desugar must succeed");
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, rt.lang(), root)
        .unwrap_or_else(|e| panic!("lower_ast declined {src:?}: {e}"));
    // `product(id, 1+1) >>> Seq[5, Op{inc}]` — the op reference applies
    // to the lhs output, not an `elem(0)` re-read of it.
    let handl_language::Term::Seq { snd, .. } = &term else {
        panic!("expected Seq root, got {term:?}");
    };
    let handl_language::Term::Seq { snd: op, .. } = snd.as_ref() else {
        panic!("expected nested Seq (the `5 >>> inc` composition), got {snd:?}");
    };
    assert!(
        matches!(
            op.as_ref(),
            handl_language::Term::Op { name, args, .. } if name == "inc" && args.is_empty()
        ),
        "op-decl rhs should be the lazy `Op{{inc}}` reference, got {op:?}"
    );
}

// ---------------------------------------------------------------------------
// `f >>> g` where BOTH operands are env-bound quoted arrows: the node's
// value is the composite `Arrow{first, second}` record (`eval.flow`),
// not `Seq[f, g]` — which runs the composite on the ambient input wherever
// the value escapes the pipeline (bound name, call callee, return).
// ---------------------------------------------------------------------------

/// `g := f >>> f` binds the composite arrow *value*: the slot push
/// `product(id, flow(f, g))` evaluates `flow` once at the bind site and
/// `g(1)` applies the composite — `f(f(1))` = 3. Under the `Seq[f, f]`
/// reading the bound term applied the composition to the block input
/// (`f(f(Null))` — "non-numeric argument").
#[test]
fn flow_quoted_arrows_compose_to_value() {
    assert_lowered("do {\n  f := (n) ~> n + 1\n  g := f >>> f\n  g(1)\n}");
    // Composite returned from a function body, then applied.
    assert_lowered("do {\n  f := (n) ~> n + 1\n  (((x) ~> f >>> f)(0))(5)\n}");
    // `>>>` node in App callee position — evaluates to the composite.
    assert_lowered("do {\n  f := (n) ~> n + 1\n  g := (x) ~> (f >>> f)(x)\n  g(1)\n}");
}

/// The composite value itself is an `Arrow`-tagged record in both the
/// interpreter and the engine (the field payloads differ — serialized
/// `Term`s vs `__arrow__` closures — so only the tag is compared).
#[test]
fn flow_quoted_arrows_yield_arrow_record() {
    let rt = runtime();
    let src = "do {\n  f := (n) ~> n + 1\n  f >>> f\n}";
    let toks = rt.lex(&Source::new(src)).expect("lex must succeed");
    let ast = rt.parse(&toks).expect("parse+desugar must succeed");
    let root = AstRef::new(ast.len() - 1);
    let term = lower_ast(&ast, rt.lang(), root).expect("must lower");
    let got = interpret(&term, Value::Null).expect("interp");
    assert_eq!(
        got.get("__tag__").and_then(Value::as_str),
        Some("Arrow"),
        "composite should be an Arrow record, got {got:?}"
    );
}

/// Value-lhs composition keeps the `Seq` inlining (`10 >>> f` runs the
/// rhs body on the lhs value) — the form compiled backends can run.
#[test]
fn flow_value_lhs_still_applies() {
    assert_lowered("do {\n  f := (n) ~> n + 1\n  10 >>> f\n}");
    assert_lowered("((x) ~> x >>> ((n) ~> n + 1))(5)");
}
