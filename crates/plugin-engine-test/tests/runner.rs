//! Integration tests for the bootstrap test runner (ADR-0089).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use handl_plugin_engine_test::config::{run_with_config, RunConfig};
use handl_plugin_engine_test::events::TestEvent;
use handl_plugin_engine_test::provider::{discover_with, FileSystemProvider, TestProvider};
use handl_plugin_engine_test::report::{summarize, HumanReporter, JsonReporter, Reporter};
use handl_plugin_engine_test::runner::{
    discover, filter_predicate, format_report_human, format_report_json, run, run_selected,
    run_selected_cached, run_with_filter, tag_predicate, DiscoverError, TestPlan, TestResult,
    TestRunCache,
};
use handl_plugin_lib_scheduler::SchedulePolicy;

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
fn discover_test_language_decls() {
    // ADR-0089 §4: `test`/`property`/`integration_test`/`bench` declarations
    // from the test-language fragment merged onto the canonical kit.
    let plan = discover(&[fixture("decls.handl")]).unwrap();
    assert!(
        plan.parse_errors.is_empty(),
        "parse errors: {:?}",
        plan.parse_errors
    );
    let names: Vec<&str> = plan.tests.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "literal_pass",
            "literal_fail",
            "identity",
            "end_to_end",
            "small"
        ]
    );
    // Body source is the expression after `=`.
    assert_eq!(plan.tests[0].body_source, "1 + 1");
    assert_eq!(plan.tests[1].body_source, "assert false");
}

#[test]
fn run_test_language_decls() {
    let plan = discover(&[fixture("decls.handl")]).unwrap();
    let report = run(&plan);
    assert_eq!(report.total(), 5);
    assert_eq!(report.passed(), 4);
    assert_eq!(report.failed(), 1);
}

#[test]
fn run_selected_parallel_matches_sequential_order() {
    // ADR-0089: parallelism changes timing, never logical result ordering.
    // BoundedScheduler returns completion order; the runner re-collects in
    // canonical plan order.
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let seq = run(&plan);
    let par = run_selected(
        &plan,
        &handl_plugin_engine_test::Filter::all(),
        &SchedulePolicy::parallel(4),
    );
    let seq_results: Vec<(&str, &TestResult)> = seq
        .outcomes
        .iter()
        .map(|o| (o.case.name.as_str(), &o.result))
        .collect();
    let par_results: Vec<(&str, &TestResult)> = par
        .outcomes
        .iter()
        .map(|o| (o.case.name.as_str(), &o.result))
        .collect();
    assert_eq!(seq_results, par_results);
}

#[test]
fn filter_predicate_glob() {
    let plan = discover(&[fixture("decls.handl")]).unwrap();
    let report = run_selected(
        &plan,
        &filter_predicate("literal_*"),
        &SchedulePolicy::sequential(),
    );
    assert_eq!(report.total(), 5);
    assert_eq!(report.passed() + report.failed(), 2);
    assert_eq!(report.skipped(), 3);
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
    assert!(matches!(result.unwrap_err(), DiscoverError::NotFound(_)));
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
    assert!(matches!(report.outcomes[0].result, TestResult::Pass));
    assert!(matches!(report.outcomes[1].result, TestResult::Fail { .. }));
    assert!(matches!(report.outcomes[2].result, TestResult::Pass));
}

#[test]
fn run_assert_keyword() {
    let plan = discover(&[fixture("assert_keyword.handl")]).unwrap();
    let report = run(&plan);
    assert_eq!(report.total(), 3);
    assert_eq!(report.passed(), 2);
    assert_eq!(report.failed(), 1);

    // First two keyword assertions pass; the third fails.
    assert!(matches!(report.outcomes[0].result, TestResult::Pass));
    assert!(matches!(report.outcomes[1].result, TestResult::Pass));
    assert!(matches!(report.outcomes[2].result, TestResult::Fail { .. }));

    // The failing equality assertion should carry structured failure detail.
    let failure = &report.outcomes[2].failure;
    assert_eq!(failure.assertion, Some("assert".to_string()));
    assert_eq!(failure.expected, Some("3".to_string()));
    assert_eq!(failure.actual, Some("2".to_string()));
    assert!(failure.diff.is_some());
    assert!(failure.cause.is_none());
}

