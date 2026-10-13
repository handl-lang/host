//! End-to-end smoke tests for the LSP request handlers.
//!
//! `project/adrs/archived/ADR-LSP.md` (Accepted) requires LSP features to move
//! behind the `ToolingHost` plugin seam, leaving `handl-host-lsp` a thin
//! dispatcher. Today only 7 of 26 handler modules actually go through that seam,
//! and moving a feature without an end-to-end check is how a request regresses
//! silently — the handler glue had **no** coverage at all, so there was nothing
//! to catch it.
//!
//! These are deliberately *smoke* tests: drive the real handler with a real
//! document on disk and assert the reply is a well-formed LSP response carrying
//! the request id. They exist to notice a feature breaking as it moves, not to
//! pin feature semantics.

use serde_json::{Value, json};

use handl_host_tooling::ToolingHost;

use crate::document::DocumentStore;
use crate::handlers::{
    handle_call_hierarchy_incoming, handle_call_hierarchy_outgoing, handle_code_actions,
    handle_code_lens, handle_color_presentation, handle_document_color, handle_document_highlight,
    handle_document_link, handle_execute_command, handle_inlay_hint_resolve, handle_inlay_hints,
    handle_linked_editing, handle_moniker, handle_prepare_call_hierarchy, handle_prepare_rename,
    handle_prepare_type_hierarchy, handle_references, handle_rename, handle_selection_ranges,
    handle_signature_help, handle_type_hierarchy_subtypes, handle_type_hierarchy_supertypes,
    handle_workspace_symbols,
};
use crate::protocol::Message;
use crate::refinement::RefinementEngine;

/// A small canonical document on disk, plus the params a position-based request
/// needs. Extra fields are ignored by handlers that do not want them.
fn fixture() -> (tempfile::NamedTempFile, Value) {
    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), "do {\n  x = 1\n}\n").unwrap();
    let uri = format!("file://{}", temp.path().display());
    let params = json!({
        "textDocument": { "uri": uri },
        "position": { "line": 1, "character": 2 },
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 2, "character": 1 }
        },
        "query": "",
        "command": "handl.formatFile",
        "arguments": [],
    });
    (temp, params)
}

fn extract_body(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let header_end = s.find("\r\n\r\n").expect("LSP header terminator") + 4;
    s[header_end..].to_string()
}

fn msg(method: &str, params: &Value) -> Message {
    Message {
        method: method.to_string(),
        params: Some(params.clone()),
        id: Some(json!(7)),
    }
}

/// Assert the handler wrote a well-formed LSP reply for request id 7.
fn assert_reply(raw: &[u8]) -> Value {
    let parsed: Value = serde_json::from_str(&extract_body(raw)).expect("valid JSON reply");
    assert_eq!(parsed["id"], 7, "reply carries the request id: {parsed}");
    assert!(
        parsed.get("result").is_some() || parsed.get("error").is_some(),
        "reply has a result or an error: {parsed}"
    );
    parsed
}

// --- handlers taking (writer, id, msg, store) ------------------------------

#[test]
fn document_highlight_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/documentHighlight", &params);
    handle_document_highlight(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

#[test]
fn moniker_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/moniker", &params);
    handle_moniker(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

#[test]
fn selection_ranges_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/selectionRange", &params);
    handle_selection_ranges(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

#[test]
fn document_link_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/documentLink", &params);
    handle_document_link(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

#[test]
fn references_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/references", &params);
    handle_references(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

#[test]
fn document_color_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/documentColor", &params);
    handle_document_color(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

#[test]
fn linked_editing_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/linkedEditingRange", &params);
    handle_linked_editing(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

#[test]
fn code_actions_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/codeAction", &params);
    handle_code_actions(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

#[test]
fn prepare_rename_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/prepareRename", &params);
    handle_prepare_rename(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

#[test]
fn rename_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/rename", &params);
    handle_rename(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

#[test]
fn prepare_call_hierarchy_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/prepareCallHierarchy", &params);
    handle_prepare_call_hierarchy(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

#[test]
fn prepare_type_hierarchy_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/prepareTypeHierarchy", &params);
    handle_prepare_type_hierarchy(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

#[test]
fn type_hierarchy_supertypes_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("typeHierarchy/supertypes", &params);
    handle_type_hierarchy_supertypes(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

#[test]
fn type_hierarchy_subtypes_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("typeHierarchy/subtypes", &params);
    handle_type_hierarchy_subtypes(&mut buf, json!(7), &m, &DocumentStore::new());
    assert_reply(&buf);
}

// --- handlers taking (writer, id, msg) -------------------------------------

#[test]
fn call_hierarchy_incoming_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("callHierarchy/incomingCalls", &params);
    handle_call_hierarchy_incoming(&mut buf, json!(7), &m);
    assert_reply(&buf);
}

#[test]
fn call_hierarchy_outgoing_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("callHierarchy/outgoingCalls", &params);
    handle_call_hierarchy_outgoing(&mut buf, json!(7), &m);
    assert_reply(&buf);
}

#[test]
fn color_presentation_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/colorPresentation", &params);
    handle_color_presentation(&mut buf, json!(7), &m);
    assert_reply(&buf);
}

#[test]
fn execute_command_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("workspace/executeCommand", &params);
    handle_execute_command(&mut buf, json!(7), &m);
    assert_reply(&buf);
}

#[test]
fn inlay_hint_resolve_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("inlayHint/resolve", &params);
    handle_inlay_hint_resolve(&mut buf, json!(7), &m);
    assert_reply(&buf);
}

// --- handlers taking an engine / host as well ------------------------------

#[test]
fn code_lens_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/codeLens", &params);
    handle_code_lens(
        &mut buf,
        json!(7),
        &m,
        &DocumentStore::new(),
        &RefinementEngine::new(),
    );
    assert_reply(&buf);
}

#[test]
fn inlay_hints_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/inlayHint", &params);
    handle_inlay_hints(
        &mut buf,
        json!(7),
        &m,
        &DocumentStore::new(),
        &RefinementEngine::new(),
    );
    assert_reply(&buf);
}

#[test]
fn signature_help_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("textDocument/signatureHelp", &params);
    handle_signature_help(&mut buf, json!(7), &m, &DocumentStore::new(), &ToolingHost::new());
    assert_reply(&buf);
}

#[test]
fn workspace_symbols_responds() {
    let (_f, params) = fixture();
    let mut buf = Vec::new();
    let m = msg("workspace/symbol", &params);
    handle_workspace_symbols(&mut buf, json!(7), &m, &ToolingHost::new());
    assert_reply(&buf);
}
