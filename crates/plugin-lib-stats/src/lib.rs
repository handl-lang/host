//! # handl-lib-stats
//!
//! Statistical aggregate functions for the handl runtime.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The stats interface is declared in
//! `handl-packages/stats/stats.hnl` via `---` doc comments, `cap`
//! declarations, and `op` declarations. The [`StatsLibraryPlugin`] wraps a
//! generic [`HandlLibraryPlugin`] for the interface declaration (id, name,
//! capabilities) while contributing the [`StatsModule`] into the
//! [`ModuleRuntime`].
//!
//! The stats *logic* is handl-authored in `stats.hnl` (a record of arrows:
//! `sum`, `avg`, `product`, `min`, `max`, `median`, `stddev`); this crate
//! loads and bridges that definition via the generic
//! [`handl_plugin_lib_handl`] loader. The `Module`/`Function` trait
//! scaffolding is retained as the registration boundary.
//!
//! Non-number handling: the prior Rust impl silently skipped non-numeric
//! elements via `filter_map(|v| v.as_f64())`. The handl port reproduces this
//! filtering using `typeof.type_of(x) == "num"` (requires `TypeModule` in the
//! bridge runtime). Non-numbers are dropped before aggregation.

use handl_platform::{
    CapabilityId, CapabilityMetadata, CapabilityRequest, CapabilityResponse, ContributionContext,
    EvalContext, EvalError, Function, Module, PlatformError, Plugin, PluginContext,
};
use handl_plugin_lib_handl::HandlLibraryPlugin;
use handl_plugin_lib_utils::{MathModule, TypeModule};
use handl_plugin_module::{ModuleRule, ModuleRuntime};
use serde_json::Value;
use std::sync::OnceLock;

/// The handl source declaring the stats interface and logic — the single
/// source of truth for the stats API surface. Loaded at compile time from
/// the kit-stdlib data dir.
///
/// The `.hnl` file contains `---` doc comments, `cap` declarations, `op`
/// declarations, and the handl logic (a record of arrows). The `cap`/`op`
/// declaration lines are stripped before evaluation (see
/// [`strip_declarations`]).
const STATS_HNL: &str = include_str!("../../../handl-packages/stats/stats.hnl");

/// The function (record field) names exposed by the stats module.
const STATS_FUNCS: &[&str] = &["sum", "avg", "product", "min", "max", "median", "stddev"];

pub mod module;
pub use module::StatsModule;

// ---------------------------------------------------------------------------
// Declaration stripping
// ---------------------------------------------------------------------------

/// Strip `cap`/`op`/`handl` declaration lines and comment lines (`-- ...`)
/// from the source, leaving only the handl logic (the record of arrows).
///
/// This is necessary because `stats.hnl` now contains `cap` and `op`
/// declarations (ADR-0077 Phase 4) alongside the handl logic. The
/// [`HandlModule`] evaluates the source as a single handl expression, so
/// declaration lines (which are not valid handl expressions) must be removed
/// first.
fn strip_declarations(src: &str) -> String {
    src.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("--") {
                return false;
            }
            // Strip indented continuation lines from `op` declarations
            // (e.g. `guarantee:`, `requires:`, `termination:`).
            if line.starts_with(' ') || line.starts_with('\t') {
                if trimmed.starts_with("guarantee")
                    || trimmed.starts_with("requires")
                    || trimmed.starts_with("termination")
                {
                    return false;
                }
            }
            // Strip `cap "..."`, `op "..."`, and `handl "..." = ...` lines.
            for kw in ["cap", "op", "handl"] {
                if let Some(rest) = trimmed.strip_prefix(kw) {
                    if rest.is_empty()
                        || rest.starts_with(' ')
                        || rest.starts_with('\t')
                        || rest.starts_with('"')
                    {
                        return false;
                    }
                }
            }
            true
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Build the [`handl_plugin_lib_handl::HandlModule`] for stats, with
/// `MathModule` (for `math.sqrt` in `stddev`) and `TypeModule` (for
/// `typeof.type_of` non-number filtering) as dependencies.
///
/// The `cap`/`op` declaration lines are stripped from the source before
/// passing to the [`HandlModule`], since the module evaluates the source as
/// a handl expression.
fn build_stats_module() -> handl_plugin_lib_handl::HandlModule {
    let clean_source = strip_declarations(STATS_HNL);
    handl_plugin_lib_handl::HandlModule::new("stats", &clean_source, STATS_FUNCS)
        .with_dependency(MathModule)
        .with_dependency(TypeModule)
}

/// A thread-safe lazy-initialized [`handl_plugin_lib_handl::HandlModule`] for
/// stats.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by the `HandlModule`.
static STATS_MODULE: OnceLock<handl_plugin_lib_handl::HandlModule> = OnceLock::new();

/// Get the shared [`handl_plugin_lib_handl::HandlModule`] for stats,
/// initializing it on first call.
fn stats_module() -> &'static handl_plugin_lib_handl::HandlModule {
    STATS_MODULE.get_or_init(build_stats_module)
}

