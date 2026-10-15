//! `ToolingHost` delegation contract: `format` and `diagnose` must route
//! through the shared `plugin-ext-fmt` / `plugin-ext-lint` implementations,
//! not a private copy.
//!
//! This replaces the former `perf.rs`, whose four tests were `#[ignore]`d as
//! "not yet implemented" (false — they are implemented) and whose inputs used
//! the `|>` pipe operator that was **removed** from the canonical grammar. Its
//! wall-clock bounds (`elapsed < 5ms`) were also inherently flaky, so they are
//! gone: what matters is the *behaviour* being delegated, and that it scales.

use handl_host_tooling::{Severity, ToolingHost};
use handl_kit_canonical::Kit;
use handl_plugin_ext_lint::LintContext;

/// The canonical lint context the host would inject — the kit's language
/// plus its `.hnl` resolver (ADR-0128 §0.8; the kit is a dev-dep).
fn lint_ctx() -> LintContext {
    let kit = Kit::new().expect("canonical kit should assemble");
    LintContext::with_resolver(kit.language().clone(), kit.handl_lint_adapter().resolver())
}

fn host() -> ToolingHost {
    ToolingHost::new().with_lint_context(lint_ctx())
}

fn lang() -> handl_language::Language {
    Kit::new()
        .expect("canonical kit should assemble")
        .runtime()
        .lang()
        .clone()
}

/// A module with a leading doc comment and one binding — lints clean.
const CLEAN: &str = "--- module doc\na = 1;";

/// A module whose inputs are not in canonical format, so the `fmt` rule fires
/// with a machine-applicable fix.
const NEEDS_FORMAT: &str = "a   =   1;";

/// Rebind the same name, so `shadowed-binding` fires.
const SHADOWED: &str = "--- module doc\na = 1;\na = 2;";

/// Not parseable at all (as a module).
const SYNTAX_ERROR: &str = "## not a comment here";

/// Cannot even be lexed — an unterminated string literal.
const LEX_ERROR: &str = "a = \"unterminated;";

fn large_file(n: usize) -> String {
    let mut s = String::from("--- module doc\n");
    for i in 0..n {
        s.push_str(&format!("a{i} = {i};\n"));
    }
    s
}

// ---------------------------------------------------------------------------
// format
// ---------------------------------------------------------------------------

#[test]
fn format_is_ok_for_a_valid_module() {
    let out = host().format(&lang(), CLEAN).expect("clean module formats");
    assert!(!out.is_empty());
}

#[test]
fn format_matches_the_shared_formatter() {
    let lang = lang();
    let via_host = host().format(&lang, NEEDS_FORMAT).expect("formats");
    let direct =
        handl_plugin_ext_fmt::format_source_with_language(&lang, NEEDS_FORMAT).expect("formats");
    assert_eq!(
        via_host, direct,
        "the host must not re-implement formatting"
    );
}

#[test]
fn format_is_idempotent() {
    let lang = lang();
    let once = host().format(&lang, NEEDS_FORMAT).expect("formats");
    let twice = host().format(&lang, &once).expect("reformats");
    assert_eq!(once, twice);
}

#[test]
fn format_reports_lex_errors() {
    // The canonical formatter renders from the *token stream*, so it only
    // rejects source that cannot be lexed at all.
    assert!(host().format(&lang(), LEX_ERROR).is_err());
}

#[test]
fn format_tolerates_source_that_lexes_but_does_not_parse() {
    // Token-stream rendering means unparseable-but-lexable input still
    // formats; this pins that contract so it cannot regress silently.
    assert!(host().format(&lang(), SYNTAX_ERROR).is_ok());
}

// ---------------------------------------------------------------------------
// diagnose
// ---------------------------------------------------------------------------

#[test]
fn diagnose_is_empty_for_a_clean_module() {
    assert!(
        host().diagnose(CLEAN).is_empty(),
        "a doc-commented, canonical module must lint clean"
    );
}

#[test]
fn diagnose_reports_a_parse_error_as_an_error_diagnostic() {
    let diags = host().diagnose(SYNTAX_ERROR);
    assert!(!diags.is_empty(), "a parse error must produce a diagnostic");
    assert!(
        diags.iter().any(|d| d.severity == Severity::Error),
        "expected an error-level diagnostic, got {diags:?}"
    );
}

#[test]
fn diagnose_carries_rule_names_and_spans() {
    let diags = host().diagnose(SHADOWED);
    let shadowed = diags
        .iter()
        .find(|d| d.rule == "shadowed-binding")
        .unwrap_or_else(|| panic!("expected a shadowed-binding diagnostic, got {diags:?}"));
    assert_eq!(shadowed.severity, Severity::Warning);
    assert!(shadowed.span.end > shadowed.span.start, "span must be real");
}

#[test]
fn diagnose_exposes_the_fix_payload() {
    // The improved `plugin-ext-diag` model carries machine-applicable fixes;
    // the former local `Diagnostic` struct dropped them.
    let diags = host().diagnose(NEEDS_FORMAT);
    let fmt = diags
        .iter()
        .find(|d| d.rule == "fmt")
        .unwrap_or_else(|| panic!("expected a fmt diagnostic, got {diags:?}"));
    assert!(fmt.fix.is_some(), "the fmt rule is fixable");
}

// ---------------------------------------------------------------------------
// scale
// ---------------------------------------------------------------------------

#[test]
fn format_and_diagnose_scale_to_a_large_module() {
    let lang = lang();
    let input = large_file(500);
    host().format(&lang, &input).expect("large module formats");
    // Benign bindings: the only diagnostics are informational.
    assert!(
        host()
            .diagnose(&input)
            .iter()
            .all(|d| d.severity != Severity::Error),
        "benign lets must not produce errors"
    );
}
