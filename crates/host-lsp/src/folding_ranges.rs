//! `textDocument/foldingRange` support for the handl LSP server.
//!
//! This module is now a thin adapter over the data-driven `plugin-lsp` engine
//! wired through `handl-plugin-lsp-std::BasicLsp`.  The old token-walking
//! implementation has been moved into `plugin-lsp::providers::run_folding_range`
//! so the same rules can be driven from `lsp.yaml`.

use handl_host_tooling::lsp::{FoldingRangePlugin, FoldingRangeRequest};
use handl_plugin_lsp_std::BasicLsp;
use serde_json::{Value, json};
use std::sync::OnceLock;

static BASIC_LSP: OnceLock<BasicLsp> = OnceLock::new();

/// Lex `source` and return a vector of LSP FoldingRange JSON objects.
///
/// Returns an empty vector if the source cannot be lexed or the canonical
/// language cannot be assembled.
pub fn folding_ranges(source: &str) -> Vec<Value> {
    let lsp = BASIC_LSP.get_or_init(BasicLsp::new);
    match lsp.run(&FoldingRangeRequest {
        source: source.to_string(),
    }) {
        Ok(resp) => resp
            .0
            .into_iter()
            .map(|r| {
                json!({
                    "startLine": r.start_line,
                    "endLine": r.end_line,
                    "startCharacter": r.start_char,
                    "endCharacter": r.end_char,
                    "kind": r.kind,
                })
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}
