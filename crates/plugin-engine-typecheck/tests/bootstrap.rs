//! Integration tests for the bootstrap typecheck pass (ADR-0088).
//!
//! Table-driven tests covering each check rule from ADR-0074 and the grammar
//! spec. Each test parses a small handl module and asserts on the
//! diagnostics in the [`TypecheckReport`].

use handl_language::Language;
use handl_plugin_engine_typecheck::{CheckError, TypecheckReport, check_module};
use handl_plugin_ext_diag::Severity;

/// The canonical language for the bootstrap checker — supplied by the caller
/// (the dev-dep on `handl-kit-data-canonical` is allowed to close the
/// runtime dependency cycle in test builds only).
fn lang() -> &'static Language {
    static LANG: std::sync::OnceLock<Language> = std::sync::OnceLock::new();
    LANG.get_or_init(|| {
        handl_kit_data_canonical::language_from_yaml().expect("canonical language assembles")
    })
}

/// Assert that the report contains a diagnostic with the given rule and
/// severity.
fn has_diag(report: &TypecheckReport, rule: &str, severity: Severity) -> bool {
    report
        .diagnostics
        .iter()
        .any(|d| d.rule == rule && d.severity == severity)
}

/// Assert that the report has no diagnostics.
fn assert_clean(report: &TypecheckReport) {
    assert!(
        report.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        report
            .diagnostics
            .iter()
            .map(|d| format!("{} [{}]: {}", d.severity, d.rule, d.message))
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// duplicate-binding
// ---------------------------------------------------------------------------

#[test]
fn duplicate_binding_semicolon() {
    let report = check_module(lang(), "a = 1; a = 2").unwrap();
    assert!(
        has_diag(&report, "duplicate-binding", Severity::Error),
        "expected duplicate-binding error, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn duplicate_binding_newline() {
    let report = check_module(lang(), "a = 1\na = 2").unwrap();
    assert!(
        has_diag(&report, "duplicate-binding", Severity::Error),
        "expected duplicate-binding error, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn duplicate_binding_span_points_at_name() {
    let report = check_module(lang(), "a = 1; a = 2").unwrap();
    let dup = report
        .diagnostics
        .iter()
        .find(|d| d.rule == "duplicate-binding")
        .expect("duplicate-binding diagnostic");
    // The span should cover the second `a` (at byte offset 7 in "a = 1; a = 2").
    assert_eq!(dup.span.start, 7);
    assert_eq!(dup.span.end, 8);
}

// ---------------------------------------------------------------------------
// unbound-export
// ---------------------------------------------------------------------------

#[test]
fn unbound_export() {
    let report = check_module(lang(), "export sort from \"collection\"").unwrap();
    assert!(
        has_diag(&report, "unbound-export", Severity::Error),
        "expected unbound-export error, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn bound_export_is_clean() {
    let report = check_module(lang(), "sort = x ~> x; export sort from \"collection\"").unwrap();
    assert!(
        !has_diag(&report, "unbound-export", Severity::Error),
        "should not have unbound-export when name is bound, got: {:?}",
        report.diagnostics
    );
}

// ---------------------------------------------------------------------------
// unused-import
// ---------------------------------------------------------------------------

#[test]
fn unused_import() {
    let report = check_module(lang(), "import id from \"data/atoms\"; main = 1").unwrap();
    assert!(
        has_diag(&report, "unused-import", Severity::Warning),
        "expected unused-import warning, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn unused_import_span_points_at_name() {
    let report = check_module(lang(), "import id from \"data/atoms\"; main = 1").unwrap();
    let imp = report
        .diagnostics
        .iter()
        .find(|d| d.rule == "unused-import")
        .expect("unused-import diagnostic");
    // `id` starts at byte offset 7 in "import id from ...".
    assert_eq!(imp.span.start, 7);
    assert_eq!(imp.span.end, 9);
}

// ---------------------------------------------------------------------------
// unbound-wire
// ---------------------------------------------------------------------------

#[test]
fn unbound_wire() {
    let report = check_module(lang(), "main = foo").unwrap();
    assert!(
        has_diag(&report, "unbound-wire", Severity::Error),
        "expected unbound-wire error, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn unbound_wire_span_points_at_name() {
    let report = check_module(lang(), "main = foo").unwrap();
    let wire = report
        .diagnostics
        .iter()
        .find(|d| d.rule == "unbound-wire")
        .expect("unbound-wire diagnostic");
    // `foo` starts at byte offset 7 in "main = foo".
    assert_eq!(wire.span.start, 7);
    assert_eq!(wire.span.end, 10);
}

#[test]
fn bound_wire_from_binding_is_clean() {
    let report = check_module(lang(), "double = x ~> x * 2; main = double(3)").unwrap();
    assert_clean(&report);
}

// ---------------------------------------------------------------------------
// clean modules
// ---------------------------------------------------------------------------

#[test]
fn clean_module() {
    let report = check_module(lang(), "double = x ~> x * 2").unwrap();
    assert_clean(&report);
}

#[test]
fn clean_module_with_import_and_call() {
    let report = check_module(
        lang(),
        "import sort from \"collection\"; main = sort([3, 1, 2])",
    )
    .unwrap();
    assert_clean(&report);
}

#[test]
fn all_imports_used() {
    let src =
        "import sort from \"collection\"; import map from \"fn/map\"; main = sort(map([3, 1, 2]))";
    let report = check_module(lang(), src).unwrap();
    assert!(
        !has_diag(&report, "unused-import", Severity::Warning),
        "all imports are used, got: {:?}",
        report.diagnostics
    );
}

// ---------------------------------------------------------------------------
// not-yet-checked (rec)
// ---------------------------------------------------------------------------

#[test]
fn rec_not_yet_checked() {
    // `rec` fixpoint typing is deferred (ADR-0088). Using `rec` should emit
    // an Info `not-yet-checked` diagnostic rather than silently passing.
    let src = "foo = rec { x = 1 } ( x )";
    let result = check_module(lang(), src);
    // If the rec syntax doesn't parse in the current grammar, skip the
    // assertion (the task allows skipping when rec is not testable).
    if let Ok(report) = result {
        assert!(
            has_diag(&report, "not-yet-checked", Severity::Info),
            "expected not-yet-checked Info for rec, got: {:?}",
            report.diagnostics
        );
    }
    // If it's a parse error, the rec surface syntax isn't testable yet —
    // that's acceptable per ADR-0088.
}

// ---------------------------------------------------------------------------
// error recovery
// ---------------------------------------------------------------------------

#[test]
fn error_recovery_reports_all() {
    // Two unbound wires + one unused import — all should be reported.
    let src = "import id from \"data/atoms\"; main = foo; bar = baz";
    let report = check_module(lang(), src).unwrap();
    let unbound_count = report
        .diagnostics
        .iter()
        .filter(|d| d.rule == "unbound-wire")
        .count();
    assert!(
        unbound_count >= 2,
        "expected at least 2 unbound-wire errors, got {} (all: {:?})",
        unbound_count,
        report.diagnostics
    );
    assert!(
        has_diag(&report, "unused-import", Severity::Warning),
        "expected unused-import warning, got: {:?}",
        report.diagnostics
    );
}

#[test]
fn error_recovery_duplicate_and_unbound() {
    let src = "a = 1; a = 2; main = foo";
    let report = check_module(lang(), src).unwrap();
    assert!(
        has_diag(&report, "duplicate-binding", Severity::Error),
        "expected duplicate-binding, got: {:?}",
        report.diagnostics
    );
    assert!(
        has_diag(&report, "unbound-wire", Severity::Error),
        "expected unbound-wire, got: {:?}",
        report.diagnostics
    );
}

// ---------------------------------------------------------------------------
// empty module
// ---------------------------------------------------------------------------

#[test]
fn empty_module() {
    let report = check_module(lang(), "").unwrap();
    assert_clean(&report);
}

#[test]
fn whitespace_only_module() {
    let report = check_module(lang(), "   \n  \n").unwrap();
    assert_clean(&report);
}

#[test]
fn comment_only_module() {
    let report = check_module(lang(), "-- this is a comment\n-- another comment\n").unwrap();
    assert_clean(&report);
}

// ---------------------------------------------------------------------------
// parse error
// ---------------------------------------------------------------------------

#[test]
fn parse_error() {
    let result = check_module(lang(), "import from");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CheckError::Parse(_)));
}

#[test]
fn parse_error_message_is_nonempty() {
    let result = check_module(lang(), "import from");
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.starts_with("parse error:"));
    assert!(msg.len() > "parse error: ".len());
}

// ---------------------------------------------------------------------------
// API shape
// ---------------------------------------------------------------------------

#[test]
fn check_module_returns_report() {
    let report = check_module(lang(), "x = 1").unwrap();
    // The report should have a diagnostics vector (possibly empty).
    let _ = &report.diagnostics;
}

#[test]
fn typecheck_report_default_is_clean() {
    let report = TypecheckReport::default();
    assert!(report.diagnostics.is_empty());
    assert!(report.is_clean());
}

#[test]
fn check_error_display() {
    let err = CheckError::Parse("boom".to_string());
    assert_eq!(format!("{err}"), "parse error: boom");
}

#[test]
fn check_error_equality() {
    assert_eq!(
        CheckError::Parse("x".to_string()),
        CheckError::Parse("x".to_string())
    );
    assert_ne!(
        CheckError::Parse("x".to_string()),
        CheckError::Parse("y".to_string())
    );
}

// ---------------------------------------------------------------------------
// diagnostic severity correctness
// ---------------------------------------------------------------------------

#[test]
fn duplicate_binding_is_error_not_warning() {
    let report = check_module(lang(), "a = 1; a = 2").unwrap();
    let dup = report
        .diagnostics
        .iter()
        .find(|d| d.rule == "duplicate-binding")
        .expect("duplicate-binding diagnostic");
    assert_eq!(dup.severity, Severity::Error);
}

#[test]
fn unused_import_is_warning_not_error() {
    let report = check_module(lang(), "import id from \"data/atoms\"; main = 1").unwrap();
    let imp = report
        .diagnostics
        .iter()
        .find(|d| d.rule == "unused-import")
        .expect("unused-import diagnostic");
    assert_eq!(imp.severity, Severity::Warning);
}

#[test]
fn unbound_wire_is_error() {
    let report = check_module(lang(), "main = foo").unwrap();
    let wire = report
        .diagnostics
        .iter()
        .find(|d| d.rule == "unbound-wire")
        .expect("unbound-wire diagnostic");
    assert_eq!(wire.severity, Severity::Error);
}

// ---------------------------------------------------------------------------
// function param scoping
// ---------------------------------------------------------------------------

#[test]
fn function_param_does_not_trigger_unbound_wire() {
    // `x` is a function param, not an unbound wire.
    let report = check_module(lang(), "double = x ~> x * 2").unwrap();
    assert_clean(&report);
}

#[test]
fn function_body_unbound_wire_detected() {
    // `y` is not a param and not in scope → unbound wire.
    let report = check_module(lang(), "double = x ~> y * 2").unwrap();
    assert!(
        has_diag(&report, "unbound-wire", Severity::Error),
        "expected unbound-wire for y, got: {:?}",
        report.diagnostics
    );
}

// ---------------------------------------------------------------------------
// object literals & field access (field names are not references)
// ---------------------------------------------------------------------------

#[test]
fn object_literal_keys_not_unbound_wires() {
    // `x` and `y` are field names, not references — no unbound-wire errors.
    let report = check_module(lang(), "r = { x: 1, y: 2 }").unwrap();
    assert_clean(&report);
}

#[test]
fn field_access_target_checked() {
    // `r` is a binding (in scope); `x` is a field name (not a reference).
    let report = check_module(lang(), "r = { x: 1 }; p = r.x").unwrap();
    assert_clean(&report);
}

#[test]
fn field_access_unbound_target_detected() {
    // `q` is not in scope → unbound wire. `x` is a field name (not flagged).
    let report = check_module(lang(), "p = q.x").unwrap();
    assert!(
        has_diag(&report, "unbound-wire", Severity::Error),
        "expected unbound-wire for q, got: {:?}",
        report.diagnostics
    );
    // The field name `x` should NOT be flagged.
    let unbound_count = report
        .diagnostics
        .iter()
        .filter(|d| d.rule == "unbound-wire")
        .count();
    assert_eq!(
        unbound_count, 1,
        "only the target `q` should be flagged, not the field `x`"
    );
}

#[test]
fn object_value_unbound_wire_detected() {
    // `foo` in the value position is an unbound wire; `x` (key) is not.
    let report = check_module(lang(), "r = { x: foo }").unwrap();
    assert!(
        has_diag(&report, "unbound-wire", Severity::Error),
        "expected unbound-wire for foo, got: {:?}",
        report.diagnostics
    );
    let unbound_count = report
        .diagnostics
        .iter()
        .filter(|d| d.rule == "unbound-wire")
        .count();
    assert_eq!(
        unbound_count, 1,
        "only `foo` (value) should be flagged, not `x` (key)"
    );
}

// ---------------------------------------------------------------------------
// `from "<spec>" import ...` — canonical import surface (ADR-0096)
// ---------------------------------------------------------------------------

/// `from "str" import * as str` scopes the alias — qualified calls like
/// `str.trim` are not flagged as unbound wires.
#[test]
fn from_import_star_as_scopes_alias() {
    let report = check_module(
        lang(),
        "from \"str\" import * as str; main = str.trim(\"x\")",
    )
    .unwrap();
    assert_clean(&report);
}

/// `from "str" import name` (named projection, no `as`) scopes the name.
#[test]
fn from_import_named_scopes_name() {
    let report = check_module(
        lang(),
        "from \"str\" import reverse; main = reverse(\"ab\")",
    )
    .unwrap();
    assert_clean(&report);
}

/// `from "str" import name as local` scopes the alias, not the remote name.
#[test]
fn from_import_named_as_scopes_alias_not_name() {
    let report = check_module(
        lang(),
        "from \"str\" import reverse as rev; main = rev(\"ab\")",
    )
    .unwrap();
    assert_clean(&report);

    // The remote name itself is not bound.
    let report = check_module(
        lang(),
        "from \"str\" import reverse as rev; main = reverse(\"ab\")",
    )
    .unwrap();
    assert!(
        has_diag(&report, "unbound-wire", Severity::Error),
        "expected unbound-wire for `reverse`, got: {:?}",
        report.diagnostics
    );
}

/// `from "str" import *` (no alias) with a bare module-name specifier binds
/// the module name itself — `str.trim` resolves qualified via the module
/// chain at runtime, so `str` is in scope.
#[test]
fn from_import_star_bare_name_scopes_module_name() {
    let report = check_module(lang(), "from \"str\" import *; main = str.trim(\"x\")").unwrap();
    assert_clean(&report);
}

/// `from "m" import name` where the name is used inside a `do { ... }`
/// block: the block-level import scopes its bound names for following
/// siblings (mirroring `prim_import` runtime semantics).
#[test]
fn block_level_import_scopes_siblings() {
    let src = "do { from \"str\" import reverse\nreverse(\"ab\") }";
    let report = check_module(lang(), src).unwrap();
    assert_clean(&report);
}

/// A `do { ... }` block binds `assign` targets sequentially — a sibling
/// referring to an earlier block binding is not an unbound wire.
#[test]
fn block_assign_binds_for_siblings() {
    let report = check_module(lang(), "do { x :: 1\nx + 1 }").unwrap();
    assert_clean(&report);
}
