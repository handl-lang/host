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
    /// The rule name, e.g. `"unused-import"`. Owned: rule names come from
    /// `Language` data (ADR-0097), not Rust constants.
    pub rule: String,
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
    pub fn new(
        rule: impl Into<String>,
        severity: Severity,
        span: Span,
        message: impl Into<String>,
    ) -> Self {
        Diagnostic {
            rule: rule.into(),
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
    OutOfBounds {
        start: usize,
        end: usize,
        len: usize,
    },
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

// ---------------------------------------------------------------------------
// Source-chunk splitting (shared by the bootstrap typechecker, the linter,
// and the test runner — AGENTS.md "Source splitting")
// ---------------------------------------------------------------------------

/// A top-level statement chunk: its trimmed text and byte offset in the
/// original source.
///
/// The v0.1 Pratt parser parses a single statement per call, so tools that
/// operate on whole files split the source into statements first. This is
/// the single shared implementation of that split (previously each of the
/// typechecker, linter, and test runner carried a private copy that had
/// diverged).
#[derive(Clone, Debug)]
pub struct StatementChunk {
    /// The trimmed statement text.
    pub text: String,
    /// Byte offset of the trimmed text's first byte in the original source.
    pub offset: usize,
}

/// Split `src` into top-level statement chunks.
///
/// The v0.1 surface separates top-level declarations by newlines or
/// semicolons. Splits at bracket-depth zero, respecting string literals and
/// `--` comments. A `--` comment that begins at a statement boundary (only
/// whitespace precedes it in the pending chunk) is dropped entirely; a
/// comment inside or trailing a statement is left in the chunk text — the
/// lexer strips it at parse time. This is what keeps a `--` line inside a
/// `{ ... }` block from truncating the in-progress statement.
///
/// Newlines are whitespace to the grammar, so a depth-0 `\n` is only a
/// *candidate* boundary: it is not a split point when the pending chunk
/// ends mid-expression (last significant char is an operator — no Handl
/// statement ends with one) or when the following line cannot begin a
/// statement (starts with `=`, a closer, an infix operator, …). `;` is
/// always a hard separator.
pub fn split_statements(src: &str) -> Vec<StatementChunk> {
    let bytes = src.as_bytes();
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut depth: i32 = 0;
    // Last significant byte of the pending chunk — every code byte outside
    // strings and comments.
    let mut last_sig: Option<u8> = None;
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];

        // String literal — skip to the closing quote (respecting escapes).
        if b == b'"' {
            i += 1;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => {
                        i += 2;
                        continue;
                    }
                    b'"' => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
            last_sig = Some(b'"');
            continue;
        }

        // Comment: `--` (or `---`) to end of line. Only advance `start`
        // past the comment when it sits at a statement boundary (the
        // pending chunk is whitespace-only) — advancing it mid-statement
        // would discard the statement text preceding the comment and leave
        // a dangling tail (e.g. a bare `}`) behind.
        if b == b'-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
            let comment_start = i;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            if src[start..comment_start].trim().is_empty() {
                start = i;
                last_sig = None;
            }
            continue;
        }

        // Bracket nesting.
        match b {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ => {}
        }

        // Statement separators at depth zero.
        if depth == 0 && (b == b'\n' || b == b';') {
            if b == b';' || is_newline_boundary(src, start, last_sig, i) {
                push_chunk(src, &mut chunks, start, i);
                i += 1;
                start = i;
                last_sig = None;
                continue;
            }
        }

        if !b.is_ascii_whitespace() {
            last_sig = Some(b);
        }
        i += 1;
    }

    // Trailing chunk.
    push_chunk(src, &mut chunks, start, src.len());
    chunks
}

