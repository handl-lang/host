//! # handl-lib-stats
//!
//! Statistical aggregate functions for the handl runtime.
//!
//! The stats *logic* is handl-authored in
//! `kit-stdlib/data/stats/stats.hnl` (a record of procs: `sum`, `avg`,
//! `product`, `min`, `max`, `median`, `stddev`); this crate only loads +
//! evaluates + bridges that definition (see [`bridge`]). The `Module`/
//! `Plugin` trait scaffolding is retained as the registration boundary.
//!
//! Non-number handling: the prior Rust impl silently skipped non-numeric
//! elements via `filter_map(|v| v.as_f64())`. The handl port reproduces this
//! filtering using `type.type_of(x) == "num"` (requires `TypeModule` in the
//! bridge runtime). Non-numbers are dropped before aggregation.

/// Handl-authored stats loader/bridge (shared by the `Function` and `Plugin`
/// impls).
mod bridge;
pub(crate) use bridge::stats_via_handl;

pub mod module;
pub use module::StatsModule;
pub mod plugin;
pub use plugin::LibraryPlugin;

use handl_platform::{EvalContext, EvalError, Function};
use serde_json::Value;

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
/// arguments and delegates to [`stats_via_handl`].
#[derive(Clone, Copy)]
pub struct Sum;
impl Function for Sum {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        stats_via_handl("sum", input)
    }
}

/// `stats.avg(xs)` — arithmetic mean of the numeric elements of `xs`.
///
/// The logic is handl-authored in `stats.hnl`; this impl only validates
/// arguments and delegates to [`stats_via_handl`].
#[derive(Clone, Copy)]
pub struct Avg;
impl Function for Avg {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        stats_via_handl("avg", input)
    }
}

/// `stats.product(xs)` — product of the numeric elements of `xs`.
///
/// The logic is handl-authored in `stats.hnl`; this impl only validates
/// arguments and delegates to [`stats_via_handl`].
#[derive(Clone, Copy)]
pub struct Product;
impl Function for Product {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        stats_via_handl("product", input)
    }
}

/// `stats.min(xs)` — minimum of the numeric elements of `xs` (null if
/// empty).
///
/// The logic is handl-authored in `stats.hnl`; this impl only validates
/// arguments and delegates to [`stats_via_handl`].
#[derive(Clone, Copy)]
pub struct Min;
impl Function for Min {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        stats_via_handl("min", input)
    }
}

/// `stats.max(xs)` — maximum of the numeric elements of `xs` (null if
/// empty).
///
/// The logic is handl-authored in `stats.hnl`; this impl only validates
/// arguments and delegates to [`stats_via_handl`].
#[derive(Clone, Copy)]
pub struct Max;
impl Function for Max {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        stats_via_handl("max", input)
    }
}

/// `stats.median(xs)` — median of the numeric elements of `xs` (0 if
/// empty).
///
/// The logic (including an insertion sort) is handl-authored in `stats.hnl`;
/// this impl only validates arguments and delegates to [`stats_via_handl`].
#[derive(Clone, Copy)]
pub struct Median;
impl Function for Median {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        stats_via_handl("median", input)
    }
}

/// `stats.stddev(xs)` — population standard deviation of the numeric
/// elements of `xs` (0 if fewer than 2 elements).
///
/// The logic (calling `math.sqrt`) is handl-authored in `stats.hnl`; this
/// impl only validates arguments and delegates to [`stats_via_handl`].
#[derive(Clone, Copy)]
pub struct Stddev;
impl Function for Stddev {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_array(args)?;
        stats_via_handl("stddev", input)
    }
}

// Re-export the `StatsModule` from `module.rs` for backward compatibility
// with callers that import it from the crate root.

#[cfg(test)]
mod tests {
    use super::stats_via_handl;
    use serde_json::json;

    /// Helper: eval a stats function on a JSON array and compare as f64.
    fn check(func: &str, input: serde_json::Value, expected: f64) {
        let result = stats_via_handl(func, &input).unwrap_or_else(|e| panic!("{func} failed: {e}"));
        let actual = result.as_f64().unwrap_or_else(|| {
            panic!("{func} did not return a number: {result:?}")
        });
        assert!(
            (actual - expected).abs() < 1e-9,
            "{func}({input:?}) = {actual}, expected {expected}"
        );
    }

    #[test]
    fn sum_works() {
        check("sum", json!([1, 2, 3, 4, 5]), 15.0);
    }

    #[test]
    fn avg_works() {
        check("avg", json!([1, 2, 3, 4, 5]), 3.0);
        check("avg", json!([1, 2]), 1.5);
    }

    #[test]
    fn product_works() {
        check("product", json!([1, 2, 3, 4, 5]), 120.0);
    }

    #[test]
    fn min_works() {
        check("min", json!([1, 2, 3, 4, 5]), 1.0);
    }

    #[test]
    fn max_works() {
        check("max", json!([1, 2, 3, 4, 5]), 5.0);
    }

    #[test]
    fn median_works() {
        // odd length
        check("median", json!([1, 2, 3, 4, 5]), 3.0);
        // even length
        check("median", json!([1, 2, 3, 4]), 2.5);
        // unsorted input
        check("median", json!([5, 3, 1, 4, 2]), 3.0);
    }

    #[test]
    fn stddev_works() {
        // population stddev of [1,2,3,4,5] = sqrt(2)
        check("stddev", json!([1, 2, 3, 4, 5]), 2.0f64.sqrt());
    }

    #[test]
    fn empty_array_edge_cases() {
        assert_eq!(stats_via_handl("sum", &json!([])).unwrap(), json!(0));
        assert_eq!(stats_via_handl("avg", &json!([])).unwrap(), json!(0));
        assert_eq!(stats_via_handl("product", &json!([])).unwrap(), json!(1));
        assert_eq!(stats_via_handl("min", &json!([])).unwrap(), json!(null));
        assert_eq!(stats_via_handl("max", &json!([])).unwrap(), json!(null));
        assert_eq!(stats_via_handl("median", &json!([])).unwrap(), json!(0));
        assert_eq!(stats_via_handl("stddev", &json!([])).unwrap(), json!(0));
    }

    #[test]
    fn non_numbers_are_filtered() {
        // Reproduces the prior Rust `filter_map(|v| v.as_f64())` semantics:
        // non-numeric elements are silently dropped.
        check("sum", json!([1, "two", 3, null, 5]), 9.0);
        check("avg", json!([1, "x", 3, true, 5]), 3.0);
    }

    #[test]
    fn single_element_stddev_is_zero() {
        check("stddev", json!([42]), 0.0);
    }
}
