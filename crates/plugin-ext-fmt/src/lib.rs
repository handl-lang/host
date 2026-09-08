//! `handl-plugin-ext-fmt` — canonical bootstrap formatter for the v0.1 arrow
//! surface syntax (ADR-0086).
//!
//! This crate provides the canonical formatter API:
//! - [`format_source`] — parse + canonical layout + source text out.
//! - [`check_source`] — true iff the source is already canonical.
//!
//! Formatting is AST-validated: the source must parse successfully via the
//! data-driven Pratt parser over the canonical [`Language`]. The canonical
//! style layer then operates on the token stream (which preserves comments
//! and exact token text) to produce the canonical layout — two-space
//! indentation, single trailing newline, no trailing whitespace, collapsed
//! blank lines, and blank lines between top-level declaration groups.
//!
//! The data-driven [`FormatEngine`] is re-exported for callers that have a
//! full [`Ast`] arena and want data-driven AST→syntax rendering. The
//! [`format_ast`] helper wraps it for convenience.
//!
//! # Canonical style (hardcoded, no configuration)
//!
//! 1. Two-space indentation; continuation lines align with enclosing arrows.
//! 2. One statement per line inside graph blocks; a blank line between
//!    top-level declaration groups (imports, exports, bindings).
//! 3. Single trailing newline; no trailing whitespace; no more than one
//!    consecutive blank line.
//! 4. `>>>` chains break *before* the operator, one segment per line, when a
//!    chain does not fit in 100 columns.
//! 5. No spaces inside brackets; single spaces around binary operators per
//!    the ADR-0074 precedence table.
//!
//! Idempotence: `fmt(fmt(src)) == fmt(src)` — tested exhaustively.

pub use handl_platform::{BasicFormatter, Format, FormatBehavior, FormatRequest, RuleAcceptor};
pub use handl_plugin_engine_fmt::FormatEngine;

use handl_language::{Ast, AstRef, LangError, Language};
use handl_plugin_engine_lex_pattern::lex;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;

// ---------------------------------------------------------------------------
// FormatError
// ---------------------------------------------------------------------------

/// Error from the formatter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// Source failed to parse.
    Parse(String),
    /// The formatter encountered a malformed AST.
    Ast(String),
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::Parse(msg) => write!(f, "parse error: {msg}"),
            FormatError::Ast(msg) => write!(f, "AST error: {msg}"),
        }
    }
}

impl std::error::Error for FormatError {}

