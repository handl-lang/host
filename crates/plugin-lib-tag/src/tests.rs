//! Tests for the `tag` tagged-values capability (ADR-0055).

use handl_language::{as_tag, is_tag_value, tag_value};
use handl_platform::{EvalContext, Function, Module};
use serde_json::{Value, json};

use crate::{IsTag, MatchTag, Tag, TagModule, TagOf, Untag, DEFAULT_CASE_KEY};

/// Call a `Function` with a fresh `EvalContext`.
fn call_fn(f: &dyn Function, args: &[Value]) -> Result<Value, handl_platform::EvalError> {
    let mut ctx = EvalContext::new();
    f.call(args, &mut ctx)
}

// ---------------------------------------------------------------------------
// tag.tag — minting
// ---------------------------------------------------------------------------

#[test]
fn tag_mints_tagged_value() {
    let v = call_fn(&Tag, &[json!("type.collection"), json!({ "name": "number" })]).unwrap();
    assert!(is_tag_value(&v));
    assert_eq!(as_tag(&v), Some("type.collection"));
    assert_eq!(v["name"], json!("number"));
    assert_eq!(v["__tag__"], json!("type.collection"));
}

#[test]
fn tag_null_payload_is_empty() {
    let v = call_fn(&Tag, &[json!("unit"), Value::Null]).unwrap();
    assert_eq!(as_tag(&v), Some("unit"));
    assert_eq!(v.as_object().unwrap().len(), 1);
}

#[test]
fn tag_rejects_non_string_name() {
    let err = call_fn(&Tag, &[json!(5), json!({})]).unwrap_err();
    assert!(err.to_string().contains("name must be a string"));
}

#[test]
fn tag_rejects_non_object_payload() {
    let err = call_fn(&Tag, &[json!("unit"), json!(5)]).unwrap_err();
    assert!(err.to_string().contains("payload must be an object"));
}

#[test]
fn tag_rejects_smuggled_tag_key() {
    // Forgery prevention (ADR-0055 §3): a payload carrying `__tag__` cannot
    // be re-tagged through the payload.
    let err = call_fn(&Tag, &[json!("type.pi"), json!({ "__tag__": "type.collection" })])
        .unwrap_err();
    assert!(err.to_string().contains("reserved __tag__ key"));
}

#[test]
fn tag_rejects_wrong_arity() {
    assert!(call_fn(&Tag, &[json!("unit")]).is_err());
    assert!(call_fn(&Tag, &[json!("unit"), json!({}), json!(1)]).is_err());
}

// ---------------------------------------------------------------------------
// tag.untag / tag.tag_of / tag.is_tag — inspection
// ---------------------------------------------------------------------------

#[test]
fn untag_returns_payload_without_tag_key() {
    let v = tag_value("unit", &json!({ "m": 5, "s": 1 })).unwrap();
    let payload = call_fn(&Untag, &[v]).unwrap();
    assert_eq!(payload, json!({ "m": 5, "s": 1 }));
    assert!(payload.as_object().unwrap().get("__tag__").is_none());
}

#[test]
fn untag_non_tag_value_returns_null() {
    assert_eq!(call_fn(&Untag, &[json!("number")]).unwrap(), Value::Null);
    assert_eq!(call_fn(&Untag, &[json!({ "a": 1 })]).unwrap(), Value::Null);
}

#[test]
fn tag_of_returns_tag_name() {
    let v = tag_value("type.collection", &json!({ "name": "number" })).unwrap();
    assert_eq!(call_fn(&TagOf, &[v]).unwrap(), json!("type.collection"));
}

#[test]
fn tag_of_non_tag_value_returns_null() {
    assert_eq!(call_fn(&TagOf, &[json!("number")]).unwrap(), Value::Null);
    assert_eq!(call_fn(&TagOf, &[json!({ "a": 1 })]).unwrap(), Value::Null);
}

#[test]
fn is_tag_recognizes_tagged_value() {
    let v = tag_value("capability", &json!({ "name": "fs" })).unwrap();
    assert_eq!(call_fn(&IsTag, &[v]).unwrap(), json!(true));
    assert_eq!(call_fn(&IsTag, &[json!("number")]).unwrap(), json!(false));
    assert_eq!(call_fn(&IsTag, &[json!({ "__tag__": 5 })]).unwrap(), json!(false));
}

// ---------------------------------------------------------------------------
// tag.match_tag — dispatch
// ---------------------------------------------------------------------------

#[test]
fn match_tag_dispatches_to_matching_case_value() {
    // Cases are non-arrow values: the matched case value is returned directly.
    let v = tag_value("unit", &json!({ "m": 5 })).unwrap();
    let cases = json!({ "unit": "meters", "currency": "usd" });
    let result = call_fn(&MatchTag, &[v, cases]).unwrap();
    assert_eq!(result, json!("meters"));
}

#[test]
fn match_tag_uses_default_case_when_no_match() {
    let v = tag_value("currency", &json!({ "code": "usd" })).unwrap();
    let cases = json!({ "unit": "meters", "_": "other" });
    let result = call_fn(&MatchTag, &[v, cases]).unwrap();
    assert_eq!(result, json!("other"));
}

#[test]
fn match_tag_default_case_handles_non_tag_value() {
    // A non-tag value has no tag; the "_" default handles it.
    let cases = json!({ "unit": "meters", "_": "fallback" });
    let result = call_fn(&MatchTag, &[json!("plain string"), cases]).unwrap();
    assert_eq!(result, json!("fallback"));
}

#[test]
fn match_tag_returns_null_when_no_match_and_no_default() {
    let v = tag_value("currency", &json!({})).unwrap();
    let cases = json!({ "unit": "meters" });
    let result = call_fn(&MatchTag, &[v, cases]).unwrap();
    assert_eq!(result, Value::Null);
}

#[test]
fn match_tag_non_tag_value_without_default_returns_null() {
    let cases = json!({ "unit": "meters" });
    let result = call_fn(&MatchTag, &[json!(42), cases]).unwrap();
    assert_eq!(result, Value::Null);
}

#[test]
fn match_tag_rejects_non_object_cases() {
    let err = call_fn(&MatchTag, &[json!(42), json!("not cases")]).unwrap_err();
    assert!(err.to_string().contains("cases must be an object"));
}

#[test]
fn match_tag_rejects_wrong_arity() {
    assert!(call_fn(&MatchTag, &[json!(42)]).is_err());
    assert!(call_fn(&MatchTag, &[json!(42), json!({}), json!(1)]).is_err());
}

// ---------------------------------------------------------------------------
// Module surface
// ---------------------------------------------------------------------------

#[test]
fn module_exposes_all_callables() {
    let m = TagModule;
    assert_eq!(m.name(), "tag");
    assert!(m.function("tag").is_some());
    assert!(m.function("match_tag").is_some());
    assert!(m.function("untag").is_some());
    assert!(m.function("tag_of").is_some());
    assert!(m.function("is_tag").is_some());
    assert!(m.function("nope").is_none());
    let fns = m.functions();
    let names: Vec<&str> = fns.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, vec!["tag", "match_tag", "untag", "tag_of", "is_tag"]);
}

#[test]
fn default_case_key_is_underscore() {
    assert_eq!(DEFAULT_CASE_KEY, "_");
}
