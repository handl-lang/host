//! Tests for the `tag` tagged-values capability (ADR-0055).

use handl_language::tag_value;
use handl_platform::{EvalContext, Function, Module};
use serde_json::{Value, json};

use crate::{IsTag, MatchType, TagModule, TagOf, TypeRegistry, Untag};

/// Call a `Function` with a fresh `EvalContext`.
fn call_fn(f: &dyn Function, args: &[Value]) -> Result<Value, handl_platform::EvalError> {
    let mut ctx = EvalContext::new();
    f.call(args, &mut ctx)
}

// ---------------------------------------------------------------------------
// tag.tag — minting (deleted with the `__tag__`/`{tag}` bridge)
// ---------------------------------------------------------------------------

#[test]
fn tag_mint_op_is_unregistered() {
    // The `tag.tag`/`tag.construct` producers were deleted (ADR-0152
    // §Migration): the op registry and module surface no longer mint.
    let registry = crate::tag_op_registry();
    assert!(registry.get("tag.tag").is_none());
    assert!(registry.get("tag.construct").is_none());
    assert!(registry.get("tag.untag").is_some());
    assert!(registry.get("tag.match_type").is_some());
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
    assert_eq!(
        call_fn(&IsTag, &[json!({ "__tag__": 5 })]).unwrap(),
        json!(false)
    );
}

// ---------------------------------------------------------------------------
// Module surface
// ---------------------------------------------------------------------------

#[test]
fn module_exposes_all_callables() {
    let m = TagModule;
    assert_eq!(m.name(), "tag");
    assert!(m.function("tag").is_none());
    assert!(m.function("untag").is_some());
    assert!(m.function("tag_of").is_some());
    assert!(m.function("is_tag").is_some());
    assert!(m.function("construct").is_none());
    assert!(m.function("match_type").is_some());
    assert!(m.function("match_tag").is_none());
    assert!(m.function("nope").is_none());
    let fns = m.functions();
    let names: Vec<&str> = fns.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, vec!["untag", "tag_of", "is_tag", "match_type"]);
}

// ---------------------------------------------------------------------------
// TypeRegistry (ADR-0054)
// ---------------------------------------------------------------------------

#[test]
fn registry_construct_mints_type_value() {
    let reg = TypeRegistry::new("handl.canonical");
    let v = reg
        .construct("type.collection", &json!({ "name": "number" }))
        .unwrap();
    assert_eq!(v["__tag__"], json!("type.collection"));
    assert_eq!(v["__registry__"], json!("handl.canonical"));
    assert_eq!(v["name"], json!("number"));
}

#[test]
fn registry_construct_null_payload_is_empty() {
    let reg = TypeRegistry::new("handl.canonical");
    let v = reg.construct("type.pi", &Value::Null).unwrap();
    assert_eq!(v["__tag__"], json!("type.pi"));
    assert_eq!(v["__registry__"], json!("handl.canonical"));
    assert_eq!(v.as_object().unwrap().len(), 2);
}

#[test]
fn registry_construct_rejects_non_object_payload() {
    let reg = TypeRegistry::new("handl.canonical");
    assert!(reg.construct("type.pi", &json!(5)).is_none());
    assert!(reg.construct("type.pi", &json!("x")).is_none());
}

#[test]
fn registry_construct_rejects_smuggled_tag_key() {
    let reg = TypeRegistry::new("handl.canonical");
    assert!(
        reg.construct("type.pi", &json!({ "__tag__": "type.collection" }))
            .is_none()
    );
}

#[test]
fn registry_construct_rejects_smuggled_registry_key() {
    let reg = TypeRegistry::new("handl.canonical");
    assert!(
        reg.construct("type.pi", &json!({ "__registry__": "evil" }))
            .is_none()
    );
}

#[test]
fn registry_is_type_value_recognizes_registry_scoped_type() {
    let reg = TypeRegistry::new("handl.canonical");
    let v = reg
        .construct("type.collection", &json!({ "name": "number" }))
        .unwrap();
    assert!(reg.is_type_value(&v));
    assert_eq!(reg.as_type_tag(&v), Some("type.collection"));
}

#[test]
fn registry_is_type_value_rejects_non_type_tag() {
    // A tagged value whose tag does not start with "type." is not a type value.
    let reg = TypeRegistry::new("handl.canonical");
    let v = tag_value("unit", &json!({ "m": 5 })).unwrap();
    assert!(!reg.is_type_value(&v));
    assert_eq!(reg.as_type_tag(&v), None);
}

