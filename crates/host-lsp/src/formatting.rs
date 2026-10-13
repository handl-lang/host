//! Document formatting for the `textDocument/formatting` LSP request.
//!
//! The LSP deliberately carries no formatter of its own: it delegates to the
//! canonical `plugin-ext-fmt` engine so that `textDocument/formatting` matches
//! the `hnl fmt` CLI (ADR-0085). The former hand-rolled line-based formatter
//! (and its `TextEdit` shape) was superseded by that engine and has been
//! retired — see `project/parked.yaml` and ADR-0126.

use handl_plugin_ext_fmt::format_source;
use serde_json::{Value, json};

/// Wrap the formatting edits in an LSP `textDocument/formatting` response
/// (a JSON array of `{ range, newText }` objects).
///
/// The full-document canonical formatter (`handl_plugin_ext_fmt::format_source`)
/// is used so the LSP matches the `hnl fmt` CLI.
pub fn format_document_json(source: &str) -> Value {
    match format_source(source) {
        Ok(formatted) => {
            if formatted == source {
                return json!([]);
            }
            let lines: Vec<&str> = source.split('\n').collect();
            let end_line = lines.len().saturating_sub(1);
            let end_char = lines.last().map(|s| s.chars().count()).unwrap_or(0);
            json!([{
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": end_line, "character": end_char },
                },
                "newText": formatted,
            }])
        }
        Err(_) => json!([]),
    }
}
