use crate::signature_help::signature_help;

/// Helper: find the byte offset of the first occurrence of `needle` in `hay`.
fn offset_of(hay: &str, needle: &str) -> usize {
    hay.find(needle).unwrap_or_else(|| panic!("needle not found: {needle}"))
}

/// Helper: offset pointing just past the first occurrence of `needle`.
fn offset_after(hay: &str, needle: &str) -> usize {
    offset_of(hay, needle) + needle.len()
}

#[test]
fn signature_help_inside_call_returns_signature() {
    let src = "comp(a, b)";
    // Cursor right after the `(` of `comp(`.
    let offset = offset_after(src, "comp(");
    let value = signature_help(src, offset).expect("expected signature help inside call");

    let sigs = value["signatures"].as_array().expect("signatures is array");
    assert_eq!(sigs.len(), 1, "exactly one signature");
    let label = sigs[0]["label"].as_str().expect("label is string");
    assert!(label.contains("comp"), "label: {label}");
    // activeSignature must be present.
    assert_eq!(value["activeSignature"].as_i64(), Some(0));
}

#[test]
fn signature_help_outside_call_returns_none() {
    let src = "comp(a, b)";
    // Cursor on the function name (before the `(`) — not inside a call.
    let offset = offset_of(src, "comp");
    assert!(signature_help(src, offset).is_none());

    // Cursor past the closing paren — also not inside a call.
    let offset = offset_after(src, ")");
    assert!(signature_help(src, offset).is_none());
}

#[test]
fn signature_help_counts_active_parameter_correctly() {
    let src = "choice(l, r)";
    // Cursor before `r` (after the comma) → second parameter, index 1.
    let offset = offset_of(src, "r");
    let value = signature_help(src, offset).expect("expected signature help");
    assert_eq!(value["activeParameter"].as_i64(), Some(1), "activeParameter for second arg");

    // Cursor before `l` (right after `(`) → first parameter, index 0.
    let offset = offset_after(src, "(");
    let value = signature_help(src, offset).expect("expected signature help");
    assert_eq!(value["activeParameter"].as_i64(), Some(0), "activeParameter for first arg");
}

#[test]
fn signature_help_for_arr_combinator() {
    let src = "arr(\"add\", x)";
    let offset = offset_after(src, "arr(");
    let value = signature_help(src, offset).expect("expected signature help for arr");
    let sigs = value["signatures"].as_array().unwrap();
    let label = sigs[0]["label"].as_str().unwrap();
    assert!(label.starts_with("arr("), "label: {label}");
    let params = sigs[0]["parameters"].as_array().unwrap();
    assert_eq!(params.len(), 2, "arr has two parameters");
    assert_eq!(params[0]["label"].as_str(), Some("op"));
    assert_eq!(params[1]["label"].as_str(), Some("args…"));
}

#[test]
fn signature_help_for_comp_combinator() {
    let src = "comp(a, b)";
    let offset = offset_after(src, "comp(");
    let value = signature_help(src, offset).expect("expected signature help for comp");
    let sigs = value["signatures"].as_array().unwrap();
    let label = sigs[0]["label"].as_str().unwrap();
    assert!(label.contains("comp"), "label: {label}");
    let params = sigs[0]["parameters"].as_array().unwrap();
    assert_eq!(params.len(), 2);
    assert_eq!(params[0]["label"].as_str(), Some("a (first arrow)"));
    assert_eq!(params[1]["label"].as_str(), Some("b (second arrow)"));
}

#[test]
fn signature_help_for_insert() {
    let src = "insert({ elem, acc })";
    let offset = offset_after(src, "insert(");
    let value = signature_help(src, offset).expect("expected signature help for insert");
    let sigs = value["signatures"].as_array().unwrap();
    let label = sigs[0]["label"].as_str().unwrap();
    assert!(label.contains("insert"), "label: {label}");
    assert!(label.contains("{ elem, acc }"), "label: {label}");
    let params = sigs[0]["parameters"].as_array().unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0]["label"].as_str(), Some("{ elem, acc }"));
}

#[test]
fn signature_help_for_unknown_function() {
    let src = "my_func(foo, bar, baz)";
    let offset = offset_after(src, "my_func(");
    let value = signature_help(src, offset).expect("expected signature help for unknown fn");
    let sigs = value["signatures"].as_array().unwrap();
    let label = sigs[0]["label"].as_str().unwrap();
    assert!(label.starts_with("my_func("), "label: {label}");
    // Three arguments present → three synthesized parameters.
    let params = sigs[0]["parameters"].as_array().unwrap();
    assert_eq!(params.len(), 3, "unknown fn should count 3 args");
    assert_eq!(params[0]["label"].as_str(), Some("arg1"));
    assert_eq!(params[2]["label"].as_str(), Some("arg3"));
}

#[test]
fn signature_help_nested_calls_finds_innermost() {
    let src = "comp(choice(l, r), b)";
    // Cursor inside the inner `choice(...)` call, on `r`.
    let offset = offset_of(src, "r)");
    let value = signature_help(src, offset).expect("expected signature help for innermost call");
    let sigs = value["signatures"].as_array().unwrap();
    let label = sigs[0]["label"].as_str().unwrap();
    // The innermost enclosing call is `choice`, not `comp`.
    assert!(label.starts_with("choice("), "should pick innermost call: {label}");
    // `r` is the second parameter of choice → index 1.
    assert_eq!(value["activeParameter"].as_i64(), Some(1));
}