/// Is the `\n` at byte `i` a real statement boundary? False when the
/// pending statement is mid-expression (ends on an operator char) or the
/// next line cannot begin a statement.
fn is_newline_boundary(src: &str, start: usize, last_sig: Option<u8>, i: usize) -> bool {
    let bytes = src.as_bytes();
    // No Handl statement ends with an operator or punctuation char —
    // postfix forms end in an identifier or a closer.
    const CANNOT_END: &[u8] = b"=,:;+-*/%<>!?&|~.@";
    if let Some(b) = last_sig {
        if CANNOT_END.contains(&b) {
            // `from "<spec>" import *` / `import *` legitimately END in
            // `*` — the star is the splice marker, not a pending binary
            // operator. Without this the following statement merges into
            // the import's trailing tokens.
            if !(b == b'*' && is_import_chunk(&src[start..i])) {
                return false;
            }
        }
    }
    // Scan forward past whitespace and `--` comments to the next code byte.
    let mut j = i + 1;
    loop {
        while j < bytes.len() && matches!(bytes[j], b' ' | b'\t' | b'\r' | b'\n') {
            j += 1;
        }
        if j + 1 < bytes.len() && bytes[j] == b'-' && bytes[j + 1] == b'-' {
            while j < bytes.len() && bytes[j] != b'\n' {
                j += 1;
            }
            continue;
        }
        break;
    }
    match bytes.get(j) {
        // No Handl statement begins with these; unary `-`/`+`/`!` and open
        // brackets are excluded since they legitimately start expressions.
        Some(b) => !b"=,)]}.:;|&<>~*/%?".contains(b),
        None => true,
    }
}

/// Does `chunk` start a `from`/`import` declaration? Used to exempt the
/// splice marker `import *` from the trailing-operator heuristic — a
/// `*` ending an import statement is complete, not mid-expression.
fn is_import_chunk(chunk: &str) -> bool {
    let t = chunk.trim_start();
    for kw in ["from", "import"] {
        if let Some(rest) = t.strip_prefix(kw) {
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                return true;
            }
        }
    }
    false
}

/// Push a trimmed chunk from `src[start..end]` if it is non-empty and not a
/// comment-only chunk (a `--` comment at a statement boundary that was not
/// elided during the scan).
fn push_chunk(src: &str, chunks: &mut Vec<StatementChunk>, start: usize, end: usize) {
    let slice = &src[start..end];
    let lead = slice.len() - slice.trim_start().len();
    let trimmed = slice.trim();
    if trimmed.is_empty() || trimmed.starts_with("--") {
        return;
    }
    chunks.push(StatementChunk {
        text: trimmed.to_string(),
        offset: start + lead,
    });
}

