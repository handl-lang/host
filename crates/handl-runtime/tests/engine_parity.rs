//! Engine parity corpus — the same desugared AST evaluated by all three
//! eval engines must produce the same value.
//!
//! Unlike `Runtime::run` (which first tries `handl_kit_arrow_lower` + the
//! shared `Term` interpreter and only falls back to the bound eval engine
//! on a decline), these tests call [`Runtime::eval_with_globals`], which
//! dispatches straight to the bound engine. That forces every case through
//! the tree-walk, bytecode, and cranelift backends so backend divergences
//! cannot hide behind the shared lowerer/interpreter path.
//!
//! A second sweep ([`run_pipeline`]) runs the same sources through
//! `Runtime::run` — the user-facing path — where agreement is required
//! regardless of which internal path each runtime took.
//!
//! Tree-walk is the reference engine: where the three disagree, the
//! fix should bring the divergent engine in line with tree-walk rather
//! than weaken semantics.

use std::collections::HashMap;

use handl_language::{LangError, Source, Value};
use hnl_engine_exec::EvalEngine;
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

fn runtime(engine: Box<dyn EvalEngine>) -> Runtime {
    Runtime::new(canonical_lang())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(engine)
}

fn tree_walk() -> Runtime {
    runtime(Box::new(TreeWalkEngine::new()))
}

fn bytecode() -> Runtime {
    runtime(Box::new(BytecodeEngine::new()))
}

fn cranelift() -> Runtime {
    runtime(Box::new(CraneliftEngine::new()))
}

/// Lex+parse `src` once and evaluate the (desugared) AST directly on the
/// bound eval engine — the lowerer is bypassed, so this is the pure
/// engine-semantics path.
fn eval_engine_path(rt: &Runtime, src: &str) -> Result<Value, LangError> {
    let toks = rt.lex(&Source::new(src))?;
    let ast = rt.parse(&toks)?;
    rt.eval_with_globals(&ast, &HashMap::new())
}

/// Every source must evaluate to the same `expected` value on all three
/// engines when the engine path is taken directly.
fn assert_engine_parity(src: &str, expected: Value) {
    for (name, rt) in [
        ("tree-walk", tree_walk()),
        ("bytecode", bytecode()),
        ("cranelift", cranelift()),
    ] {
        let got = eval_engine_path(&rt, src)
            .unwrap_or_else(|e| panic!("{name} engine-path failed on {src:?}: {e}"));
        assert_eq!(got, expected, "{name} engine-path on {src:?}");
    }
}

/// The user-facing `Runtime::run` path (lowerer first, engine fallback)
/// must also agree across engines.
fn assert_run_parity(src: &str, expected: Value) {
    for (name, rt) in [
        ("tree-walk", tree_walk()),
        ("bytecode", bytecode()),
        ("cranelift", cranelift()),
    ] {
        let got = rt
            .run(&Source::new(src))
            .unwrap_or_else(|e| panic!("{name} run-path failed on {src:?}: {e}"));
        assert_eq!(got, expected, "{name} run-path on {src:?}");
    }
}

/// Both the engine path and the run path, all three engines.
fn assert_parity(src: &str, expected: Value) {
    assert_engine_parity(src, expected.clone());
    assert_run_parity(src, expected);
}

/// Every engine must fail on `src` (engine path) with an error containing
/// `needle` — parity in the *error* case.
fn assert_engine_parity_err(src: &str, needle: &str) {
    for (name, rt) in [
        ("tree-walk", tree_walk()),
        ("bytecode", bytecode()),
        ("cranelift", cranelift()),
    ] {
        let err = match eval_engine_path(&rt, src) {
            Err(e) => e,
            Ok(v) => panic!("{name} engine-path should fail on {src:?}, got {v}"),
        };
        assert!(
            err.to_string().contains(needle),
            "{name} engine-path on {src:?}: expected error containing {needle:?}, got: {err}"
        );
    }
}

// ===========================================================================
// Arithmetic, comparison, unary
// ===========================================================================

