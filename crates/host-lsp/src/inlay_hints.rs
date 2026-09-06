//! Inlay hints for the `textDocument/inlayHint` LSP request.
//!
//! Surfaces inferred refinement types inline next to bindings in arrow files:
//! - Each rec state field declaration gets a hint with its inferred component type.
//! - The end of the `rec` line gets a hint showing the overall `In ~> Out` signature.
//! - Each `~>` operator gets a hint showing the arrow's input type.
//!
//! Non-arrow files and inference failures produce an empty result.

use serde_json::{Value, json};

use handl_plugin_ext_refinement::RefTy;

use crate::diagnostics::offset_to_position;

/// LSP InlayHint kind: Type.
const KIND_TYPE: u32 = 1;

/// Produce LSP InlayHint JSON objects for the given source.
///
/// Returns an empty vector if the source is not an arrow file or refinement
/// inference fails.
pub fn inlay_hints(
    source: &str,
    refinement: &crate::refinement::RefinementEngine,
    file_path: Option<&str>,
) -> Vec<Value> {
    let result = match refinement.infer_document(source, file_path) {
        Some(Ok(r)) => r,
        _ => return Vec::new(),
    };

    let mut hints: Vec<Value> = Vec::new();

    // Loop state field hints + loop signature hint.
    if let Some(fields) = parse_loop_state_fields_with_spans(source) {
        for (name, end_offset) in &fields {
            if let Some(component) = extract_product_component(
                &result.input,
                fields.iter().position(|(n, _)| n == name).unwrap_or(0),
            ) {
                let (line, character) = offset_to_position(source, *end_offset);
                hints.push(json!({
                    "position": { "line": line, "character": character },
                    "label": format!(": {}", component.pretty()),
                    "kind": KIND_TYPE,
                }));
            }
        }

        // Loop signature hint at the end of the `rec` line.
        if let Some(loop_line_end) = find_loop_line_end(source) {
            let (line, character) = offset_to_position(source, loop_line_end);
            hints.push(json!({
                "position": { "line": line, "character": character },
                "label": format!(": {} ~> {}", result.input.pretty(), result.output.pretty()),
                "kind": KIND_TYPE,
            }));
        }
    }

    // Arrow operator hints: show the arrow's input type at each `~>`.
    // Scan the source text for `~>` occurrences, skipping comment lines
    // (lines whose first non-whitespace content starts with `--`). This
    // avoids constructing a full `Kit` runtime just to lex the source —
    // the `~>` operator is unambiguous in arrow files and never appears
    // inside identifiers, numbers, or delimiters.
    let mut search_pos = 0;
    while let Some(rel) = source[search_pos..].find("~>") {
        let pos = search_pos + rel;
        search_pos = pos + "~>".len();

        // Skip occurrences inside comment lines.
        let line_start = source[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line_prefix = source[line_start..pos].trim_start();
        if line_prefix.starts_with("--") {
            continue;
        }

        let (line, character) = offset_to_position(source, pos + "~>".len());
        hints.push(json!({
            "position": { "line": line, "character": character },
            "label": format!(": {}", result.input.pretty()),
            "kind": KIND_TYPE,
        }));
    }

    hints
}

/// Parse the loop state declaration to extract field names and the byte offset
/// immediately after each field's declaration line (for hint placement).
///
/// For example, given:
/// ```text
/// rec {
///   remaining: input,
///   acc:       []
/// } {
/// ```
///
/// Returns `Some(vec![("remaining", <offset after line>), ("acc", <offset after line>)])`.
fn parse_loop_state_fields_with_spans(source: &str) -> Option<Vec<(String, usize)>> {
    let loop_start = source.find("rec")?;
    let after_loop = &source[loop_start..];

    let brace_start = after_loop.find('{')?;
    let after_brace = &after_loop[brace_start + 1..];

    let brace_end = after_brace.find('}')?;
    let state_block = &after_brace[..brace_end];

    // Absolute offset of the state block start in the full source.
    let block_start = loop_start + brace_start + 1;

    let mut fields: Vec<(String, usize)> = Vec::new();
    for line in state_block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(colon_idx) = trimmed.find(':') {
            let name = trimmed[..colon_idx].trim();
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                // Offset of the end of this line in the full source.
                let line_start_in_block = state_block.find(line).unwrap_or(0);
                let line_end_abs = block_start + line_start_in_block + line.len();
                fields.push((name.to_string(), line_end_abs));
            }
        }
    }

    if fields.is_empty() {
        None
    } else {
        Some(fields)
    }
}

/// Find the byte offset of the end of the line containing the `rec` keyword.
fn find_loop_line_end(source: &str) -> Option<usize> {
    let loop_start = source.find("rec")?;
    let after_loop_start = &source[loop_start..];
    let newline_rel = after_loop_start.find('\n')?;
    Some(loop_start + newline_rel)
}

/// Extract the nth component from a product type `RefTy`.
///
/// For `Prod(a, b)`, position 0 returns `a`, position 1 returns `b`.
/// For nested products like `Prod(Prod(a, b), c)`, position 0 returns `a`,
/// position 1 returns `b`, position 2 returns `c`.
/// Returns `None` if the position is out of bounds or the type is not a
/// product.
fn extract_product_component(ty: &RefTy, pos: usize) -> Option<RefTy> {
    extract_component_helper(ty, pos).map(|(component, _)| component)
}

/// Helper: returns (component_at_pos, remaining_pos) or None.
fn extract_component_helper(ty: &RefTy, pos: usize) -> Option<(RefTy, usize)> {
    match ty {
        RefTy::Prod(fst, snd) => {
            if pos == 0 {
                if let Some((inner, _)) = extract_component_helper(fst, 0) {
                    Some((inner, 0))
                } else {
                    Some((fst.as_ref().clone(), 0))
                }
            } else {
                if let Some((component, remaining)) = extract_component_helper(fst, pos) {
                    return Some((component, remaining));
                }
                extract_component_helper(snd, pos - 1)
            }
        }
        _ => {
            if pos == 0 {
                Some((ty.clone(), 0))
            } else {
                None
            }
        }
    }
}