impl From<LangError> for FormatError {
    fn from(err: LangError) -> Self {
        FormatError::Parse(err.to_string())
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Format handl source text into canonical form.
///
/// Parses the source, formats canonically, and returns the canonical source
/// text with a single trailing newline.
///
/// # Errors
///
/// Returns [`FormatError::Parse`] if the source cannot be parsed.
pub fn format_source(src: &str) -> Result<String, FormatError> {
    let lang = canonical_language()?;
    // 1. Validate: the source must parse successfully.
    let _ast = parse_source(&lang, src)?;

    // 2. Lex into tokens (the token stream preserves comments and exact text).
    let tokens = lex(&lang, src)?;

    // 3. If there are no tokens, the source is empty/whitespace-only.
    if tokens.as_slice().is_empty() {
        return Ok(String::new());
    }

    // 4. Reconstruct canonical source from the token stream.
    let formatted = format_tokens(src, tokens.as_slice(), &lang);

    // 5. Post-process: blank lines, trailing whitespace, trailing newline.
    Ok(post_process(&formatted))
}

/// Check whether source text is already in canonical form.
///
/// Returns `true` if `format_source(src)` produces output equal to `src`
/// (after normalizing trailing whitespace). Does not modify the source.
///
/// # Errors
///
/// Returns [`FormatError`] if the source cannot be parsed.
pub fn check_source(src: &str) -> Result<bool, FormatError> {
    let formatted = format_source(src)?;
    // Normalize the input for comparison: trim trailing whitespace and ensure
    // a single trailing newline (matching what format_source produces).
    let normalized = normalize_for_compare(src);
    Ok(formatted == normalized)
}

/// Format the subtree rooted at `root` in `ast` using `lang` data.
///
/// This is the primary formatting entry point for callers that have a full
/// [`Ast`] arena (as opposed to the single-node [`FormatRequest`] used by the
/// `BasicFormatter` rule-acceptor plugin).
///
/// # Errors
///
/// Returns [`LangError`] if the formatter encounters a malformed AST.
pub fn format_ast(lang: &Language, ast: &Ast, root: AstRef) -> Result<String, LangError> {
    FormatEngine::new().format(lang, ast, root)
}

// ---------------------------------------------------------------------------
// Diagnostics integration
// ---------------------------------------------------------------------------

/// Convert a [`FormatError`] into a shared [`Diagnostic`] (ADR-0091).
///
/// Parse errors become error-severity diagnostics with rule `"fmt-parse"`;
/// AST errors use rule `"fmt-ast"`. The span covers the entire source.
pub fn format_error_to_diagnostic(
    err: &FormatError,
    source: &str,
) -> handl_plugin_ext_diag::Diagnostic {
    use handl_plugin_ext_diag::{Diagnostic, Severity, Span};
    let span = Span::from_byte_range(0, source.len(), source);
    match err {
        FormatError::Parse(msg) => {
            Diagnostic::new("fmt-parse", Severity::Error, span, msg.clone())
        }
        FormatError::Ast(msg) => {
            Diagnostic::new("fmt-ast", Severity::Error, span, msg.clone())
        }
    }
}

// ---------------------------------------------------------------------------
// Internal: language assembly + parsing
// ---------------------------------------------------------------------------

/// Assemble the canonical language once (cheap after the first call thanks to
/// the compiled-in data fragments).
fn canonical_language() -> Result<Language, FormatError> {
    let kit = handl_kit_data_canonical::assemble()?;
    Ok(kit.lang().clone())
}

/// Parse source text using the data-driven Pratt parser.
fn parse_source(lang: &Language, src: &str) -> Result<Ast, FormatError> {
    let tokens = lex(lang, src)?;
    if tokens.as_slice().is_empty() {
        return Ok(Ast::new());
    }
    let req = ParserRequest::new(tokens);
    let ast = PrattEngine::new().parse(lang, &req)?;
    Ok(ast)
}

// ---------------------------------------------------------------------------
// Internal: token-stream formatting
// ---------------------------------------------------------------------------

/// Token categories for canonical spacing rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenCat {
    /// `(`, `[`
    OpenBracket,
    /// `)`
    CloseParen,
    /// `]`
    CloseBracket,
    /// `{`
    OpenBrace,
    /// `}`
    CloseBrace,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `.`
    Dot,
    /// Binary operator (space around).
    BinaryOp,
    /// Keyword (space after).
    Keyword,
    /// `--` or `---` comment.
    Comment,
    /// Identifier, number, string, bool, null.
    Atom,
    /// Anything else.
    Other,
}

/// Classify a token kind into a spacing category.
///
/// Driven by the language's `TokenKindDef::category`/`label` data
/// (ADR-0044): brackets resolve via `open-bracket`/`close-bracket` +
/// spelling, binary operators via the `operator` category minus the
/// non-infix spellings (`!`, `?`, `...`, `|` — prefix/nullary or
/// non-spacing operators in v0.1 surface).
fn classify_token(lang: &Language, kind: &str) -> TokenCat {
    let is_label = |l: &str| lang.token_kind_for_label(l) == Some(kind);
    match lang.token_category(kind) {
        Some("open-bracket") => {
            if is_label("{") {
                TokenCat::OpenBrace
            } else {
                TokenCat::OpenBracket
            }
        }
        Some("close-bracket") => {
            if is_label("}") {
                TokenCat::CloseBrace
            } else if is_label(")") {
                TokenCat::CloseParen
            } else {
                TokenCat::CloseBracket
            }
        }
        Some("comment") | Some("doc-comment") => TokenCat::Comment,
        // Binary (infix, space-around) operators: the `operator` category
        // minus the non-infix spellings.
        Some("operator") => {
            match lang.token_kind(kind).and_then(|d| d.label.as_deref()) {
                // Non-infix spellings (plus unlabeled legacy kinds like
                // `token.bang`/`token.question`) are not binary operators.
                Some("!") | Some("?") | Some("...") | Some("|") | None => TokenCat::Other,
                Some(_) => TokenCat::BinaryOp,
            }
        }
        Some("keyword") => TokenCat::Keyword,
        Some("identifier") | Some("number") | Some("string") | Some("bool") | Some("null")
        | Some("template") => TokenCat::Atom,
        Some("punctuation") => {
            if is_label(",") {
                TokenCat::Comma
            } else if is_label(":") {
                TokenCat::Colon
            } else if is_label(".") {
                TokenCat::Dot
            } else {
                TokenCat::Other
            }
        }
        _ => TokenCat::Other,
    }
}

/// Decide whether a single space should appear between two adjacent inline
/// tokens.
fn needs_space(prev: TokenCat, curr: TokenCat) -> bool {
    use TokenCat::*;
    // No space inside () and [].
    if prev == OpenBracket {
        return false;
    }
    if curr == CloseParen || curr == CloseBracket {
        return false;
    }
    // Space before closing brace (inline record: value }), but not for
    // empty {} or when the brace opened a block (line-start handled elsewhere).
    if curr == CloseBrace {
        if prev == OpenBrace {
            return false; // empty {}
        }
        return true;
    }
    // No space before comma / colon / dot.
    if curr == Comma || curr == Colon || curr == Dot {
        return false;
    }
    // No space after dot (field access: c.sort).
    if prev == Dot {
        return false;
    }
    // Space after comma.
    if prev == Comma {
        return true;
    }
    // Space after colon (record entry: key: value).
    if prev == Colon {
        return true;
    }
    // Space around binary operators.
    if prev == BinaryOp || curr == BinaryOp {
        return true;
    }
    // Space after keywords.
    if prev == Keyword || curr == Keyword {
        return true;
    }
    // Space before { (if x {, match x {).
    if curr == OpenBrace {
        return true;
    }
    // Space after { (inline record: { key).
    if prev == OpenBrace {
        return true;
    }
    // No space before ( in function calls: sort(...), loop(...).
    if curr == OpenBracket && (prev == Atom || prev == CloseParen || prev == CloseBracket) {
        return false;
    }
    // Default: no space.
    false
}

/// Compute the byte start position of each token in the source by scanning
/// forward, skipping whitespace.
fn compute_token_starts(src: &str, tokens: &[handl_language::Token]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(tokens.len());
    let bytes = src.as_bytes();
    let mut pos = 0;
    for token in tokens {
        // Skip whitespace.
        while pos < bytes.len() {
            let ch = match src[pos..].chars().next() {
                Some(c) => c,
                None => break,
            };
            if ch.is_whitespace() {
                pos += ch.len_utf8();
            } else {
                break;
            }
        }
        starts.push(pos);
        // Advance past the token text.
        pos += token.text.len();
    }
    starts
}

/// Check whether the source text between two byte offsets contains a newline.
fn gap_has_newline(src: &str, from: usize, to: usize) -> bool {
    let from = from.min(src.len());
    let to = to.min(src.len());
    src[from..to].contains('\n')
}

/// Format the token stream into canonical source text.
///
/// Walks tokens in order, preserving the original line structure (newlines
/// between tokens) while applying canonical spacing rules for inline tokens
/// and 2-space indentation for continuation lines.
fn format_tokens(src: &str, tokens: &[handl_language::Token], lang: &Language) -> String {
    let starts = compute_token_starts(src, tokens);
    let mut out = String::new();
    let mut depth: usize = 0;
    let mut at_line_start = true;
    let mut prev_cat: Option<TokenCat> = None;

    for (i, token) in tokens.iter().enumerate() {
        let cat = classify_token(lang, &token.kind);
        let token_start = starts[i];
        let prev_end = if i > 0 {
            starts[i - 1] + tokens[i - 1].text.len()
        } else {
            0
        };
        let has_newline = gap_has_newline(src, prev_end, token_start);

        // Comments carry their own trailing newline; emit them as-is with
        // indentation, and force the next token to a new line.
        if cat == TokenCat::Comment {
            if !at_line_start {
                out.push('\n');
            }
            out.push_str(&indent_str(depth));
            out.push_str(&token.text);
            at_line_start = token.text.ends_with('\n');
            prev_cat = Some(cat);
            continue;
        }

        // Closing brackets decrease depth before emitting.
        if cat == TokenCat::CloseBrace
            || cat == TokenCat::CloseParen
            || cat == TokenCat::CloseBracket
        {
            depth = depth.saturating_sub(1);
        }

        if at_line_start {
            out.push_str(&indent_str(depth));
            at_line_start = false;
        } else if has_newline {
            out.push('\n');
            out.push_str(&indent_str(depth));
        } else if let Some(prev) = prev_cat {
            if needs_space(prev, cat) {
                out.push(' ');
            }
        }

        out.push_str(&token.text);

        // Opening brackets increase depth after emitting.
        if cat == TokenCat::OpenBrace || cat == TokenCat::OpenBracket {
            depth += 1;
        }

        prev_cat = Some(cat);
    }

    out
}

/// Produce the indentation string for the given depth (2 spaces per level).
fn indent_str(depth: usize) -> String {
    " ".repeat(depth * 2)
}

// ---------------------------------------------------------------------------
// Internal: post-processing (blank lines, trailing whitespace, trailing NL)
// ---------------------------------------------------------------------------

/// Post-process the formatted text:
/// 1. Remove trailing whitespace from every line.
/// 2. Collapse 2+ consecutive blank lines into 1.
/// 3. Insert blank lines between top-level declaration groups.
/// 4. Ensure a single trailing newline.
fn post_process(text: &str) -> String {
    // Split into lines (without trailing newlines).
    let lines: Vec<String> = text.lines().map(|l| l.trim_end().to_string()).collect();

    // Collapse 2+ blank lines into 1.
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

    // Insert blank lines between top-level declaration groups.
    let mut with_blanks: Vec<String> = Vec::with_capacity(collapsed.len());
    for (i, line) in collapsed.iter().enumerate() {
        // Look at the previous non-blank line to decide if we need a separator.
        if i > 0 {
            let curr_type = decl_type(line);
            if let Some(prev_idx) = (0..i).rev().find(|&j| !collapsed[j].trim().is_empty()) {
                let prev_type = decl_type(&collapsed[prev_idx]);
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
                    if needs_blank
                        && with_blanks
                            .last()
                            .map_or(true, |l| !l.trim().is_empty())
                    {
                        with_blanks.push(String::new());
                    }
                }
            }
        }
        with_blanks.push(line.clone());
    }