/// Require exactly one array argument, returning it as a slice.
fn require_one_array(args: &[Value]) -> Result<&Value, EvalError> {
    if args.len() != 1 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1,
            args.len()
        )));
    }
    let arg = &args[0];
    if !arg.is_array() {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "array", "non-array"
        )));
    }
    Ok(arg)
}

/// `stats.sum(xs)` — sum of the numeric elements of `xs`.
///
/// The logic is handl-authored in `stats.hnl`; this impl only validates
/// arguments and delegates to the generic `HandlModule` loader.
#[derive(Clone, Copy)]
pub struct Sum;
impl Function for Sum {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        let func = stats_module().function("sum").expect("sum should exist");
        func.call(std::slice::from_ref(input), &mut EvalContext::new())
    }
}

/// `stats.avg(xs)` — arithmetic mean of the numeric elements of `xs`.
///
/// The logic is handl-authored in `stats.hnl`; this impl only validates
/// arguments and delegates to the generic `HandlModule` loader.
#[derive(Clone, Copy)]
pub struct Avg;
impl Function for Avg {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        let func = stats_module().function("avg").expect("avg should exist");
        func.call(std::slice::from_ref(input), &mut EvalContext::new())
    }
}

/// `stats.product(xs)` — product of the numeric elements of `xs`.
///
/// The logic is handl-authored in `stats.hnl`; this impl only validates
/// arguments and delegates to the generic `HandlModule` loader.
#[derive(Clone, Copy)]
pub struct Product;
impl Function for Product {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        let func = stats_module()
            .function("product")
            .expect("product should exist");
        func.call(std::slice::from_ref(input), &mut EvalContext::new())
    }
}

/// `stats.min(xs)` — minimum of the numeric elements of `xs` (null if
/// empty).
///
/// The logic is handl-authored in `stats.hnl`; this impl only validates
/// arguments and delegates to the generic `HandlModule` loader.
#[derive(Clone, Copy)]
pub struct Min;
impl Function for Min {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        let func = stats_module().function("min").expect("min should exist");
        func.call(std::slice::from_ref(input), &mut EvalContext::new())
    }
}

/// `stats.max(xs)` — maximum of the numeric elements of `xs` (null if
/// empty).
///
/// The logic is handl-authored in `stats.hnl`; this impl only validates
/// arguments and delegates to the generic `HandlModule` loader.
#[derive(Clone, Copy)]
pub struct Max;
impl Function for Max {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        let func = stats_module().function("max").expect("max should exist");
        func.call(std::slice::from_ref(input), &mut EvalContext::new())
    }
}

/// `stats.median(xs)` — median of the numeric elements of `xs` (0 if
/// empty).
///
/// The logic (including an insertion sort) is handl-authored in `stats.hnl`;
/// this impl only validates arguments and delegates to the generic
/// `HandlModule` loader.
#[derive(Clone, Copy)]
pub struct Median;
impl Function for Median {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        let func = stats_module()
            .function("median")
            .expect("median should exist");
        func.call(std::slice::from_ref(input), &mut EvalContext::new())
    }
}

/// `stats.stddev(xs)` — population standard deviation of the numeric
/// elements of `xs` (0 if fewer than 2 elements).
///
/// The logic (calling `math.sqrt`) is handl-authored in `stats.hnl`; this
/// impl only validates arguments and delegates to the generic `HandlModule`
/// loader.
#[derive(Clone, Copy)]
pub struct Stddev;
impl Function for Stddev {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        let func = stats_module()
            .function("stddev")
            .expect("stddev should exist");
        func.call(std::slice::from_ref(input), &mut EvalContext::new())
    }
}