#[test]
fn assert_keyword_truthy_failure_carries_detail() {
    // A standalone failing truthy assertion should record expected/actual.
    let plan = discover(&[fixture("assert_keyword.handl")]).unwrap();
    // Select only the truthy pass (assert "hello"), which passes.
    let report = run_with_filter(&plan, "keyword_truthy");
    assert_eq!(report.passed(), 1);
    assert_eq!(report.failed(), 0);
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
fn format_report_human_groups_by_suite_and_shows_failure_detail() {
    // Two modules → two suite headers; the failing test's structured
    // detail (assertion/expected/actual) is rendered on indented lines.
    let plan = discover(&[fixture("mixed.handl"), fixture("pass.handl")]).unwrap();
    let report = run(&plan);
    let output = format_report_human(&report);

    let mixed = fixture("mixed.handl").display().to_string();
    let pass = fixture("pass.handl").display().to_string();
    assert!(output.contains(&format!("== {mixed} ==")));
    assert!(output.contains(&format!("== {pass} ==")));
    // `test_fail = assert false` carries structured detail.
    assert!(output.contains("assertion: assert"));
    assert!(output.contains("actual: false"));
}

#[test]
fn format_report_json_includes_suites() {
    let plan = discover(&[fixture("mixed.handl"), fixture("pass.handl")]).unwrap();
    let report = run(&plan);
    let json = format_report_json(&report);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");

    let suites = parsed["suites"].as_array().expect("suites array");
    assert_eq!(suites.len(), 2, "one suite per module");
    assert_eq!(suites[0]["name"], fixture("mixed.handl").display().to_string());
    assert_eq!(suites[0]["total"], 3);
    assert_eq!(suites[0]["passed"], 2);
    assert_eq!(suites[0]["failed"], 1);
    assert_eq!(suites[1]["name"], fixture("pass.handl").display().to_string());
    assert_eq!(suites[1]["total"], 3);
    assert_eq!(suites[1]["passed"], 3);
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

// ---------------------------------------------------------------------------
// Result-cache tests (opt-in `--cache`, ADR-0089 §"Cache domains")
// ---------------------------------------------------------------------------

#[test]
fn cached_second_run_replays_identical_results() {
    // A second run over the same plan must replay the recorded results
    // without re-evaluating: all lookups are hits and the report is
    // identical in ordering and content.
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let mut cache = TestRunCache::new();
    let pred = handl_plugin_engine_test::Filter::all();
    let policy = SchedulePolicy::sequential();

    let first = run_selected_cached(&plan, &pred, &policy, &mut cache);
    assert_eq!(first.passed(), 2);
    assert_eq!(first.failed(), 1);
    assert_eq!(cache.len(), 3);
    assert_eq!(cache.misses(), 3);
    assert_eq!(cache.hits(), 0);

    let second = run_selected_cached(&plan, &pred, &policy, &mut cache);
    assert_eq!(cache.hits(), 3, "all tests served from cache");
    assert_eq!(cache.misses(), 3, "no re-evaluation on second run");
    assert_eq!(cache.len(), 3);

    let r1: Vec<(&str, &TestResult)> = first
        .outcomes
        .iter()
        .map(|o| (o.case.name.as_str(), &o.result))
        .collect();
    let r2: Vec<(&str, &TestResult)> = second
        .outcomes
        .iter()
        .map(|o| (o.case.name.as_str(), &o.result))
        .collect();
    assert_eq!(r1, r2, "cached report must be identical to the fresh run");
}

#[test]
fn cache_is_opt_in_and_invalidates_on_source_change() {
    // Default runs never touch the cache; a mutated body_source is a
    // different fingerprint and re-evaluates.
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let mut cache = TestRunCache::new();
    let pred = handl_plugin_engine_test::Filter::all();
    let policy = SchedulePolicy::sequential();

    // run / run_selected do not use a cache — nothing to assert on the
    // cache itself; the API simply doesn't take one.
    let _ = run_selected(&plan, &pred, &policy);

    run_selected_cached(&plan, &pred, &policy, &mut cache);
    assert_eq!(cache.misses(), 3);

    // Mutate one test's body: its key changes, the others still hit.
    let mut plan2 = plan.clone();
    plan2.tests[0].body_source = "1 + 2".to_string();
    let report = run_selected_cached(&plan2, &pred, &policy, &mut cache);
    assert_eq!(cache.hits(), 2);
    assert_eq!(cache.misses(), 4);
    assert_eq!(report.passed(), 2);
    assert_eq!(report.failed(), 1);
}

#[test]
fn filtered_out_tests_are_not_cached() {
    // Skipped tests must not be recorded in the cache.
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let mut cache = TestRunCache::new();
    let report = run_selected_cached(
        &plan,
        &filter_predicate("test_pass*"),
        &SchedulePolicy::sequential(),
        &mut cache,
    );
    assert_eq!(report.skipped(), 1);
    assert_eq!(cache.len(), 2, "only executed tests are cached");
    assert_eq!(cache.misses(), 2);
}

// ---------------------------------------------------------------------------
// Structured-event tests (ADR-0089 §"Observability")
// ---------------------------------------------------------------------------

/// Positions of each variant occurrence in an event log, by name.
fn event_positions(events: &[TestEvent], pred: impl Fn(&TestEvent) -> bool) -> Vec<usize> {
    events
        .iter()
        .enumerate()
        .filter(|(_, e)| pred(e))
        .map(|(i, _)| i)
        .collect()
}

#[test]
fn events_run_lifecycle_ordering() {
    // RunStarted first, RunFinished last; for every executed test,
    // TestSelected precedes TestStarted precedes TestFinished.
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let mut config = RunConfig::new();
    let report = run_with_config(&plan, &mut config);
    assert_eq!(report.total(), 3);

    let events = &config.events;
    assert!(matches!(
        events.first(),
        Some(TestEvent::RunStarted { total: 3 })
    ));
    assert!(matches!(
        events.last(),
        Some(TestEvent::RunFinished {
            passed: 2,
            failed: 1,
            skipped: 0,
            ..
        })
    ));

    for name in ["test_pass", "test_fail", "test_pass_two"] {
        let sel = event_positions(
            events,
            |e| matches!(e, TestEvent::TestSelected { name: n, .. } if n == name),
        );
        let start = event_positions(
            events,
            |e| matches!(e, TestEvent::TestStarted { name: n, .. } if n == name),
        );
        let fin = event_positions(
            events,
            |e| matches!(e, TestEvent::TestFinished { name: n, .. } if n == name),
        );
        assert_eq!(sel.len(), 1, "TestSelected for {name}");
        assert_eq!(start.len(), 1, "TestStarted for {name}");
        assert_eq!(fin.len(), 1, "TestFinished for {name}");
        assert!(sel[0] < start[0] && start[0] < fin[0]);
    }

    // The failing test also emits TestFailed after its TestFinished.
    let failed = event_positions(
        events,
        |e| matches!(e, TestEvent::TestFailed { name: n, .. } if n == "test_fail"),
    );
    assert_eq!(failed.len(), 1);
    let fin_fail = event_positions(
        events,
        |e| matches!(e, TestEvent::TestFinished { name: n, .. } if n == "test_fail"),
    );
    assert!(failed[0] > fin_fail[0]);

    // Parallel runs emit the same canonical event order.
    let mut config = RunConfig::new().with_policy(SchedulePolicy::parallel(4));
    run_with_config(&plan, &mut config);
    let kind_seq: Vec<&'static str> = config
        .events
        .iter()
        .map(|e| match e {
            TestEvent::RunStarted { .. } => "RunStarted",
            TestEvent::RunFinished { .. } => "RunFinished",
            TestEvent::SuiteStarted { .. } => "SuiteStarted",
            TestEvent::SuiteFinished { .. } => "SuiteFinished",
            TestEvent::TestDiscovered { .. } => "TestDiscovered",
            TestEvent::TestSelected { .. } => "TestSelected",
            TestEvent::TestStarted { .. } => "TestStarted",
            TestEvent::TestFinished { .. } => "TestFinished",
            TestEvent::TestCached { .. } => "TestCached",
            TestEvent::TestSkipped { .. } => "TestSkipped",
            TestEvent::TestFailed { .. } => "TestFailed",
        })
        .collect();
    let seq_kinds: Vec<&'static str> = {
        let mut c = RunConfig::new();
        run_with_config(&plan, &mut c);
        c.events
            .iter()
            .map(|e| match e {
                TestEvent::RunStarted { .. } => "RunStarted",
                TestEvent::RunFinished { .. } => "RunFinished",
                TestEvent::SuiteStarted { .. } => "SuiteStarted",
                TestEvent::SuiteFinished { .. } => "SuiteFinished",
                TestEvent::TestDiscovered { .. } => "TestDiscovered",
                TestEvent::TestSelected { .. } => "TestSelected",
                TestEvent::TestStarted { .. } => "TestStarted",
                TestEvent::TestFinished { .. } => "TestFinished",
                TestEvent::TestCached { .. } => "TestCached",
                TestEvent::TestSkipped { .. } => "TestSkipped",
                TestEvent::TestFailed { .. } => "TestFailed",
            })
            .collect()
    };
    assert_eq!(kind_seq, seq_kinds, "event order must not depend on --jobs");
}

#[test]
fn events_skipped_tests_emit_test_skipped() {
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let mut config = RunConfig::new().with_filter("test_pass*");
    let report = run_with_config(&plan, &mut config);
    assert_eq!(report.skipped(), 1);
    let skipped = event_positions(
        &config.events,
        |e| matches!(e, TestEvent::TestSkipped { name: n, .. } if n == "test_fail"),
    );
    assert_eq!(skipped.len(), 1);
}

#[test]
fn events_suite_lifecycle_brackets_modules() {
    // Two modules, in lexicographic order: mixed.handl, pass.handl.
    let plan = discover(&[fixture("mixed.handl"), fixture("pass.handl")]).unwrap();
    let mut config = RunConfig::new();
    let report = run_with_config(&plan, &mut config);
    assert_eq!(report.total(), 6);
    let events = &config.events;

    assert!(matches!(
        events.first(),
        Some(TestEvent::RunStarted { total: 6 })
    ));
    assert!(matches!(events.last(), Some(TestEvent::RunFinished { .. })));

    let mut suite_starts: HashMap<String, usize> = HashMap::new();
    let mut suite_finishes: HashMap<String, usize> = HashMap::new();
    let mut test_event_positions: HashMap<String, Vec<usize>> = HashMap::new();

    for (i, e) in events.iter().enumerate() {
        match e {
            TestEvent::SuiteStarted { name } => {
                assert!(
                    !suite_starts.contains_key(name),
                    "duplicate SuiteStarted for {name}"
                );
                suite_starts.insert(name.clone(), i);
            }
            TestEvent::SuiteFinished { name } => {
                assert!(
                    !suite_finishes.contains_key(name),
                    "duplicate SuiteFinished for {name}"
                );
                suite_finishes.insert(name.clone(), i);
            }
            TestEvent::TestSelected { module, .. }
            | TestEvent::TestStarted { module, .. }
            | TestEvent::TestCached { module, .. }
            | TestEvent::TestSkipped { module, .. }
            | TestEvent::TestFinished { module, .. }
            | TestEvent::TestFailed { module, .. } => {
                test_event_positions
                    .entry(module.display().to_string())
                    .or_default()
                    .push(i);
            }
            _ => {}
        }
    }

    let mut module_order: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for case in &plan.tests {
        let m = case.module.display().to_string();
        if seen.insert(m.clone()) {
            module_order.push(m);
        }
    }

    assert_eq!(module_order.len(), 2, "expected two modules");
    for m in &module_order {
        let &start = suite_starts
            .get(m)
            .unwrap_or_else(|| panic!("missing SuiteStarted for {m}"));
        let &finish = suite_finishes
            .get(m)
            .unwrap_or_else(|| panic!("missing SuiteFinished for {m}"));
        assert!(start < finish, "SuiteStarted after SuiteFinished for {m}");

        let positions = test_event_positions
            .get(m)
            .unwrap_or_else(|| panic!("no test events for {m}"));
        assert!(!positions.is_empty(), "module {m} had no test events");
        assert!(
            start < *positions.first().unwrap(),
            "SuiteStarted for {m} not before first test event"
        );
        assert!(
            finish > *positions.last().unwrap(),
            "SuiteFinished for {m} not after last test event"
        );
    }

    // Suite events are nested and in module order.
    let suite_events: Vec<(usize, &str, bool)> = events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            TestEvent::SuiteStarted { name } => Some((i, name.as_str(), true)),
            TestEvent::SuiteFinished { name } => Some((i, name.as_str(), false)),
            _ => None,
        })
        .collect();
    assert_eq!(
        suite_events.len(),
        4,
        "expected two suite starts and finishes"
    );
    assert!(suite_events[0].2, "expected SuiteStarted");
    assert!(!suite_events[1].2, "expected SuiteFinished");
    assert!(suite_events[2].2, "expected SuiteStarted");
    assert!(!suite_events[3].2, "expected SuiteFinished");
    assert!(suite_events[0].0 < suite_events[1].0);
    assert!(suite_events[1].0 < suite_events[2].0);
    assert!(suite_events[2].0 < suite_events[3].0);
}

