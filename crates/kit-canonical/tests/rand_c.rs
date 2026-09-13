//! # rand_c.rs — libsodium-backed and stub `rand` backend smoke tests.
//!
//! `rand-c` loads directly from `handl-packages/rand-c` and uses
//! `ffi.call` to `libsodium.so` `randombytes_random`. `rand-stub` is the
//! deterministic, no-effect backend.

use handl_kit_canonical::Kit;

fn eval_rand_c(input: &str) -> serde_json::Value {
    let kit = Kit::new().expect("kit must build");
    let src = format!(
        "do {{\n  from \"../../handl-packages/rand-c\" import {{ int, float, bool, pick, weighted_pick, choice, sample, shuffle, bytes, string, gauss, exponential }};\n  {}\n}}",
        input
    );
    kit.eval(&src).expect("eval must succeed")
}

fn eval_rand_stub(input: &str) -> serde_json::Value {
    let kit = Kit::new().expect("kit must build");
    let src = format!(
        "do {{\n  from \"../../handl-packages/rand-stub\" import {{ int, float, bool, pick, weighted_pick, choice, sample, shuffle, bytes, string, gauss, exponential }};\n  {}\n}}",
        input
    );
    kit.eval(&src).expect("eval must succeed")
}

// ---------------------------------------------------------------------------
// rand-c.* — libsodium-backed random values (loaded from file)
// ---------------------------------------------------------------------------

#[test]
fn rand_c_int_is_bounded() {
    let v = eval_rand_c("int(6)").as_f64().expect("int is a number");
    assert!((0.0..=6.0).contains(&v), "rand-c int out of range: {v}");
}

#[test]
fn rand_c_float_is_bounded() {
    let v = eval_rand_c("float(10)")
        .as_f64()
        .expect("float is a number");
    assert!((0.0..10.0).contains(&v), "rand-c float out of range: {v}");
}

#[test]
fn rand_c_bool_is_boolean() {
    let v = eval_rand_c("bool()");
    assert!(v.is_boolean(), "rand-c bool must be a boolean");
}

#[test]
fn rand_c_pick_is_from_input() {
    let v = eval_rand_c("pick([\"a\", \"b\", \"c\"])");
    assert!(
        ["a", "b", "c"].iter().any(|s| *s == v.as_str().unwrap_or("")),
        "rand-c pick must be an element: {v}"
    );
}

#[test]
fn rand_c_choice_length() {
    let v = eval_rand_c("choice([\"a\", \"b\"], 4)");
    let arr = v.as_array().expect("choice returns an array");
    assert_eq!(arr.len(), 4, "rand-c choice length must match count");
}

#[test]
fn rand_c_sample_is_subset() {
    let v = eval_rand_c("sample([\"a\", \"b\", \"c\"], 2)");
    let arr = v.as_array().expect("sample returns an array");
    assert_eq!(arr.len(), 2, "rand-c sample length must match count");
    for item in arr {
        let s = item.as_str().unwrap_or("");
        assert!(["a", "b", "c"].contains(&s), "rand-c sample must be from input: {item}");
    }
}

#[test]
fn rand_c_shuffle_is_permutation() {
    let v = eval_rand_c("shuffle([\"a\", \"b\", \"c\"])");
    let arr = v.as_array().expect("shuffle returns an array");
    assert_eq!(arr.len(), 3, "rand-c shuffle length must match input");
    for item in arr {
        let s = item.as_str().unwrap_or("");
        assert!(["a", "b", "c"].contains(&s), "rand-c shuffle must be a permutation: {item}");
    }
}

#[test]
fn rand_c_bytes_length_and_range() {
    let v = eval_rand_c("bytes(8)");
    let arr = v.as_array().expect("bytes returns an array");
    assert_eq!(arr.len(), 8, "rand-c bytes length must match n");
    for item in arr {
        let b = item.as_f64().unwrap_or(-1.0);
        assert!((0.0..=255.0).contains(&b), "rand-c byte out of range: {item}");
    }
}

#[test]
fn rand_c_string_length_and_charset() {
    let v = eval_rand_c("string(6, \"abc\")");
    let s = v.as_str().expect("string returns a string");
    assert_eq!(s.len(), 6, "rand-c string length must match len");
    for c in s.chars() {
        assert!("abc".contains(c), "rand-c string char not in charset: {c}");
    }
}

#[test]
fn rand_c_weighted_pick_rejects_length_mismatch() {
    let v = eval_rand_c("weighted_pick([1, 2], [\"a\"])");
    assert!(v.is_null(), "rand-c weighted_pick must reject length mismatch: {v}");
}

#[test]
fn rand_c_weighted_pick_is_from_input() {
    let v = eval_rand_c("weighted_pick([1, 2, 3], [\"a\", \"b\", \"c\"])");
    assert!(
        ["a", "b", "c"].iter().any(|s| *s == v.as_str().unwrap_or("")),
        "rand-c weighted_pick must be from items: {v}"
    );
}

// ---------------------------------------------------------------------------
// rand-stub.* — deterministic, no-effect backend
// ---------------------------------------------------------------------------

#[test]
fn rand_stub_is_deterministic() {
    let i = eval_rand_stub("int(6)").as_f64().unwrap();
    let f = eval_rand_stub("float(10)").as_f64().unwrap();
    let b = eval_rand_stub("bool()");
    assert_eq!(i, 4.0, "rand-stub int is fixed");
    assert_eq!(f, 0.5, "rand-stub float is fixed");
    assert_eq!(b, serde_json::json!(true), "rand-stub bool is fixed");
}

#[test]
fn rand_stub_pick_is_first() {
    assert_eq!(eval_rand_stub("pick([\"x\", \"y\", \"z\"])"), serde_json::json!("x"));
}

#[test]
fn rand_stub_weighted_pick_is_first() {
    assert_eq!(
        eval_rand_stub("weighted_pick([2, 1, 1], [\"x\", \"y\", \"z\"])"),
        serde_json::json!("x")
    );
}

#[test]
fn rand_stub_choice_sample_shuffle_are_deterministic() {
    assert_eq!(eval_rand_stub("choice([\"a\", \"b\"], 3)"), serde_json::json!(["a", "b", "a"]));
    assert_eq!(eval_rand_stub("sample([\"a\", \"b\", \"c\"], 2)"), serde_json::json!(["a", "b"]));
    assert_eq!(eval_rand_stub("shuffle([\"a\", \"b\", \"c\"])"), serde_json::json!(["c", "b", "a"]));
}

#[test]
fn rand_stub_bytes_and_string_are_deterministic() {
    assert_eq!(eval_rand_stub("bytes(3)"), serde_json::json!([4, 4, 4]));
    assert_eq!(eval_rand_stub("string(4, \"abc\")"), serde_json::json!("aaaa"));
}

#[test]
fn rand_c_gauss_is_number() {
    let v = eval_rand_c("gauss(5, 2)");
    assert!(v.is_number(), "rand-c gauss must return a number: {v}");
}

#[test]
fn rand_c_exponential_is_non_negative() {
    let v = eval_rand_c("exponential(1.5)")
        .as_f64()
        .expect("exponential must return a number");
    assert!(v >= 0.0, "rand-c exponential must be non-negative: {v}");
}

#[test]
fn rand_stub_gauss_is_mean() {
    assert_eq!(eval_rand_stub("gauss(5, 2)"), serde_json::json!(5));
}

#[test]
fn rand_stub_exponential_is_zero() {
    assert_eq!(eval_rand_stub("exponential(1)"), serde_json::json!(0.0));
}
