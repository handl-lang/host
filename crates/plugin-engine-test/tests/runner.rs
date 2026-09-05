//! Integration tests for the bootstrap test runner (ADR-0089).

use std::path::PathBuf;

use handl_plugin_engine_test::runner::{
    discover, format_report_human, format_report_json, run, run_with_filter, DiscoverError,
    TestPlan, TestResult,
};

/// Helper: path to a fixture file.
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

// ---------------------------------------------------------------------------
// Discovery tests
// ---------------------------------------------------------------------------

#[test]
fn discover_empty_paths() {
    let plan = discover(&[]).unwrap();
    assert!(plan.tests.is_empty());
    assert!(plan.parse_errors.is_empty());
}

#[test]
fn discover_single_file() {
    let plan = discover(&[fixture("pass.handl")]).unwrap();
    assert!(plan.parse_errors.is_empty());
    // Should discover test_addition, test_number, test_string.
    assert_eq!(plan.tests.len(), 3);
    assert_eq!(plan.tests[0].name, "test_addition");
    assert_eq!(plan.tests[1].name, "test_number");
    assert_eq!(plan.tests[2].name, "test_string");
}

#[test]
fn discover_directory() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let plan = discover(&[dir]).unwrap();
    // All .handl files in the fixtures directory should be found.
    // Files are sorted lexicographically: fail, mixed, no_tests, parse_error, pass.
    assert!(!plan.tests.is_empty());

    // Verify lexicographic ordering of modules.
    let mut prev_module = String::new();
    for test in &plan.tests {
        let module = test.module.display().to_string();
        if module != prev_module {
            // Module paths should be non-decreasing.
            assert!(
                module >= prev_module,
                "modules not in lexicographic order: {prev_module} before {module}"
            );
            prev_module = module;
        }
    }
}

#[test]
fn discover_nonexistent_path() {
    let result = discover(&[PathBuf::from("/nonexistent/path/file.handl")]);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        DiscoverError::NotFound(_)
    ));
}

#[test]
fn discover_parse_error() {
    let plan = discover(&[fixture("parse_error.handl")]).unwrap();
    // The parse error should be recorded, not propagated.
    assert_eq!(plan.parse_errors.len(), 1);
    assert!(plan.tests.is_empty());
    let (path, msg) = &plan.parse_errors[0];
    assert!(path.display().to_string().contains("parse_error.handl"));
    assert!(!msg.is_empty());
}

#[test]
fn discover_no_tests() {
    let plan = discover(&[fixture("no_tests.handl")]).unwrap();
    assert!(plan.tests.is_empty());
    assert!(plan.parse_errors.is_empty());
}

#[test]
fn discover_preserves_source_order() {
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    assert_eq!(plan.tests.len(), 3);
    // Source order: test_pass, test_fail, test_pass_two.
    assert_eq!(plan.tests[0].name, "test_pass");
    assert_eq!(plan.tests[1].name, "test_fail");
    assert_eq!(plan.tests[2].name, "test_pass_two");
}

// ---------------------------------------------------------------------------
// Execution tests
// ---------------------------------------------------------------------------

#[test]
fn run_all_pass() {
    let plan = discover(&[fixture("pass.handl")]).unwrap();
    let report = run(&plan);
    assert_eq!(report.total(), 3);
    assert_eq!(report.passed(), 3);
    assert_eq!(report.failed(), 0);
    assert_eq!(report.skipped(), 0);
    assert!(!report.has_failures());
}

#[test]
fn run_with_failure() {
    let plan = discover(&[fixture("fail.handl")]).unwrap();
    let report = run(&plan);
    assert_eq!(report.total(), 1);
    assert_eq!(report.passed(), 0);
    assert_eq!(report.failed(), 1);
    assert!(report.has_failures());

    // The failure should have a message.
    match &report.outcomes[0].result {
        TestResult::Fail { message } => {
            assert!(!message.is_empty());
        }
        other => panic!("expected Fail, got {other:?}"),
    }
}

