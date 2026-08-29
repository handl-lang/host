use serde_json::json;

use crate::document::get_document_text;

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
