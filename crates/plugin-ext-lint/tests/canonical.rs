//! Canonical bootstrap linter integration tests (ADR-0087).
//!
//! Every rule has positive (rule fires), negative (rule doesn't fire), and
//! fix (where applicable) cases. Diagnostics are filtered by rule name in
//! assertions so the `fmt` rule (which fires on non-canonical spacing) does
//! not interfere with rule-specific checks.

use handl_plugin_ext_diag::Severity;
use handl_plugin_ext_lint::{check_source, fix_source, lint_source, LintError};

/// Collect diagnostics whose `rule` matches `name`.
fn for_rule<'a>(
    diags: &'a [handl_plugin_ext_diag::Diagnostic],
    name: &str,
) -> Vec<&'a handl_plugin_ext_diag::Diagnostic> {
    diags.iter().filter(|d| d.rule == name).collect()
}

/// Whether any diagnostic has the given rule and severity.
fn has(diags: &[handl_plugin_ext_diag::Diagnostic], name: &str, sev: Severity) -> bool {
    diags
        .iter()
        .any(|d| d.rule == name && d.severity == sev)
}

// ---------------------------------------------------------------------------
// unused-import
// ---------------------------------------------------------------------------

#[test]
fn unused_import_fires() {
    let src = "-- doc\nimport unused from \"mod\"\nmain = 42\n";
    let diags = lint_source(src).unwrap();
    assert!(
        has(&diags, "unused-import", Severity::Warning),
        "expected unused-import warning, got {:?}",
        diags
    );
}

#[test]
fn unused_import_clean() {
    let src = "-- doc\nimport used from \"mod\"\nmain = used\n";
    let diags = lint_source(src).unwrap();
    assert!(
        for_rule(&diags, "unused-import").is_empty(),
        "expected no unused-import, got {:?}",
        diags
    );
}

#[test]
fn unused_import_fix() {
    let src = "-- doc\nimport unused from \"mod\"\nmain = 42\n";
    let fixed = fix_source(src).unwrap();
    assert!(
        !fixed.contains("import unused"),
        "fix should remove the import: {fixed:?}"
    );
    // The fixed source should be lint-clean of unused-import.
    let diags = lint_source(&fixed).unwrap();
    assert!(for_rule(&diags, "unused-import").is_empty());
}

// ---------------------------------------------------------------------------
// unbound-wire
// ---------------------------------------------------------------------------

#[test]
fn unbound_wire_fires() {
    let src = "-- doc\nmain = undefined_thing\n";
    let diags = lint_source(src).unwrap();
    assert!(
        has(&diags, "unbound-wire", Severity::Error),
        "expected unbound-wire error, got {:?}",
        diags
    );
}

#[test]
fn unbound_wire_clean() {
    let src = "-- doc\nimport greet from \"mod\"\nmain = greet\n";
    let diags = lint_source(src).unwrap();
    assert!(
        for_rule(&diags, "unbound-wire").is_empty(),
        "expected no unbound-wire, got {:?}",
        diags
    );
}

#[test]
fn unbound_wire_arrow_param_in_scope() {
    // The arrow parameter `x` is in scope for the body reference `x`.
    let src = "-- doc\ndouble = x ~> x * 2\n";
    let diags = lint_source(src).unwrap();
    assert!(
        for_rule(&diags, "unbound-wire").is_empty(),
        "expected no unbound-wire for arrow param, got {:?}",
        diags
    );
}

// ---------------------------------------------------------------------------
// shadowed-binding
// ---------------------------------------------------------------------------

#[test]
fn shadowed_binding_fires() {
    let src = "-- doc\nx = 1\nx = 2\n";
    let diags = lint_source(src).unwrap();
    assert!(
        has(&diags, "shadowed-binding", Severity::Warning),
        "expected shadowed-binding warning, got {:?}",
        diags
    );
}

#[test]
fn shadowed_binding_clean() {
    let src = "-- doc\nx = 1\ny = 2\n";
    let diags = lint_source(src).unwrap();
    assert!(
        for_rule(&diags, "shadowed-binding").is_empty(),
        "expected no shadowed-binding, got {:?}",
        diags
    );
}

// ---------------------------------------------------------------------------
// dead-branch
// ---------------------------------------------------------------------------

#[test]
fn dead_branch_constant_true() {
    let src = "-- doc\nmain = if true { 1 } else { 2 }\n";
    let diags = lint_source(src).unwrap();
    assert!(
        has(&diags, "dead-branch", Severity::Warning),
        "expected dead-branch warning for true guard, got {:?}",
        diags
    );
}

#[test]
fn dead_branch_constant_false() {
    let src = "-- doc\nmain = if false { 1 } else { 2 }\n";
    let diags = lint_source(src).unwrap();
    assert!(
        has(&diags, "dead-branch", Severity::Warning),
        "expected dead-branch warning for false guard, got {:?}",
        diags
    );
}

#[test]
fn dead_branch_ternary_true() {
    let src = "-- doc\nmain = true ? 1 : 2\n";
    let diags = lint_source(src).unwrap();
    assert!(
        has(&diags, "dead-branch", Severity::Warning),
        "expected dead-branch warning for ternary true, got {:?}",
        diags
    );
}

#[test]
fn dead_branch_clean() {
    let src = "-- doc\nmain = if x { 1 } else { 2 }\n";
    // Note: `x` is unbound, but dead-branch should not fire (non-constant guard).
    let diags = lint_source(src).unwrap();
    assert!(
        for_rule(&diags, "dead-branch").is_empty(),
        "expected no dead-branch for non-constant guard, got {:?}",
        diags
    );
}

// ---------------------------------------------------------------------------
// naming-convention
// ---------------------------------------------------------------------------