#[test]
fn run_mixed() {
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let report = run(&plan);
    assert_eq!(report.total(), 3);
    assert_eq!(report.passed(), 2);
    assert_eq!(report.failed(), 1);
    assert_eq!(report.skipped(), 0);
    assert!(report.has_failures());

    // The middle test (test_fail) should be the failing one.
    assert!(matches!(
        report.outcomes[0].result,
        TestResult::Pass
    ));
    assert!(matches!(
        report.outcomes[1].result,
        TestResult::Fail { .. }
    ));
    assert!(matches!(
        report.outcomes[2].result,
        TestResult::Pass
    ));
}

#[test]
fn run_with_filter_matches_some() {
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    // Filter matches "test_pass" (both test_pass and test_pass_two).
    let report = run_with_filter(&plan, "test_pass");
    assert_eq!(report.total(), 3);
    // test_pass and test_pass_two match, test_fail is skipped.
    assert_eq!(report.passed(), 2);
    assert_eq!(report.skipped(), 1);
    assert_eq!(report.failed(), 0);
}

#[test]
fn run_with_filter_matches_none() {
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let report = run_with_filter(&plan, "nonexistent");
    assert_eq!(report.total(), 3);
    assert_eq!(report.skipped(), 3);
    assert_eq!(report.passed(), 0);
    assert_eq!(report.failed(), 0);
}

#[test]
fn run_with_filter_empty_matches_all() {
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let report = run_with_filter(&plan, "");
    assert_eq!(report.total(), 3);
    assert_eq!(report.passed(), 2);
    assert_eq!(report.failed(), 1);
    assert_eq!(report.skipped(), 0);
}

#[test]
fn run_empty_plan() {
    let plan = TestPlan::default();
    let report = run(&plan);
    assert_eq!(report.total(), 0);
    assert_eq!(report.passed(), 0);
    assert_eq!(report.failed(), 0);
    assert_eq!(report.skipped(), 0);
    assert!(!report.has_failures());
}

// ---------------------------------------------------------------------------
// Reporting tests
// ---------------------------------------------------------------------------

#[test]
fn format_report_human_output() {
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let report = run(&plan);
    let output = format_report_human(&report);

    // Each test should have a line.
    assert!(output.contains("PASS  test_pass"));
    assert!(output.contains("FAIL  test_fail"));
    assert!(output.contains("PASS  test_pass_two"));

    // Summary line.
    assert!(output.contains("3 tests:"));
    assert!(output.contains("2 passed"));
    assert!(output.contains("1 failed"));
    assert!(output.contains("0 skipped"));
}

#[test]
fn format_report_json_valid() {
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let report = run(&plan);
    let json = format_report_json(&report);

    // Should be valid JSON.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert!(parsed["tests"].is_array());

    let tests = parsed["tests"].as_array().unwrap();
    assert_eq!(tests.len(), 3);

    // Check statuses.
    assert_eq!(tests[0]["status"], "pass");
    assert_eq!(tests[1]["status"], "fail");
    assert_eq!(tests[2]["status"], "pass");

    // The failing test should have a message.
    assert!(tests[1]["message"].is_string());

    // Summary.
    assert_eq!(parsed["summary"]["total"], 3);
    assert_eq!(parsed["summary"]["passed"], 2);
    assert_eq!(parsed["summary"]["failed"], 1);
    assert_eq!(parsed["summary"]["skipped"], 0);
}

#[test]
fn report_counts() {
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let report = run(&plan);
    assert_eq!(report.total(), 3);
    assert_eq!(report.passed(), 2);
    assert_eq!(report.failed(), 1);
    assert_eq!(report.skipped(), 0);
}

// ---------------------------------------------------------------------------
// Determinism test
// ---------------------------------------------------------------------------

#[test]
fn determinism_source_order() {
    // Running the same plan twice should produce identical results.
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let report1 = run(&plan);
    let report2 = run(&plan);

    assert_eq!(report1.total(), report2.total());
    assert_eq!(report1.passed(), report2.passed());
    assert_eq!(report1.failed(), report2.failed());

    // Test names should be in the same order.
    for (o1, o2) in report1.outcomes.iter().zip(report2.outcomes.iter()) {
        assert_eq!(o1.case.name, o2.case.name);
        assert_eq!(o1.result, o2.result);
    }
}
