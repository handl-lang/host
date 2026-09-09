//! Refinement inference integration for the LSP.
//!
//! Bridges `handl-plugin-ext-refinement` into the language server so that
//! `.hnl` arrow files get annotation-free refinement inference surfaced
//! through hover, diagnostics, and code lenses.
//!
//! The integration is *generic*: it gracefully no-ops on `.hnl` files that
//! are not arrow-graph files (the arrow parser is minimal and only handles
//! the arrow subset). When a file does parse as an arrow file, the engine:
//!
//! 1. Parses the in-memory source into `Term` IR via `parse_arrow_file`.
//! 2. Resolves cross-file `App` references by loading sibling `.hnl` arrow
//!    files from the arrows root (located from the document's file path).
//! 3. Runs `infer_refinement` with the standard contract registry.
//!
//! This mirrors the `infer_file` end-to-end path but operates on the
//! in-memory document text (which may differ from disk) rather than
//! re-reading the file.

use std::collections::HashMap;

use handl_language::Term;
use handl_plugin_ext_refinement::{
    ContractRegistry, ParsedFile, RefTy, RefinementResult, infer_refinement, load_all_arrows,
    parse_arrow_file, resolve_apps,
};

use crate::diagnostics::offset_to_position;

/// A refinement inference engine, holding the contract registry.
///
/// Constructed once at LSP startup and shared across requests.
pub struct RefinementEngine {
    registry: ContractRegistry,
}

impl RefinementEngine {
    /// Create a new engine with the standard axiom-based contract registry.
    pub fn new() -> Self {
        Self {
            registry: ContractRegistry::from_axioms(),
        }
    }

    /// Create a new engine with a custom contract registry.
    #[allow(dead_code)]
    pub fn with_registry(registry: ContractRegistry) -> Self {
        Self { registry }
    }

    /// Parse an in-memory source string as an arrow file.
    ///
    /// Returns `None` if the source is not a valid arrow file. This is
    /// expected for general `.hnl` files that are not arrow-graph files;
    /// callers should treat `None` as "not an arrow file, skip refinement".
    fn parse_source(source: &str) -> Option<ParsedFile> {
        parse_arrow_file(source).ok()
    }

