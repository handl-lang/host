//! Phase-2 WS-G — runtime-minted types/cells are legal values (ADR-0154 §6).
//!
//! A `.hnl` program mints a `data`-shaped former + constructor at runtime —
//! the same `{"__former__": <seeded sym>, "indices": […]}` /
//! `{"__ctor__": <ctor sym>, …, "type": F(i⃗)}` encodings `desugar.data`
//! emits — and the nominal machinery reads them honestly: `instanceof`
//! walks the ctor's `type`-link into the former's head, `type.ty` resolves
//! the link. The encoding is *runtime-constructible* end-to-end:
//! `sym.seeded` mints the deterministic nominal, ordinary object literals
//! carry it, no declaration form is required (laws-as-values).
//!
//! Also covered here: `eval.checked` — the checked `eval` path
//! (`lex → parse_raw → typecheck → desugar → eval` → `{value, type}`),
//! versus raw `eval`, whose result is the value alone (dynamic =
//! `type.unknown`).

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

fn eval_err(src: &str) -> String {
    let wrapped = format!("do {{ {src} }}");
    Kit::new()
        .expect("canonical kit must assemble")
        .eval(&wrapped)
        .unwrap_err()
        .to_string()
}

// ---------------------------------------------------------------------------
// Runtime-minted data-shaped former + ctor (the ADR-0154 §6 promise)
// ---------------------------------------------------------------------------

#[test]
fn runtime_minted_former_and_ctor_are_nominal_values() {
    let out = eval(
        r#"
        rt_former := {"__former__": sym.seeded("data.RTNat"), "indices": []}
        rt_former
        "#,
    );
    let head = out
        .get("__former__")
        .and_then(|f| f.get("__sym__"))
        .unwrap_or_else(|| panic!("former must carry a __sym__ head: {out}"));
    assert_eq!(head["name"], "RTNat");
    assert!(
        head["handle"].as_u64().is_some(),
        "seeded sym has a handle: {head}"
    );
}

#[test]
fn runtime_minted_ctor_instanceof_former() {
    // `z = {"__ctor__": cSym, "type": F}` — `instanceof` reads the
    // ctor's `type`-link head then parent-walks (ADR-0154 §2): the
    // minted value is a member of its minted former.
    let is_member = eval(
        r#"
        rt_former := {"__former__": sym.seeded("data.RTNat"), "indices": []}
        z := {"__ctor__": sym.seeded("data.RTNat.Zero"), "type": rt_former}
        type.instanceof(z, rt_former)
        "#,
    );
    assert_eq!(is_member, json!(true));
}

#[test]
fn runtime_minted_ctor_is_not_a_member_of_other_types() {
    // Nominal membership is honest: the minted ctor is `data.RTNat`,
    // not `data.RTBool` and not `type.Int`.
    let out = eval(
        r#"
        rt_former := {"__former__": sym.seeded("data.RTNat"), "indices": []}
        other_former := {"__former__": sym.seeded("data.RTBool"), "indices": []}
        z := {"__ctor__": sym.seeded("data.RTNat.Zero"), "type": rt_former}
        [type.instanceof(z, other_former), type.instanceof(z, type.Int())]
        "#,
    );
    assert_eq!(out, json!([false, false]));
}

#[test]
fn runtime_minted_ctor_type_link_resolves() {
    // `type.ty(v)` reads `v`'s `type` link — the minted ctor's link is
    // the minted former, head `data.RTNat`.
    let out = eval(
        r#"
        rt_former := {"__former__": sym.seeded("data.RTNat"), "indices": []}
        z := {"__ctor__": sym.seeded("data.RTNat.Zero"), "type": rt_former}
        type.ty(z)
        "#,
    );
    assert_eq!(
        out["__former__"]["__sym__"]["name"],
        json!("RTNat"),
        "ctor's type link must be the minted former: {out}"
    );
}

#[test]
fn runtime_minted_ctor_flows_through_eval() {
    // The minted encoding crosses the eval machinery: an arrow mints
    // the ctor when applied (dynamic-extent construction, not a
    // declaration), and `instanceof` still recognizes it.
    let out = eval(
        r#"
        rt_former := {"__former__": sym.seeded("data.RTMaybe"), "indices": []}
        mint := x ~> ({"__ctor__": sym.seeded("data.RTMaybe.None"), "type": rt_former})
        [mint(0), type.instanceof(mint(0), rt_former)]
        "#,
    );
    assert_eq!(out[1], json!(true), "value: {}", out[0]);
}

#[test]
fn seeded_former_mint_is_idempotent() {
    // Two `sym.seeded` mints of the same seed are the *same* nominal —
    // a ctor carrying one mint is a member of the other spelling.
    let out = eval(
        r#"
        z := {"__ctor__": sym.seeded("data.RTPair.Mk"), "type": {"__former__": sym.seeded("data.RTPair"), "indices": []}}
        type.instanceof(z, {"__former__": sym.seeded("data.RTPair"), "indices": []})
        "#,
    );
    assert_eq!(out, json!(true));
}

// ---------------------------------------------------------------------------
// `eval.checked` — the checked eval path ({value, type})
// ---------------------------------------------------------------------------

#[test]
fn eval_checked_on_source_returns_value_and_type() {
    let out = eval(r#"eval.checked(self, "1 + 2")"#);
    assert_eq!(out["value"], json!(3), "eval.checked value: {out}");
    let typ = &out["type"];
    assert!(
        !typ.is_null(),
        "eval.checked must expose the synthesized type, got: {out}"
    );
}

#[test]
fn eval_checked_type_error_never_evals() {
    // `missing_fn(1)` — an unbound callee fails the check; eval must
    // not run.
    let err = eval_err(r#"eval.checked(self, "missing_fn(1)")"#);
    assert!(
        err.contains("eval.checked"),
        "eval.checked must surface a checked-path error, got: {err}"
    );
}

#[test]
fn eval_eval_raw_is_unchanged() {
    // Raw `eval` stays dynamic: it evaluates an AST value and returns
    // the value alone (no {value, type} record). The meta pipeline is
    // `eval(self, parse(self, lex(self, src)))`.
    let out = eval(r#"eval(self, parse(self, lex(self, "1 + 2")))"#);
    assert_eq!(out, json!(3));
}
