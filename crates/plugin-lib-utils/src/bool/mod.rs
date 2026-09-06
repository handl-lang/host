//! The `bool` module — boolean utility functions.
//!
//! Provides functions callable as `bool.and(a, b)`, `bool.or(a, b)`,
//! `bool.not(a)`, `bool.xor(a, b)`, `bool.nand(a, b)`, `bool.nor(a, b)`,
//! `bool.implies(a, b)`, `bool.eq(a, b)` in handl expressions.
//!
//! The bool *logic* is handl-authored in `handl-packages/utils/bool.hnl`
//! (a record of arrows); this module loads and bridges that definition via
//! the generic [`HandlModule`](handl_plugin_lib_handl::HandlModule) loader.
//! The per-module Rust `Function` impls have been eliminated (ADR-0077
//! Phase 1): the [`HandlModule`] evaluates the `.hnl` source directly,
//! exposing each record field as a callable
//! [`Function`](handl_platform::Function).

use handl_platform::{Function, Module};
use handl_plugin_lib_handl::HandlModule;
use std::sync::OnceLock;

/// The handl source defining the bool record — the single source of truth
/// for the bool logic. Loaded at compile time from the kit-stdlib data dir.
///
/// `bool.hnl` is a record of arrows (`and`, `or`, `not`, `xor`, `nand`,
/// `nor`, `implies`, `eq`). Comment lines (`--- ...`) and `cap` declaration
/// lines are stripped by the loader's `strip_comment_lines` and
/// `strip_declaration_lines`.
const BOOL_HNL: &str = include_str!("../../../../handl-packages/utils/bool.hnl");

/// The function (record field) names exposed by the bool module.
const BOOL_FUNCS: &[&str] = &["and", "or", "not", "xor", "nand", "nor", "implies", "eq"];

/// Build the [`HandlModule`] for bool.
///
/// The bool functions use only native handl operators (`&&`, `||`, `!`,
/// `==`, `!=`), so no dependency modules are required.
fn build_bool_module() -> HandlModule {
    HandlModule::new("bool", BOOL_HNL, BOOL_FUNCS)
}

/// A thread-safe lazy-initialized [`HandlModule`] for bool.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
static BOOL_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for bool, initializing it on first call.
fn bool_module() -> &'static HandlModule {
    BOOL_MODULE.get_or_init(build_bool_module)
}

/// The `bool` module — boolean utility functions.
///
/// Provides `bool.and`, `bool.or`, `bool.not`, `bool.xor`, `bool.nand`,
/// `bool.nor`, `bool.implies`, `bool.eq`. The logic is handl-authored in
/// `bool.hnl` and loaded via the generic
/// [`HandlModule`](handl_plugin_lib_handl::HandlModule); this struct only
/// delegates [`Module`] lookups to the shared [`HandlModule`].
#[derive(Clone, Copy)]
pub struct BoolModule;

impl Module for BoolModule {
    fn name(&self) -> &str {
        "bool"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        bool_module().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        bool_module().functions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::EvalContext;
    use serde_json::json;

    /// Helper: call a `bool.<func>` function on JSON bool args and compare.
    fn check_bool(func: &str, args: &[serde_json::Value], expected: bool) {
        let f = BoolModule
            .function(func)
            .unwrap_or_else(|| panic!("{func} should exist"));
        let result = f
            .call(args, &mut EvalContext::new())
            .unwrap_or_else(|e| panic!("bool.{func} failed: {e}"));
        assert_eq!(result, json!(expected), "bool.{func}({:?})", args);
    }

    #[test]
    fn module_name_is_bool() {
        assert_eq!(BoolModule.name(), "bool");
    }

    #[test]
    fn module_exposes_all_functions() {
        let names: Vec<&str> = BoolModule
            .functions()
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        for &expected in BOOL_FUNCS {
            assert!(names.contains(&expected), "missing {expected}");
        }
    }

    #[test]
    fn module_function_lookup() {
        for &func in BOOL_FUNCS {
            assert!(BoolModule.function(func).is_some(), "{func} missing");
        }
        assert!(BoolModule.function("nonexistent").is_none());
    }

    #[test]
    fn and_truth_table() {
        check_bool("and", &[json!(true), json!(true)], true);
        check_bool("and", &[json!(true), json!(false)], false);
        check_bool("and", &[json!(false), json!(true)], false);
        check_bool("and", &[json!(false), json!(false)], false);
    }

    #[test]
    fn or_truth_table() {
        check_bool("or", &[json!(true), json!(true)], true);
        check_bool("or", &[json!(true), json!(false)], true);
        check_bool("or", &[json!(false), json!(true)], true);
        check_bool("or", &[json!(false), json!(false)], false);
    }

    #[test]
    fn not_truth_table() {
        check_bool("not", &[json!(true)], false);
        check_bool("not", &[json!(false)], true);
    }

    #[test]
    fn xor_truth_table() {
        check_bool("xor", &[json!(true), json!(true)], false);
        check_bool("xor", &[json!(true), json!(false)], true);
        check_bool("xor", &[json!(false), json!(true)], true);
        check_bool("xor", &[json!(false), json!(false)], false);
    }

    #[test]
    fn nand_truth_table() {
        check_bool("nand", &[json!(true), json!(true)], false);
        check_bool("nand", &[json!(true), json!(false)], true);
        check_bool("nand", &[json!(false), json!(true)], true);
        check_bool("nand", &[json!(false), json!(false)], true);
    }

    #[test]
    fn nor_truth_table() {
        check_bool("nor", &[json!(true), json!(true)], false);
        check_bool("nor", &[json!(true), json!(false)], false);
        check_bool("nor", &[json!(false), json!(true)], false);
        check_bool("nor", &[json!(false), json!(false)], true);
    }

    #[test]
    fn implies_truth_table() {
        // a -> b is false only when a is true and b is false.
        check_bool("implies", &[json!(true), json!(true)], true);
        check_bool("implies", &[json!(true), json!(false)], false);
        check_bool("implies", &[json!(false), json!(true)], true);
        check_bool("implies", &[json!(false), json!(false)], true);
    }

    #[test]
    fn eq_truth_table() {
        check_bool("eq", &[json!(true), json!(true)], true);
        check_bool("eq", &[json!(true), json!(false)], false);
        check_bool("eq", &[json!(false), json!(true)], false);
        check_bool("eq", &[json!(false), json!(false)], true);
    }
}