    /// Extract the named-arrow entries from a parsed file.
    ///
    /// For single-expression files, the file stem (from `file_path`) is used
    /// as the key so that the file can be referenced by name from siblings.
    fn entries(parsed: &ParsedFile, file_path: Option<&str>) -> HashMap<String, Term> {
        match parsed {
            ParsedFile::Single(scheme) => {
                let name = file_path
                    .and_then(|p| {
                        std::path::Path::new(p)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "arrow".to_string());
                let mut map = HashMap::new();
                map.insert(name, scheme.clone());
                map
            }
            ParsedFile::Named(map) => map.clone(),
        }
    }

    /// The first (or single) arrow from a parsed file, for whole-file inference.
    fn first_arrow(parsed: &ParsedFile) -> Option<Term> {
        match parsed {
            ParsedFile::Single(scheme) => Some(scheme.clone()),
            ParsedFile::Named(map) => map.values().next().cloned(),
        }
    }

    /// Run refinement inference on an in-memory document.
    ///
    /// `file_path` (a filesystem path, not a URI) is used to locate the
    /// arrows root for cross-file reference resolution. If `None` or if the
    /// arrows root cannot be found, references are resolved against the
    /// document's own named arrows only.
    ///
    /// Returns `Ok(result)` if inference succeeded, `Err(message)` if the
    /// source parsed as an arrow file but inference failed, or `None` if the
    /// source is not an arrow file.
    pub fn infer_document(
        &self,
        source: &str,
        file_path: Option<&str>,
    ) -> Option<Result<RefinementResult, String>> {
        let parsed = Self::parse_source(source)?;

        let scheme = Self::first_arrow(&parsed)?;

        // Build a graph map for cross-file reference resolution.
        let mut graph_map = if let Some(root) = file_path.and_then(find_arrows_root) {
            load_all_arrows(&root).unwrap_or_default()
        } else {
            HashMap::new()
        };

        // Overlay the in-memory entries so the current document's latest
        // content takes precedence over the on-disk version.
        for (name, scheme) in Self::entries(&parsed, file_path) {
            graph_map.insert(name, scheme);
        }

        let resolved = resolve_apps(&scheme, &graph_map);
        Some(infer_refinement(&resolved, &self.registry))
    }

    /// Build a markdown hover string showing the inferred refinement contract.
    ///
    /// Returns `None` if the source is not an arrow file or inference fails.
    pub fn hover_markdown(&self, source: &str, file_path: Option<&str>) -> Option<String> {
        let result = self.infer_document(source, file_path)?.ok()?;
        let mut md = String::new();

        // Prepend doc comments from the source file (if any).
        if let Some(docs) = crate::hover::extract_doc_comments(source) {
            md.push_str(&docs);
            md.push_str("\n\n---\n\n");
        }

        md.push_str(&format_refinement_hover(&result));
        Some(md)
    }

    /// Build a markdown hover string for a specific property (field) of the
    /// rec state, showing its inferred refinement type.
    ///
    /// `property_name` is the field name (e.g. `acc`, `remaining`) and
    /// `source` is the full file source. Returns `None` if the source is not
    /// an arrow file, inference fails, or the property is not a known rec
    /// state field.
    pub fn property_hover_markdown(
        &self,
        source: &str,
        file_path: Option<&str>,
        property_name: &str,
    ) -> Option<String> {
        let result = self.infer_document(source, file_path)?.ok()?;

        // Parse the rec state declaration to get field names and their
        // positions in the product type.
        let fields = parse_rec_state_fields(source)?;

        // Find the position of the property in the state product.
        let pos = fields.iter().position(|f| f == property_name)?;

        // Extract the component at that position from the input and output
        // refinement types.
        let input_component = extract_product_component(&result.input, pos)?;
        let output_component = extract_product_component(&result.output, pos)?;

        // Check if the guarantee mentions this property.
        let guarantee_str = result.guarantee.simplify().pretty();
        let mentions_property = guarantee_str.contains(property_name)
            || guarantee_str.contains(&format!("output.{}", pos))
            || guarantee_str.contains(&format!("input.{}", pos));

        let mut md = String::new();
        md.push_str(&format!("### `{}`\n\n", property_name));
        md.push_str(&format!(
            "**Input type**  `{}`\n\n",
            input_component.pretty()
        ));
        md.push_str(&format!(
            "**Output type** `{}`\n\n",
            output_component.pretty()
        ));

        if mentions_property {
            md.push_str(&format!("**Guarantee** `{}`\n", guarantee_str));
        }

        // Show relevant loop invariants that mention this field.
        let relevant_invs: Vec<_> = result
            .loop_invariants
            .iter()
            .filter(|inv| {
                let s = inv.invariant.simplify().pretty();
                s.contains(property_name)
                    || s.contains(&format!("output.{}", pos))
                    || s.contains(&format!("input.{}", pos))
            })
            .collect();

        if !relevant_invs.is_empty() {
            md.push_str("\n---\n\n**Loop invariants**\n\n");
            for inv in relevant_invs {
                let smt = if inv.verified_by_smt { " ✓" } else { "" };
                let method = shorten_discovery_method(&inv.how_discovered);
                md.push_str(&format!(
                    "- `{}` — _{}_{}\n",
                    inv.invariant.simplify().pretty(),
                    method,
                    smt,
                ));
            }
        }

        Some(md)
    }

    /// Build code lenses showing the inferred contract summary.
    ///
    /// Returns one code lens at the top of the file with the guarantee and
    /// termination summary. Returns an empty vector if the source is not an
    /// arrow file or inference fails.
    pub fn code_lenses(&self, source: &str) -> Vec<CodeLens> {
        let result = match self.infer_document(source, None) {
            Some(Ok(r)) => r,
            _ => return Vec::new(),
        };
        let line_count = source.lines().count().max(1);
        vec![CodeLens {
            line: 0,
            character: 0,
            title: format_refinement_summary(&result),
        }]
        .into_iter()
        .take(if line_count > 0 { 1 } else { 0 })
        .collect()
    }

    /// Build LSP diagnostics from refinement inference failures.
    ///
    /// Returns diagnostics only when the source parses as an arrow file but
    /// inference fails (e.g. unknown base arrow). Non-arrow files produce no
    /// diagnostics — a parse failure of the minimal arrow parser is not an
    /// error for a general `.hnl` file.
    pub fn diagnostics(&self, source: &str, file_path: Option<&str>) -> Vec<RefinementDiagnostic> {
        match self.infer_document(source, file_path) {
            Some(Ok(_)) => Vec::new(),
            Some(Err(msg)) => {
                let (line, character) = offset_to_position(source, 0);
                vec![RefinementDiagnostic {
                    line,
                    character,
                    message: format!("refinement: {}", msg),
                }]
            }
            None => Vec::new(),
        }
    }
}

impl Default for RefinementEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// A code lens to display in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLens {
    pub line: u32,
    pub character: u32,
    pub title: String,
}

/// A refinement-sourced diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefinementDiagnostic {
    pub line: u32,
    pub character: u32,
    pub message: String,
}