#[test]
fn arithmetic_ops() {
    assert_parity("1 + 2 * 3", json!(7));
    assert_parity("(1 + 2) * 3", json!(9));
    assert_parity("10 - 4 - 3", json!(3));
    assert_parity("10 % 3", json!(1));
    // `**` produces a Float.
    assert_parity("2 ** 8", json!(256.0));
    // Int/Int division is integer division (the `Num::Div` contract every
    // engine implements; the term interpreter mirrors it — see the
    // `hnl-engine` fix note in this file's header history).
    assert_parity("7 / 2", json!(3));
    assert_parity("7.0 / 2", json!(3.5));
    assert_parity("6 / 3", json!(2));
    assert_parity("0 - 5", json!(-5));
    assert_parity("-(3 + 4)", json!(-7));
}

#[test]
fn comparison_ops() {
    assert_parity("1 < 2", json!(true));
    assert_parity("2 <= 2", json!(true));
    assert_parity("3 > 2", json!(true));
    assert_parity("3 >= 4", json!(false));
    assert_parity("1 == 1", json!(true));
    assert_parity("1 != 2", json!(true));
    assert_parity("\"a\" == \"a\"", json!(true));
    assert_parity("\"a\" == \"b\"", json!(false));
}

#[test]
fn cross_numeric_equality() {
    // `==` compares numeric values across Int/Float representations.
    assert_parity("1 == 1.0", json!(true));
    assert_parity("2 != 2.0", json!(false));
    assert_parity("1.5 == 1.5", json!(true));
}

#[test]
fn unary_ops() {
    assert_parity("!true", json!(false));
    assert_parity("!false", json!(true));
    assert_parity("!!true", json!(true));
}

// ===========================================================================
// Conditionals: if, ternary, nullish, &&, ||, ?.
// ===========================================================================

#[test]
fn if_else() {
    assert_parity("if 1 < 2 { \"y\" } else { \"n\" }", json!("y"));
    assert_parity("if 2 < 1 { \"y\" } else { \"n\" }", json!("n"));
    assert_parity("if true { 1 } else { 0 }", json!(1));
    // Truthiness in condition position: `""`/`0` falsy, `[]` truthy.
    assert_parity("if 0 { 1 } else { 2 }", json!(2));
    assert_parity("if \"\" { 1 } else { 2 }", json!(2));
    assert_parity("if [] { 1 } else { 2 }", json!(1));
    assert_parity("!0", json!(true));
    assert_parity("!\"\"", json!(true));
    assert_parity("!\"x\"", json!(false));
}

#[test]
fn ternary() {
    assert_parity("1 < 2 ? \"y\" : \"n\"", json!("y"));
    assert_parity("2 < 1 ? \"y\" : \"n\"", json!("n"));
}

#[test]
fn nullish_and_opt_chain() {
    assert_parity("null ?? 7", json!(7));
    // `x ?? d` desugars to an Option-style match on `{_tag: ...}` records:
    // a plain non-record scrutinee falls through to the default arm.
    assert_parity("5 ?? 7", json!(7));
    assert_parity("{ \"_tag\": \"Some\", \"value\": 5 } ?? 7", json!(5));
    assert_parity("{ \"_tag\": \"None\" } ?? 7", json!(7));
    // `?.` desugars to the same `{_tag: ...}` Option shape: a plain record
    // is not a `Some`, so the chain yields `None` — identical on all
    // engines because they share the desugared AST.
    assert_parity("do { o := { x: 5 }; o?.x }", json!({"_tag": "None"}));
    assert_parity("null?.x", json!({"_tag": "None"}));
}

