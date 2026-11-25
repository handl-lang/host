//! `transport(p, v)` — the call-form alias for the `p # v` transport
//! eliminator (project/design/apply-forms-unification.md §9, decided:
//! both spellings exist).
//!
//! The judge intercepts the callee *name* `transport` in call position
//! (the `ua`/`Univalence`/`Glue` precedent — `transport` stays an
//! ordinary identifier everywhere else) and runs the same transport
//! judgment; `transport` rides the canonical wire prelude so the
//! unbound-wire scope pass admits it. These tests exercise the full
//! production `hnl typecheck` pipeline (`Kit::typecheck_source` → NBE
//! engine + `prelude_wire_names`).
//!
//! Sources avoid `data` constructor names (`Zero`, `VCons`, …): inside
//! the `do { … }` wrapper the scope pass does not bind them as wires —
//! the same constraint `tests/smoke/*.hnl` top-level decls avoid by
//! living at module scope.

use handl_kit_prelude::Kit;

fn kit() -> Kit {
    Kit::new().expect("canonical kit must assemble")
}

/// A univalence path `Bool ~ Bool` and a homogeneous `Path(Bool, …)`
/// witness — the two transport shapes, no data decls required.
const SETUP: &str = "e : Equiv(Bool, Bool) := { to: x ~> x, fro: y ~> y }\n  pu := ua(e)\n  cp : Path(Bool, true, true) := i ~> true";

#[test]
fn transport_call_form_typechecks() {
    let kit = kit();
    // `pu : Bool ~ Bool`, `cp : Path(Bool, true, true)` — both
    // transport `true : Bool` to `Bool`, the `p # v` reading.
    kit.typecheck_source(&format!(
        "do {{\n{SETUP}\n  t1 : Bool := transport(pu, true)\n  t2 : Bool := transport(cp, true)\n  t3 := transport(refl, true)\n}}"
    ))
    .expect("transport(p, v) should inhabit Bool");
}

#[test]
fn transport_call_form_rejects_mismatch() {
    let kit = kit();
    // Same transport checked against `Nat` — must fail like `p # v`
    // does (Bool does not inhabit Nat).
    let err = kit
        .typecheck_source(&format!(
            "do {{\n{SETUP}\n  t : Nat := transport(cp, true)\n}}"
        ))
        .expect_err("transport(p, v) should not inhabit Nat");
    assert!(
        err.to_string().contains("typecheck error"),
        "expected a typecheck error, got: {err}"
    );
}

#[test]
fn transport_call_form_unbound_without_wire() {
    // Sanity: the callee name really is the wire — a *different* arity
    // warns rather than erroring (the `ua` precedent: the call form
    // is captured by name before ordinary application).
    let kit = kit();
    kit.typecheck_source(&format!(
        "do {{\n{SETUP}\n  t := transport(pu)\n}}"
    ))
    .expect("wrong-arity transport(...) warns, not errors");
}
