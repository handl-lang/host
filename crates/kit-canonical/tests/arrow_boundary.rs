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
//! Now arguments are bound as per-call globals (`__arg_N`) and arrows carry
//! a `__home__` frame id so the receiving eval can route application back to
//! the AST that minted them — see `plugin-engine-exec-tree-walk`'s
//! home-frame registry and `HandlFunction::call`.

use handl_kit_canonical::Kit;
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
/// caller: `constantly(7)` returns `_ ~> c` whose `body` indexes the nested
/// eval's AST.
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