#[cfg(feature = "lang-span")]
mod lang_span {
    use super::{Span, compute_line_col};

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
        let d = Diagnostic::new(
            "unused-import",
            Severity::Warning,
            s,
            "import never referenced",
        );
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
        let d = Diagnostic::new(
            "unused-import",
            Severity::Warning,
            s,
            "import projection never referenced",
        );
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
        assert_eq!(
            out,
            "f.hnl:1:1: error [r1]: first\nf.hnl:2:1: info [r2]: second"
        );
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
        let f1 = Fix {
            span: span(0, 1, src),
            replacement: "X".into(),
        };
        let f2 = Fix {
            span: span(4, 6, src),
            replacement: "YZ".into(),
        };
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
        let f1 = Fix {
            span: span(0, 3, src),
            replacement: "X".into(),
        };
        let f2 = Fix {
            span: span(2, 4, src),
            replacement: "Y".into(),
        };
        let err = apply_fixes(src, &[f1, f2]).unwrap_err();
        assert!(matches!(err, FixError::Overlapping { .. }));
    }

    #[test]
    fn apply_fixes_adjacent_not_overlapping() {
        let src = "abcdef";
        let f1 = Fix {
            span: span(0, 2, src),
            replacement: "XY".into(),
        };
        let f2 = Fix {
            span: span(2, 4, src),
            replacement: "ZW".into(),
        };
        let out = apply_fixes(src, &[f1, f2]).unwrap();
        assert_eq!(out, "XYZWef");
    }

    #[test]
    fn apply_fixes_out_of_bounds() {
        let src = "abc";
        let f = Fix {
            span: span(1, 10, src),
            replacement: "X".into(),
        };
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

    #[test]
    fn split_statements_continues_on_trailing_operator() {
        // `x := 1 +\n  2` — the newline is mid-expression; one chunk.
        // Trailing whitespace after the operator must not mask it.
        let chunks = split_statements("x := 1 +  \n  2\ny := 3");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "x := 1 +  \n  2");
        assert_eq!(chunks[1].text, "y := 3");
    }

    #[test]
    fn split_statements_continues_on_impossible_lead() {
        // `assert(..)\n= expect` — `=` cannot begin a statement; one chunk.
        let chunks = split_statements("t := assert(1 == 1)\n  = true\nu := 2");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "t := assert(1 == 1)\n  = true");
        assert_eq!(chunks[1].text, "u := 2");
    }

    #[test]
    fn split_statements_still_splits_plain_lines() {
        // Both sides plausible statements — the newline splits.
        let chunks = split_statements("x := 1\ny := f(x)\nz");
        assert_eq!(chunks.len(), 3);
    }

    #[test]
    fn split_statements_semicolon_stays_hard() {
        // `;` splits even mid-expression-adjacent shapes.
        let chunks = split_statements("x := 1; y := 2");
        assert_eq!(chunks.len(), 2);
    }

    #[test]
    fn split_statements_import_star_splits_at_newline() {
        // `from "spec" import *` ends in `*` — the splice marker, not a
        // pending binary operator; the next line is a new statement.
        let chunks = split_statements("from \"pkg\" import *\nout := f(1)\nout");
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "from \"pkg\" import *");
        assert_eq!(chunks[1].text, "out := f(1)");
        // A bare `import *` likewise.
        let chunks = split_statements("import *\nx");
        assert_eq!(chunks.len(), 2);
        // But `a *\n b` in a non-import statement still merges.
        let chunks = split_statements("x := a *\n  b\ny := 1");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "x := a *\n  b");
    }

    #[test]
    fn split_statements_boundary_comment_before_continuation() {
        // Comment line between the statement head and a `=`-led
        // continuation still merges.
        let chunks = split_statements("t := assert(1 == 1)\n-- expect\n  = true");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "t := assert(1 == 1)\n-- expect\n  = true");
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
    // --- boundary/escape branches of the shared statement splitter ---

    #[test]
    fn split_statements_respects_escaped_quotes_in_strings() {
        // The `\\` arm of the string scanner: an escaped quote must not close
        // the literal, so the `;` inside it is not a statement boundary.
        let chunks = split_statements("a = \"x\\\"y; z\"\nb = 2");
        assert_eq!(chunks.len(), 2, "{chunks:?}");
        assert_eq!(chunks[1].text, "b = 2");
    }

    #[test]
    fn split_statements_elides_a_comment_only_leading_region() {
        let chunks = split_statements("-- header\nx = 1");
        assert_eq!(chunks.len(), 1, "{chunks:?}");
        assert_eq!(chunks[0].text, "x = 1");
    }

    #[test]
    fn split_statements_treats_import_star_as_a_complete_statement() {
        // `from "x" import *` ends in `*`, which is otherwise a pending binary
        // operator; `is_import_chunk` exempts it, so the following line splits.
        let chunks = split_statements("from \"x\" import *\ny = 1");
        assert_eq!(chunks.len(), 2, "{chunks:?}");
        assert_eq!(chunks[0].text, "from \"x\" import *");
    }

    #[test]
    fn split_statements_handles_a_statement_ending_at_eof() {
        // `bytes.get(j)` is `None` once past the end — still a boundary.
        let chunks = split_statements("a = 1\n");
        assert_eq!(chunks.len(), 1, "{chunks:?}");
    }

    #[test]
    fn split_statements_drops_a_comment_only_tail() {
        let chunks = split_statements("a = 1;\n-- just a comment\n");
        assert_eq!(chunks.len(), 1, "{chunks:?}");
        // The `;` is the separator, so it is not part of the chunk text.
        assert_eq!(chunks[0].text, "a = 1");
    }

    #[test]
    fn apply_fixes_to_fixpoint_accepts_convergence_on_the_final_check() {
        // The loop exhausts `max_iterations`, but the source has converged by
        // then — the trailing check must still return `Ok`. (`Cell`, because
        // the closure parameter is `Fn`, not `FnMut`.)
        let calls = std::cell::Cell::new(0usize);
        let out = apply_fixes_to_fixpoint(
            "a",
            |s| {
                let n = calls.get();
                calls.set(n + 1);
                if n == 0 {
                    vec![Fix {
                        span: Span::from_byte_range(0, 1, s),
                        replacement: "b".into(),
                    }]
                } else {
                    Vec::new()
                }
            },
            1,
        )
        .expect("convergence on the final check is success");
        assert_eq!(out, "b");
    }

    #[test]
    fn apply_fixes_to_fixpoint_reports_non_convergence() {
        // A fix closure that always produces more work never converges, so the
        // loop exhausts `max_iterations` and the final check still finds work.
        let err = apply_fixes_to_fixpoint(
            "a",
            |s| {
                vec![Fix {
                    span: Span::from_byte_range(s.len(), s.len(), s),
                    replacement: "b".into(),
                }]
            },
            2,
        )
        .expect_err("must not converge");
        assert!(
            matches!(err, FixError::NotConverged { max_iterations: 2 }),
            "{err:?}"
        );
    }
}
