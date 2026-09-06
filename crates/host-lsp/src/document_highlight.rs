use serde_json::{Value, json};

use handl_kit_canonical::{SpannedToken, is_comment_kind, is_identifier_kind};

use crate::diagnostics::offset_to_position;
use crate::kit_resolver::kit_for_file;
use crate::symbols::{find_references, identifier_at_offset};

/// LSP DocumentHighlightKind values.
const KIND_READ: u64 = 1;
const KIND_WRITE: u64 = 2;

/// Compute `textDocument/documentHighlight` results for the identifier at
/// `offset` in `source`.
///
/// Returns `None` when the cursor is not on an identifier, otherwise a list of
/// LSP DocumentHighlight JSON objects (one per occurrence of the identifier in
/// the file). Occurrences that are declarations — an identifier immediately
/// followed by `=` or `:` — are marked as Write (kind 2); all others are Read
/// (kind 1).
pub fn document_highlight(source: &str, offset: usize) -> Option<Vec<Value>> {
    // Resolve the identifier under the cursor.
    let (name, _) = identifier_at_offset(source, offset)?;

    // Find every occurrence of that identifier in the file.
    let references = find_references(source, &name);

    // Build a set of declaration spans so we can classify each reference as a
    // read or a write. A declaration is an identifier followed (ignoring
    // comments) by `=` or `:`.
    let write_spans = declaration_spans(source, &name);

    let mut highlights = Vec::with_capacity(references.len());
    for span in references {
        let (start_line, start_char) = offset_to_position(source, span.start);
        let (end_line, end_char) = offset_to_position(source, span.end);
        let kind = if write_spans.contains(&(span.start, span.end)) {
            KIND_WRITE
        } else {
            KIND_READ
        };
        highlights.push(json!({
            "range": {
                "start": { "line": start_line, "character": start_char },
                "end": { "line": end_line, "character": end_char },
            },
            "kind": kind,
        }));
    }

    Some(highlights)
}

/// Return the spans of identifier occurrences that are declarations: an
/// identifier matching `name` whose next non-comment token is `=` or `:`.
fn declaration_spans(source: &str, name: &str) -> std::collections::HashSet<(usize, usize)> {
    let mut spans = std::collections::HashSet::new();
    let kit = match kit_for_file(None) {
        Ok(k) => k,
        Err(_) => return spans,
    };
    let tokens = match kit.lex_with_spans(source) {
        Ok(t) => t,
        Err(_) => return spans,
    };
    let mut i = 0;
    while i < tokens.len() {
        if is_identifier_kind(&tokens[i].kind) && tokens[i].text == name {
            // Find the next non-comment token.
            let mut j = i + 1;
            while j < tokens.len() && is_comment_kind(&tokens[j].kind) {
                j += 1;
            }
            if j < tokens.len() && (&tokens[j].text == "::" || &tokens[j].text == ":") {
                spans.insert((tokens[i].span.start, tokens[i].span.end));
            }
        }
        i += 1;
    }
    spans
}

/// Lex the source into spanned tokens, returning `None` on failure.
#[allow(dead_code)]
fn lex(source: &str) -> Option<Vec<SpannedToken>> {
    let kit = kit_for_file(None).ok()?;
    kit.lex_with_spans(source).ok()
}