// ---------------------------------------------------------------------------
// StatsLibraryPlugin (wraps HandlLibraryPlugin for interface declaration)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `stats` implementation via the
/// generic [`HandlLibraryPlugin`] with `cap` declarations.
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The stats interface is declared in `stats.hnl` via `---` doc comments,
/// `cap` declarations, and `op` declarations; the [`HandlLibraryPlugin`]
/// inner is used for the interface declaration — id, name, and capabilities
/// parsed from `stats.hnl` — while the custom [`Plugin::contribute`]
/// implementation registers the [`StatsModule`] (which delegates to the
/// generic `HandlModule` loader for the handl-authored logic).
///
/// The actual stats logic (sum, avg, product, min, max, median, stddev) is
/// handl-authored in `stats.hnl` and loaded via the `HandlModule`; the
/// `cap`/`op` declaration lines are stripped before evaluation.
pub struct StatsLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl StatsLibraryPlugin {
    /// Create a new `StatsLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "stats",
                "lib.stats",
                "lib-stats",
                STATS_HNL,
                &[],
            ),
        }
    }
}

impl Default for StatsLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for StatsLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(&self, _capability: &CapabilityId) -> Option<CapabilityMetadata> {
        None
    }

    fn contribute(&self, ctx: &mut ContributionContext) -> Result<(), PlatformError> {
        // Register the StatsModule (which provides Function impls that
        // delegate to the generic HandlModule loader).
        ctx.contribute::<ModuleRuntime>(ModuleRule::new(StatsModule))?;
        Ok(())
    }

    fn run(
        &self,
        _request: &CapabilityRequest,
        _ctx: &mut PluginContext,
    ) -> Result<CapabilityResponse, PlatformError> {
        Err(PlatformError::NotImplemented)
    }
}

