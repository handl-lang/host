//! Post-processing stage (ADR-0095, stage 4).
//!
//! The post-process stage turns laid-out text into canonical file text:
//! trailing whitespace removal, blank-line collapsing, blank lines between
//! top-level declaration groups, and a single trailing newline. Each concern
//! is a separate [`PostProcessRule`]; [`canonical_post_process`] composes the
//! canonical v0.1 set (ADR-0086) in the required order.
//!
//! Like the layout stage, post-processing is a seam: a host or kit can swap,
//! drop, or insert rules for a different dialect without touching the render
//! stage ([`crate::token::TokenFormatter`]).

/// A single, composable post-processing rule applied to laid-out text.
///
/// Rules run sequentially from high-level text to canonical text and must be
/// idempotent-friendly: re-running the canonical pipeline over its own output
/// is a fixpoint (`fmt(fmt(src)) == fmt(src)`, ADR-0086).
pub trait PostProcessRule {
    /// Short name for diagnostics / rule selection.
    fn name(&self) -> &str;

    /// Apply this rule to the current text and return the transformed text.
    fn apply(&self, text: &str) -> String;
}

/// Remove trailing whitespace from every line.
#[derive(Debug, Clone, Copy)]
pub struct TrailingWhitespaceRule;

impl PostProcessRule for TrailingWhitespaceRule {
    fn name(&self) -> &str {
        "trailing-whitespace"
    }

    fn apply(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        let lines: Vec<String> = text.lines().map(|l| l.trim_end().to_string()).collect();
        lines.join("\n")
    }
}

/// Collapse two or more consecutive blank lines into one.
#[derive(Debug, Clone, Copy)]
pub struct CollapseBlankLinesRule;

impl PostProcessRule for CollapseBlankLinesRule {
    fn name(&self) -> &str {
        "collapse-blank-lines"
    }

    fn apply(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
        let mut collapsed: Vec<String> = Vec::with_capacity(lines.len());
        let mut prev_blank = false;
        for line in &lines {
            let is_blank = line.trim().is_empty();
            if is_blank && prev_blank {
                continue;
            }
            collapsed.push(line.clone());
            prev_blank = is_blank;
        }
        collapsed.join("\n")
    }
}

/// Insert a blank line between top-level declaration groups (imports,
/// exports, bindings) but not between consecutive imports/exports.
///
/// Expects collapsed input (see [`CollapseBlankLinesRule`]).
#[derive(Debug, Clone, Copy)]
pub struct BlankLineBetweenDeclGroupsRule;

impl PostProcessRule for BlankLineBetweenDeclGroupsRule {
    fn name(&self) -> &str {
        "blank-line-between-decl-groups"
    }

    fn apply(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
        let mut with_blanks: Vec<String> = Vec::with_capacity(lines.len());
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                let curr_type = decl_type(line);
                if let Some(prev_idx) = (0..i).rev().find(|&j| !lines[j].trim().is_empty()) {
                    let prev_type = decl_type(&lines[prev_idx]);
                    // Insert a blank line when transitioning between groups:
                    // - binding → binding
                    // - import/export → binding
                    // - binding → import/export
                    // But NOT between consecutive imports or consecutive exports.
                    if let (Some(pt), Some(ct)) = (prev_type, curr_type) {
                        let needs_blank = match (pt, ct) {
                            (DeclType::Import, DeclType::Import) => false,
                            (DeclType::Export, DeclType::Export) => false,
                            (DeclType::Import, DeclType::Export)
                            | (DeclType::Export, DeclType::Import) => false,
                            (DeclType::Comment, _) | (_, DeclType::Comment) => false,
                            _ => true,
                        };
                        if needs_blank && with_blanks.last().map_or(true, |l| !l.trim().is_empty())
                        {
                            with_blanks.push(String::new());
                        }
                    }
                }
            }
            with_blanks.push(line.clone());
        }
        with_blanks.join("\n")
    }
}

/// Ensure exactly one trailing newline, and none for empty text.
#[derive(Debug, Clone, Copy)]
pub struct SingleTrailingNewlineRule;

impl PostProcessRule for SingleTrailingNewlineRule {
    fn name(&self) -> &str {
        "single-trailing-newline"
    }

    fn apply(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        let mut result = text.to_string();
        while result.ends_with('\n') {
            result.pop();
        }
        if !result.is_empty() {
            result.push('\n');
        }
        result
    }
}

/// Apply a sequence of post-processing rules in order.
pub fn post_process_pipeline(text: &str, rules: &[&dyn PostProcessRule]) -> String {
    let mut out = text.to_string();
    for rule in rules {
        out = rule.apply(&out);
    }
    out
}

/// Apply the canonical v0.1 post-processing rule set (ADR-0086).
pub fn canonical_post_process(text: &str) -> String {
    post_process_pipeline(
        text,
        &[
            &TrailingWhitespaceRule,
            &CollapseBlankLinesRule,
            &BlankLineBetweenDeclGroupsRule,
            &SingleTrailingNewlineRule,
        ],
    )
}

// ---------------------------------------------------------------------------
// Internal: top-level declaration classification
// ---------------------------------------------------------------------------

/// Top-level declaration type for blank-line decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclType {
    Import,
    Export,
    Binding,
    Comment,
}

/// Classify a line's declaration type. Returns `None` for non-declaration
/// lines (e.g. continuation lines, block content).
fn decl_type(line: &str) -> Option<DeclType> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("--") {
        return Some(DeclType::Comment);
    }
    if trimmed.starts_with("import")
        && (trimmed.len() == 6 || !trimmed[6..].chars().next().unwrap().is_alphanumeric())
    {
        return Some(DeclType::Import);
    }
    if trimmed.starts_with("export")
        && (trimmed.len() == 6 || !trimmed[6..].chars().next().unwrap().is_alphanumeric())
    {
        return Some(DeclType::Export);
    }
    // Binding: `identifier ::` at the start of the line (indent 0).
    if !line.starts_with(' ') && !line.starts_with('\t') {
        if let Some(bind_pos) = trimmed.find("::") {
            let name = trimmed[..bind_pos].trim();
            // The name must be a simple identifier (all word chars).
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Some(DeclType::Binding);
            }
        }
    }
    None
}