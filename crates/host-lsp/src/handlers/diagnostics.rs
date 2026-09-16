use std::io::Write;

use crate::diagnostics::parse_diagnostics;
use crate::protocol::send_notification;
use crate::refinement::RefinementEngine;
use serde_json::json;
pub fn handle_publish_diagnostics(
    writer: &mut impl Write,
    uri: &str,
    source: &str,
    refinement: &RefinementEngine,
) {
    let mut diagnostics = Vec::new();

    // Append parse diagnostics (real syntax errors caught by the lexer and
    // the arrow-file parser) with severity 1 (Error). These surface as the
    // user types, before refinement inference runs.
    for diag in parse_diagnostics(source) {
        diagnostics.push(json!({
            "range": {
                "start": { "line": diag.line, "character": diag.character },
                "end": { "line": diag.line, "character": diag.character + 1 },
            },
            "severity": 1,
            "source": "handl-parse",
            "message": diag.message,
        }));
    }

    // Append refinement diagnostics for arrow files that fail inference.
    // Non-arrow files produce no refinement diagnostics. Refinement
    // diagnostics keep their existing severity (Warning).
    let file_path = uri.strip_prefix("file://").map(|s| s.to_string());
    for diag in refinement.diagnostics(source, file_path.as_deref()) {
        diagnostics.push(json!({
            "range": {
                "start": { "line": diag.line, "character": diag.character },
                "end": { "line": diag.line, "character": diag.character + 1 },
            },
            "severity": 2,
            "source": "handl-refinement",
            "message": diag.message,
        }));
    }

    let params = json!({ "uri": uri, "diagnostics": diagnostics });
    send_notification(writer, "textDocument/publishDiagnostics", params);
}