#[test]
fn logical_and_or_short_circuit() {
    assert_parity("true && false", json!(false));
    assert_parity("true && true", json!(true));
    assert_parity("false || true", json!(true));
    assert_parity("false || false", json!(false));
    // Short-circuit: the RHS must not be evaluated (unbound name).
    assert_parity("false && missing_name", json!(false));
    assert_parity("true || missing_name", json!(true));
    // `&&`/`||` produce Bool, not operand values.
    assert_parity("true && (41 + 1 == 42)", json!(true));
    // Truthiness: `0`/`""`/`null`/`false` are falsy; everything else —
    // including `[]` — is truthy (`is_truthy` in `hnl-engine-exec`).
    assert_parity("1 && 2", json!(true));
    assert_parity("\"\" || \"x\"", json!(true));
    assert_parity("null || false", json!(false));
    assert_parity("0 && true", json!(false));
    // A bare unbound `var` evaluates to its own name string — the
    // self-evaluating-name fallback — so it is truthy-safe on the RHS.
    assert_parity("definitely_unbound_name", json!("definitely_unbound_name"));
}

// ===========================================================================
// match — every pattern shape
// ===========================================================================

#[test]
fn match_literal_and_binder() {
    assert_parity(
        "match 2 { 1: \"one\", 2: \"two\", _: \"other\" }",
        json!("two"),
    );
    assert_parity("match \"x\" { \"x\": 1, _: 0 }", json!(1));
    assert_parity("match 1.0 { 1: \"hit\", _: \"miss\" }", json!("hit"));
    assert_parity("match null { null: \"n\", _: \"z\" }", json!("n"));
    assert_parity("match 7 { x: x + 1 }", json!(8));
    assert_parity("match true { true: \"yes\", _: \"z\" }", json!("yes"));
    assert_parity(
        "match false { true: \"yes\", false: \"no\", _: \"z\" }",
        json!("no"),
    );
}

#[test]
fn match_array_patterns() {
    assert_parity("match [1, 2] { []: 0, [h, ...t]: h, _: 9 }", json!(1));
    assert_parity("match [] { []: 0, [h, ...t]: h, _: 9 }", json!(0));
    assert_parity("match \"abc\" { []: 0, _: 9 }", json!(9));
    assert_parity(
        "match [1, 2, 3] { [a, b, c]: a + b + c, _: 0 }",
        json!(6),
    );
    assert_parity("match [1, 2, 3] { [...t]: t, _: 0 }", json!([1, 2, 3]));
    assert_parity(
        "match [\"-\", \"a\"] { [\"-\", ...t]: t, _: [] }",
        json!(["a"]),
    );
    assert_parity("match [1, 2] { [1, 2]: \"hit\", _: \"miss\" }", json!("hit"));
    assert_parity("match [1, 3] { [1, 2]: \"hit\", _: \"miss\" }", json!("miss"));
    assert_parity("match [1] { [1, 2]: \"hit\", _: \"miss\" }", json!("miss"));
    assert_parity(
        "match [[1, 2], [3]] { [[a, b], ...t]: a + b, _: 0 }",
        json!(3),
    );
}

#[test]
fn match_object_patterns() {
    assert_parity(
        "match { \"a\": 5, \"b\": 2 } { { \"a\": x }: x, _: 0 }",
        json!(5),
    );
    assert_parity(
        "match { \"a\": 5, \"b\": 2 } { { \"a\": 5 }: \"hit\", _: \"miss\" }",
        json!("hit"),
    );
    assert_parity(
        "match { \"a\": 5, \"b\": 2 } { { \"a\": 6 }: \"hit\", _: \"miss\" }",
        json!("miss"),
    );
    // Missing key / non-object falls through.
    assert_parity(
        "match { \"a\": 5 } { { \"c\": x }: x, _: 0 }",
        json!(0),
    );
    assert_parity("match 5 { { \"a\": x }: x, _: 0 }", json!(0));
    // Rest binding captures the remaining fields.
    assert_parity(
        "match { \"a\": 5, \"b\": 2 } { { \"a\": x, ...r }: r.b, _: 0 }",
        json!(2),
    );
    // Nested sub-patterns under entries.
    assert_parity(
        "match { \"k\": { \"v\": 7 } } { { \"k\": { \"v\": x } }: x, _: 0 }",
        json!(7),
    );
    assert_parity(
        "match { \"a\": [1, 2] } { { \"a\": [x, ...t] }: x, _: 0 }",
        json!(1),
    );
}

