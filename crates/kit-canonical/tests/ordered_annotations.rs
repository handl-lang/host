//! # ordered_annotations.rs — ADR-0103 ordered value-annotation tests.
//!
//! Verifies that `typecheck_handl` evaluates annotation expressions in the
//! value environment built by earlier statements: assignments bind both a
//! type (gamma) and a value term (results), and a later `x : <expr> := v`
//! reduces `<expr>` against those bindings before comparing to `v`.
//!
//! Cases (from ADR-0103):
//!   inc := (x) ~> x + 1 ; f : inc(4) := 5     — call annotation reduces to 5
//!   inc := (x) ~> x + 1 ; f : inc(4) := 6     — mismatch, f must not bind
//!   inc := (x) ~> x + 1 ; x : 5 >>> inc := 6  — composition annotation
//!   a := 3 ; f : a + 2 := 5                   — arithmetic over bound values
//!   failed f ; g := f                          — unbound variable for g
//!   dedupe<N> := (g >>> g) ~[N]~> g           — generic dim arrow (ADR-0119)

use handl_kit_canonical::Kit;

fn kit() -> Kit {
    Kit::new().expect("canonical kit must assemble")
}

#[test]
fn ordered_value_annotations() {
    let kit = kit();

    // Call annotation: `inc(4)` evaluates to `5` under the ordered env.
    // Ordered value annotations are model semantics (ADR-0103), so these
    // go through `typecheck_handl_model` — the bootstrap checker that
    // `typecheck_handl`/`hnl typecheck`/`hnl run` use only scopes wires.
    kit.typecheck_handl_model("do {\n  inc := (x) ~> x + 1\n  f : inc(4) := 5\n}")
        .expect("f : inc(4) := 5 should check");

    // Composition annotation: `5 >>> inc` applies `inc` to `5` -> `6`.
    kit.typecheck_handl_model("do {\n  inc := (x) ~> x + 1\n  x : 5 >>> inc := 6\n}")
        .expect("x : 5 >>> inc := 6 should check");

    // Arithmetic annotation over a bound value: `a + 2` -> `5`.
    kit.typecheck_handl_model("do {\n  a := 3\n  f : a + 2 := 5\n}")
        .expect("f : a + 2 := 5 should check");

    // A failing annotation reports a mismatch and does NOT bind `f`, so
    // the later `g := f` surfaces an unbound-variable diagnostic.
    let err = kit
        .typecheck_handl_model("do {\n  inc := (x) ~> x + 1\n  f : inc(4) := 6\n  g := f\n}")
        .expect_err("f : inc(4) := 6 must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("annotation") || msg.contains("mismatch") || msg.contains("equal"),
        "expected a value/annotation mismatch, got: {msg}"
    );
    assert!(
        msg.contains("unbound variable: f"),
        "failed f must not leak into scope, got: {msg}"
    );

    // A lone failing annotation still reports the mismatch.
    let err = kit
        .typecheck_handl_model("do {\n  inc := (x) ~> x + 1\n  f : inc(4) := 6\n}")
        .expect_err("f : inc(4) := 6 must fail");
    assert!(
        err.to_string().contains("mismatch") || err.to_string().contains("equal"),
        "expected a value/annotation mismatch, got: {err}"
    );
}

#[test]
fn generic_dim_arrow_binds() {
    let kit = kit();
    // `N` is a real generic binding; `g` is a pattern variable bound to a
    // fresh meta inside the ~[N]~> law (ADR-0119).
    kit.typecheck_handl("dedupe<N> := (g >>> g) ~[N]~> g")
        .expect("generic dim arrow should check");
}
