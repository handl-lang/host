use serde_json::json;

use crate::document::{
    DocumentStore, get_document_text, position_to_offset, resolve_document_text,
};
use handl_host_tooling::TextContentChange;

#[test]
fn extracts_text_from_file_uri() {
    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), "x = 42").unwrap();
    let uri = format!("file://{}", temp.path().display());
    let params = Some(json!({ "textDocument": { "uri": uri } }));

    let text = get_document_text(&params);
    assert_eq!(text, "x = 42");
}

#[test]
fn returns_empty_for_missing_params() {
    assert_eq!(get_document_text(&None), "");
}

#[test]
fn returns_empty_for_missing_text_document() {
    let params = Some(json!({ "other": true }));
    assert_eq!(get_document_text(&params), "");
}

#[test]
fn returns_empty_for_non_file_uri() {
    let params = Some(json!({ "textDocument": { "uri": "http://example.com/foo.hnl" } }));
    assert_eq!(get_document_text(&params), "");
}

#[test]
fn returns_empty_for_nonexistent_file() {
    let params = Some(json!({ "textDocument": { "uri": "file:///nonexistent/path.hnl" } }));
    assert_eq!(get_document_text(&params), "");
}

#[test]
fn document_store_open_and_get() {
    let mut store = DocumentStore::new();
    store.open("file:///foo.hnl", "x = 42");
    assert_eq!(store.get("file:///foo.hnl"), Some("x = 42"));
    assert_eq!(store.get("file:///bar.hnl"), None);
}

#[test]
fn document_store_close_removes_entry() {
    let mut store = DocumentStore::new();
    store.open("file:///foo.hnl", "x = 42");
    assert_eq!(store.get("file:///foo.hnl"), Some("x = 42"));
    store.close("file:///foo.hnl");
    assert_eq!(store.get("file:///foo.hnl"), None);
}

#[test]
fn document_store_update_full_replaces_text() {
    let mut store = DocumentStore::new();
    store.open("file:///foo.hnl", "x = 42");
    store.update_full("file:///foo.hnl", "y = 99");
    assert_eq!(store.get("file:///foo.hnl"), Some("y = 99"));
}

#[test]
fn document_store_apply_incremental_inserts() {
    let mut store = DocumentStore::new();
    store.open("file:///foo.hnl", "x = 42");
    let result = store.apply_incremental("file:///foo.hnl", TextContentChange::insert(0, "let "));
    assert_eq!(result, Some("let x = 42"));
    assert_eq!(store.get("file:///foo.hnl"), Some("let x = 42"));
}

#[test]
fn document_store_apply_incremental_replaces_range() {
    let mut store = DocumentStore::new();
    store.open("file:///foo.hnl", "x = 42");
    let result = store.apply_incremental("file:///foo.hnl", TextContentChange::replace(0, 1, "y"));
    assert_eq!(result, Some("y = 42"));
}

#[test]
fn document_store_apply_incremental_unknown_uri_returns_none() {
    let mut store = DocumentStore::new();
    let result = store.apply_incremental("file:///missing.hnl", TextContentChange::insert(0, "x"));
    assert_eq!(result, None);
}

#[test]
fn resolve_document_text_prefers_store_over_disk() {
    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), "disk content").unwrap();
    let uri = format!("file://{}", temp.path().display());
    let params = Some(json!({ "textDocument": { "uri": uri.clone() } }));

    let mut store = DocumentStore::new();
    store.open(&uri, "store content");
    let text = resolve_document_text(&store, &params);
    assert_eq!(text, "store content");
}

#[test]
fn resolve_document_text_falls_back_to_disk() {
    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), "disk content").unwrap();
    let uri = format!("file://{}", temp.path().display());
    let params = Some(json!({ "textDocument": { "uri": uri } }));

    let store = DocumentStore::new();
    let text = resolve_document_text(&store, &params);
    assert_eq!(text, "disk content");
}

#[test]
fn resolve_document_text_empty_for_missing_params() {
    let store = DocumentStore::new();
    assert_eq!(resolve_document_text(&store, &None), "");
}

#[test]
fn position_to_offset_single_line() {
    assert_eq!(position_to_offset("hello", 0, 0), 0);
    assert_eq!(position_to_offset("hello", 0, 3), 3);
    assert_eq!(position_to_offset("hello", 0, 5), 5);
    assert_eq!(position_to_offset("hello", 0, 99), 5);
}

#[test]
fn position_to_offset_multiline() {
    let text = "abc\ndef\nghi";
    assert_eq!(position_to_offset(text, 0, 1), 1);
    assert_eq!(position_to_offset(text, 1, 0), 4);
    assert_eq!(position_to_offset(text, 1, 2), 6);
    assert_eq!(position_to_offset(text, 2, 0), 8);
    assert_eq!(position_to_offset(text, 5, 0), text.len());
}

#[test]
fn position_to_offset_unicode() {
    let text = "héllo";
    assert_eq!(position_to_offset(text, 0, 0), 0);
    assert_eq!(position_to_offset(text, 0, 1), 1);
    assert_eq!(position_to_offset(text, 0, 2), 3);
}