#[test]
fn match_runtime_object_keys() {
    // Runtime-valued computed object-pattern keys: the arrow-lowered
    // path resolves the key via the `[v, in, keys]` extended pair and
    // the two-arg `has_field`/`field_access`/`obj_except` forms; the
    // engine path evaluates the key expression per try_object_pattern.
    assert_parity(
        "((k) ~> match { \"a\": 1, \"b\": 2 } { {[k]: x}: x, _: 0 })(\"a\")",
        json!(1),
    );
    // Key misses the object → wildcard arm.
    assert_parity(
        "((k) ~> match { \"a\": 1, \"b\": 2 } { {[k]: x}: x, _: 0 })(\"zz\")",
        json!(0),
    );
    // Non-object scrutinee: is_obj gates the key-forcing stage.
    assert_parity(
        "((k) ~> match 5 { {[k]: x}: x, _: 0 })(\"a\")",
        json!(0),
    );
    // Nested object pattern carrying a runtime key.
    assert_parity(
        "((k) ~> match { \"a\": { \"b\": 7 } } { { \"a\": {[k]: x} }: x, _: 0 })(\"b\")",
        json!(7),
    );
    // `...rest` with a dynamic key — the rest object excludes the
    // evaluated matched key.
    assert_parity(
        "((k) ~> match { \"b\": 1, \"a\": 2 } { {[k]: x, ...r}: r.b, _: 0 })(\"a\")",
        json!(1),
    );
    // Env-bound runtime key (non-const expression).
    assert_parity(
        "do { o :: {\"k\": \"ab\"}, match { \"ab\": 7 } { {[o.k]: x}: x, _: 0 } }",
        json!(7),
    );
    // Mixed static + runtime keys.
    assert_parity(
        "((k) ~> match { \"a\": 1, \"b\": 2 } { { \"a\": 1, [k]: x }: x, _: 0 })(\"b\")",
        json!(2),
    );
}

#[test]
fn match_tag_record_patterns() {
    // `{_tag: "Some", value: x}` — the `??`/`?.` desugar shape.
    assert_parity(
        "match { \"_tag\": \"Some\", \"value\": 5 } { { \"_tag\": \"Some\", \"value\": x }: x, _: 0 }",
        json!(5),
    );
    assert_parity(
        "match { \"_tag\": \"None\" } { { \"_tag\": \"Some\", \"value\": x }: x, _: 0 }",
        json!(0),
    );
}

#[test]
fn match_no_arm_matched_errors() {
    assert_engine_parity_err("match 3 { 1: \"one\", 2: \"two\" }", "no arm matched");
}

// ===========================================================================
// while fold / for desugar
// ===========================================================================

#[test]
fn while_fold() {
    // `while <init> { <step> }`: the step arrow returns `{out: v}` to stop
    // or `{state: s}` to continue — a fold over an explicit state record.
    assert_parity(
        "while { n: 5, acc: 0 } { st ~> if st.n == 0 { { out: st.acc } } else { { state: { n: st.n - 1, acc: st.acc + st.n } } } }",
        json!(15),
    );
    assert_parity(
        "while { xs: [1, 2, 3], acc: 0 } { st ~> match st.xs { []: { out: st.acc }, [h, ...t]: { state: { xs: t, acc: st.acc + h } } } }",
        json!(6),
    );
    // Imperative form: `x = e` updates an enclosing-scope binding.
    assert_parity(
        "do { x := 0; while x < 5 { x = x + 1 }; x }",
        json!(5),
    );
    // Post-test `do { body } while cond` — desugars to the same loop
    // machinery; runs the body at least once.
    assert_parity(
        "do { x := 0; do { x = x + 1 } while x < 5; x }",
        json!(5),
    );
    assert_parity(
        "do { x := 10; do { x = x + 1 } while x < 5; x }",
        json!(11),
    );
}