#[test]
fn registry_is_type_value_rejects_value_without_registry() {
    // A bare tagged value (no __registry__) is not a registry-scoped type value.
    let reg = TypeRegistry::new("handl.canonical");
    let v = tag_value("type.collection", &json!({ "name": "number" })).unwrap();
    assert!(!reg.is_type_value(&v));
    assert_eq!(reg.as_type_tag(&v), None);
}

#[test]
fn registry_is_type_value_rejects_different_registry() {
    let a = TypeRegistry::new("handl.canonical");
    let b = TypeRegistry::new("other");
    let v = a
        .construct("type.collection", &json!({ "name": "number" }))
        .unwrap();
    assert!(a.is_type_value(&v));
    assert!(!b.is_type_value(&v));
    assert_eq!(b.as_type_tag(&v), None);
}

#[test]
fn registry_is_type_value_rejects_non_tagged_value() {
    let reg = TypeRegistry::new("handl.canonical");
    assert!(!reg.is_type_value(&json!("number")));
    assert!(!reg.is_type_value(&json!({ "a": 1 })));
    assert!(!reg.is_type_value(&json!(42)));
}

#[test]
fn registry_match_type_dispatches_on_type_tag() {
    let reg = TypeRegistry::new("handl.canonical");
    let v = reg
        .construct("type.collection", &json!({ "name": "number" }))
        .unwrap();
    let cases = json!({ "type.collection": "is-collection", "type.pi": "is-pi" });
    assert_eq!(reg.match_type(&v, &cases), Some(json!("is-collection")));
}

#[test]
fn registry_match_type_returns_none_when_no_case() {
    let reg = TypeRegistry::new("handl.canonical");
    let v = reg
        .construct("type.sigma", &json!({ "name": "x" }))
        .unwrap();
    let cases = json!({ "type.collection": "is-collection" });
    assert_eq!(reg.match_type(&v, &cases), None);
}

#[test]
fn registry_match_type_returns_none_for_non_type_value() {
    let reg = TypeRegistry::new("handl.canonical");
    let cases = json!({ "type.collection": "is-collection" });
    // A non-tagged value is not a type value.
    assert_eq!(reg.match_type(&json!("number"), &cases), None);
    // A type value from a different registry is not recognized.
    let other = TypeRegistry::new("other");
    let v = other
        .construct("type.collection", &json!({ "name": "number" }))
        .unwrap();
    assert_eq!(reg.match_type(&v, &cases), None);
}

#[test]
fn registry_match_type_no_default_fallback() {
    // match_type has no "_" default-case fallback (unlike match_tag).
    let reg = TypeRegistry::new("handl.canonical");
    let v = reg
        .construct("type.sigma", &json!({ "name": "x" }))
        .unwrap();
    let cases = json!({ "type.collection": "is-collection", "_": "fallback" });
    assert_eq!(reg.match_type(&v, &cases), None);
}

// ---------------------------------------------------------------------------
// tag.match_type host function (ADR-0054) — `tag.construct` producer deleted
// ---------------------------------------------------------------------------

#[test]
fn match_type_host_function_dispatches() {
    let reg = TypeRegistry::new(crate::CANONICAL_REGISTRY_ID);
    let v = reg
        .construct("type.collection", &json!({ "name": "number" }))
        .unwrap();
    let cases = json!({ "type.collection": "is-collection", "type.pi": "is-pi" });
    let result = call_fn(&MatchType, &[v, cases]).unwrap();
    assert_eq!(result, json!("is-collection"));
}

#[test]
fn match_type_host_function_returns_null_when_no_case() {
    let reg = TypeRegistry::new(crate::CANONICAL_REGISTRY_ID);
    let v = reg.construct("type.sigma", &json!({ "name": "x" })).unwrap();
    let cases = json!({ "type.collection": "is-collection" });
    let result = call_fn(&MatchType, &[v, cases]).unwrap();
    assert_eq!(result, Value::Null);
}

#[test]
fn match_type_host_function_returns_null_for_non_type_value() {
    let cases = json!({ "type.collection": "is-collection" });
    let result = call_fn(&MatchType, &[json!("plain string"), cases]).unwrap();
    assert_eq!(result, Value::Null);
}

#[test]
fn match_type_host_function_no_default_fallback() {
    let reg = TypeRegistry::new(crate::CANONICAL_REGISTRY_ID);
    let v = reg.construct("type.sigma", &json!({ "name": "x" })).unwrap();
    let cases = json!({ "type.collection": "is-collection", "_": "fallback" });
    let result = call_fn(&MatchType, &[v, cases]).unwrap();
    assert_eq!(result, Value::Null);
}

#[test]
fn match_type_host_function_rejects_wrong_arity() {
    assert!(call_fn(&MatchType, &[json!("x")]).is_err());
    assert!(call_fn(&MatchType, &[json!("x"), json!({}), json!(1)]).is_err());
}
