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
use handl_plugin_engine_test as engine;

/// Capability id served by [`TestPlugin`].
pub const TEST_CAPABILITY: &str = "test";

/// Adapts the `hnl test` runner to the capability contract.
pub struct TestPlugin {
    capabilities: Vec<CapabilityId>,
}

impl TestPlugin {
    /// Create the plugin, declaring the `test` capability.
    pub fn new() -> Self {
        Self {
            capabilities: vec![CapabilityId::from(TEST_CAPABILITY)],
        }
    }
}

impl Default for TestPlugin {
    fn default() -> Self {
        Self::new()
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
        run_test(request)
    }
}

/// The ADR-0089 on-disk result cache location, relative to the CWD — the same
/// path `hnl test --cache` uses.
const CACHE_PATH: &str = ".handl/cache/test-runner/cache.json";

/// Discover, then either list the cases or run them.
fn run_test(request: &CapabilityRequest) -> Result<CapabilityResponse, PlatformError> {
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
        engine::discover_with(&engine::ChangedProvider, &paths, None)
    } else {
        engine::discover(&paths)
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

    let report = engine::run_with_config(&plan, &mut config);

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
    use handl_platform::{KitRegistry, PluginRegistry};

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
        let plugin = TestPlugin::new();
        assert_eq!(plugin.id(), "ext.test");
        assert_eq!(plugin.capabilities().len(), 1);
        assert_eq!(plugin.capabilities()[0].as_str(), "test");
        assert!(plugin.metadata(&CapabilityId::from("test")).is_some());
        assert!(plugin.metadata(&CapabilityId::from("lint")).is_none());
    }

    #[test]
    fn a_wrong_capability_is_unknown() {
        let plugin = TestPlugin::new();
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
        let plugin = TestPlugin::new();
        let err = with_ctx(|ctx| plugin.run(&req(serde_json::json!({})), ctx))
            .expect_err("a payload without `paths` must fail");
        assert!(
            matches!(err, PlatformError::InvalidMetadata(_)),
            "got {err:?}"
        );
    }

    #[test]
    fn empty_paths_is_invalid_metadata() {
        let plugin = TestPlugin::new();
        let err = with_ctx(|ctx| plugin.run(&req(serde_json::json!({ "paths": [] })), ctx))
            .expect_err("an empty `paths` must fail rather than silently no-op");
        assert!(
            matches!(err, PlatformError::InvalidMetadata(_)),
            "got {err:?}"
        );
    }

    #[test]
    fn a_non_array_tags_is_invalid_metadata() {
        let plugin = TestPlugin::new();
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
        let plugin = TestPlugin::new();
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
        let plugin = TestPlugin::new();
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
}