#[test]
fn for_desugar_collects() {
    // `for x in xs { body }` currently desugars to a fold that yields
    // `[]` — parity is what matters here, not the collection semantics.
    assert_parity("for x in [1, 2, 3] { x }", json!([]));
}

// ===========================================================================
// Arrows: literals, application, currying, rest params, combinators
// ===========================================================================

#[test]
fn arrow_literals_and_application() {
    assert_parity("((x) ~> x + 1)(5)", json!(6));
    assert_parity("do { f := (x) ~> x * 2; f(21) }", json!(42));
    assert_parity("do { f := (a, b) ~> a + b; f(1, 2) }", json!(3));
}

#[test]
fn arrow_currying() {
    // `f :: x ~> y ~> body` — applying more args than the param list
    // curries: `(f 1) 2`.
    assert_parity("do { f := (x) ~> (y) ~> x + y; f(1, 2) }", json!(3));
    assert_parity("do { f := (x) ~> (y) ~> x * y; f(6)(7) }", json!(42));
}

#[test]
fn arrow_rest_params() {
    // `[a, ...rest] ~> body` binds surplus args as an array.
    assert_parity(
        "do { f := [h, ...t] ~> h; f(1, 2, 3) }",
        json!(1),
    );
    assert_parity(
        "do { f := [h, ...t] ~> t; f(1, 2, 3) }",
        json!([2, 3]),
    );
}

#[test]
fn arrow_combinators() {
    // `f app x` — apply combinator.
    assert_parity("do { f := (x) ~> x + 1; f app 5 }", json!(6));
    // `x >>> f` — flow composition: apply f to x.
    assert_parity("do { f := (x) ~> x * 2; 21 >>> f }", json!(42));
    // `f *** g` — the categorical product node evaluates to a `Pair`,
    // which the engines serialize as `{"__pair__": [a, b]}`.
    assert_engine_parity("5 *** 6", json!({"__pair__": [5, 6]}));
    // KNOWN GAP (interpreter↔engine, not engine↔engine): the lowered
    // run-path's `product` op yields a plain `[a, b]` array — the
    // `TaggedValue::Pair` `{"__pair__":…}` shape can't be used for
    // internal dispatch pairs (`elem` indexing requires arrays).
    assert_run_parity("5 *** 6", json!([5, 6]));
    // `arr op` — lift a named native op into an arrow.
    assert_parity("(arr add) app [1, 2]", json!([1, 2]));
}

#[test]
fn arrow_block_bodies_and_binders() {
    assert_parity(
        "do { f := (x) ~> do { y := x + 1; y * 2 }; f(5) }",
        json!(12),
    );
    // Arrow bodies see the ambient `in` via `~>` rebasing.
    assert_parity(
        "((xs) ~> match xs { []: 1, _: 2 })([])",
        json!(1),
    );
    assert_parity(
        "((xs) ~> match xs { []: 1, _: 2 })([3])",
        json!(2),
    );
    // Higher-order composition through nested closures.
    assert_parity(
        "do { c := (f) ~> (g) ~> (x) ~> f(g(x)); h := c((n) ~> n + 1)((n) ~> n * 2); h(5) }",
        json!(11),
    );
    // Closures compose with `>>>` chains.
    assert_parity(
        "do { inc := (n) ~> n + 1; dbl := (n) ~> n * 2; 5 >>> inc >>> dbl }",
        json!(12),
    );
    // `f >>> g` with both operands arrow values is sequential 1-cell
    // composition — the bound composite applies `first` then `second`.
    // (Regression: bytecode/cranelift used to *apply* `dbl` to the `inc`
    // arrow value — `expected number, got Arrow`.)
    assert_parity(
        "do { inc := (n) ~> n + 1; dbl := (n) ~> n * 2; f := inc >>> dbl; f(5) }",
        json!(12),
    );
    // `arr` dispatch object bound then called — `call` packs args into
    // one array; the dispatch consumes the first positional arg.
    // (Regression: cranelift used to dispatch on the whole args array.)
    assert_parity("do { f := arr add; f(1, 2) }", json!(1));
}

