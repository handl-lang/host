//! `handl-plugin-ext-fmt` — canonical bootstrap formatter for the v0.1 arrow
//! surface syntax (ADR-0086), composed over the staged engine pipeline
//! (ADR-0095).
//!
//! This crate is a **thin surface** over the language-generic engine in
//! `handl-plugin-engine-fmt`. It owns only what is canonical to the v0.1
//! surface:
//! - the canonical [`Language`] assembly [`canonical_language`],
//! - the **parse** stage (lex + Pratt) that validates and tokenizes source,
//! - the selection of the canonical v0.1 layout/post-process rule sets,
//! - the [`FormatError`]/diagnostics (ADR-0091) surface.
//!
//! The engine owns the pipeline stages:
//! ```text
//! source → parse → layout → render → post-process → canonical source
//! ```
//! where **layout** ([`TokenCat`]/[`FormattingRule`]), **render**
//! ([`TokenFormatter`] for the token stream, [`FormatEngine`] for an `Ast`),
//! and **post-process** ([`PostProcessRule`]) all live in
//! `handl-plugin-engine-fmt` and are reusable/replaceable.
//!
//! Public API (unchanged, thin wrappers over the pipeline):
//! - [`format_source`] — parse + canonical layout + source text out.
//! - [`check_source`] — true iff the source is already canonical.
//! - [`format_ast`] — data-driven `Ast` → source via [`FormatEngine`].
//! - [`format_error_to_diagnostic`] — shared diagnostics (ADR-0091).
//!
//! # Canonical style (v0.1, ADR-0086; selected here, implemented in the engine)
//!
//! 1. Two-space indentation; continuation lines align with enclosing arrows.
//! 2. One statement per line inside graph blocks; a blank line between
//!    top-level declaration groups (imports, exports, bindings).
//! 3. Single trailing newline; no trailing whitespace; no more than one
//!    consecutive blank line.
//! 4. `>>>` chains break *before* the operator, one segment per line, when a
//!    chain does not fit in `FormatOptions::width` columns.
//! 5. No spaces inside brackets; single spaces around binary operators per
//!    the ADR-0074 precedence table.
//!
//! Idempotence: `fmt(fmt(src)) == fmt(src)` — tested exhaustively.

pub use handl_platform::{BasicFormatter, Format, FormatBehavior, FormatRequest, RuleAcceptor};
pub use handl_plugin_engine_fmt::{
    CanonicalFormattingRule, FormatEngine, FormatOptions, FormattingRule, PostProcessRule, TokenCat,
    TokenFormatter, canonical_post_process, classify_token, needs_space,
};

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
/// text with a single trailing newline. Composes the staged engine pipeline:
/// parse → layout (canonical [`FormattingRule`]) → render
/// ([`TokenFormatter`]) → post-process ([`canonical_post_process`]).
///
/// # Errors
///
/// Returns [`FormatError::Parse`] if the source cannot be parsed.
pub fn format_source(src: &str) -> Result<String, FormatError> {
    let lang = canonical_language()?;
    // Stage 1 · parse: the source must parse successfully (AST-validated).
    let _ast = parse_source(&lang, src)?;
    // Stage 1 (cont.) · lex: the token stream preserves comments and exact text.
    let tokens = lex(&lang, src)?;
    // If there are no tokens, the source is empty/whitespace-only.
    if tokens.as_slice().is_empty() {
        return Ok(String::new());
    }
    // Stages 2+3 · layout + render over the token stream with the canonical
    // v0.1 rule set.
    let opts = FormatOptions::default();
    let formatter = TokenFormatter::canonical(opts.indent_size);
    let rendered = formatter.format(src, tokens.as_slice(), &lang);
    // Stage 4 · post-process: blank lines, trailing whitespace, trailing newline.
    Ok(canonical_post_process(&rendered))
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
        FormatError::Parse(msg) => Diagnostic::new("fmt-parse", Severity::Error, span, msg.clone()),
        FormatError::Ast(msg) => Diagnostic::new("fmt-ast", Severity::Error, span, msg.clone()),
    }
}

// ---------------------------------------------------------------------------
// Internal: language assembly + parsing (the pipeline's parse stage)
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
// Internal: normalization for `check_source` comparison
// ---------------------------------------------------------------------------

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
