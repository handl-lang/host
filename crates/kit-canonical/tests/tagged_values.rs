//! # tagged_values.rs — ADR-0055 tagged-values capability end-to-end tests.
//!
//! Verifies the `tag.*` host functions (offered by `plugin-lib-tag`'s
//! `TagModule`, wired into the canonical kit) are callable from handl source
//! via the `tag` module global, and that the `__tag__` reserved key is
//! unforgeable by user object literals.

use handl_kit_canonical::Kit;
use handl_language::{as_tag, is_tag_value};

fn eval(input: &str) -> serde_json::Value {
    let kit = Kit::new().expect("kit must build");
    kit.eval(input).expect("eval must succeed")
}

fn eval_err(input: &str) -> String {
    let kit = Kit::new().expect("kit must build");
    kit.eval(input).unwrap_err().to_string()
}

// ---------------------------------------------------------------------------
// tag.tag — minting via source syntax
// ---------------------------------------------------------------------------

#[test]
fn tag_tag_mints_tagged_value_from_source() {
    let v = eval(r#"tag.tag("type.collection", { name: "number" })"#);
    assert!(is_tag_value(&v), "result should be a tagged value: {v}");
    assert_eq!(as_tag(&v), Some("type.collection"));
    assert_eq!(v["name"], serde_json::json!("number"));
    assert_eq!(v["__tag__"], serde_json::json!("type.collection"));
}

#[test]
fn tag_tag_null_payload_mints_empty_tagged_value() {
    let v = eval(r#"tag.tag("unit", null)"#);
    assert_eq!(as_tag(&v), Some("unit"));
    assert_eq!(v.as_object().unwrap().len(), 1);
}

// ---------------------------------------------------------------------------
// tag.untag / tag.tag_of / tag.is_tag — inspection via source syntax
// ---------------------------------------------------------------------------

#[test]
fn tag_untag_returns_payload_from_source() {
    let v = eval(r#"tag.untag(tag.tag("unit", { m: 5, s: 1 }))"#);
    assert_eq!(v, serde_json::json!({ "m": 5, "s": 1 }));
    assert!(v.as_object().unwrap().get("__tag__").is_none());
}

#[test]
fn tag_tag_of_returns_name_from_source() {
    let v = eval(r#"tag.tag_of(tag.tag("capability", { name: "fs" }))"#);
    assert_eq!(v, serde_json::json!("capability"));
}

#[test]
fn tag_is_tag_recognizes_tagged_value_from_source() {
    assert_eq!(
        eval(r#"tag.is_tag(tag.tag("unit", { v: 5 }))"#),
        serde_json::json!(true)
    );
    assert_eq!(eval(r#"tag.is_tag("plain string")"#), serde_json::json!(false));
    assert_eq!(eval(r#"tag.is_tag({ a: 1 })"#), serde_json::json!(false));
}

// ---------------------------------------------------------------------------
// Forgery prevention — user object literals cannot produce __tag__
// ---------------------------------------------------------------------------

#[test]
fn user_object_literal_cannot_forge_tag_key() {
    // A user object literal that tries to set `__tag__` is rejected by the
    // eval engines (the reserved-key guard added per ADR-0055 §3).
    let err = eval_err(r#"{ "__tag__": "type.collection", name: "number" }"#);
    assert!(
        err.contains("reserved"),
        "expected a 'reserved' key error, got: {err}"
    );
}

#[test]
fn user_object_literal_without_reserved_key_is_unaffected() {
    let v = eval(r#"{ name: "number", value: 5 }"#);
    assert_eq!(v["name"], serde_json::json!("number"));
    assert_eq!(v["value"], serde_json::json!(5));
    assert!(!is_tag_value(&v));
}

// ---------------------------------------------------------------------------
// Structural equality (ADR-0055 §5)
// ---------------------------------------------------------------------------

#[test]
fn tagged_values_equal_by_structure() {
    // Two tagged values with the same tag + payload are equal.
    let a = eval(r#"tag.tag("type.collection", { name: "number" })"#);
    let b = eval(r#"tag.tag("type.collection", { name: "number" })"#);
    assert_eq!(a, b);
    // Different payload ⇒ not equal.
    let c = eval(r#"tag.tag("type.collection", { name: "string" })"#);
    assert_ne!(a, c);
    // A tagged value is not equal to a plain string.
    assert_ne!(a, serde_json::json!("number"));
}