// ===========================================================================
// Self-recursive `:=` — the env-engine path (lowerer declines by design)
// ===========================================================================

#[test]
fn self_recursive_bindings() {
    assert_parity(
        "do { alen := (xs) ~> match xs { []: 0, [_, ...t]: 1 + alen(t) }; alen([1, 2, 3]) }",
        json!(3),
    );
    assert_parity(
        "do { fib := (n) ~> if n < 2 { n } else { fib(n - 1) + fib(n - 2) }; fib(10) }",
        json!(55),
    );
    // Mutual-style self-reference through a later binding is out of scope;
    // a *mention* of the bound name that is not a `var` must not break.
    assert_parity("do { s := \"s\"; s }", json!("s"));
}

// ===========================================================================
// Records, arrays, field access, index
// ===========================================================================

#[test]
fn object_and_array_literals() {
    assert_parity("[1, 2, 3]", json!([1, 2, 3]));
    assert_parity("{ \"a\": 1, \"b\": 2 }", json!({"a": 1, "b": 2}));
    assert_parity("{ a: 1 }", json!({"a": 1}));
    assert_parity("[]", json!([]));
    assert_parity("[[1, 2], [3]]", json!([[1, 2], [3]]));
    assert_parity(
        "{ \"nested\": { \"x\": [1, { \"y\": 2 }] } }",
        json!({"nested": {"x": [1, {"y": 2}]}}),
    );
}

#[test]
fn field_access_and_index() {
    assert_parity("do { r := { a: 1, b: 2 }; r.a }", json!(1));
    assert_parity("do { r := { a: 1, b: 2 }; r[\"b\"] }", json!(2));
    assert_parity("do { xs := [10, 20, 30]; xs[1] }", json!(20));
    assert_parity("{ a: 42 }.a", json!(42));
    assert_parity("[7, 8, 9][2]", json!(9));
}

// ===========================================================================
// Strings
// ===========================================================================

#[test]
fn string_ops() {
    assert_parity("\"a\" + \"b\"", json!("ab"));
    assert_parity("\"x\" == \"x\"", json!(true));
    assert_parity("\"x\" != \"y\"", json!(true));
    // Backtick template strings use a `!Foreign eval_template` scheme —
    // no host implementation is registered by default, so every engine
    // errors identically. (By-design gap: templates need a host fn.)
    // Message wording differs per engine ("unregistered foreign" /
    // "unregistered ffi: host::") — pin the shared op name.
    assert_engine_parity_err("do { n := 3; `n=${n}` }", "eval_template");
}

// ===========================================================================
// Blocks, bindings, scoping
// ===========================================================================

#[test]
fn do_blocks_and_scoping() {
    assert_parity("do { x := 1; y := 2; x + y }", json!(3));
    assert_parity("do { x := 1; do { x := 2; x } }", json!(2));
    // Inner binding does not leak.
    assert_parity("do { x := 1; do { y := 9; y }; x }", json!(1));
    // Shadowing a language global works.
    assert_parity("do { true := 7; true }", json!(7));
}

#[test]
fn type_annot_is_erased() {
    // `(x : T)` in parameter position: the annotation is compile-time
    // only — the bound value is the argument.
    assert_parity("((x : Int) ~> x)(5)", json!(5));
}

#[test]
fn fanout_is_foreign_and_errors_everywhere() {
    // `f &&& g` lowers to `!Foreign fn.fanout_fn`, which no bare engine
    // registers — all three must fail identically. (By-design: the FP
    // host functions live behind an opt-in module registry.)
    // Message wording differs per engine ("unregistered foreign" /
    // "unregistered ffi: host::") — pin the shared op name.
    assert_engine_parity_err(
        "do { f := (x) ~> x + 1; g := (x) ~> x * 10; (f &&& g) app 5 }",
        "fn.fanout_fn",
    );
}

#[test]
fn paren_and_nested_blocks() {
    assert_parity("((1))", json!(1));
    assert_parity("do { do { do { 42 } } }", json!(42));
}