/// Format a `RefinementResult` as a markdown hover string.
fn format_refinement_hover(result: &RefinementResult) -> String {
    let guarantee = result.guarantee.simplify();
    let guarantee_str = guarantee.pretty();

    let mut md = String::new();

    md.push_str("### Inferred Contract\n\n");
    md.push_str(&format!("**Input**  `{}`\n\n", result.input.pretty()));
    md.push_str(&format!("**Output** `{}`\n\n", result.output.pretty()));
    md.push_str(&format!("**Guarantee** `{}`\n", guarantee_str));

    // Loop invariants — skip if identical to the guarantee (redundant).
    let distinct_invs: Vec<_> = result
        .loop_invariants
        .iter()
        .filter(|inv| inv.invariant.simplify() != guarantee)
        .collect();

    if !distinct_invs.is_empty() {
        md.push_str("\n---\n\n**Loop invariants**\n\n");
        for inv in distinct_invs {
            let smt = if inv.verified_by_smt { " ✓" } else { "" };
            let method = shorten_discovery_method(&inv.how_discovered);
            md.push_str(&format!(
                "- `{}` — _{}_{}\n",
                inv.invariant.simplify().pretty(),
                method,
                smt,
            ));
        }
    }

    if let Some(term) = &result.termination {
        if let Some(label) = format_measure(term) {
            md.push_str(&format!("\n**Terminates** by {}\n", label));
        }
    }

    md
}

/// Shorten the "how discovered" string for display.
fn shorten_discovery_method(s: &str) -> &str {
    // "fixpoint iteration (converged at iteration 0)" → "fixpoint"
    if s.starts_with("fixpoint") {
        "fixpoint"
    } else if s.starts_with("wp") {
        "weakest precondition"
    } else {
        s
    }
}

/// Format a `Measure` for display, returning `None` when there's nothing
/// useful to show (e.g. `Measure::None` means no measure was discovered).
fn format_measure(measure: &handl_plugin_ext_refinement::types::Measure) -> Option<String> {
    use handl_plugin_ext_refinement::types::Measure;
    match measure {
        Measure::None => None,
        Measure::ListLength(v) => Some(format!("list length `|{}| ↓`", v)),
        Measure::Counter(v, d) => Some(format!("counter `{} ↓ {}`", v, d)),
        Measure::Constant => Some("constant (side-channel)".to_string()),
        Measure::Nested(inner) => Some(format!("nested ({})", format_measure(inner)?)),
    }
}

/// Format a one-line summary of a `RefinementResult` for code lenses.
pub(crate) fn format_refinement_summary(result: &RefinementResult) -> String {
    let guarantee = result.guarantee.simplify().pretty();
    let term = result
        .termination
        .as_ref()
        .and_then(|m| format_measure(m))
        .unwrap_or_else(|| "no measure".to_string());
    let inv_count = result.loop_invariants.len();
    format!(
        "guarantee: {} | terminates: {} | invariants: {}",
        guarantee, term, inv_count
    )
}

/// Find the arrows root directory from a file path.
///
/// Looks for `/handl-packages/` in the path and returns everything up to and
/// including it. Returns `None` if the path doesn.t contain `/handl-packages/`.
pub(crate) fn find_arrows_root(path: &str) -> Option<String> {
    let marker = "/handl-packages/";
    let idx = path.find(marker)?;
    Some(path[..idx + marker.len()].to_string())
}

/// Parse the rec state declaration from an arrow file to extract field
/// names in order.
///
/// For example, given:
/// ```text
/// rec {
///   remaining: input,
///   acc:       []
/// } (
/// ```
///
/// Returns `Some(vec!["remaining", "acc"])`.
fn parse_rec_state_fields(source: &str) -> Option<Vec<String>> {
    // Find `rec {` and extract fields until the closing `}`.
    let rec_start = source.find("rec")?;
    let after_rec = &source[rec_start..];

    // Find the opening brace after `rec`.
    let brace_start = after_rec.find('{')?;
    let after_brace = &after_rec[brace_start + 1..];

    // Find the closing brace.
    let brace_end = after_brace.find('}')?;
    let state_block = &after_brace[..brace_end];

    // Parse field names: each line has `name: value,` or `name: value`.
    let mut fields = Vec::new();
    for line in state_block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Field declaration: `name: value` or `name: value,`
        if let Some(colon_idx) = trimmed.find(':') {
            let name = trimmed[..colon_idx].trim();
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                fields.push(name.to_string());
            }
        }
    }

    if fields.is_empty() {
        None
    } else {
        Some(fields)
    }
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
                // Try to go deeper into fst if it's also a Prod.
                if let Some((inner, _)) = extract_component_helper(fst, 0) {
                    Some((inner, 0))
                } else {
                    Some((fst.as_ref().clone(), 0))
                }
            } else {
                // Try fst first (for nested products).
                if let Some((component, remaining)) = extract_component_helper(fst, pos) {
                    return Some((component, remaining));
                }
                // Then try snd.
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
