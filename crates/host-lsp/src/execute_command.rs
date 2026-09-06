//! `workspace/executeCommand` support for the handl LSP server.
//!
//! Exposes a small set of custom commands that VSCode can invoke through the
//! command palette / code lenses:
//!
//! * `handl.showRefinement` — return the inferred refinement contract for the
//!   given file as a markdown string.
//! * `handl.runArrow` — return a placeholder message (arrows cannot actually
//!   be executed from within the LSP).
//! * `handl.formatFile` — trigger formatting of the given file, returning the
//!   resulting `TextEdit` array.
//! * `handl.showContract` — show the inferred contract as a plain string.
//!
//! Each command returns a JSON object of the form `{ "result": ... }` on
//! success or `{ "error": "..." }` on failure (unknown command, missing
//! arguments, etc.).

// These entry points are not yet wired into the LSP dispatch loop (handlers.rs
// is intentionally untouched); suppress dead-code warnings until they are.
#![allow(dead_code)]

use serde_json::{Value, json};

use crate::formatting::format_document_json;
use crate::refinement::RefinementEngine;

/// Execute a `workspace/executeCommand` request.
///
/// `command` is the command identifier (e.g. `handl.showRefinement`) and
/// `arguments` is the JSON array of arguments supplied by the client. The
/// first argument is conventionally an object with a `uri` and/or `source`
/// field describing the target document.
pub fn execute_command(command: &str, arguments: &[Value]) -> Value {
    match command {
        "handl.showRefinement" => cmd_show_refinement(arguments),
        "handl.runArrow" => cmd_run_arrow(arguments),
        "handl.formatFile" => cmd_format_file(arguments),
        "handl.showContract" => cmd_show_contract(arguments),
        other => json!({ "error": format!("unknown command: {}", other) }),
    }
}

// ---------------------------------------------------------------------------
// Command implementations
// ---------------------------------------------------------------------------

/// `handl.showRefinement` — return the inferred refinement as markdown.
fn cmd_show_refinement(arguments: &[Value]) -> Value {
    let (source, file_path) = match extract_source(arguments) {
        Some(v) => v,
        None => return json!({ "error": "missing source/uri argument" }),
    };

    let engine = RefinementEngine::new();
    match engine.hover_markdown(&source, file_path.as_deref()) {
        Some(md) => json!({ "result": md }),
        None => json!({ "error": "no refinement available for this file" }),
    }
}

/// `handl.runArrow` — return a placeholder message.
///
/// Arrows cannot be executed from within the LSP process; this command simply
/// acknowledges the request and points the user at the runner.
fn cmd_run_arrow(arguments: &[Value]) -> Value {
    let label = arguments
        .first()
        .and_then(|a| a.get("uri"))
        .and_then(|u| u.as_str())
        .unwrap_or("the requested arrow");
    json!({
        "result": format!(
            "Cannot run {} from the language server. Use the handl runner (`handl run`) to execute arrows.",
            label
        )
    })
}

/// `handl.formatFile` — trigger formatting, returning the `TextEdit` array.
fn cmd_format_file(arguments: &[Value]) -> Value {
    let (source, _) = match extract_source(arguments) {
        Some(v) => v,
        None => return json!({ "error": "missing source/uri argument" }),
    };
    json!({ "result": format_document_json(&source) })
}

/// `handl.showContract` — show the inferred contract as a plain string.
fn cmd_show_contract(arguments: &[Value]) -> Value {
    let (source, file_path) = match extract_source(arguments) {
        Some(v) => v,
        None => return json!({ "error": "missing source/uri argument" }),
    };

    let engine = RefinementEngine::new();
    match engine.infer_document(&source, file_path.as_deref()) {
        Some(Ok(result)) => {
            let contract = format!(
                "input:  {}\noutput: {}\nguarantee: {}",
                result.input.pretty(),
                result.output.pretty(),
                result.guarantee.simplify().pretty(),
            );
            json!({ "result": contract })
        }
        Some(Err(msg)) => json!({ "error": format!("refinement failed: {}", msg) }),
        None => json!({ "error": "no contract available for this file" }),
    }
}

// ---------------------------------------------------------------------------
// Argument helpers
// ---------------------------------------------------------------------------

/// Extract `(source, file_path)` from the first command argument.
///
/// The argument may carry the source text directly (`{ "source": "..." }`)
/// or a `uri` referencing a file on disk (`{ "uri": "file:///..." }`). When
/// only a URI is supplied, the file is read from disk.
fn extract_source(arguments: &[Value]) -> Option<(String, Option<String>)> {
    let arg = arguments.first()?;
    // Direct source text takes precedence.
    if let Some(source) = arg.get("source").and_then(|s| s.as_str()) {
        let file_path = arg.get("uri").and_then(|u| u.as_str()).map(uri_to_path);
        return Some((source.to_string(), file_path));
    }
    // Otherwise read from the URI.
    let uri = arg.get("uri").and_then(|u| u.as_str())?;
    let path = uri_to_path(uri);
    let source = std::fs::read_to_string(&path).ok()?;
    Some((source, Some(path)))
}

/// Convert a `file://` URI to a filesystem path.
fn uri_to_path(uri: &str) -> String {
    uri.strip_prefix("file://").unwrap_or(uri).to_string()
}