// ===========================================================================
// Error parity
// ===========================================================================

#[test]
fn unbound_call_errors_everywhere() {
    // A bare `var` self-evaluates (see `definitely_unbound_name`), but
    // *calling* it goes through `call_host` and errors.
    assert_engine_parity_err("missing_fn(1)", "call_host");
}

#[test]
fn arity_mismatch_errors_everywhere() {
    assert_engine_parity_err("do { f := (a, b) ~> a + b; f(1) }", "expects 2 args, got 1");
}

#[test]
fn type_mismatch_and_bounds_errors() {
    assert_engine_parity_err("1 + \"a\"", "type mismatch");
    assert_engine_parity_err("do { xs := [1]; xs[5] }", "out of bounds");
    assert_engine_parity_err("do { xs := [1, 2, 3]; xs[-1] }", "non-negative");
    // Strings are not indexable.
    assert_engine_parity_err("\"abc\"[0]", "index");
}

#[test]
fn division_and_modulo_by_zero_errors() {
    assert_engine_parity_err("1 / 0", "division by zero");
    assert_engine_parity_err("1 % 0", "modulo by zero");
}

// ===========================================================================
// Tagged values — `tag.*` host functions (needs a module registry wired)
// ===========================================================================

fn runtime_with_tag(engine: Box<dyn EvalEngine>) -> Runtime {
    let modules: std::rc::Rc<dyn handl_platform::ModuleLookup> = std::rc::Rc::new(
        handl_plugin_module::ModuleRegistry::new().with(handl_plugin_lib_tag::TagModule),
    );
    let engine = engine.with_modules(modules);
    Runtime::new(canonical_lang())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(engine)
}

#[test]
fn tag_mint_untag_tag_of() {
    // `tag.tag`/`tag.untag` dispatch through `call_host` → the module
    // registry — the `field_access` string-concat fallback builds the
    // qualified name `tag.tag` from `tag.tag` surface syntax.
    for (name, rt) in [
        ("tree-walk", runtime_with_tag(Box::new(TreeWalkEngine::new()))),
        ("bytecode", runtime_with_tag(Box::new(BytecodeEngine::new()))),
        ("cranelift", runtime_with_tag(Box::new(CraneliftEngine::new()))),
    ] {
        let src = r#"do { v := tag.tag("ok", { "value": 42 }); tag.untag(v) }"#;
        let got = eval_engine_path(&rt, src)
            .unwrap_or_else(|e| panic!("{name} engine-path failed on {src:?}: {e}"));
        assert_eq!(got, json!({"value": 42}), "{name} on {src:?}");

        let src = r#"do { v := tag.tag("ok", { "value": 42 }); tag.tag_of(v) }"#;
        let got = eval_engine_path(&rt, src)
            .unwrap_or_else(|e| panic!("{name} engine-path failed on {src:?}: {e}"));
        assert_eq!(got, json!("ok"), "{name} on {src:?}");

        let src = r#"tag.untag(5)"#;
        let got = eval_engine_path(&rt, src)
            .unwrap_or_else(|e| panic!("{name} engine-path failed on {src:?}: {e}"));
        assert_eq!(got, Value::Null, "{name} on {src:?}");
    }
}

#[test]
fn tagged_value_shape_and_match() {
    // A minted tag flows through `match` object patterns on every engine.
    for (name, rt) in [
        ("tree-walk", runtime_with_tag(Box::new(TreeWalkEngine::new()))),
        ("bytecode", runtime_with_tag(Box::new(BytecodeEngine::new()))),
        ("cranelift", runtime_with_tag(Box::new(CraneliftEngine::new()))),
    ] {
        let src = r#"do { v := tag.tag("ok", { "value": 42 }); v.__tag__ }"#;
        let got = eval_engine_path(&rt, src)
            .unwrap_or_else(|e| panic!("{name} engine-path failed on {src:?}: {e}"));
        assert_eq!(got, json!("ok"), "{name} on {src:?}");
    }
}
