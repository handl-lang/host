//! Arrow values crossing the `.hnl`-module boundary.
//!
//! `comb.*`/`transform.*` functions are implemented in `.hnl` and loaded via
//! `HandlModule`/`HandlFunction`. Arguments used to be serialized into the
//! applied source text, so a closure argument (`{"__arrow__": ...}`) was
//! re-parsed as a *user* object literal and rejected by the reserved-key
//! check:
//!
//! ```text
//! comb.flip: engine error: object: key '__arrow__' is reserved in object in call
//! ```
//!
//! Now arguments are bound as per-call globals (`__arg_N`) and arrows are
//! self-contained (`body_def` + `binds` + `handlers`, ADR-0106) — an arrow
//! crossing the boundary applies against its own serialized definition, so
//! no provenance stamping is needed.

use handl_kit_prelude::Kit;
use serde_json::json;

/// Eval like `hnl eval <expr>`: the input is wrapped in a `do { ... }`
/// block so `;`-separated statements sequence (same as `host-cli`).
fn eval(src: &str) -> serde_json::Value {
    let wrapped = format!("do {{ {src} }}");
    Kit::new()
        .expect("canonical kit must assemble")
        .eval(&wrapped)
        .unwrap_or_else(|e| panic!("`{src}` must eval: {e}"))
}

/// A closure passed *into* a `.hnl` function must be invocable inside it:
/// `flip` binds `f` and calls `f(y, x)` in the nested eval.
#[test]
fn flip_receives_arrow_arg() {
    assert_eq!(eval("comb.flip((a, b) ~> a - b)(1, 5)"), json!(4));
}

/// A closure *returned* by a `.hnl` function must be invocable by the
/// caller: `constantly(7)` returns `_ ~> c` whose `body_def` is its own
/// serialized AST arena with `c` substituted in.
#[test]
fn constantly_returns_arrow() {
    assert_eq!(eval("comb.constantly(7)(99)"), json!(7));
}

/// Arrows passed in a structure (an array of closures) must each stay
/// invocable inside the `.hnl` implementation.
#[test]
fn pipe_and_compose_route_arrows() {
    assert_eq!(eval("comb.pipe([3, x ~> x + 1, x ~> x * 2])"), json!(8));
    assert_eq!(eval("comb.compose([x ~> x + 1, x ~> x * 2])(3)"), json!(7));
    assert_eq!(eval("comb.flow([x ~> x + 1, x ~> x * 2])(3)"), json!(8));
}

/// Native higher-order functions keep working: the arrow arg reaches the
/// host `Function` as a value and is invoked via `ctx.try_call_value`.
#[test]
fn transform_map_still_works() {
    assert_eq!(eval("transform.map([1, 2], x ~> x * 2)"), json!([2, 4]));
    assert_eq!(
        eval("transform.filter([1, 2, 3, 4], x ~> x % 2 == 0)"),
        json!([2, 4])
    );
    assert_eq!(
        eval("transform.reduce([1, 2, 3], 0, (a, b) ~> a + b)"),
        json!(6)
    );
}

/// A curried arrow applied to more args than its parameter list applies
/// successively: `f(1, 5)` on `x ~> y ~> body` is `(f 1) 5`.
#[test]
fn curried_application() {
    assert_eq!(eval("g :: x ~> y ~> x - y; g(1, 5)"), json!(-4));
}

/// `combinators.*` Rust ops must accept `__arrow__` callbacks via
/// `ctx.try_call_value` — the legacy `FunctionValue` + `eval_expr` path
/// cannot see arrow wire values (I-007).
#[test]
fn combinators_ops_accept_arrow_callbacks() {
    assert_eq!(eval("combinators.apply(x ~> x + 1, [4])"), json!(5));
    assert_eq!(
        eval("combinators.flat_map(x ~> [x, x * 2], [1, 2, 3])"),
        json!([1, 2, 2, 4, 3, 6])
    );
    assert_eq!(
        eval("comb.flat_map(x ~> [x, x * 2], [1, 2, 3])"),
        json!([1, 2, 2, 4, 3, 6])
    );
    assert_eq!(
        eval("combinators.pipe(3, x ~> x + 1, x ~> x * 2)"),
        json!(8)
    );
    assert_eq!(eval("combinators.some(x ~> x > 2, [1, 2, 3])"), json!(true));
    assert_eq!(
        eval("combinators.every(x ~> x > 0, [1, 2, 3])"),
        json!(true)
    );
    assert_eq!(eval("combinators.find(x ~> x == 2, [1, 2, 3])"), json!(2));
}

/// `collections.*` host `Function`s take the same `__arrow__` callbacks —
/// `call_callable` routes them through `ctx.try_call_value` (I-007).
#[test]
fn collections_ops_accept_arrow_callbacks() {
    assert_eq!(
        eval("hash_set.unique_by([1, 2, 3, 4], x ~> x % 2)"),
        json!([1, 2])
    );
    assert_eq!(
        eval("hash_map.map_values(x ~> x * 2, {a: 1, b: 2})"),
        json!({"a": 2, "b": 4})
    );
}

/// I-010: the `array` sub-module's native ops resolve through the
/// `collections` aggregate's dotted dispatch — `array.<op>` and
/// `collections.array.<op>` spellings both reach the op-backed functions.
#[test]
fn array_native_ops_resolve_and_run() {
    assert_eq!(
        eval("array.take_while([1, 2, 3, 4], x ~> x < 3)"),
        json!([1, 2])
    );
    assert_eq!(
        eval("array.drop_while([1, 2, 3, 4], x ~> x < 3)"),
        json!([3, 4])
    );
    assert_eq!(
        eval("collections.array.foldr([1, 2, 3], (a, b) ~> a + b, 0)"),
        json!(6)
    );
    assert_eq!(
        eval("array.scan([1, 2, 3], (a, b) ~> a + b, 0)"),
        json!([1, 3, 6])
    );
    assert_eq!(eval("array.find_index([1, 2, 3], x ~> x == 2)"), json!(1.0));
    // Non-arrow native ops resolve through the same path.
    assert_eq!(eval("array.push([1], 2)"), json!([1, 2]));
    assert_eq!(
        eval("collections.array.range(0, 3)"),
        json!([0.0, 1.0, 2.0])
    );
}

/// A user-authored object literal may still not forge the `__arrow__`
/// sentinel — the reserved-key check is unchanged.
#[test]
fn user_cannot_forge_arrow_object() {
    let err = Kit::new()
        .expect("canonical kit must assemble")
        .eval("{ __arrow__: true, params: [], body: 0 }")
        .expect_err("user literal must not forge __arrow__");
    assert!(
        err.to_string().contains("__arrow__"),
        "expected reserved-key error, got: {err}"
    );
}
