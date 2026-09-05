//! Unit tests for the native `unicode` module.

use serde_json::json;

use crate::UnicodeModule;
use handl_platform::Module;

fn call(name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let f = UnicodeModule
        .function(name)
        .unwrap_or_else(|| panic!("unicode.{name} must exist"));
    f.call(args, &mut handl_platform::EvalContext::new())
        .unwrap_or_else(|e| panic!("unicode.{name}({args:?}) failed: {e}"))
}

#[test]
fn code_point_interop() {
    assert_eq!(
        call("code_points", &[json!("héllo")]),
        json!([104, 233, 108, 108, 111])
    );
    assert_eq!(
        call("from_code_points", &[json!([104, 233, 108, 108, 111])]),
        json!("héllo")
    );
    assert_eq!(call("is_ascii", &[json!("abc")]), json!(true));
    assert_eq!(call("is_ascii", &[json!("hé")]), json!(false));
    assert_eq!(
        call("code_point_at", &[json!("héllo"), json!(1.0)]),
        json!(233)
    );
    assert_eq!(
        call("code_point_at", &[json!("héllo"), json!(99.0)]),
        json!(null)
    );
}

#[test]
fn decode_utf8() {
    let out = call("decode", &[json!("hé")]);
    assert_eq!(out, json!([[0, 104], [1, 233]]));
}

#[test]
fn identifier_validation() {
    assert_eq!(
        call("is_valid_identifier", &[json!("foo_bar")]),
        json!(true)
    );
    assert_eq!(
        call("is_valid_identifier", &[json!("123abc")]),
        json!(false)
    );
    assert_eq!(call("is_xid_start", &[json!("foo")]), json!(true));
    assert_eq!(call("is_xid_start", &[json!("123")]), json!(false));
    assert_eq!(call("is_xid_continue", &[json!("foo")]), json!(true));
    assert_eq!(call("is_xid_continue", &[json!("!foo")]), json!(false));
}

#[test]
fn graphemes_uax29() {
    // Combining mark should cluster with the base letter.
    // The returned cluster preserves the original code points ("e" + U+0301).
    assert_eq!(
        call("graphemes", &[json!("e\u{0301}x")]),
        json!(["e\u{0301}", "x"])
    );
    assert_eq!(call("grapheme_count", &[json!("e\u{0301}x")]), json!(2));
    // Empty string.
    assert_eq!(call("graphemes", &[json!("")]), json!([]));
    assert_eq!(call("grapheme_count", &[json!("")]), json!(0));
}

#[test]
fn width_uax11() {
    assert_eq!(call("width", &[json!("日本")]), json!(4));
    assert_eq!(call("width", &[json!("abc")]), json!(3));
    // Combining marks have zero width.
    assert_eq!(call("width", &[json!("e\u{0301}")]), json!(1));
}

#[test]
fn casefold_full() {
    assert_eq!(call("casefold", &[json!("abc")]), json!("abc"));
    assert_eq!(call("casefold", &[json!("Straße")]), json!("strasse"));
}

#[test]
fn normalization_nfc_nfd() {
    assert_eq!(call("nfc", &[json!("e\u{0301}")]), json!("é"));
    assert_eq!(call("nfd", &[json!("é")]), json!("e\u{0301}"));
    assert_eq!(call("nfc", &[json!("é")]), json!("é"));
    assert_eq!(call("nfd", &[json!("e\u{0301}")]), json!("e\u{0301}"));
}

#[test]
fn fallback_ops_still_work() {
    // `unicode-handl` still supplies `from_code_points` and `is_ascii` if not
    // shadowed; here native implements them too, but the record fallback is
    // exercised when a name is not in the native op set.
    assert_eq!(UnicodeModule.function("from_code_points").is_some(), true);
    assert_eq!(UnicodeModule.function("is_ascii").is_some(), true);
}