/// Build a [`HandlLibraryPlugin`] for stats, with `MathModule` (for
/// `math.sqrt` in `stddev`) and `TypeModule` (for `typeof.type_of`
/// non-number filtering) as dependencies.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`StatsLibraryPlugin`].
pub fn stats_handl_plugin() -> HandlLibraryPlugin {
    let clean_source = strip_declarations(STATS_HNL);
    HandlLibraryPlugin::new("stats", "lib.stats", "lib-stats", &clean_source, STATS_FUNCS)
        .with_dependency(MathModule)
        .with_dependency(TypeModule)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`StatsLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_stats::LibraryPlugin`. Use
/// `LibraryPlugin::new()` to construct.
pub use StatsLibraryPlugin as LibraryPlugin;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_module::ModuleRegistry;
    use serde_json::json;

    /// Helper: call a stats `Function` on a JSON array and compare as f64.
    fn check_func(func: &dyn Function, input: serde_json::Value, expected: f64) {
        let result =
            func.call(&[input], &mut EvalContext::new()).unwrap_or_else(|e| panic!("failed: {e}"));
        let actual = result
            .as_f64()
            .unwrap_or_else(|| panic!("did not return a number: {result:?}"));
        assert!(
            (actual - expected).abs() < 1e-9,
            "got {actual}, expected {expected}"
        );
    }

    #[test]
    fn sum_works() {
        check_func(&Sum, json!([1, 2, 3, 4, 5]), 15.0);
    }

    #[test]
    fn avg_works() {
        check_func(&Avg, json!([1, 2, 3, 4, 5]), 3.0);
        check_func(&Avg, json!([1, 2]), 1.5);
    }

    #[test]
    fn product_works() {
        check_func(&Product, json!([1, 2, 3, 4, 5]), 120.0);
    }

    #[test]
    fn min_works() {
        check_func(&Min, json!([1, 2, 3, 4, 5]), 1.0);
    }

    #[test]
    fn max_works() {
        check_func(&Max, json!([1, 2, 3, 4, 5]), 5.0);
    }

    #[test]
    fn median_works() {
        // odd length
        check_func(&Median, json!([1, 2, 3, 4, 5]), 3.0);
        // even length
        check_func(&Median, json!([1, 2, 3, 4]), 2.5);
        // unsorted input
        check_func(&Median, json!([5, 3, 1, 4, 2]), 3.0);
    }

    #[test]
    fn stddev_works() {
        // population stddev of [1,2,3,4,5] = sqrt(2)
        check_func(&Stddev, json!([1, 2, 3, 4, 5]), 2.0f64.sqrt());
    }

    #[test]
    fn empty_array_edge_cases() {
        assert_eq!(Sum.call(&[json!([])], &mut EvalContext::new()).unwrap(), json!(0));
        assert_eq!(Avg.call(&[json!([])], &mut EvalContext::new()).unwrap(), json!(0));
        assert_eq!(
            Product.call(&[json!([])], &mut EvalContext::new()).unwrap(),
            json!(1)
        );
        assert_eq!(Min.call(&[json!([])], &mut EvalContext::new()).unwrap(), json!(null));
        assert_eq!(Max.call(&[json!([])], &mut EvalContext::new()).unwrap(), json!(null));
        assert_eq!(
            Median.call(&[json!([])], &mut EvalContext::new()).unwrap(),
            json!(0)
        );
        assert_eq!(
            Stddev.call(&[json!([])], &mut EvalContext::new()).unwrap(),
            json!(0)
        );
    }

    #[test]
    fn non_numbers_are_filtered() {
        // Reproduces the prior Rust `filter_map(|v| v.as_f64())` semantics:
        // non-numeric elements are silently dropped.
        check_func(&Sum, json!([1, "two", 3, null, 5]), 9.0);
        check_func(&Avg, json!([1, "x", 3, true, 5]), 3.0);
    }

    #[test]
    fn single_element_stddev_is_zero() {
        check_func(&Stddev, json!([42]), 0.0);
    }

    // -----------------------------------------------------------------------
    // ADR-0077 Phase 4: plugin scaffolding tests
    // -----------------------------------------------------------------------

    #[test]
    fn plugin_id_and_name() {
        let plugin = StatsLibraryPlugin::new();
        assert_eq!(plugin.id(), "lib.stats");
        assert_eq!(plugin.name(), "lib-stats");
    }

    #[test]
    fn plugin_exposes_declared_capabilities() {
        let plugin = StatsLibraryPlugin::new();
        let caps = plugin.capabilities();
        assert_eq!(caps.len(), 7);
        let names: Vec<&str> = caps.iter().map(|c| c.as_str()).collect();
        assert!(names.contains(&"stats.sum"));
        assert!(names.contains(&"stats.avg"));
        assert!(names.contains(&"stats.product"));
        assert!(names.contains(&"stats.min"));
        assert!(names.contains(&"stats.max"));
        assert!(names.contains(&"stats.median"));
        assert!(names.contains(&"stats.stddev"));
    }

    #[test]
    fn handl_plugin_id_and_name() {
        let plugin = stats_handl_plugin();
        assert_eq!(plugin.id(), "lib.stats");
        assert_eq!(plugin.name(), "lib-stats");
    }

    #[test]
    fn stats_module_works_in_module_registry() {
        let module = StatsModule;
        let registry = ModuleRegistry::new().with(module);
        let lookup: &dyn handl_platform::ModuleLookup = &registry;

        let func = lookup
            .lookup("stats.sum")
            .expect("stats.sum should resolve");
        let mut ctx = EvalContext::new();
        let result = func.call(&[json!([1, 2, 3, 4, 5])], &mut ctx).unwrap();
        assert_eq!(result, json!(15));
    }

    #[test]
    fn strip_declarations_removes_cap_and_op_lines() {
        let src = "--- doc\n\ncap \"stats.sum\"\nop \"stats.sum\" : Array -> Number\n\nlet x = 1 in x";
        let cleaned = strip_declarations(src);
        assert!(!cleaned.contains("cap"));
        assert!(!cleaned.contains("op "));
        assert!(cleaned.contains("let x = 1 in x"));
    }

    #[test]
    fn strip_declarations_preserves_handl_logic() {
        let cleaned = strip_declarations(STATS_HNL);
        // The handl logic should be preserved.
        assert!(cleaned.contains("filter_nums"));
        assert!(cleaned.contains("math.sqrt"));
        // No declaration lines should remain.
        assert!(!cleaned.contains("cap \""));
        assert!(!cleaned.contains("op \""));
    }
}
