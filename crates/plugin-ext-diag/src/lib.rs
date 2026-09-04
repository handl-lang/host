//! Shared diagnostics infrastructure for handl tooling.
//!
//! Implements ADR-0091: the single home for `Diagnostic`, `Severity`, `Span`,
//! and `Fix` types, plus a shared human/JSON renderer and span-ordered,
//! idempotent fix application with a re-parse fixpoint policy (ADR-0087).
//!
//! The crate is generic — it knows spans and text, not the v0.1 grammar. It
//! serves every handl language. No hardcoded node-kind strings live here.

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use handl_platform::Span as PlatformSpan;

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// A hard error; the program is invalid or analysis cannot proceed.
    #[default]
    Error,
    /// A warning; the program is valid but suspicious.
    Warning,
    /// Informational; no action required.
    Info,
}

impl Severity {
    /// Lowercase render label (`error`, `warning`, `info`).
    pub fn as_label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_label())
    }
}

/// A source span with byte offsets and line/column positions.
///
/// Line/col are 1-based; `start`/`end` are byte offsets (inclusive/exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Span {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
    /// 1-based start line.
    pub start_line: u32,
    /// 1-based start column (byte offset within the line).
    pub start_col: u32,
    /// 1-based end line.
    pub end_line: u32,
    /// 1-based end column (byte offset within the line).
    pub end_col: u32,
}

