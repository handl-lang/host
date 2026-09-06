//! `workspace/didChangeWatchedFiles` support for the handl LSP server.
//!
//! When files on disk change (arrows, contracts, etc.) the LSP client sends a
//! `workspace/didChangeWatchedFiles` notification listing the affected URIs.
//! The server must then re-run diagnostics for any *open* document that
//! references one of the changed files — e.g. an arrow that `App`s a changed
//! sibling arrow, or a file that imports a changed contract.
//!
//! This module provides:
//!
//! * [`watched_files_changed`] — given the changed URIs and the set of open
//!   documents (with their source text), returns the URIs of open documents
//!   that need re-diagnosticing because they reference a changed file.
//! * [`register_watched_files`] — builds the
//!   `DidChangeWatchedFilesRegistrationOptions` JSON to send during
//!   `initialize` so the client registers glob watchers for `**/*.hnl`.

// These entry points are not yet wired into the LSP dispatch loop (handlers.rs
// is intentionally untouched); suppress dead-code warnings until they are.
#![allow(dead_code)]

use std::collections::HashSet;

use serde_json::{Value, json};

use crate::kit_resolver::kit_for_file;

/// An open document: its URI and current in-memory source text.
pub struct OpenDocument<'a> {
    pub uri: &'a str,
    pub source: &'a str,
}

/// Given a list of changed file URIs, return the URIs of open documents that
/// need re-diagnosticing (those that reference the changed files).
///
/// A document "references" a changed file when the bare file stem of a
/// changed `.hnl` file appears as an identifier token in the document's
/// source — this is how arrows refer to sibling arrows (by stem) and how
/// contracts are named. The document's own URI is excluded: a file changing
/// itself is already handled by `didChange`/`didSave`.
pub fn watched_files_changed(uris: &[String]) -> Vec<String> {
    // This overload (without open documents) returns the changed URIs
    // themselves minus any non-`.hnl` files, so callers that simply want the
    // set of affected files get a sensible answer. The full reference-based
    // filtering is performed by [`watched_files_changed_for`].
    uris.iter()
        .filter(|uri| uri.ends_with(".hnl"))
        .cloned()
        .collect()
}

/// Given a list of changed file URIs and the currently open documents,
/// return the URIs of open documents that reference any of the changed files
/// and therefore need re-diagnosticing.
pub fn watched_files_changed_for<'a>(
    uris: &[String],
    open_documents: &[OpenDocument<'a>],
) -> Vec<String> {
    // Collect the bare stems of the changed `.hnl` files.
    let changed_stems: HashSet<String> = uris
        .iter()
        .filter_map(|uri| file_stem_from_uri(uri))
        .collect();

    if changed_stems.is_empty() {
        return Vec::new();
    }

    let mut affected: Vec<String> = Vec::new();
    for doc in open_documents {
        // A document never references itself via this path — its own changes
        // are handled by didChange/didSave.
        let own_stem = file_stem_from_uri(doc.uri);
        let references = references_any_stem(doc.source, &changed_stems, own_stem.as_deref());
        if references {
            affected.push(doc.uri.to_string());
        }
    }
    affected
}

/// Build `DidChangeWatchedFilesRegistrationOptions` JSON registering glob
/// watchers for every `**/*.hnl` file under `arrows_root`.
///
/// The returned object is suitable for sending as the `registerOptions` of a
/// `client/registerCapability` request for the `workspace/didChangeWatchedFiles`
/// capability.
pub fn register_watched_files(arrows_root: &str) -> Value {
    // Normalise to a trailing-slash-free base, then build a glob pattern.
    let base = arrows_root.trim_end_matches('/');
    let pattern = if base.is_empty() {
        "**/*.hnl".to_string()
    } else {
        format!("{}/**/*.hnl", base)
    };

    json!({
        "watchers": [
            { "globPattern": pattern }
        ]
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the bare file stem (without extension) from a `file://` URI or a
/// plain path. Returns `None` for non-`.hnl` files.
fn file_stem_from_uri(uri: &str) -> Option<String> {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    if !path.ends_with(".hnl") {
        return None;
    }
    let normalised = path.replace('\\', "/");
    let filename = normalised.rsplit('/').next()?;
    let stem = filename.strip_suffix(".hnl")?;
    Some(stem.to_string())
}

/// Whether `source` contains an identifier token matching any of `stems`,
/// excluding the document's own stem (so a file doesn't flag itself).
fn references_any_stem(source: &str, stems: &HashSet<String>, own_stem: Option<&str>) -> bool {
    let kit = match kit_for_file(None) {
        Ok(k) => k,
        Err(_) => {
            // Fall back to a substring scan if lexing fails.
            return stems
                .iter()
                .any(|s| Some(s.as_str()) != own_stem && source.contains(s));
        }
    };
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => {
            return stems
                .iter()
                .any(|s| Some(s.as_str()) != own_stem && source.contains(s));
        }
    };

    for t in &tokens {
        if handl_kit_canonical::is_identifier_kind(&t.kind)
            && stems.contains(&t.text)
            && Some(t.text.as_str()) != own_stem
        {
            return true;
        }
    }
    false
}
