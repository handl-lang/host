//! # tagged_values.rs — ADR-0055 tagged-values capability end-to-end tests.
//!
//! Post-deletion (ADR-0152 §Migration): the `tag.tag`/`tag.construct` mint
//! producers are gone — user code cannot mint `__tag__` records. The
//! surviving `tag.*` surface is inspection-only: `tag.untag` /
//! `tag.tag_of` / `tag.is_tag` on host-internal tagged records (and the
//! `__tag__` reserved key stays unforgeable by user object literals).

use handl_kit_prelude::Kit;
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
// Deleted mint producers — `tag.tag`/`tag.construct` fail at eval
// ---------------------------------------------------------------------------

#[test]
fn tag_tag_is_deleted_errors_from_source() {
    let err = eval_err(r#"tag.tag("type.collection", { name: "number" })"#);
    assert!(
        err.contains("tag.tag"),
        "expected an error naming the deleted `tag.tag` op, got: {err}"
    );
}

#[test]
fn tag_construct_is_deleted_errors_from_source() {
    let err = eval_err(r#"tag.construct("type.pi", { param: 1, body: 2 })"#);
    assert!(
        err.contains("tag.construct"),
        "expected an error naming the deleted `tag.construct` op, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// tag.untag / tag.tag_of / tag.is_tag — inspection via source syntax
// ---------------------------------------------------------------------------

#[test]
fn tag_inspection_ops_on_untagged_values() {
    // Non-tagged inputs: untag → null, tag_of → null, is_tag → false.
    assert_eq!(eval(r#"tag.untag(5)"#), serde_json::Value::Null);
    assert_eq!(eval(r#"tag.tag_of(5)"#), serde_json::Value::Null);
    assert_eq!(
        eval(r#"tag.is_tag("plain string")"#),
        serde_json::json!(false)
    );
    assert_eq!(eval(r#"tag.is_tag({ a: 1 })"#), serde_json::json!(false));
}

#[test]
fn tag_inspection_on_host_internal_arrow_record() {
    // `f >>> g` yields a host-internal `{"__tag__": "Arrow", …}` composite
    // record — the inspection ops read it without any user-side mint.
    let tagged = eval(r#"((x) ~> x) >>> ((y) ~> y)"#);
    assert!(
        is_tag_value(&tagged),
        "arrow composite should be a tagged value: {tagged}"
    );
    assert_eq!(as_tag(&tagged), Some("Arrow"));
    assert_eq!(
        eval(r#"tag.is_tag(((x) ~> x) >>> ((y) ~> y))"#),
        serde_json::json!(true)
    );
    assert_eq!(
        eval(r#"tag.tag_of(((x) ~> x) >>> ((y) ~> y))"#),
        serde_json::json!("Arrow")
    );
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
