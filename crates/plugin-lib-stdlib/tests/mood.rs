use handl_platform::Function;
use serde_json::Value as JsonValue;

use handl_platform::EvalContext;
use handl_lib_mood::ops::{MoodCnd, MoodInd, MoodInt, MoodNeg, MoodNorm, MoodPot};

fn ctx() -> EvalContext {
    EvalContext::new()
}

fn ok(v: JsonValue) -> serde_json::Value {
    serde_json::Value::from(JsonValue::Object(serde_json::Map::from_iter([(
        "ok".to_string(),
        v,
    )])))
}

fn err(v: JsonValue) -> serde_json::Value {
    serde_json::Value::from(JsonValue::Object(serde_json::Map::from_iter([(
        "err".to_string(),
        v,
    )])))
}

fn call(f: &dyn Function, args: &[serde_json::Value]) -> serde_json::Value {
    let mut c = ctx();
    f.call(args, &mut c).unwrap()
}

#[test]
fn int_matches_variant() {
    assert!(
        call(
            &MoodInt,
            &[ok(JsonValue::from(42)), serde_json::Value::from("ok")]
        )
        .as_bool()
        .unwrap()
    );
    assert!(
        !call(
            &MoodInt,
            &[
                err(JsonValue::String("bad".to_string())),
                serde_json::Value::from("ok")
            ]
        )
        .as_bool()
        .unwrap()
    );
}

#[test]
fn int_matches_type() {
    assert!(
        call(
            &MoodInt,
            &[
                serde_json::Value::from("hello"),
                serde_json::Value::from("type.string")
            ]
        )
        .as_bool()
        .unwrap()
    );
    assert!(
        !call(
            &MoodInt,
            &[
                serde_json::Value::from(42.0),
                serde_json::Value::from("type.string")
            ]
        )
        .as_bool()
        .unwrap()
    );
}

#[test]
fn pot_extracts_ok() {
    let result = call(
        &MoodPot,
        &[ok(JsonValue::from(42)), serde_json::Value::from("ok")],
    );
    assert_eq!(result.as_f64(), Some(42.0));
}

#[test]
fn pot_returns_null_on_mismatch() {
    let result = call(
        &MoodPot,
        &[err(JsonValue::from("bad")), serde_json::Value::from("ok")],
    );
    assert!(result.is_null());
}

#[test]
fn neg_extracts_err() {
    let result = call(
        &MoodNeg,
        &[err(JsonValue::from("bad")), serde_json::Value::from("ok")],
    );
    assert_eq!(result.as_str(), Some("bad"));
}

#[test]
fn neg_returns_null_when_matches() {
    let result = call(
        &MoodNeg,
        &[ok(JsonValue::from(42)), serde_json::Value::from("ok")],
    );
    assert!(result.is_null());
}

#[test]
fn neg_keeps_non_matching_value() {
    let result = call(
        &MoodNeg,
        &[
            serde_json::Value::from(5.0),
            serde_json::Value::from("type.string"),
        ],
    );
    assert_eq!(result.as_f64(), Some(5.0));
}

#[test]
fn norm_wraps_result() {
    let ok_result = call(
        &MoodNorm,
        &[ok(JsonValue::from(42)), serde_json::Value::from("ok")],
    );
    assert_eq!(ok_result.get("ok").unwrap().as_f64(), Some(42.0));

    let err_result = call(
        &MoodNorm,
        &[err(JsonValue::from("bad")), serde_json::Value::from("ok")],
    );
    assert!(err_result.get("err").is_some());
}

#[test]
fn cnd_falls_back() {
    let value = call(
        &MoodCnd,
        &[
            err(JsonValue::from("bad")),
            serde_json::Value::from("ok"),
            serde_json::Value::from("default"),
        ],
    );
    assert_eq!(value.as_str(), Some("default"));
}

#[test]
fn ind_strict() {
    let value = call(
        &MoodInd,
        &[ok(JsonValue::from(42)), serde_json::Value::from("ok")],
    );
    assert_eq!(value.as_f64(), Some(42.0));
}
