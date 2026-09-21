use serde_json::{Number, json};

use super::helpers;
use super::{Arrow, Tag, Value, ValueAbi};

fn roundtrip(v: ValueAbi) {
    let j = v.to_json();
    let back = ValueAbi::from_json(&j).expect("from_json should succeed");
    assert_eq!(v, back, "round-trip failed for {:?}", v);
}

#[test]
fn roundtrips_null() {
    roundtrip(ValueAbi::Null);
    roundtrip(ValueAbi::Bool(true));
    roundtrip(ValueAbi::Bool(false));
}

#[test]
fn roundtrips_numbers() {
    roundtrip(ValueAbi::Number(Number::from(0)));
    roundtrip(ValueAbi::Number(Number::from(-42i64)));
    roundtrip(ValueAbi::Number(Number::from_f64(3.14).unwrap()));
}

#[test]
fn roundtrips_string() {
    roundtrip(ValueAbi::String("hello".to_string()));
    roundtrip(ValueAbi::String("".to_string()));
}

#[test]
fn roundtrips_array() {
    roundtrip(ValueAbi::Array(vec![
        ValueAbi::Null,
        ValueAbi::Bool(true),
        ValueAbi::String("x".to_string()),
        ValueAbi::Number(Number::from(7)),
    ]));
}

#[test]
fn roundtrips_object() {
    let mut map = std::collections::HashMap::new();
    map.insert("a".to_string(), ValueAbi::Number(Number::from(1)));
    map.insert("b".to_string(), ValueAbi::String("two".to_string()));
    roundtrip(ValueAbi::Object(map));
}

#[test]
fn roundtrips_bytes() {
    roundtrip(ValueAbi::Bytes(vec![0, 1, 255, 128]));
}

#[test]
fn roundtrips_pair() {
    roundtrip(ValueAbi::Pair(
        Box::new(ValueAbi::String("fst".to_string())),
        Box::new(ValueAbi::Number(Number::from(2))),
    ));
}

#[test]
fn roundtrips_either() {
    roundtrip(ValueAbi::EitherLeft(Box::new(ValueAbi::Number(
        Number::from(1),
    ))));
    roundtrip(ValueAbi::EitherRight(Box::new(ValueAbi::Bool(false))));
}

#[test]
fn roundtrips_arrow() {
    let mut binds = std::collections::HashMap::new();
    binds.insert("x".to_string(), ValueAbi::Number(Number::from(10)));
    let arrow = Arrow {
        params: vec!["x".to_string(), "y".to_string()],
        body_def: Some(Box::new(ValueAbi::Object(
            [("__ast__".to_string(), ValueAbi::Null)].into_iter().collect(),
        ))),
        binds,
        rest: Some("xs".to_string()),
        array_pattern: Some(false),
        handlers: vec![ValueAbi::Null],
    };
    roundtrip(ValueAbi::Arrow(arrow));
}

#[test]
fn roundtrips_resource() {
    roundtrip(ValueAbi::Resource { ptr: 0x1234 });
}

#[test]
fn handle_serializes_as_resource() {
    let handle = ValueAbi::Handle(0xAB);
    let expected = json!({"__resource__": {"ptr": 0xABu64}});
    assert_eq!(handle.to_json(), expected);
    let parsed = ValueAbi::from_json(&expected).expect("resource parses");
    assert_eq!(parsed, ValueAbi::Resource { ptr: 0xAB });
}

#[test]
fn invalid_json_rejected() {
    assert!(ValueAbi::from_json(&json!({"__either__": "up", "value": 1})).is_err());
    assert!(ValueAbi::from_json(&json!({"__bytes__": [256]})).is_err());
    assert!(ValueAbi::from_json(&json!({"__arrow__": false})).is_err());
    assert!(ValueAbi::from_json(&json!({"__pair__": [1]})).is_err());
    assert!(ValueAbi::from_json(&json!({"__resource__": {"ptr": "not a number"}})).is_err());
}

