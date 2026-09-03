//! # handl-lib-stats
//!
//! Statistical aggregate functions for the handl runtime.
//!
//! The stats *logic* is handl-authored in
//! `kit-stdlib/data/stats/stats.hnl` (a record of arrows: `sum`, `avg`,
//! `product`, `min`, `max`, `median`, `stddev`); this crate loads and bridges
//! that definition via the generic [`handl_plugin_lib_handl`] loader. The
//! `Module`/`Plugin` trait scaffolding is retained as the registration
//! boundary.
//!
//! Non-number handling: the prior Rust impl silently skipped non-numeric
//! elements via `filter_map(|v| v.as_f64())`. The handl port reproduces this
//! filtering using `typeof.type_of(x) == "num"` (requires `TypeModule` in the
//! bridge runtime). Non-numbers are dropped before aggregation.

/// The handl source defining the stats record — the single source of truth
/// for the stats logic. Loaded at compile time from the kit-stdlib data dir.
const STATS_HNL: &str = include_str!("../../../kit-stdlib/data/stats/stats.hnl");

/// The function (record field) names exposed by the stats module.
const STATS_FUNCS: &[&str] = &["sum", "avg", "product", "min", "max", "median", "stddev"];

pub mod module;
pub use module::StatsModule;
pub mod plugin;
pub use plugin::LibraryPlugin;

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::HandlModule;
use handl_plugin_lib_utils::{MathModule, TypeModule};
use serde_json::Value;
use std::sync::OnceLock;

/// Build the [`HandlModule`] for stats, with `MathModule` (for `math.sqrt`
/// in `stddev`) and `TypeModule` (for `typeof.type_of` non-number filtering)
/// as dependencies.
fn build_stats_module() -> HandlModule {
    HandlModule::new("stats", STATS_HNL, STATS_FUNCS)
        .with_dependency(MathModule)
        .with_dependency(TypeModule)
}

/// A thread-safe lazy-initialized [`HandlModule`] for stats.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
static STATS_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for stats, initializing it on first call.
fn stats_module() -> &'static HandlModule {
    STATS_MODULE.get_or_init(build_stats_module)
}

/// Require exactly one array argument, returning it as a slice.
fn require_one_array(args: &[Value]) -> Result<&Value, EvalError> {
    if args.len() != 1 {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, args.len()
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
/// arguments and delegates to the generic [`HandlModule`] loader.
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
/// arguments and delegates to the generic [`HandlModule`] loader.
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
/// arguments and delegates to the generic [`HandlModule`] loader.
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
/// arguments and delegates to the generic [`HandlModule`] loader.
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
/// arguments and delegates to the generic [`HandlModule`] loader.
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
/// [`HandlModule`] loader.
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
/// impl only validates arguments and delegates to the generic [`HandlModule`]
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
