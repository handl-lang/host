//! Tests for `crate::range_formatting`.
//!
//! Expectations are expressed **against the canonical formatter**
//! (`plugin-ext-fmt`) rather than hand-written strings, because the whole point
//! of this module is that the LSP agrees with `hnl fmt`: the previous version
//! had its own indenter and disagreed.

use handl_plugin_ext_fmt::format_source;
use serde_json::Value;

use crate::range_formatting::range_format;

/// Convert a (line, character) position into a byte offset within `source`.
/// Positions beyond the end of the text clamp to `source.len()`.
fn pos_to_offset(source: &str, line: usize, char: usize) -> usize {
    let mut cur_line = 0;
    let mut cur_char = 0;
    for (i, ch) in source.char_indices() {
        if cur_line == line && cur_char == char {
            return i;
        }
        if ch == '\n' {
            cur_line += 1;
            cur_char = 0;
        } else {
            cur_char += 1;
        }
    }
    source.len()
}

/// Apply `(start_line, start_char, end_line, end_char, new_text)` edits.
fn apply_edits(source: &str, mut edits: Vec<(usize, usize, usize, usize, String)>) -> String {
    edits.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut result = source.to_string();
    for (sl, sc, el, ec, nt) in edits {
        let start = pos_to_offset(&result, sl, sc);
        let end = pos_to_offset(&result, el, ec);
        result.replace_range(start..end, &nt);
    }
    result
}

/// Apply the JSON `TextEdit` objects returned by `range_format` to `source`.
fn apply_range_edits(source: &str, edits: &[Value]) -> String {
    let mut raw: Vec<(usize, usize, usize, usize, String)> = Vec::new();
    for e in edits {
        let range = &e["range"];
        let sl = range["start"]["line"].as_u64().unwrap() as usize;
        let sc = range["start"]["character"].as_u64().unwrap() as usize;
        let el = range["end"]["line"].as_u64().unwrap() as usize;
        let ec = range["end"]["character"].as_u64().unwrap() as usize;
        let nt = e["newText"].as_str().unwrap().to_string();
        raw.push((sl, sc, el, ec, nt));
    }
    apply_edits(source, raw)
}

/// The last addressable line index of `source`.
fn last_line(source: &str) -> u32 {
    source.split('\n').count().saturating_sub(1) as u32
}

/// Inputs spanning the shapes the canonical formatter may or may not rewrite:
/// arrow-dialect (which it tends to leave alone), canonical-looking source,
/// trailing whitespace, missing final newline.
fn samples() -> Vec<&'static str> {
    vec![
        "loop {\nfoo: 1\nbar: 2\n}\n",
        "do {\nx = 1   \n}\n",
        "x = 1\n",
        "x = 1",
        "# comment\nx = 1\n",
        "do {\n  do {\n    x = 1\n  }\n}\n",
    ]
}

#[test]
fn whole_document_range_matches_the_canonical_formatter() {
    for source in samples() {
        let edits = range_format(source, 0, last_line(source));
        assert_eq!(
            apply_range_edits(source, &edits),
            format_source(source).unwrap(),
            "range-formatting the whole document must equal the canonical \
             formatter (input {source:?})"
        );
    }
}

#[test]
fn already_canonical_source_yields_no_edits() {
    for source in samples() {
        let canonical = format_source(source).unwrap();
        assert!(
            range_format(&canonical, 0, last_line(&canonical)).is_empty(),
            "canonical source should need no edits: {canonical:?}"
        );
    }
}

#[test]
fn empty_source_yields_no_edits() {
    assert!(range_format("", 0, 3).is_empty());
}

#[test]
fn inverted_range_yields_no_edits() {
    assert!(range_format("x = 1\n", 3, 1).is_empty());
}

#[test]
fn range_past_the_end_yields_no_edits() {
    assert!(range_format("x = 1\n", 9, 12).is_empty());
}

#[test]
fn a_partial_range_only_touches_lines_it_can_express() {
    // Whatever the canonical formatter decides, a partial-range edit must be
    // confined to the requested lines: applying it can never change a line
    // outside `[start_line, end_line]`.
    for source in samples() {
        let canonical = format_source(source).unwrap();
        if canonical == source || canonical.split('\n').count() != source.split('\n').count() {
            continue; // nothing to project onto a partial range
        }
        let target = last_line(source).min(1);
        let edits = range_format(source, target, target);
        if edits.is_empty() {
            continue;
        }
        let before: Vec<&str> = source.split('\n').collect();
        let edited = apply_range_edits(source, &edits);
        let after: Vec<&str> = edited.split('\n').collect();
        assert_eq!(before.len(), after.len(), "line count must be preserved");
        for (i, (b, a)) in before.iter().zip(after.iter()).enumerate() {
            if i as u32 != target {
                assert_eq!(b, a, "line {i} is outside the requested range");
            }
        }
    }
}
