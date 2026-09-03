// Test names use the language's `return` keyword intentionally.
#![allow(non_snake_case)]

use crate::code_actions::code_actions;

// ---------------------------------------------------------------------------
// b. Add missing `return`
// ---------------------------------------------------------------------------

#[test]
fn code_actions_add_return_for_object_literal() {
    // The match arm `[]: { out: x }` has a bare object literal — no
    // `return` prefix.  Placing the cursor on the `{` should yield a
    // quick fix.
    let src = "match x {\n  []: { out: x }\n}";
    // The object-literal `{` is at byte offset 16.
    let actions = code_actions(src, 16, 16, &[]);
    assert!(
        actions.iter().any(|a| {
            a.get("title").and_then(|t| t.as_str()) == Some("Add missing return")
        }),
        "expected an 'Add missing return' action, got: {:?}",
        actions,
    );
}

#[test]
fn code_actions_no_return_needed_when_already_present() {
    // `return { out: x }` already has the prefix — no fix expected.
    let src = "match x {\n  []: return { out: x }\n}";
    // The object-literal `{` is at byte offset 25.
    let actions = code_actions(src, 25, 25, &[]);
    assert!(
        actions.iter().all(|a| {
            a.get("title").and_then(|t| t.as_str()) != Some("Add missing return")
        }),
        "should not offer return fix when already present, got: {:?}",
        actions,
    );
}

// ---------------------------------------------------------------------------
// c. Convert `--` to `---` doc comment
// ---------------------------------------------------------------------------

#[test]
fn code_actions_convert_comment_to_doc_comment() {
    // `-- foo` at the very top of the file should get a conversion action.
    let src = "-- foo\nx = 42";
    let actions = code_actions(src, 0, 0, &[]);
    assert!(
        actions.iter().any(|a| {
            a.get("title").and_then(|t| t.as_str())
                == Some("Convert `--` to `---` doc comment")
        }),
        "expected a comment-conversion action, got: {:?}",
        actions,
    );
}

#[test]
fn code_actions_no_comment_conversion_for_inline_comments() {
    // `-- foo` appears *after* code, so it is not at the top of the file.
    let src = "x = 42\n-- foo";
    // Cursor on the comment line (byte offset 7 = start of `-- foo`).
    let actions = code_actions(src, 7, 7, &[]);
    assert!(
        actions.iter().all(|a| {
            a.get("title").and_then(|t| t.as_str())
                != Some("Convert `--` to `---` doc comment")
        }),
        "should not convert inline comments, got: {:?}",
        actions,
    );
}

// ---------------------------------------------------------------------------
// a. Wrap in `do` block
// ---------------------------------------------------------------------------

#[test]
fn code_actions_wrap_in_do_for_binding() {
    // `x <- foo` contains a `<-` binding and is not inside a `do` block.
    let src = "  x <- foo";
    // Cursor on the binding line (byte offset 4 = the `<` of `<-`).
    let actions = code_actions(src, 4, 4, &[]);
    assert!(
        actions.iter().any(|a| {
            a.get("title").and_then(|t| t.as_str()) == Some("Wrap in `do` block")
        }),
        "expected a 'Wrap in `do` block' action, got: {:?}",
        actions,
    );
}

#[test]
fn code_actions_no_wrap_when_already_in_do() {
    // The `<-` binding is already inside `do { … }` — no wrap action.
    let src = "do {\n  x <- foo\n}";
    // Cursor on the binding line (byte offset 9 = the `<` of `<-`).
    let actions = code_actions(src, 9, 9, &[]);
    assert!(
        actions.iter().all(|a| {
            a.get("title").and_then(|t| t.as_str()) != Some("Wrap in `do` block")
        }),
        "should not wrap when already inside do, got: {:?}",
        actions,
    );
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

#[test]
fn code_actions_empty_source_returns_empty() {
    let actions = code_actions("", 0, 0, &[]);
    assert!(actions.is_empty(), "empty source should yield no actions");
}

#[test]
fn code_actions_returns_valid_json() {
    // Use a source that produces at least one action.
    let src = "-- foo\nx = 42";
    let actions = code_actions(src, 0, 0, &[]);
    assert!(
        !actions.is_empty(),
        "expected at least one action for this source"
    );
    for a in &actions {
        assert!(
            a.get("title").and_then(|t| t.as_str()).is_some(),
            "each action must have a title: {:?}",
            a,
        );
        assert_eq!(a["kind"], "quickfix", "kind must be quickfix: {:?}", a);
        let edit = a
            .get("edit")
            .expect("each action must have an edit");
        let changes = edit
            .get("changes")
            .expect("edit must have changes");
        assert!(
            changes.get("file:///untitled.hnl").is_some(),
            "changes must target the untitled URI: {:?}",
            changes,
        );
    }
}

// ---------------------------------------------------------------------------
// d. Extract to named arrow
// ---------------------------------------------------------------------------

#[test]
fn code_actions_extract_to_named_arrow_for_complex_selection() {
    // A selection containing `>>>` should get an extract action.
    let src = "x = id >>> id";
    // Select the substring `id >>> id` (offsets 4..13).
    let actions = code_actions(src, 4, 13, &[]);
    assert!(
        actions.iter().any(|a| {
            a.get("title").and_then(|t| t.as_str()) == Some("Extract to named arrow")
        }),
        "expected an 'Extract to named arrow' action, got: {:?}",
        actions,
    );
}

#[test]
fn code_actions_no_extract_for_simple_selection() {
    // A simple selection without `>>>`, `|||`, or `~>` gets no extract.
    let src = "x = 42";
    let actions = code_actions(src, 4, 6, &[]);
    assert!(
        actions.iter().all(|a| {
            a.get("title").and_then(|t| t.as_str()) != Some("Extract to named arrow")
        }),
        "should not extract simple selection, got: {:?}",
        actions,
    );
}