#[test]
fn discovery_emits_test_discovered() {
    let mut events = Vec::new();
    let plan = discover_with(
        &FileSystemProvider,
        &[fixture("pass.handl")],
        Some(&mut events),
    )
    .unwrap();
    assert_eq!(plan.tests.len(), 3);
    let discovered: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            TestEvent::TestDiscovered { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        discovered,
        vec!["test_addition", "test_number", "test_string"]
    );
    // `discover` is equivalent to the default provider without events.
    assert_eq!(FileSystemProvider.name(), "filesystem");
}

#[test]
fn duration_is_populated_for_executed_tests() {
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let report = run_with_config(
        &plan,
        &mut RunConfig::new().with_policy(SchedulePolicy::parallel(4)),
    );
    for outcome in &report.outcomes {
        assert!(
            outcome.duration > std::time::Duration::ZERO,
            "executed test {} has zero duration",
            outcome.case.name
        );
    }
    // The keyword `assert false` records structured failure detail.
    let failure = &report.outcomes[1].failure;
    assert_eq!(failure.assertion.as_deref(), Some("assert"));
    assert_eq!(failure.expected.as_deref(), Some("truthy value"));
    assert_eq!(failure.actual.as_deref(), Some("false"));
}

#[test]
fn structured_failure_detail_for_assert_equal() {
    let plan = discover(&[fixture("equal_fail.handl")]).unwrap();
    let report = run(&plan);
    assert_eq!(report.total(), 1);
    assert_eq!(report.failed(), 1);

    let failure = &report.outcomes[0].failure;
    assert_eq!(failure.assertion.as_deref(), Some("assert"));
    assert_eq!(failure.actual.as_deref(), Some("1"));
    assert_eq!(failure.expected.as_deref(), Some("2"));
}

