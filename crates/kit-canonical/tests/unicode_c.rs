//! # unicode_c.rs — `unicode-c` end-to-end smoke test.
//!
//! `unicode_c` is the `unicode` contract backend over the system ICU4C
//! common library (`ubrk_*` grapheme segmentation, `unorm2_*` NFC/NFD,
//! `u_strFoldCase`, `u_hasBinaryProperty`, East Asian Width), wired into
//! the canonical kit as a table module.
//!
//! The contract ops are exercised through `Kit::eval` on the *file* import
//! (the same shape `hnl test` uses); `unicode_modules_are_wired` proves the
//! table registration, and the module's own bare-name imports (`result`,
//! `bytes`) exercise the table lookup the kit wires.
//!
//! Module eval is slow (every `import`/record field walks the AST), so all
//! ops are exercised inside a *single* eval — a second module load (e.g. a
//! separate probe module importing `unicode_c` by name) would roughly
//! double the suite's wall time. On a host without ICU4C the module still
//! loads and `available` is false — the table-backed ops raise, so the
//! value checks are gated on `available`.

use handl_kit_canonical::{Kit, canonical_wire_names};
use serde_json::Value;
use std::sync::OnceLock;

/// One `Kit::eval` that loads `unicode_c` once and runs every contract op,
/// returning a `{ name: result }` record of plain values.
fn results() -> &'static Value {
    static RESULTS: OnceLock<Value> = OnceLock::new();
    RESULTS.get_or_init(|| {
        let kit = Kit::new().expect("kit must build");
        let src = r#"do {
  from "../../handl-packages/unicode-c" import * as u
  if u.available {
    {
      available: u.available,
      icu_version: u.icu_version,
      code_points: u.code_points("héllo"),
      from_code_points: u.from_code_points([104, 233]),
      is_ascii_yes: u.is_ascii("abc"),
      is_ascii_no: u.is_ascii("é"),
      code_point_at: u.code_point_at("abc", 1),
      decode_ascii: u.decode("aé"),
      decode_astral: u.decode("😀x"),
      ident_ok: u.is_valid_identifier("_abc"),
      ident_leading_digit: u.is_valid_identifier("1abc"),
      xid_start_cjk: u.is_xid_start("日"),
      xid_continue_digit: u.is_xid_continue("1"),
      graphemes_ascii: u.graphemes("abc"),
      grapheme_count_ascii: u.grapheme_count("abc"),
      grapheme_count_combining: u.grapheme_count(u.from_code_points([101, 769])),
      grapheme_count_zwj: u.grapheme_count(u.from_code_points([128104, 8205, 128105, 8205, 128103])),
      grapheme_count_crlf: u.grapheme_count(u.from_code_points([13, 10])),
      grapheme_count_flag: u.grapheme_count(u.from_code_points([127482, 127480])),
      graphemes_covering: str.join(u.graphemes("a😀日本"), ""),
      width_ascii: u.width("abc"),
      width_cjk: u.width("日本"),
      width_combining: u.width(u.from_code_points([101, 769])),
      casefold_ascii: u.casefold("ABC"),
      casefold_sharp_s: u.casefold("ß"),
      nfd_acute: u.nfd("é"),
      nfc_composes: u.nfc(u.from_code_points([101, 769])),
      nfc_hangul_round_trip: u.nfc(u.nfd("한")),
      nfd_ascii: u.nfd("abc")
    }
  } else {
    { available: u.available }
  }
}
"#;
        kit.eval(src).expect("eval must succeed")
    })
}

fn field(name: &str) -> Value {
    results()
        .get(name)
        .cloned()
        .unwrap_or_else(|| panic!("missing result field {name}"))
}

fn icu_available() -> bool {
    field("available") == Value::Bool(true)
}

/// The kit registers `unicode_c` (the ICU4C backend) and `unicode_handl`
/// (the pure-handl fallback) as wire names — the check the typechecker's
/// unbound-wire scope uses.
#[test]
fn unicode_modules_are_wired() {
    let names = canonical_wire_names();
    assert!(names.contains("unicode_c"), "unicode_c not wired");
    assert!(names.contains("unicode_handl"), "unicode_handl not wired");
    assert!(names.contains("graphemes"), "unicode_c fields not wired");
}

#[test]
fn unicode_c_available_is_bool() {
    assert!(field("available").is_boolean(), "{:?}", field("available"));
}

#[test]
fn code_point_interop() {
    assert_eq!(field("code_points"), serde_json::json!([104, 233, 108, 108, 111]));
    assert_eq!(field("from_code_points"), Value::from("hé"));
    assert_eq!(field("is_ascii_yes"), Value::Bool(true));
    assert_eq!(field("is_ascii_no"), Value::Bool(false));
    assert_eq!(field("code_point_at"), Value::from(98));
}

#[test]
fn decode_pairs_with_byte_offsets() {
    assert_eq!(field("decode_ascii"), serde_json::json!([[0, 97], [1, 233]]));
    assert_eq!(field("decode_astral"), serde_json::json!([[0, 128512], [4, 120]]));
}

#[test]
fn identifier_categories() {
    if !icu_available() {
        return;
    }
    assert_eq!(field("ident_ok"), Value::Bool(true));
    assert_eq!(field("ident_leading_digit"), Value::Bool(false));
    assert_eq!(field("xid_start_cjk"), Value::Bool(true));
    assert_eq!(field("xid_continue_digit"), Value::Bool(true));
}

#[test]
fn grapheme_clusters() {
    if !icu_available() {
        return;
    }
    assert_eq!(field("graphemes_ascii"), serde_json::json!(["a", "b", "c"]));
    assert_eq!(field("grapheme_count_ascii"), Value::from(3));
    // base + combining acute
    assert_eq!(field("grapheme_count_combining"), Value::from(1));
    // ZWJ family emoji
    assert_eq!(field("grapheme_count_zwj"), Value::from(1));
    // CRLF
    assert_eq!(field("grapheme_count_crlf"), Value::from(1));
    // regional-indicator flag pair
    assert_eq!(field("grapheme_count_flag"), Value::from(1));
    // covering law: concat(graphemes(s)) == s
    assert_eq!(field("graphemes_covering"), Value::from("a😀日本"));
}

#[test]
fn display_width() {
    if !icu_available() {
        return;
    }
    assert_eq!(field("width_ascii"), Value::from(3));
    assert_eq!(field("width_cjk"), Value::from(4));
    assert_eq!(field("width_combining"), Value::from(1));
}

#[test]
fn case_folding() {
    if !icu_available() {
        return;
    }
    assert_eq!(field("casefold_ascii"), Value::from("abc"));
    assert_eq!(field("casefold_sharp_s"), Value::from("ss"));
}

#[test]
fn normalization() {
    if !icu_available() {
        return;
    }
    assert_eq!(field("nfd_acute"), Value::from("e\u{0301}"));
    assert_eq!(field("nfc_composes"), Value::from("é"));
    assert_eq!(field("nfc_hangul_round_trip"), Value::from("한"));
    assert_eq!(field("nfd_ascii"), Value::from("abc"));
}