impl Span {
    /// Construct a span from a byte range, computing line/col from `source`.
    pub fn from_byte_range(start: usize, end: usize, source: &str) -> Span {
        let (start_line, start_col) = compute_line_col(source, start);
        let (end_line, end_col) = compute_line_col(source, end);
        Span {
            start,
            end,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Construct a span from a `handl_platform::Span` (byte offsets only),
    /// computing line/col from `source`.
    pub fn from_platform_span(span: PlatformSpan, source: &str) -> Span {
        Self::from_byte_range(span.start, span.end, source)
    }

    /// The byte length of the span.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether the span is empty (`start == end`).
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Compute the 1-based (line, column) for a byte offset in `source`.
///
/// Column is a byte offset within the line (1-based). An offset at a newline
/// character reports the line it terminates. An offset past the end clamps to
/// the last line / one-past-end column.
pub fn compute_line_col(source: &str, byte_offset: usize) -> (u32, u32) {
    let bytes = source.as_bytes();
    let off = byte_offset.min(bytes.len());
    let mut line: u32 = 1;
    let mut col: u32 = 1;
    for &b in &bytes[..off] {
        if b == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// A text edit fix for a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Fix {
    /// The range to replace.
    pub span: Span,
    /// The text to insert in place of `span`.
    pub replacement: String,
}

/// A diagnostic with rule name, severity, span, message, and optional fix.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Diagnostic {
    /// The rule name, e.g. `"unused-import"`.
    pub rule: &'static str,
    /// Severity of the diagnostic.
    pub severity: Severity,
    /// Source span the diagnostic refers to.
    pub span: Span,
    /// Human-readable message.
    pub message: String,
    /// Optional automatic fix.
    pub fix: Option<Fix>,
}

impl Diagnostic {
    /// Create a new diagnostic with no fix.
    pub fn new(rule: &'static str, severity: Severity, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            rule,
            severity,
            span,
            message: message.into(),
            fix: None,
        }
    }

    /// Attach a fix to the diagnostic.
    pub fn with_fix(mut self, fix: Fix) -> Self {
        self.fix = Some(fix);
        self
    }
}

/// Errors from fix application.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FixError {
    /// Two fixes have overlapping spans.
    #[error("overlapping fixes: {first_start}..{first_end} overlaps {second_start}..{second_end}")]
    Overlapping {
        first_start: usize,
        first_end: usize,
        second_start: usize,
        second_end: usize,
    },
    /// A fix span is out of bounds for the source.
    #[error("fix span {start}..{end} out of bounds for source of length {len}")]
    OutOfBounds { start: usize, end: usize, len: usize },
    /// The fixpoint loop did not converge within the iteration budget.
    #[error("fixpoint did not converge within {max_iterations} iterations")]
    NotConverged { max_iterations: usize },
}

/// Render a single diagnostic in human format:
/// `file:line:col: severity [rule]: message`.
pub fn render_diagnostic(diag: &Diagnostic, file_path: &str) -> String {
    format!(
        "{}:{}:{}: {} [{}]: {}",
        file_path,
        diag.span.start_line,
        diag.span.start_col,
        diag.severity,
        diag.rule,
        diag.message,
    )
}

/// Render all diagnostics, newline-separated, in human format.
pub fn render_diagnostics(diags: &[Diagnostic], file_path: &str) -> String {
    diags
        .iter()
        .map(|d| render_diagnostic(d, file_path))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render diagnostics as a JSON array for CI consumers.
pub fn render_json(diags: &[Diagnostic]) -> String {
    serde_json::to_string(diags).expect("Diagnostic is always serializable")
}

/// Apply fixes to `source` in span order (descending, so byte offsets don't
/// shift), idempotently. Returns an error on overlapping or out-of-bounds
/// fixes.
pub fn apply_fixes(source: &str, fixes: &[Fix]) -> Result<String, FixError> {
    if fixes.is_empty() {
        return Ok(source.to_string());
    }

    // Validate bounds and detect overlaps. Sort a copy by start ascending.
    let mut ordered: Vec<&Fix> = fixes.iter().collect();
    ordered.sort_by_key(|f| f.span.start);

    let len = source.len();
    for w in ordered.windows(2) {
        let a = &w[0].span;
        let b = &w[1].span;
        // Overlap unless a ends at or before b starts.
        if a.end > b.start {
            return Err(FixError::Overlapping {
                first_start: a.start,
                first_end: a.end,
                second_start: b.start,
                second_end: b.end,
            });
        }
    }
    for f in &ordered {
        if f.span.start > f.span.end || f.span.end > len {
            return Err(FixError::OutOfBounds {
                start: f.span.start,
                end: f.span.end,
                len,
            });
        }
    }

    // Apply in descending start order so earlier offsets stay valid.
    let mut result = source.to_string();
    for f in ordered.iter().rev() {
        result.replace_range(f.span.start..f.span.end, &f.replacement);
    }
    Ok(result)
}

/// Apply fixes, re-parse (via `compute_fixes`), and repeat until a fixpoint is
/// reached (no new fixes) or `max_iterations` is exceeded.
///
/// Idempotent: if `compute_fixes` returns no fixes on the first call, the
/// source is returned unchanged immediately.
pub fn apply_fixes_to_fixpoint(
    source: &str,
    compute_fixes: impl Fn(&str) -> Vec<Fix>,
    max_iterations: usize,
) -> Result<String, FixError> {
    let mut current = source.to_string();
    for _ in 0..max_iterations {
        let fixes = compute_fixes(&current);
        if fixes.is_empty() {
            return Ok(current);
        }
        current = apply_fixes(&current, &fixes)?;
    }
    // One final check: did it converge?
    if compute_fixes(&current).is_empty() {
        return Ok(current);
    }
    Err(FixError::NotConverged { max_iterations })
}

#[cfg(feature = "lang-span")]
mod lang_span {
    use super::{compute_line_col, Span};

    impl Span {
        /// Construct a span from a `handl_language::Span` (byte offsets only),
        /// computing line/col from `source`.
        pub fn from_lang_span(span: handl_language::Span, source: &str) -> Span {
            let (start_line, start_col) = compute_line_col(source, span.start);
            let (end_line, end_col) = compute_line_col(source, span.end);
            Span {
                start: span.start,
                end: span.end,
                start_line,
                start_col,
                end_line,
                end_col,
            }
        }
    }

    impl From<handl_language::Span> for Span {
        /// Convert a language span into a diagnostic span with unknown line/col
        /// (defaults to 1:1). Use [`Span::from_lang_span`] when source text is
        /// available to compute accurate positions.
        fn from(span: handl_language::Span) -> Self {
            Span {
                start: span.start,
                end: span.end,
                start_line: 1,
                start_col: 1,
                end_line: 1,
                end_col: 1,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: usize, end: usize, source: &str) -> Span {
        Span::from_byte_range(start, end, source)
    }

    #[test]
    fn severity_label_and_display() {
        assert_eq!(Severity::Error.as_label(), "error");
        assert_eq!(Severity::Warning.as_label(), "warning");
        assert_eq!(Severity::Info.as_label(), "info");
        assert_eq!(format!("{}", Severity::Warning), "warning");
        assert_eq!(Severity::default(), Severity::Error);
    }

    #[test]
    fn span_from_byte_range_start_of_file() {
        let src = "hello\nworld\n";
        let s = span(0, 5, src);
        assert_eq!(s.start, 0);
        assert_eq!(s.end, 5);
        assert_eq!((s.start_line, s.start_col), (1, 1));
        assert_eq!((s.end_line, s.end_col), (1, 6));
    }

    #[test]
    fn span_from_byte_range_mid_line() {
        let src = "hello\nworld\n";
        // 'r' in "world" is at byte offset 8 (h-e-l-l-o-\n-w-o-r-l-d)
        let s = span(8, 9, src);
        assert_eq!((s.start_line, s.start_col), (2, 3));
        assert_eq!((s.end_line, s.end_col), (2, 4));
    }

    #[test]
    fn span_from_byte_range_multi_line() {
        let src = "abc\ndef\nghi\n";
        // span from 'b' (offset 1) to 'i' (offset 9)
        let s = span(1, 9, src);
        assert_eq!((s.start_line, s.start_col), (1, 2));
        assert_eq!((s.end_line, s.end_col), (3, 2));
    }

    #[test]
    fn span_from_platform_span() {
        let src = "ab\ncd\n";
        let ps = PlatformSpan { start: 3, end: 4 };
        let s = Span::from_platform_span(ps, src);
        assert_eq!(s.start, 3);
        assert_eq!(s.end, 4);
        // "ab\ncd\n": offset 3 is 'c' on line 2, col 1.
        assert_eq!((s.start_line, s.start_col), (2, 1));
    }

    #[test]
    fn span_len_and_empty() {
        let s = Span::from_byte_range(2, 5, "abcdef");
        assert_eq!(s.len(), 3);
        assert!(!s.is_empty());
        let e = Span::from_byte_range(3, 3, "abcdef");
        assert_eq!(e.len(), 0);
        assert!(e.is_empty());
    }

    #[test]
    fn compute_line_col_offset_zero() {
        assert_eq!(compute_line_col("anything", 0), (1, 1));
    }

    #[test]
    fn compute_line_col_at_newline() {
        // "ab\ncd": offset 3 is the 'c' right after the newline.
        let src = "ab\ncd";
        assert_eq!(compute_line_col(src, 3), (2, 1));
        // offset 2 points at the newline itself -> still line 1, col 3.
        assert_eq!(compute_line_col(src, 2), (1, 3));
    }

    #[test]
    fn compute_line_col_past_end_clamps() {
        let src = "ab\ncd";
        // offset beyond length clamps to last char position.
        let (line, col) = compute_line_col(src, 100);
        assert_eq!(line, 2);
        assert_eq!(col, 3); // 'd' is col 2, one past = 3
    }

    #[test]
    fn compute_line_col_multiple_lines() {
        let src = "a\nb\nc\nd\n";
        assert_eq!(compute_line_col(src, 6), (4, 1));
    }

    #[test]
    fn diagnostic_new_and_with_fix() {
        let s = Span::default();
        let d = Diagnostic::new("unused-import", Severity::Warning, s, "import never referenced");
        assert_eq!(d.rule, "unused-import");
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.message, "import never referenced");
        assert!(d.fix.is_none());

        let fix = Fix {
            span: s,
            replacement: String::new(),
        };
        let d2 = d.with_fix(fix);
        assert!(d2.fix.is_some());
    }

    #[test]
    fn render_diagnostic_format() {
        let src = "import x\n";
        let s = span(0, 8, src);
        let d = Diagnostic::new("unused-import", Severity::Warning, s, "import projection never referenced");
        let rendered = render_diagnostic(&d, "src/main.hnl");
        assert_eq!(
            rendered,
            "src/main.hnl:1:1: warning [unused-import]: import projection never referenced"
        );
    }

    #[test]
    fn render_diagnostics_multiple() {
        let src = "ab\ncd\n";
        let d1 = Diagnostic::new("r1", Severity::Error, span(0, 1, src), "first");
        let d2 = Diagnostic::new("r2", Severity::Info, span(3, 4, src), "second");
        let out = render_diagnostics(&[d1, d2], "f.hnl");
        assert_eq!(out, "f.hnl:1:1: error [r1]: first\nf.hnl:2:1: info [r2]: second");
    }

    #[test]
    fn render_json_valid_array() {
        let src = "ab\n";
        let d = Diagnostic::new("r1", Severity::Error, span(0, 1, src), "boom");
        let json = render_json(&[d]);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert!(parsed.is_array());
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["rule"], "r1");
        assert_eq!(arr[0]["severity"], "error");
        assert_eq!(arr[0]["message"], "boom");
    }

    #[test]
    fn apply_fixes_non_overlapping_descending() {
        let src = "abcdef";
        let f1 = Fix { span: span(0, 1, src), replacement: "X".into() };
        let f2 = Fix { span: span(4, 6, src), replacement: "YZ".into() };
        let out = apply_fixes(src, &[f1, f2]).unwrap();
        assert_eq!(out, "XbcdYZ");
    }

    #[test]
    fn apply_fixes_empty() {
        let out = apply_fixes("abc", &[]).unwrap();
        assert_eq!(out, "abc");
    }

    #[test]
    fn apply_fixes_rejects_overlapping() {
        let src = "abcdef";
        let f1 = Fix { span: span(0, 3, src), replacement: "X".into() };
        let f2 = Fix { span: span(2, 4, src), replacement: "Y".into() };
        let err = apply_fixes(src, &[f1, f2]).unwrap_err();
        assert!(matches!(err, FixError::Overlapping { .. }));
    }

    #[test]
    fn apply_fixes_adjacent_not_overlapping() {
        let src = "abcdef";
        let f1 = Fix { span: span(0, 2, src), replacement: "XY".into() };
        let f2 = Fix { span: span(2, 4, src), replacement: "ZW".into() };
        let out = apply_fixes(src, &[f1, f2]).unwrap();
        assert_eq!(out, "XYZWef");
    }

    #[test]
    fn apply_fixes_out_of_bounds() {
        let src = "abc";
        let f = Fix { span: span(1, 10, src), replacement: "X".into() };
        let err = apply_fixes(src, &[f]).unwrap_err();
        assert!(matches!(err, FixError::OutOfBounds { .. }));
    }

    #[test]
    fn apply_fixes_idempotent_when_no_fixes() {
        let src = "abc";
        let out = apply_fixes_to_fixpoint(src, |_| Vec::new(), 5).unwrap();
        assert_eq!(out, "abc");
    }

    #[test]
    fn apply_fixes_to_fixpoint_converges() {
        // Each iteration replaces the first 'a' with 'b' until no 'a' remains.
        let src = "aaa";
        let compute = |s: &str| -> Vec<Fix> {
            if let Some(idx) = s.find('a') {
                vec![Fix {
                    span: Span::from_byte_range(idx, idx + 1, s),
                    replacement: "b".into(),
                }]
            } else {
                Vec::new()
            }
        };
        let out = apply_fixes_to_fixpoint(src, compute, 10).unwrap();
        assert_eq!(out, "bbb");
    }

    #[test]
    fn apply_fixes_to_fixpoint_respects_max_iterations() {
        // Always produces a fix -> never converges.
        let src = "x";
        let compute = |s: &str| -> Vec<Fix> {
            vec![Fix {
                span: Span::from_byte_range(0, 1, s),
                replacement: "x".into(),
            }]
        };
        let err = apply_fixes_to_fixpoint(src, compute, 3).unwrap_err();
        assert!(matches!(err, FixError::NotConverged { max_iterations: 3 }));
    }

    #[test]
    fn apply_fixes_to_fixpoint_converges_within_budget() {
        // Needs exactly 3 iterations; budget of 5 should succeed.
        let src = "aaa";
        let compute = |s: &str| -> Vec<Fix> {
            if let Some(idx) = s.find('a') {
                vec![Fix {
                    span: Span::from_byte_range(idx, idx + 1, s),
                    replacement: "b".into(),
                }]
            } else {
                Vec::new()
            }
        };
        let out = apply_fixes_to_fixpoint(src, compute, 5).unwrap();
        assert_eq!(out, "bbb");
    }

    #[cfg(feature = "lang-span")]
    #[test]
    fn from_lang_span_computes_line_col() {
        let src = "ab\ncd\n";
        let ls = handl_language::Span { start: 3, end: 4 };
        let s = Span::from_lang_span(ls, src);
        assert_eq!(s.start, 3);
        assert_eq!(s.end, 4);
        // "ab\ncd\n": offset 3 is 'c' on line 2, col 1.
        assert_eq!((s.start_line, s.start_col), (2, 1));
    }

    #[cfg(feature = "lang-span")]
    #[test]
    fn from_lang_span_without_source_defaults() {
        let ls = handl_language::Span { start: 5, end: 9 };
        let s: Span = ls.into();
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 9);
        assert_eq!((s.start_line, s.start_col), (1, 1));
    }
}