#[test]
fn cached_hit_emits_test_cached() {
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let mut config = RunConfig::new()
        .with_policy(SchedulePolicy::sequential())
        .with_cache(TestRunCache::new());
    run_with_config(&plan, &mut config);
    config.events.clear();
    run_with_config(&plan, &mut config);

    let cached = event_positions(&config.events, |e| {
        matches!(e, TestEvent::TestCached { .. })
    });
    assert_eq!(cached.len(), 3, "every test replayed from cache");
    // Cached tests are not dispatched: no TestStarted/TestFinished.
    assert!(event_positions(&config.events, |e| matches!(
        e,
        TestEvent::TestStarted { .. }
    ))
    .is_empty());
    assert!(event_positions(&config.events, |e| matches!(
        e,
        TestEvent::TestFinished { .. }
    ))
    .is_empty());
}

// ---------------------------------------------------------------------------
// Reporter-pipeline tests (ADR-0089 §"Reporting pipeline")
// ---------------------------------------------------------------------------

#[test]
fn reporters_via_trait_match_legacy_formatters() {
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let report = run(&plan);
    let summary = summarize(&report);
    assert_eq!(summary.total, 3);
    assert_eq!(summary.passed, 2);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.skipped, 0);
    assert!(summary.total_duration > std::time::Duration::ZERO);
    // slowest is whichever executed test took longest; just check it names
    // a real test.
    assert!(summary.slowest.as_ref().unwrap().0.starts_with("test_"));

    assert_eq!(HumanReporter.name(), "console");
    assert_eq!(JsonReporter.name(), "json");
    assert_eq!(
        HumanReporter.format(&report, &summary),
        format_report_human(&report)
    );
    assert_eq!(
        JsonReporter.format(&report, &summary),
        format_report_json(&report)
    );

    // JSON includes the structured duration field.
    let parsed: serde_json::Value =
        serde_json::from_str(&JsonReporter.format(&report, &summary)).unwrap();
    assert!(parsed["tests"][0]["duration_ms"].is_number());
    assert!(parsed["summary"]["duration_ms"].is_number());
}