#[test]
fn helper_name_constants_are_correct() {
    assert_eq!(helpers::HELPER_V2_PAIR, "helper_v2_pair");
    assert_eq!(helpers::HELPER_V2_CLONE, "helper_v2_clone");
    assert_eq!(helpers::HELPER_V2_INT_ARITH, "helper_v2_int_arith");
    assert_eq!(helpers::HELPER_V2_LOOP_CHECK, "helper_v2_loop_check");
    assert_eq!(
        helpers::HELPER_V2_DERIVED_STEP_ARG_INCOMING,
        "helper_v2_derived_step_arg_incoming"
    );
}

#[test]
fn helper_registry_contains_all_helpers() {
    assert_eq!(helpers::ALL.len(), 24);
    assert!(helpers::ALL.iter().all(|n| n.starts_with("helper_v2_")));
    assert!(helpers::ALL.contains(&"helper_v2_pair"));
    assert!(helpers::ALL.contains(&"helper_v2_apply_closure"));
    assert_eq!(helpers::ALL[0], helpers::HELPER_V2_CLONE);
    assert_eq!(
        helpers::ALL.last().copied().unwrap(),
        helpers::HELPER_V2_LOOP_CHECK
    );
}

#[test]
fn value_abi_is_clone_debug_partialeq() {
    let v = ValueAbi::Number(Number::from(42));
    let c = v.clone();
    assert_eq!(v, c);
    assert_ne!(v, ValueAbi::Number(Number::from(0)));
    let _ = format!("{:?}", v);

    let arrow = ValueAbi::Arrow(Arrow::default());
    let _ = arrow.clone();
    let _ = format!("{:?}", arrow);
    assert_eq!(arrow, arrow);
}

#[test]
fn tag_enum_traits() {
    let t = Tag::Pair;
    assert_eq!(t, Tag::Pair);
    assert_ne!(t, Tag::Null);
    assert_eq!(t.as_str(), "pair");
    let _ = format!("{:?}", t);
    let c = t;
    assert_eq!(t, c);
}

#[test]
fn value_alias_points_to_value_abi() {
    let v: Value = ValueAbi::Null;
    assert_eq!(v.tag(), Tag::Null);
    assert_eq!(v.to_json(), json!(null));
}

#[test]
fn value_abi_check_valid() {
    let mut binds = std::collections::HashMap::new();
    binds.insert("x".to_string(), ValueAbi::Number(Number::from(1)));
    let arrow = Arrow {
        params: vec!["x".to_string()],
        binds,
        ..Arrow::default()
    };
    let values = vec![
        ValueAbi::Null,
        ValueAbi::Bool(true),
        ValueAbi::Number(Number::from(42)),
        ValueAbi::String("ok".to_string()),
        ValueAbi::Bytes(vec![0, 255]),
        ValueAbi::Array(vec![ValueAbi::Number(Number::from(1))]),
        ValueAbi::Object({
            let mut m = std::collections::HashMap::new();
            m.insert("a".to_string(), ValueAbi::Number(Number::from(1)));
            m
        }),
        ValueAbi::Pair(
            Box::new(ValueAbi::Number(Number::from(1))),
            Box::new(ValueAbi::Number(Number::from(2))),
        ),
        ValueAbi::EitherLeft(Box::new(ValueAbi::Number(Number::from(1)))),
        ValueAbi::EitherRight(Box::new(ValueAbi::Bool(false))),
        ValueAbi::Arrow(arrow),
    ];
    for v in values {
        assert!(v.check().is_ok(), "check failed for {v:?}");
    }
}

#[test]
fn value_abi_check_invalid() {
    let mut map = std::collections::HashMap::new();
    map.insert(
        "__either__".to_string(),
        ValueAbi::String("left".to_string()),
    );
    let v = ValueAbi::Object(map);
    assert!(
        v.check().is_err(),
        "reserved __either__ key should be rejected"
    );
}
