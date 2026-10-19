//! The `test` capability plugin.
//!
//! ## Payload contract (capability `test`)
//!
//! ```json
//! { "paths": ["tests"], "filter": "name", "tags": ["slow"],
//!   "list": false, "changed": false, "jobs": 4, "report": "human",
//!   "cache": false }
//! ```
//!
//! - `paths` (required, array of strings) — roots to discover under. The
//!   caller resolves defaults (`tests/` or `.`); the capability applies none.
//! - `filter` (optional, string) — substring / pattern test-name filter.
//! - `tags` (optional, array of strings) — tag expressions, ANDed together.
//! - `list` (optional, bool, default `false`) — discover and return the case
//!   list instead of running.
//! - `changed` (optional, bool, default `false`) — discover via the git
//!   changed-files provider instead of the filesystem provider.
//! - `jobs` (optional, number or `"auto"`) — parallelism policy. Omitted =
//!   the runner default.
//! - `report` (optional, `"human"` | `"json"`, default `"human"`) — how to
//!   render the run report returned in `report`.
//! - `cache` (optional, bool, default `false`) — use the ADR-0089 on-disk
//!   result cache.
//!
//! ## Response
//!
//! - `{ "status": "list", "cases": [ { "name", "module" } ], "parseErrors": [ … ] }`
//! - `{ "status": "ok" | "failed", "summary": { "total", "passed", "failed",
//!   "skipped" }, "report": "<rendered>", "parseErrors": [ … ] }`
//!
//! `parseErrors` is `[ { "file": "<path>", "message": "<why>" } ]` — modules
//! that failed to parse during discovery. A non-empty `parseErrors` is a
//! failure regardless of `status`, matching `hnl test`'s exit code.

use std::path::PathBuf;

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, PlatformError, Plugin,
    PluginContext,
};
use hnl_engine_test::{self as engine, TestKitSource};

/// Capability id served by [`TestPlugin`].
pub const TEST_CAPABILITY: &str = "test";

/// Adapts the `hnl test` runner to the capability contract.
///
/// Holds the injected [`TestKitSource`] base-runtime factory (ADR-0128
/// §"The 0.8 seam"): `Plugin: Send + Sync` forbids holding a `Kit`, and a
/// `Runtime` cannot cross threads, so the host supplies a `Send + Sync`
/// factory the runner invokes on whichever thread needs it.
pub struct TestPlugin {
    capabilities: Vec<CapabilityId>,
    source: TestKitSource,
}

impl TestPlugin {
    /// Create the plugin over the injected base-runtime factory,
    /// declaring the `test` capability.
    pub fn new(source: TestKitSource) -> Self {
        Self {
            capabilities: vec![CapabilityId::from(TEST_CAPABILITY)],
            source,
        }
    }
}

impl Plugin for TestPlugin {
    fn id(&self) -> &str {
        "ext.test"
    }

    fn name(&self) -> &str {
        "Test runner"
    }

    fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    fn metadata(&self, capability: &CapabilityId) -> Option<CapabilityMetadata> {
        if capability.as_str() != TEST_CAPABILITY {
            return None;
        }
        Some(serde_json::json!({
            "id": TEST_CAPABILITY,
            "description": "Discover and run handl test modules (ADR-0089)",
        }))
    }

    fn run(
        &self,
        request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        if request.capability.as_str() != TEST_CAPABILITY {
            return Err(PlatformError::UnknownCapability(request.capability.clone()));
        }
        run_test(&self.source, request)
    }
}

/// The ADR-0089 on-disk result cache location, relative to the CWD — the same
/// path `hnl test --cache` uses.
const CACHE_PATH: &str = ".handl/cache/test-runner/cache.json";