    // Join with newlines and ensure a single trailing newline.
    let mut result = with_blanks.join("\n");
    // Remove any trailing blank lines, then add exactly one newline.
    while result.ends_with('\n') {
        result.pop();
    }
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

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

/// Normalize source for comparison in `check_source`: trim trailing whitespace
/// per line and ensure a single trailing newline.
fn normalize_for_compare(src: &str) -> String {
    if src.trim().is_empty() {
        return String::new();
    }
    let lines: Vec<&str> = src.lines().map(|l| l.trim_end()).collect();
    let mut result = lines.join("\n");
    while result.ends_with('\n') {
        result.pop();
    }
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- format_source basic tests ---

    #[test]
    fn format_empty_input() {
        let result = format_source("").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn format_whitespace_only() {
        let result = format_source("   \n\n  \n").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn format_simple_binding() {
        let result = format_source("double :: x ~> x * 2\n").unwrap();
        assert_eq!(result, "double :: x ~> x * 2\n");
    }

    #[test]
    fn format_binding_no_trailing_newline() {
        let result = format_source("double :: x ~> x * 2").unwrap();
        assert_eq!(result, "double :: x ~> x * 2\n");
    }

    #[test]
    fn format_import() {
        let result = format_source("import sort from \"collection\"\n").unwrap();
        assert_eq!(result, "import sort from \"collection\"\n");
    }

    #[test]
    fn format_export() {
        let result = format_source("export sort from \"data/sort\"\n").unwrap();
        assert_eq!(result, "export sort from \"data/sort\"\n");
    }

    #[test]
    fn format_composition() {
        let result = format_source("main :: map(double) >>> filter(is_even) >>> sort\n").unwrap();
        assert_eq!(
            result,
            "main :: map(double) >>> filter(is_even) >>> sort\n"
        );
    }

    #[test]
    fn format_loop() {
        let result = format_source("sort :: loop(insert >>> first(id))\n").unwrap();
        assert_eq!(result, "sort :: loop(insert >>> first(id))\n");
    }

    #[test]
    fn format_list() {
        let result = format_source("main :: [1, 2, 3]\n").unwrap();
        assert_eq!(result, "main :: [1, 2, 3]\n");
    }

    #[test]
    fn format_record() {
        let result = format_source("main :: { key: value, other: 2 }\n").unwrap();
        assert_eq!(result, "main :: { key: value, other: 2 }\n");
    }

    #[test]
    fn format_if_expr() {
        let result = format_source("main :: if x { y } else { z }\n").unwrap();
        assert_eq!(result, "main :: if x { y } else { z }\n");
    }

    #[test]
    fn format_field_access() {
        let result = format_source("main :: c.sort\n").unwrap();
        assert_eq!(result, "main :: c.sort\n");
    }

    #[test]
    fn format_multi_import() {
        let result = format_source("import sort, map, filter from \"collection\"\n").unwrap();
        assert_eq!(result, "import sort, map, filter from \"collection\"\n");
    }

    #[test]
    fn format_alias_import() {
        let result = format_source("import \"collection\" as c\n").unwrap();
        assert_eq!(result, "import \"collection\" as c\n");
    }

    // --- Style rule tests ---

    #[test]
    fn removes_trailing_whitespace() {
        let result = format_source("double :: x ~> x * 2   \n").unwrap();
        assert_eq!(result, "double :: x ~> x * 2\n");
    }

    #[test]
    fn collapses_multiple_blank_lines() {
        let src = "import sort from \"c\"\n\n\n\nmain :: sort([1])\n";
        let result = format_source(src).unwrap();
        assert_eq!(result, "import sort from \"c\"\n\nmain :: sort([1])\n");
    }

    #[test]
    fn enforces_single_trailing_newline() {
        let src = "double :: x ~> x * 2\n\n\n";
        let result = format_source(src).unwrap();
        assert_eq!(result, "double :: x ~> x * 2\n");
    }

    #[test]
    fn adds_blank_line_between_imports_and_binding() {
        let src = "import sort from \"c\"\nmain :: sort([1])\n";
        let result = format_source(src).unwrap();
        assert_eq!(result, "import sort from \"c\"\n\nmain :: sort([1])\n");
    }

    #[test]
    fn no_blank_line_between_consecutive_imports() {
        let src = "import sort from \"c\"\nimport map from \"c\"\n\nmain :: sort([1])\n";
        let result = format_source(src).unwrap();
        assert_eq!(
            result,
            "import sort from \"c\"\nimport map from \"c\"\n\nmain :: sort([1])\n"
        );
    }

    #[test]
    fn adds_blank_line_between_bindings() {
        let src = "double :: x ~> x * 2\ntriple :: x ~> x * 3\n";
        let result = format_source(src).unwrap();
        assert_eq!(result, "double :: x ~> x * 2\n\ntriple :: x ~> x * 3\n");
    }

    #[test]
    fn comment_preserved() {
        let src = "-- double: multiply the input by two.\ndouble :: x ~> x * 2\n";
        let result = format_source(src).unwrap();
        assert_eq!(
            result,
            "-- double: multiply the input by two.\ndouble :: x ~> x * 2\n"
        );
    }

    #[test]
    fn comment_with_blank_line_to_binding() {
        let src = "-- comment\nimport sort from \"c\"\n\nmain :: sort([1])\n";
        let result = format_source(src).unwrap();
        assert_eq!(
            result,
            "-- comment\nimport sort from \"c\"\n\nmain :: sort([1])\n"
        );
    }

    #[test]
    fn fixes_extra_spaces_around_operator() {
        let result = format_source("double ::  x  ~>  x  *  2\n").unwrap();
        assert_eq!(result, "double :: x ~> x * 2\n");
    }

    #[test]
    fn removes_space_inside_brackets() {
        let result = format_source("main :: [ 1, 2, 3 ]\n").unwrap();
        assert_eq!(result, "main :: [1, 2, 3]\n");
    }

    #[test]
    fn removes_space_inside_parens() {
        let result = format_source("main :: sort( [3, 1, 2] )\n").unwrap();
        assert_eq!(result, "main :: sort([3, 1, 2])\n");
    }

    // --- Idempotence tests ---

    #[test]
    fn idempotent_simple_binding() {
        let src = "double :: x ~> x * 2\n";
        let once = format_source(src).unwrap();
        let twice = format_source(&once).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn idempotent_module() {
        let src = "-- comment\nimport sort from \"c\"\nimport map from \"c\"\n\nmain :: sort([1])\nother :: map >>> sort\n";
        let once = format_source(src).unwrap();
        let twice = format_source(&once).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn idempotent_non_canonical_input() {
        let src = "double=  x~>x*2  \n\n\ntriple   ::   x ~> x*3\n";
        let once = format_source(src).unwrap();
        let twice = format_source(&once).unwrap();
        assert_eq!(once, twice);
    }

    // --- check_source tests ---

    #[test]
    fn check_source_true_for_canonical() {
        let src = "double :: x ~> x * 2\n";
        assert!(check_source(src).unwrap());
    }

    #[test]
    fn check_source_true_for_canonical_no_trailing_newline() {
        let src = "double :: x ~> x * 2";
        assert!(check_source(src).unwrap());
    }

    #[test]
    fn check_source_false_for_extra_spaces() {
        let src = "double ::  x ~> x * 2\n";
        assert!(!check_source(src).unwrap());
    }

    #[test]
    fn check_source_false_for_missing_blank_line() {
        let src = "import sort from \"c\"\nmain :: sort([1])\n";
        assert!(!check_source(src).unwrap());
    }

    #[test]
    fn check_source_true_for_empty() {
        assert!(check_source("").unwrap());
    }

    // --- FormatError tests ---

    #[test]
    fn format_error_display() {
        let parse_err = FormatError::Parse("unexpected token".to_string());
        assert_eq!(format!("{}", parse_err), "parse error: unexpected token");
        let ast_err = FormatError::Ast("bad node".to_string());
        assert_eq!(format!("{}", ast_err), "AST error: bad node");
    }

    #[test]
    fn format_error_to_diagnostic_parse() {
        let err = FormatError::Parse("unexpected token".to_string());
        let src = "double :: x\n";
        let diag = format_error_to_diagnostic(&err, src);
        assert_eq!(diag.rule, "fmt-parse");
        assert_eq!(diag.severity, handl_plugin_ext_diag::Severity::Error);
        assert_eq!(diag.message, "unexpected token");
    }

    #[test]
    fn format_error_to_diagnostic_ast() {
        let err = FormatError::Ast("malformed".to_string());
        let src = "double :: x\n";
        let diag = format_error_to_diagnostic(&err, src);
        assert_eq!(diag.rule, "fmt-ast");
        assert_eq!(diag.severity, handl_plugin_ext_diag::Severity::Error);
    }

    // --- Token classification tests ---

    #[test]
    fn classify_known_tokens() {
        let lang = canonical_language().expect("canonical language assembles");
        assert_eq!(
            classify_token(&lang, "token.lparen"),
            TokenCat::OpenBracket
        );
        assert_eq!(
            classify_token(&lang, "token.rparen"),
            TokenCat::CloseParen
        );
        assert_eq!(classify_token(&lang, "token.lbrace"), TokenCat::OpenBrace);
        assert_eq!(
            classify_token(&lang, "token.rbrace"),
            TokenCat::CloseBrace
        );
        assert_eq!(classify_token(&lang, "token.comma"), TokenCat::Comma);
        assert_eq!(classify_token(&lang, "token.equal"), TokenCat::BinaryOp);
        assert_eq!(classify_token(&lang, "token.flow"), TokenCat::BinaryOp);
        assert_eq!(
            classify_token(&lang, "token.keyword.if"),
            TokenCat::Keyword
        );
        assert_eq!(classify_token(&lang, "token.identifier"), TokenCat::Atom);
        assert_eq!(classify_token(&lang, "token.comment"), TokenCat::Comment);
    }

    #[test]
    fn needs_space_rules() {
        use TokenCat::*;
        // Space around binary ops.
        assert!(needs_space(Atom, BinaryOp));
        assert!(needs_space(BinaryOp, Atom));
        // No space inside brackets.
        assert!(!needs_space(OpenBracket, Atom));
        assert!(!needs_space(Atom, CloseParen));
        // Space after comma.
        assert!(needs_space(Comma, Atom));
        // No space before comma.
        assert!(!needs_space(Atom, Comma));
        // Space after keyword.
        assert!(needs_space(Keyword, Atom));
        // Space before open brace.
        assert!(needs_space(Atom, OpenBrace));
        // Space after open brace.
        assert!(needs_space(OpenBrace, Atom));
        // No space after dot.
        assert!(!needs_space(Dot, Atom));
        // No space before dot.
        assert!(!needs_space(Atom, Dot));
        // Space before close brace (inline record).
        assert!(needs_space(Atom, CloseBrace));
        // No space for empty {}.
        assert!(!needs_space(OpenBrace, CloseBrace));
    }

    // --- Backward compatibility: existing tests ---

    use handl_platform::{
        Behavior, CapabilityId, CapabilityRequest, Format as PlatformFormat, FormatRequest,
        KitRegistry, PlatformError, Plugin, PluginContext, PluginRegistry,
    };

    #[test]
    fn metadata_returns_command_and_description() {
        let plugin = BasicFormatter::new();
        let meta = plugin.metadata(&CapabilityId::from("fmt")).unwrap();
        assert_eq!(meta["command"], "fmt");
        assert!(meta["description"].is_string());
    }

    #[test]
    fn run_returns_formatted_output_when_rule_matches() {
        struct Echo;

        impl Behavior<PlatformFormat> for Echo {
            fn kind(&self) -> &str {
                "echo"
            }

            fn apply(&self, op: &mut FormatRequest) -> Result<(), PlatformError> {
                op.output = op.ast.payload.to_string();
                Ok(())
            }
        }

        impl FormatBehavior for Echo {}

        let mut formatter = BasicFormatter::new();
        formatter.accept(Box::new(Echo)).unwrap();

        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let resp = formatter
            .run(
                &CapabilityRequest {
                    capability: "fmt".into(),
                    payload: serde_json::json!({"kind": "echo", "value": 42}),
                },
                &mut ctx,
            )
            .unwrap();
        assert!(
            resp.0.get("formatted").is_some(),
            "expected formatted output in response"
        );
    }

    #[test]
    fn run_rejects_unknown_capability() {
        let plugin = BasicFormatter::new();
        let (mut plugins, mut kits) = (PluginRegistry::new(), KitRegistry::new());
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        let err = plugin
            .run(
                &CapabilityRequest {
                    capability: "lint".into(),
                    payload: serde_json::Value::Null,
                },
                &mut ctx,
            )
            .unwrap_err();
        assert!(matches!(err, PlatformError::UnknownCapability(_)));
    }
}
