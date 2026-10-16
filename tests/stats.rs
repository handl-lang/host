//! Ported from the deleted `plugin-lib-stats` crate's test module.
//! The `Sum`/`Avg`/… Rust structs are gone; the same assertions now run
//! against the table-built `stats` [`HandlModule`].

use handl_platform::{EvalContext, Function, Module, ModuleLookup};
use handl_plugin_lib_stdlib::{library, module_ref};
use handl_plugin_module::ModuleRegistry;
use serde_json::json;


/// Install the canonical default language once per test binary (ADR-0128
/// §"The 0.8 seam"): `HandlModule` bridge runtimes resolve it lazily.
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_canonical::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

fn stats_fn(name: &str) -> &'static dyn Function {
    ensure_lang();
    module_ref("stats")
        .expect("stats module")
        .function(name)
        .expect("function exists")
}

/// Helper: call a stats `Function` on a JSON array and compare as f64.
fn check_func(func: &dyn Function, input: serde_json::Value, expected: f64) {
    let result = func
        .call(&[input], &mut EvalContext::new())
        .unwrap_or_else(|e| panic!("failed: {e}"));
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
    check_func(stats_fn("sum"), json!([1, 2, 3, 4, 5]), 15.0);
}

#[test]
fn avg_works() {
    check_func(stats_fn("avg"), json!([1, 2, 3, 4, 5]), 3.0);
    check_func(stats_fn("avg"), json!([1, 2]), 1.5);
}

#[test]
fn product_works() {
    check_func(stats_fn("product"), json!([1, 2, 3, 4, 5]), 120.0);
}

#[test]
fn min_works() {
    check_func(stats_fn("min"), json!([1, 2, 3, 4, 5]), 1.0);
}

#[test]
fn max_works() {
    check_func(stats_fn("max"), json!([1, 2, 3, 4, 5]), 5.0);
}

#[test]
fn median_works() {
    check_func(stats_fn("median"), json!([1, 2, 3, 4, 5]), 3.0);
    check_func(stats_fn("median"), json!([1, 2, 3, 4]), 2.5);
    check_func(stats_fn("median"), json!([5, 3, 1, 4, 2]), 3.0);
}

#[test]
fn stddev_works() {
    // population stddev of [1,2,3,4,5] = sqrt(2)
    check_func(stats_fn("stddev"), json!([1, 2, 3, 4, 5]), 2.0f64.sqrt());
}

#[test]
fn empty_array_edge_cases() {
    assert_eq!(
        stats_fn("sum")
            .call(&[json!([])], &mut EvalContext::new())
            .unwrap(),
        json!(0)
    );
    assert_eq!(
        stats_fn("avg")
            .call(&[json!([])], &mut EvalContext::new())
            .unwrap(),
        json!(0)
    );
    assert_eq!(
        stats_fn("product")
            .call(&[json!([])], &mut EvalContext::new())
            .unwrap(),
        json!(1)
    );
    assert_eq!(
        stats_fn("min")
            .call(&[json!([])], &mut EvalContext::new())
            .unwrap(),
        json!(null)
    );
    assert_eq!(
        stats_fn("max")
            .call(&[json!([])], &mut EvalContext::new())
            .unwrap(),
        json!(null)
    );
    assert_eq!(
        stats_fn("median")
            .call(&[json!([])], &mut EvalContext::new())
            .unwrap(),
        json!(0)
    );
    assert_eq!(
        stats_fn("stddev")
            .call(&[json!([])], &mut EvalContext::new())
            .unwrap(),
        json!(0)
    );
}

#[test]
fn non_numbers_are_filtered() {
    check_func(stats_fn("sum"), json!([1, "two", 3, null, 5]), 9.0);
    check_func(stats_fn("avg"), json!([1, "x", 3, true, 5]), 3.0);
}

#[test]
fn single_element_stddev_is_zero() {
    check_func(stats_fn("stddev"), json!([42]), 0.0);
}

#[test]
fn plugin_id_and_name() {
    let plugin = library("stats").unwrap();
    assert_eq!(plugin.id(), "lib.stats");
    assert_eq!(plugin.name(), "lib-stats");
}

#[test]
fn plugin_exposes_declared_capabilities() {
    let plugin = library("stats").unwrap();
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
fn stats_module_works_in_module_registry() {
    ensure_lang();
    let module = handl_plugin_lib_stdlib::module("stats").expect("stats");
    let registry = ModuleRegistry::new().with(module);
    let lookup: &dyn ModuleLookup = &registry;

    let func = lookup
        .lookup("stats.sum")
        .expect("stats.sum should resolve");
    let mut ctx = EvalContext::new();
    let result = func.call(&[json!([1, 2, 3, 4, 5])], &mut ctx).unwrap();
    assert_eq!(result, json!(15));
}