/// Discover, then either list the cases or run them.
fn run_test(
    source: &TestKitSource,
    request: &CapabilityRequest,
) -> Result<CapabilityResponse, PlatformError> {
    let payload = &request.payload;
    let paths = parse_paths(payload)?;
    let filter = payload.get("filter").and_then(|v| v.as_str()).unwrap_or("");
    let tags = parse_string_array(payload, "tags")?;
    let list = payload
        .get("list")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let changed = payload
        .get("changed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let use_cache = payload
        .get("cache")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let report_kind = payload
        .get("report")
        .and_then(|v| v.as_str())
        .unwrap_or("human");

    let plan = if changed {
        engine::discover_with(&engine::ChangedProvider, source, &paths, None)
    } else {
        engine::discover(source, &paths)
    }
    .map_err(|e| PlatformError::Plugin(format!("test: discovery error: {e}")))?;

    let parse_errors = parse_errors_json(&plan);

    // `--filter` AND every `--tag` expression, matching the CLI's composition.
    let mut predicate = engine::filter_predicate(filter);
    for expr in &tags {
        predicate = predicate.and(engine::tag_expr_predicate(expr));
    }

    if list {
        let cases: Vec<serde_json::Value> = plan
            .tests
            .iter()
            .filter(|case| predicate.eval(case))
            .map(|case| {
                serde_json::json!({
                    "name": case.name,
                    "module": case.module.display().to_string(),
                    "tags": case.tags,
                })
            })
            .collect();
        return Ok(CapabilityResponse(serde_json::json!({
            "status": "list",
            "cases": cases,
            "parseErrors": parse_errors,
        })));
    }

    let mut warnings: Vec<String> = Vec::new();
    let mut config = engine::RunConfig::new();
    config.predicate = predicate;
    if let Some(policy) = parse_jobs(payload)? {
        config = config.with_policy(policy);
    }
    if use_cache {
        let cache_path = std::path::Path::new(CACHE_PATH);
        let cache = if cache_path.exists() {
            match engine::TestRunCache::load(cache_path) {
                Ok(c) => c,
                Err(e) => {
                    warnings.push(format!("failed to load cache at {CACHE_PATH}: {e}"));
                    engine::TestRunCache::new()
                }
            }
        } else {
            engine::TestRunCache::new()
        };
        config = config.with_cache(cache);
    }

    let report = engine::run_with_config(source, &plan, &mut config);

    // Save the cache only if one was loaded and the run produced a cache to
    // persist (ADR-0089). `cache` is only set when the caller asked for it.
    if use_cache
        && let Some(cache) = config.cache.as_ref()
        && let Err(e) = cache.save(std::path::Path::new(CACHE_PATH))
    {
        warnings.push(format!("failed to save cache at {CACHE_PATH}: {e}"));
    }

    let summary = engine::summarize(&report);
    let rendered = match report_kind {
        "human" | "console" => <engine::HumanReporter as engine::Reporter>::format(
            &engine::HumanReporter,
            &report,
            &summary,
        ),
        "json" => <engine::JsonReporter as engine::Reporter>::format(
            &engine::JsonReporter,
            &report,
            &summary,
        ),
        other => {
            return Err(PlatformError::InvalidMetadata(format!(
                "test: unknown report format `{other}`"
            )));
        }
    };

    let status = if report.has_failures() {
        "failed"
    } else {
        "ok"
    };
    Ok(CapabilityResponse(serde_json::json!({
        "status": status,
        "summary": {
            "total": summary.total,
            "passed": summary.passed,
            "failed": summary.failed,
            "skipped": summary.skipped,
        },
        "report": rendered,
        "parseErrors": parse_errors,
        "warnings": warnings,
    })))
}

/// Read the required `paths` array of strings.
fn parse_paths(payload: &serde_json::Value) -> Result<Vec<PathBuf>, PlatformError> {
    let raw = payload.get("paths").ok_or_else(|| {
        PlatformError::InvalidMetadata("test: missing array field `paths`".into())
    })?;
    let items = raw.as_array().ok_or_else(|| {
        PlatformError::InvalidMetadata("test: `paths` must be an array of strings".into())
    })?;
    let paths: Vec<PathBuf> = items
        .iter()
        .filter_map(|v| v.as_str().map(PathBuf::from))
        .collect();
    if paths.is_empty() {
        // An empty list would silently discover nothing; require the caller to
        // resolve its own defaults (`tests/`, `.`) up front.
        return Err(PlatformError::InvalidMetadata(
            "test: `paths` must not be empty".into(),
        ));
    }
    Ok(paths)
}

/// Read an optional array-of-strings field.
fn parse_string_array(
    payload: &serde_json::Value,
    field: &str,
) -> Result<Vec<String>, PlatformError> {
    match payload.get(field) {
        None | Some(serde_json::Value::Null) => Ok(Vec::new()),
        Some(serde_json::Value::Array(items)) => Ok(items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()),
        Some(_) => Err(PlatformError::InvalidMetadata(format!(
            "test: `{field}` must be an array of strings"
        ))),
    }
}

/// Translate the discovery parse errors into JSON.
fn parse_errors_json(plan: &engine::TestPlan) -> Vec<serde_json::Value> {
    plan.parse_errors
        .iter()
        .map(|(file, message)| {
            serde_json::json!({ "file": file.display().to_string(), "message": message })
        })
        .collect()
}

/// Parse the optional `jobs` policy: a positive integer, or `"auto"`.
fn parse_jobs(
    payload: &serde_json::Value,
) -> Result<Option<handl_plugin_lib_scheduler::SchedulePolicy>, PlatformError> {
    let raw = match payload.get("jobs") {
        None | Some(serde_json::Value::Null) => return Ok(None),
        Some(v) => v,
    };
    if let Some(name) = raw.as_str() {
        if name == "auto" {
            return Ok(Some(handl_plugin_lib_scheduler::SchedulePolicy::default()));
        }
        return Err(PlatformError::InvalidMetadata(format!(
            "test: `jobs` must be a positive integer or `auto`, got `{name}`"
        )));
    }
    if let Some(n) = raw.as_u64() {
        let n = usize::try_from(n)
            .map_err(|_| PlatformError::InvalidMetadata("test: `jobs` is out of range".into()))?;
        if n == 0 {
            return Err(PlatformError::InvalidMetadata(
                "test: `jobs` must be a positive integer or `auto`".into(),
            ));
        }
        return Ok(Some(handl_plugin_lib_scheduler::SchedulePolicy::parallel(
            n,
        )));
    }
    Err(PlatformError::InvalidMetadata(
        "test: `jobs` must be a positive integer or `auto`".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    use handl_platform::{KitRegistry, PluginRegistry};

    thread_local! {
        /// Thread-cached canonical kit — the host-side cache shape the
        /// injected factory mirrors (ADR-0128 §0.8; the kit is a dev-dep).
        static TEST_KIT: RefCell<Option<handl_kit_prelude::Kit>> =
            const { RefCell::new(None) };
    }

    /// The canonical base-runtime factory: capture-less over the
    /// thread-local kit, so it is `Send + Sync`.
    fn canonical_source() -> TestKitSource {
        std::sync::Arc::new(|| {
            TEST_KIT.with(|cell| {
                let mut slot = cell.borrow_mut();
                if slot.is_none() {
                    *slot = Some(
                        handl_kit_prelude::Kit::new().expect("canonical kit assembles"),
                    );
                }
                let kit = slot.as_ref().expect("kit populated above");
                Ok::<_, String>(engine::base_runtime(
                    kit.runtime(),
                    kit.module_root().map(std::path::Path::to_path_buf),
                ))
            })
        })
    }

    /// A plugin wired with the canonical source.
    fn plugin() -> TestPlugin {
        TestPlugin::new(canonical_source())
    }

    fn with_ctx<R>(f: impl FnOnce(&mut PluginContext) -> R) -> R {
        let mut plugins = PluginRegistry::new();
        let mut kits = KitRegistry::new();
        let mut ctx = PluginContext {
            plugins: &mut plugins,
            kits: &mut kits,
        };
        f(&mut ctx)
    }

    fn req(payload: serde_json::Value) -> CapabilityRequest {
        CapabilityRequest {
            capability: CapabilityId::from(TEST_CAPABILITY),
            payload,
        }
    }

    /// An empty directory, so discovery finds no tests.
    fn empty_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hnl-ext-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn declares_the_test_capability() {
        let plugin = plugin();
        assert_eq!(plugin.id(), "ext.test");
        assert_eq!(plugin.capabilities().len(), 1);
        assert_eq!(plugin.capabilities()[0].as_str(), "test");
        assert!(plugin.metadata(&CapabilityId::from("test")).is_some());
        assert!(plugin.metadata(&CapabilityId::from("lint")).is_none());
    }

    #[test]
    fn a_wrong_capability_is_unknown() {
        let plugin = plugin();
        let err = with_ctx(|ctx| {
            plugin.run(
                &CapabilityRequest {
                    capability: CapabilityId::from("lint"),
                    payload: serde_json::json!({ "paths": ["."] }),
                },
                ctx,
            )
        })
        .expect_err("a mismatched capability must fail");
        assert!(
            matches!(err, PlatformError::UnknownCapability(_)),
            "got {err:?}"
        );
    }

    #[test]
    fn missing_paths_is_invalid_metadata() {
        let plugin = plugin();
        let err = with_ctx(|ctx| plugin.run(&req(serde_json::json!({})), ctx))
            .expect_err("a payload without `paths` must fail");
        assert!(
            matches!(err, PlatformError::InvalidMetadata(_)),
            "got {err:?}"
        );
    }

    #[test]
    fn empty_paths_is_invalid_metadata() {
        let plugin = plugin();
        let err = with_ctx(|ctx| plugin.run(&req(serde_json::json!({ "paths": [] })), ctx))
            .expect_err("an empty `paths` must fail rather than silently no-op");
        assert!(
            matches!(err, PlatformError::InvalidMetadata(_)),
            "got {err:?}"
        );
    }

    #[test]
    fn a_non_array_tags_is_invalid_metadata() {
        let plugin = plugin();
        let err = with_ctx(|ctx| {
            plugin.run(
                &req(serde_json::json!({ "paths": ["."], "tags": "slow" })),
                ctx,
            )
        })
        .expect_err("a string `tags` must fail");
        assert!(
            matches!(err, PlatformError::InvalidMetadata(_)),
            "got {err:?}"
        );
    }

    #[test]
    fn list_mode_on_an_empty_tree_returns_an_empty_case_list() {
        let plugin = plugin();
        let dir = empty_dir();
        let res = with_ctx(|ctx| {
            plugin
                .run(
                    &req(serde_json::json!({
                        "paths": [dir.display().to_string()],
                        "list": true,
                    })),
                    ctx,
                )
                .expect("list runs")
        });
        assert_eq!(res.0.get("status").and_then(|v| v.as_str()), Some("list"));
        let cases = res
            .0
            .get("cases")
            .and_then(|v| v.as_array())
            .expect("cases");
        assert!(cases.is_empty(), "expected no cases, got {}", res.0);
    }

    #[test]
    fn an_unknown_report_format_is_invalid_metadata() {
        let plugin = plugin();
        let dir = empty_dir();
        let err = with_ctx(|ctx| {
            plugin.run(
                &req(serde_json::json!({
                    "paths": [dir.display().to_string()],
                    "report": "tap",
                })),
                ctx,
            )
        })
        .expect_err("an unknown report format must fail");
        assert!(
            matches!(err, PlatformError::InvalidMetadata(_)),
            "got {err:?}"
        );
    }

    #[test]
    fn jobs_accepts_auto_and_positive_integers() {
        assert!(
            parse_jobs(&serde_json::json!({}))
                .expect("absent ok")
                .is_none()
        );
        assert!(
            parse_jobs(&serde_json::json!({ "jobs": "auto" }))
                .expect("auto ok")
                .is_some()
        );
        assert!(
            parse_jobs(&serde_json::json!({ "jobs": 4 }))
                .expect("4 ok")
                .is_some()
        );
    }

    #[test]
    fn jobs_rejects_zero_and_non_numeric_strings() {
        for bad in [
            serde_json::json!({ "jobs": 0 }),
            serde_json::json!({ "jobs": "fast" }),
        ] {
            let err = parse_jobs(&bad).expect_err("must fail");
            assert!(
                matches!(err, PlatformError::InvalidMetadata(_)),
                "got {err:?}"
            );
        }
    }
    // ── Default / name ───────────────────────────────────────────────

    #[test]
    fn name_is_reported() {
        let plugin = plugin();
        assert_eq!(plugin.name(), "Test runner");
        assert_eq!(plugin.capabilities().len(), 1);
    }

    #[test]
    fn a_non_array_paths_is_invalid_metadata() {
        let plugin = plugin();
        let err = with_ctx(|ctx| plugin.run(&req(serde_json::json!({ "paths": "tests" })), ctx))
            .expect_err("a string `paths` must fail");
        assert!(
            matches!(err, PlatformError::InvalidMetadata(_)),
            "got {err:?}"
        );
    }

    #[test]
    fn jobs_rejects_fractional_and_non_numeric_values() {
        for bad in [
            serde_json::json!({ "jobs": 1.5 }),
            serde_json::json!({ "jobs": [] }),
        ] {
            let err = parse_jobs(&bad).expect_err("must fail");
            assert!(
                matches!(err, PlatformError::InvalidMetadata(_)),
                "got {err:?}"
            );
        }
    }

    // ── Project-driven runs ──────────────────────────────────────────

    /// A scratch project with one subdirectory per scenario, so a run picks up
    /// exactly the modules it means to.
    struct Project {
        root: PathBuf,
    }

    impl Project {
        fn new(tag: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "hnl-ext-test-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            for (dir, body) in [
                (
                    "pass",
                    "-- A module with passing tests.\ntest_addition :: 1 + 1\n",
                ),
                (
                    "fail",
                    "-- A module with a failing test.\ntest_fail = assert false\n",
                ),
                ("bad", "-- A module with a syntax error.\ntest_bad = 1 +\n"),
            ] {
                let d = root.join(dir);
                std::fs::create_dir_all(&d).expect("scenario dir");
                std::fs::write(d.join("case.handl"), body).expect("scenario module");
            }
            Self { root }
        }

        /// Payload `paths` for one scenario directory.
        fn paths(&self, scenario: &str) -> serde_json::Value {
            let d = self.root.join(scenario);
            serde_json::json!([d.display().to_string()])
        }
    }

    impl Drop for Project {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn run_payload(payload: serde_json::Value) -> serde_json::Value {
        let plugin = plugin();
        with_ctx(|ctx| plugin.run(&req(payload), ctx).expect("test runs").0)
    }

    fn status_of(response: &serde_json::Value) -> &str {
        response
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("no status in {response}"))
    }

    #[test]
    fn an_explicit_null_tags_is_treated_as_absent() {
        let project = Project::new("nulltags");
        let res = run_payload(serde_json::json!({
            "paths": project.paths("pass"),
            "list": true,
            "tags": serde_json::Value::Null,
        }));

        assert_eq!(status_of(&res), "list");
        assert_eq!(
            res["cases"].as_array().map(Vec::len),
            Some(1),
            "a null `tags` must not filter anything out, got {res}"
        );
    }

    #[test]
    fn a_filter_narrows_the_case_list() {
        let project = Project::new("filter");
        let res = run_payload(serde_json::json!({
            "paths": project.paths("pass"),
            "list": true,
            "filter": "no_such_case_name",
        }));

        assert_eq!(status_of(&res), "list");
        assert_eq!(res["cases"], serde_json::json!([]));
    }

    #[test]
    fn a_passing_project_reports_ok_with_a_summary_and_a_human_report() {
        let project = Project::new("pass");
        let res = run_payload(serde_json::json!({ "paths": project.paths("pass") }));

        assert_eq!(status_of(&res), "ok");
        assert_eq!(res["summary"]["total"], 1);
        assert_eq!(res["summary"]["failed"], 0);
        assert!(
            res["report"].as_str().is_some_and(|r| !r.is_empty()),
            "the human report must be rendered, got {res}"
        );
        assert_eq!(res["warnings"], serde_json::json!([]));
        assert_eq!(res["parseErrors"], serde_json::json!([]));
    }

    #[test]
    fn a_failing_project_reports_failed() {
        let project = Project::new("fail");
        let res = run_payload(serde_json::json!({ "paths": project.paths("fail") }));

        assert_eq!(status_of(&res), "failed");
        assert_eq!(res["summary"]["failed"], 1);
    }

    #[test]
    fn the_json_reporter_renders_parseable_json() {
        let project = Project::new("json");
        let res = run_payload(serde_json::json!({
            "paths": project.paths("pass"),
            "report": "json",
        }));

        let rendered = res["report"].as_str().expect("json report is text");
        let parsed: serde_json::Value =
            serde_json::from_str(rendered).expect("the json report must be valid JSON");
        assert!(parsed.is_object(), "got {parsed}");
    }

    #[test]
    fn parse_errors_are_surfaced_in_the_response() {
        let project = Project::new("bad");
        let res = run_payload(serde_json::json!({ "paths": project.paths("bad") }));

        let errors = res["parseErrors"].as_array().expect("parseErrors array");
        assert!(
            !errors.is_empty(),
            "the unparseable module must be reported"
        );
        assert!(errors[0].get("file").is_some(), "got {errors:?}");
        assert!(errors[0].get("message").is_some(), "got {errors:?}");
    }

    #[test]
    fn list_mode_reports_the_discovered_cases() {
        let project = Project::new("list");
        let res = run_payload(serde_json::json!({
            "paths": project.paths("pass"),
            "list": true,
        }));

        assert_eq!(status_of(&res), "list");
        let cases = res["cases"].as_array().expect("cases array");
        assert_eq!(cases.len(), 1, "got {cases:?}");
        assert!(cases[0].get("name").is_some());
        assert!(
            cases[0]
                .get("module")
                .and_then(|m| m.as_str())
                .is_some_and(|m| m.ends_with("case.handl")),
            "got {cases:?}"
        );
        assert!(cases[0].get("tags").is_some());
    }

    #[test]
    fn a_tag_expression_narrows_the_case_list() {
        let project = Project::new("tags");
        // No case in the project declares a tag, so a tag expression matches
        // nothing — `--filter`/`--tag` are ANDed.
        let res = run_payload(serde_json::json!({
            "paths": project.paths("pass"),
            "list": true,
            "tags": ["slow"],
        }));

        assert_eq!(status_of(&res), "list");
        assert_eq!(res["cases"], serde_json::json!([]));
    }

    #[test]
    fn changed_discovery_degrades_gracefully_outside_a_repository() {
        let project = Project::new("changed");
        // A scratch dir is not a git work tree, so the changed-files provider
        // finds nothing; the run must still succeed.
        let res = run_payload(serde_json::json!({
            "paths": project.paths("pass"),
            "changed": true,
        }));

        assert!(matches!(status_of(&res), "ok" | "failed"), "got {res}");
    }

    #[test]
    fn an_explicit_jobs_policy_is_applied() {
        let project = Project::new("jobs");
        let res = run_payload(serde_json::json!({
            "paths": project.paths("pass"),
            "jobs": 2,
        }));

        assert_eq!(status_of(&res), "ok");
        assert_eq!(res["summary"]["total"], 1);
    }

    /// `CACHE_PATH` is relative to the process CWD, so every cache test shares
    /// one path. Serialize them instead of letting them race.
    fn cache_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn the_result_cache_round_trips() {
        let _guard = cache_lock();
        let project = Project::new("cache");
        let payload = serde_json::json!({
            "paths": project.paths("pass"),
            "cache": true,
        });

        // First run creates the cache, second run loads and re-saves it.
        for _ in 0..2 {
            let res = run_payload(payload.clone());
            assert_eq!(status_of(&res), "ok");
            assert_eq!(
                res["warnings"],
                serde_json::json!([]),
                "the cache must load and save cleanly, got {res}"
            );
        }

        let cache = std::path::Path::new(CACHE_PATH);
        assert!(cache.exists(), "the run must persist {CACHE_PATH}");
        let _ = std::fs::remove_file(cache);
    }

    #[test]
    fn a_corrupt_cache_is_reported_and_ignored() {
        let _guard = cache_lock();
        let project = Project::new("badcache");
        let cache = std::path::Path::new(CACHE_PATH);
        if let Some(parent) = cache.parent() {
            std::fs::create_dir_all(parent).expect("cache dir");
        }
        std::fs::write(cache, "{ not json").expect("write a corrupt cache");

        let res = run_payload(serde_json::json!({
            "paths": project.paths("pass"),
            "cache": true,
        }));

        assert_eq!(
            status_of(&res),
            "ok",
            "a corrupt cache must not fail the run"
        );
        let warnings = res["warnings"].as_array().expect("warnings array");
        assert!(
            warnings.iter().any(|w| w
                .as_str()
                .is_some_and(|s| s.contains("failed to load cache"))),
            "expected a load warning, got {res}"
        );
        let _ = std::fs::remove_file(cache);
    }

    #[test]
    fn an_unwritable_cache_location_is_reported_as_a_warning() {
        let _guard = cache_lock();
        let project = Project::new("nosave");
        let cache = std::path::Path::new(CACHE_PATH);
        // A *directory* where the cache file belongs makes both the load and
        // the save fail without touching permissions, so the test behaves the
        // same for root and non-root runners.
        let _ = std::fs::remove_file(cache);
        std::fs::create_dir_all(cache).expect("occupy the cache path");

        let res = run_payload(serde_json::json!({
            "paths": project.paths("pass"),
            "cache": true,
        }));

        assert_eq!(
            status_of(&res),
            "ok",
            "a cache failure must not fail the run"
        );
        let warnings = res["warnings"].as_array().expect("warnings array");
        assert!(
            warnings.iter().any(|w| w
                .as_str()
                .is_some_and(|s| s.contains("failed to save cache"))),
            "expected a save warning, got {res}"
        );

        let _ = std::fs::remove_dir_all(cache);
    }
}