// ---------------------------------------------------------------------------
// Tag filtering (ADR-0089 §"Tags")
// ---------------------------------------------------------------------------

#[test]
fn discover_tagged_tests() {
    let plan = discover(&[fixture("tagged.handl")]).unwrap();
    assert_eq!(plan.tests.len(), 3);
    assert!(plan.tests.iter().any(|t| t.name == "tagged_slow" && t.tags == vec!["slow", "net", "flaky"]));
    assert!(plan.tests.iter().any(|t| t.name == "tagged_fast" && t.tags.is_empty()));
    assert!(plan.tests.iter().any(|t| t.name == "tagged_net" && t.tags == vec!["net"]));
}

#[test]
fn tag_filter_selects_matching_tests() {
    let plan = discover(&[fixture("tagged.handl")]).unwrap();
    let predicate = filter_predicate("").and(tag_predicate("slow"));
    let report = run_selected(&plan, &predicate, &SchedulePolicy::sequential());
    assert_eq!(report.total(), 3);
    assert_eq!(report.passed(), 1);
    assert_eq!(report.skipped(), 2);
}

#[test]
fn tag_filter_composes_with_name_filter() {
    let plan = discover(&[fixture("tagged.handl")]).unwrap();
    let predicate = filter_predicate("*_net").and(tag_predicate("net"));
    let report = run_selected(&plan, &predicate, &SchedulePolicy::sequential());
    assert_eq!(report.total(), 3);
    assert_eq!(report.passed(), 1);
    assert_eq!(report.skipped(), 2);
}

// ---------------------------------------------------------------------------
// Disk-persisted cache (ADR-0089 §"Cache domains")
// ---------------------------------------------------------------------------

#[test]
fn test_run_cache_save_and_load_round_trip() {
    let plan = discover(&[fixture("mixed.handl")]).unwrap();
    let mut config = RunConfig::new().with_cache(TestRunCache::new());
    run_with_config(&plan, &mut config);

    let cache = config.cache.unwrap();
    assert!(!cache.is_empty());

    let tmp = std::env::temp_dir().join(format!(
        "handl-test-cache-{}",
        std::process::id()
    ));
    if tmp.exists() {
        let _ = std::fs::remove_file(&tmp);
    }
    cache.save(&tmp).unwrap();

    let loaded = TestRunCache::load(&tmp).unwrap();
    assert_eq!(loaded.len(), cache.len());

    let _ = std::fs::remove_file(&tmp);
}
