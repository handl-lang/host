//! # intl_c.rs — `intl-c` end-to-end smoke test.
//!
//! `intl_c` is the `intl` contract backend over the system ICU4C library,
//! wired into the canonical kit as a table module. This test exercises every
//! contract op through `Kit::eval` on the file import — the same path
//! `hnl test` uses (`module_prep`).
//!
//! NOTE: module eval under the term-interpreter is slow (~4 min for the
//! ~1000-line `intl-c.hnl` — every `import`/`private`/record field walks the
//! AST). All ops are exercised inside a *single* eval so the module is
//! loaded once. On a host without ICU4C the module still loads — the ops
//! just return `err "icu-unavailable"` — so the test asserts on
//! `intl_c.available` and skips the value checks.

use handl_kit_canonical::Kit;
use serde_json::Value;
use std::sync::OnceLock;

/// One `Kit::eval` that loads `intl-c` once and runs every contract op,
/// returning a `{ name: result }` record of raw outputs. Results are the
/// backend's `{__tag__: "ok", value}` / `{__tag__: "err", msg}` values.
fn results() -> &'static Value {
    static RESULTS: OnceLock<Value> = OnceLock::new();
    RESULTS.get_or_init(|| {
        let kit = Kit::new().expect("kit must build");
        let src = r#"do {
  from "../../handl-packages/intl-c" import * as intl_c
  val := (r) ~> if tag.tag_of(r) == "ok" { tag.untag(r).value } else { { __err__: tag.untag(r).value } }
  if intl_c.available {
    {
      available: intl_c.available,
      icu_version: intl_c.icu_version,
      compare_lt: val(intl_c.compare_strings("apple", "banana", "en", {})),
      compare_gt: val(intl_c.compare_strings("b", "a", "en", {})),
      compare_eq: val(intl_c.compare_strings("a", "a", "en", {})),
      compare_numeric: val(intl_c.compare_strings("a2", "a10", "en", { numeric: true })),
      format_decimal: val(intl_c.format_number(1234.5, "en", {})),
      format_percent: val(intl_c.format_number(0.5, "en", { style: "percent" })),
      format_currency: val(intl_c.format_number(1234.5, "en", { style: "currency", currency: "USD" })),
      format_parts: val(intl_c.format_number_to_parts(-1234.5, "en", { style: "currency", currency: "USD" })),
      plural_one: val(intl_c.select_plural(1, "en", {})),
      plural_other: val(intl_c.select_plural(2, "en", {})),
      plural_ordinal: val(intl_c.select_plural(2, "en", { type: "ordinal" })),
      list_and: val(intl_c.format_list(["a", "b", "c"], "en", {})),
      list_or: val(intl_c.format_list(["a", "b"], "en", { type: "disjunction" })),
      rel_yesterday: val(intl_c.format_relative_time(-1, "day", "en", {})),
      rel_days: val(intl_c.format_relative_time(3, "day", "en", { numeric: "always" })),
      date_epoch: val(intl_c.format_date(0, "en", {})),
      date_iso: val(intl_c.format_date("2024-03-15", "en", {})),
      date_parts: val(intl_c.format_date_to_parts("2024-03-15", "en", {})),
      duration: val(intl_c.format_duration(3661, "en", {})),
      name_lang: val(intl_c.display_name("en", "language", "en", {})),
      name_region: val(intl_c.display_name("US", "region", "en", {})),
      seg_words: val(intl_c.segment("hello world", "word", "en", {})),
      seg_graphemes: val(intl_c.segment("abc", "grapheme", "en", {})),
      locale_rec: val(intl_c.locale("en-US", {})),
      canonical: val(intl_c.get_canonical_locales(["EN-us", "de"])),
      supported: val(intl_c.supported_locales_of(["en-US", "tlh", "de"], {}))
    }
  } else {
    { available: intl_c.available }
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

#[test]
fn intl_c_available_is_bool() {
    assert!(field("available").is_boolean(), "{:?}", field("available"));
}

#[test]
fn compare_strings_orders() {
    if !icu_available() {
        return;
    }
    assert_eq!(field("compare_lt"), Value::from(-1));
    assert_eq!(field("compare_gt"), Value::from(1));
    assert_eq!(field("compare_eq"), Value::from(0));
    assert_eq!(field("compare_numeric"), Value::from(-1));
}

#[test]
fn format_number_styles() {
    if !icu_available() {
        return;
    }
    assert_eq!(field("format_decimal"), Value::from("1,234.5"));
    assert_eq!(field("format_percent"), Value::from("50%"));
    assert_eq!(field("format_currency"), Value::from("$1,234.50"));
}

#[test]
fn format_number_to_parts_splits_fields() {
    if !icu_available() {
        return;
    }
    let text = serde_json::to_string(&field("format_parts")).unwrap();
    for t in ["currency", "integer", "minusSign"] {
        assert!(text.contains(&format!("\"{t}\"")), "{text}");
    }
}

#[test]
fn select_plural_cardinal_ordinal() {
    if !icu_available() {
        return;
    }
    assert_eq!(field("plural_one"), Value::from("one"));
    assert_eq!(field("plural_other"), Value::from("other"));
    assert_eq!(field("plural_ordinal"), Value::from("two"));
}

#[test]
fn format_list_and_or() {
    if !icu_available() {
        return;
    }
    assert_eq!(field("list_and"), Value::from("a, b, and c"));
    assert_eq!(field("list_or"), Value::from("a or b"));
}

#[test]
fn format_relative_time_past_future() {
    if !icu_available() {
        return;
    }
    assert_eq!(field("rel_yesterday"), Value::from("yesterday"));
    assert_eq!(field("rel_days"), Value::from("in 3 days"));
}

#[test]
fn format_date_and_parts() {
    if !icu_available() {
        return;
    }
    let s = field("date_epoch");
    assert!(s.as_str().map_or(false, |s| !s.is_empty()), "{s}");
    let s = field("date_iso");
    let s = s.as_str().expect("date string");
    assert!(s.contains("15") || s.contains("Mar"), "{s}");
    let text = serde_json::to_string(&field("date_parts")).unwrap();
    for t in ["year", "day"] {
        assert!(text.contains(&format!("\"{t}\"")), "{text}");
    }
}

#[test]
fn format_duration_seconds() {
    if !icu_available() {
        return;
    }
    let s = field("duration");
    assert_eq!(s, Value::from("1 hr, 1 min, 1 sec"), "{s}");
}

#[test]
fn display_name_language_region() {
    if !icu_available() {
        return;
    }
    assert_eq!(field("name_lang"), Value::from("English"));
    assert_eq!(field("name_region"), Value::from("United States"));
}

#[test]
fn segment_words_and_graphemes() {
    if !icu_available() {
        return;
    }
    let segs = field("seg_words");
    let segs = segs.as_array().expect("segments");
    assert!(segs.len() >= 3, "{segs:?}");
    assert_eq!(segs[0].get("segment"), Some(&Value::from("hello")));
    assert_eq!(segs[0].get("isWordLike"), Some(&Value::from(true)));
    let segs = field("seg_graphemes");
    assert_eq!(segs.as_array().unwrap().len(), 3);
}

#[test]
fn locale_parses_and_canonicalizes() {
    if !icu_available() {
        return;
    }
    let loc = field("locale_rec");
    assert_eq!(loc.get("language"), Some(&Value::from("en")));
    assert_eq!(loc.get("region"), Some(&Value::from("US")));
    assert_eq!(field("canonical"), serde_json::json!(["en-US", "de"]));
}

#[test]
fn supported_locales_of_filters() {
    if !icu_available() {
        return;
    }
    let locs = field("supported");
    let locs = locs.as_array().expect("list");
    assert!(locs.contains(&Value::from("en-US")), "{locs:?}");
    assert!(locs.contains(&Value::from("de")), "{locs:?}");
}