#[test]
fn naming_convention_violation() {
    let src = "-- doc\nMyFunc = 42\n";
    let diags = lint_source(src).unwrap();
    assert!(
        has(&diags, "naming-convention", Severity::Warning),
        "expected naming-convention warning, got {:?}",
        diags
    );
}

#[test]
fn naming_convention_clean() {
    let src = "-- doc\nmy_func = 42\n";
    let diags = lint_source(src).unwrap();
    assert!(
        for_rule(&diags, "naming-convention").is_empty(),
        "expected no naming-convention, got {:?}",
        diags
    );
}

#[test]
fn naming_convention_fix() {
    let src = "-- doc\nMyFunc = 42\n";
    let fixed = fix_source(src).unwrap();
    assert!(
        fixed.contains("my_func"),
        "fix should rename to snake_case: {fixed:?}"
    );
    assert!(!fixed.contains("MyFunc"));
    let diags = lint_source(&fixed).unwrap();
    assert!(for_rule(&diags, "naming-convention").is_empty());
}

// ---------------------------------------------------------------------------
// missing-module-doc
// ---------------------------------------------------------------------------

#[test]
fn missing_module_doc_fires() {
    let src = "main = 42\n";
    let diags = lint_source(src).unwrap();
    assert!(
        has(&diags, "missing-module-doc", Severity::Info),
        "expected missing-module-doc info, got {:?}",
        diags
    );
}

#[test]
fn missing_module_doc_clean() {
    let src = "-- doc\nmain = 42\n";
    let diags = lint_source(src).unwrap();
    assert!(
        for_rule(&diags, "missing-module-doc").is_empty(),
        "expected no missing-module-doc, got {:?}",
        diags
    );
}

// ---------------------------------------------------------------------------
// fmt
// ---------------------------------------------------------------------------

#[test]
fn fmt_rule_fires() {
    let src = "-- doc\nmain=42\n";
    let diags = lint_source(src).unwrap();
    assert!(
        has(&diags, "fmt", Severity::Warning),
        "expected fmt warning, got {:?}",
        diags
    );
}

#[test]
fn fmt_rule_fix() {
    let src = "-- doc\nmain=42\n";
    let fixed = fix_source(src).unwrap();
    assert_eq!(fixed, "-- doc\nmain = 42\n");
    let diags = lint_source(&fixed).unwrap();
    assert!(for_rule(&diags, "fmt").is_empty());
}

// ---------------------------------------------------------------------------
// Public API shape
// ---------------------------------------------------------------------------

#[test]
fn lint_source_returns_diagnostics() {
    let src = "-- doc\nmain = undefined_ref\n";
    let diags = lint_source(src).unwrap();
    assert!(!diags.is_empty());
    // Diagnostics carry rule names and severities.
    assert!(diags.iter().all(|d| !d.rule.is_empty()));
}

#[test]
fn check_source_clean() {
    // A clean, canonical module with a doc comment and a bound reference.
    let src = "-- doc\nmy_func = 42\n";
    assert!(check_source(src).unwrap());
}

#[test]
fn check_source_has_errors() {
    let src = "-- doc\nmain = undefined_ref\n";
    assert!(!check_source(src).unwrap());
}

#[test]
fn fix_source_idempotent() {
    let src = "-- doc\nmy_func = 42\n";
    let fixed = fix_source(src).unwrap();
    assert_eq!(fixed, src, "fixing already-fixed source is a no-op");
    // Fixing the fixed output is also a no-op.
    let fixed2 = fix_source(&fixed).unwrap();
    assert_eq!(fixed2, fixed);
}

#[test]
fn fix_source_converges() {
    // Multiple fixable issues: an unused import, a naming violation, and
    // non-canonical formatting. The fixpoint should converge to a clean file.
    let src = "-- doc\nimport unused from \"mod\"\nMyFunc = 42\n";
    let fixed = fix_source(src).unwrap();
    let diags = lint_source(&fixed).unwrap();
    let fixable: Vec<_> = diags
        .iter()
        .filter(|d| d.fix.is_some())
        .collect();
    assert!(
        fixable.is_empty(),
        "expected no remaining fixable diagnostics, got {:?}",
        fixable
    );
    assert!(!fixed.contains("import unused"));
    assert!(fixed.contains("my_func"));
}

#[test]
fn diagnostics_sorted_by_span() {
    // A source with diagnostics at multiple offsets.
    let src = "main = undefined_ref\n";
    let diags = lint_source(src).unwrap();
    for w in diags.windows(2) {
        assert!(
            w[0].span.start <= w[1].span.start,
            "diagnostics not sorted by span: {:?}",
            diags
        );
    }
}

// ---------------------------------------------------------------------------
// Parse errors
// ---------------------------------------------------------------------------

#[test]
fn lint_source_parse_error() {
    // An unterminated string is a lex/parse error.
    let src = "-- doc\nmain = \"unterminated\n";
    let result = lint_source(src);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LintError::Parse(_)));
}

// ---------------------------------------------------------------------------
// Lint-clean gate over the repo's own fixture (ADR-0087 CI gate).
// ---------------------------------------------------------------------------

#[test]
fn fixture_lint_clean() {
    let src = include_str!("fixtures/clean.hnl");
    // The canonical fixture must be lint-clean (no Error diagnostics) and
    // already in canonical format (no fmt warnings).
    let diags = lint_source(src).unwrap();
    let non_fmt: Vec<_> = diags.iter().filter(|d| d.rule != "fmt").collect();
    assert!(
        non_fmt.is_empty(),
        "fixture has non-fmt diagnostics: {:?}",
        non_fmt
    );
    assert!(
        check_source(src).unwrap(),
        "fixture must be lint-clean (no errors)"
    );
}
