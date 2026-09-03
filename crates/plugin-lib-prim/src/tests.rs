use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup, Plugin};
use handl_plugin_module::ModuleRegistry;
use serde_json::json;

use crate::{prim_op_registry, PrimLibraryPlugin, PrimModule, prim_handl_plugin};

fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let func = module.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx).expect("call succeeds")
}

fn call_err(
    module: &dyn Module,
    name: &str,
    args: &[serde_json::Value],
) -> Result<serde_json::Value, EvalError> {
    let func = module.function(name).expect("function exists");
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx)
}

fn n(v: i64) -> serde_json::Value {
    serde_json::Value::from(v)
}

fn s(v: &str) -> serde_json::Value {
    serde_json::Value::from(v.to_string())
}

#[test]
fn module_name() {
    assert_eq!(PrimModule.name(), "prim");
}

#[test]
fn unknown_function() {
    assert!(PrimModule.function("nope").is_none());
}

#[test]
fn add_positive() {
    assert_eq!(
        call(&PrimModule, "add", &[n(2), n(3)]).as_i64(),
        Some(5)
    );
}

#[test]
fn add_negative() {
    assert_eq!(
        call(&PrimModule, "add", &[n(-4), n(7)]).as_i64(),
        Some(3)
    );
}

#[test]
fn add_zero() {
    assert_eq!(
        call(&PrimModule, "add", &[n(0), n(0)]).as_i64(),
        Some(0)
    );
}

#[test]
fn add_missing_args_errors() {
    let result = call_err(&PrimModule, "add", &[n(1)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn add_non_integer_errors() {
    let result = call_err(&PrimModule, "add", &[s("a"), n(1)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn compare_less() {
    assert_eq!(
        call(&PrimModule, "compare", &[n(1), n(2)]).as_i64(),
        Some(-1)
    );
}

#[test]
fn compare_equal() {
    assert_eq!(
        call(&PrimModule, "compare", &[n(2), n(2)]).as_i64(),
        Some(0)
    );
}

#[test]
fn compare_greater() {
    assert_eq!(
        call(&PrimModule, "compare", &[n(3), n(1)]).as_i64(),
        Some(1)
    );
}

#[test]
fn compare_strings() {
    assert_eq!(
        call(&PrimModule, "compare", &[s("a"), s("b")]).as_i64(),
        Some(-1)
    );
}

#[test]
fn compare_missing_args_errors() {
    let result = call_err(&PrimModule, "compare", &[n(1)]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

#[test]
fn alloc_returns_empty_array() {
    let result = call(&PrimModule, "alloc", &[n(5)]);
    assert!(result.is_array());
    assert_eq!(result.as_array().unwrap().len(), 0);
}

#[test]
fn alloc_zero_returns_empty_array() {
    let result = call(&PrimModule, "alloc", &[n(0)]);
    assert!(result.is_array());
    assert_eq!(result.as_array().unwrap().len(), 0);
}

#[test]
fn alloc_missing_args_errors() {
    let result = call_err(&PrimModule, "alloc", &[]);
    assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
}

// -----------------------------------------------------------------------
// ADR-0077 Phase 2: op/handl form tests
// -----------------------------------------------------------------------

#[test]
fn op_registry_registers_all_three_ops() {
    let registry = prim_op_registry();
    assert!(registry.contains("prim.add"));
    assert!(registry.contains("prim.compare"));
    assert!(registry.contains("prim.alloc"));
    assert_eq!(registry.len(), 3);
}

#[test]
fn op_registry_add_dispatches() {
    let registry = std::sync::Arc::new(prim_op_registry());
    let op_func = handl_plugin_lib_handl::OpFunction::new("prim.add", registry);
    let mut ctx = EvalContext::new();
    let result = op_func.call(&[json!(2), json!(3)], &mut ctx).unwrap();
    assert_eq!(result, json!(5));
}

#[test]
fn op_registry_compare_dispatches() {
    let registry = std::sync::Arc::new(prim_op_registry());
    let op_func = handl_plugin_lib_handl::OpFunction::new("prim.compare", registry);
    let mut ctx = EvalContext::new();
    let result = op_func.call(&[json!(1), json!(2)], &mut ctx).unwrap();
    assert_eq!(result, json!(-1));
}

#[test]
fn op_registry_alloc_dispatches() {
    let registry = std::sync::Arc::new(prim_op_registry());
    let op_func = handl_plugin_lib_handl::OpFunction::new("prim.alloc", registry);
    let mut ctx = EvalContext::new();
    let result = op_func.call(&[json!(5)], &mut ctx).unwrap();
    assert!(result.is_array());
    assert_eq!(result.as_array().unwrap().len(), 0);
}

#[test]
fn prim_ops_have_no_checkers_trusted() {
    // The prim atoms are trusted — no runtime checkers attached.
    let registry = prim_op_registry();
    assert!(registry.get_checker("prim.add").is_none());
    assert!(registry.get_checker("prim.compare").is_none());
    assert!(registry.get_checker("prim.alloc").is_none());
}

#[test]
fn plugin_id_and_name() {
    let plugin = PrimLibraryPlugin::new();
    assert_eq!(plugin.id(), "lib.prim");
    assert_eq!(plugin.name(), "lib-prim");
}

#[test]
fn handl_plugin_id_and_name() {
    let plugin = prim_handl_plugin();
    assert_eq!(plugin.id(), "lib.prim");
    assert_eq!(plugin.name(), "lib-prim");
}

#[test]
fn prim_module_works_in_module_registry() {
    let module = PrimModule;
    let registry = ModuleRegistry::new().with(module);
    let lookup: &dyn ModuleLookup = &registry;

    let func = lookup
        .lookup("prim.add")
        .expect("prim.add should resolve");
    let mut ctx = EvalContext::new();
    let result = func.call(&[json!(2), json!(3)], &mut ctx).unwrap();
    assert_eq!(result, json!(5));
}

// -----------------------------------------------------------------------
// ADR-0077 Phase 5: capability declaration tests
// -----------------------------------------------------------------------

#[test]
fn prim_plugin_exposes_declared_capabilities() {
    let plugin = PrimLibraryPlugin::new();
    let caps = plugin.capabilities();
    assert_eq!(caps.len(), 3);
    assert_eq!(caps[0].as_str(), "prim.add");
    assert_eq!(caps[1].as_str(), "prim.compare");
    assert_eq!(caps[2].as_str(), "prim.alloc");
}

#[test]
fn prim_handl_plugin_exposes_declared_capabilities() {
    let plugin = prim_handl_plugin();
    let caps = plugin.capabilities();
    assert_eq!(caps.len(), 3);
    assert_eq!(caps[0].as_str(), "prim.add");
    assert_eq!(caps[1].as_str(), "prim.compare");
    assert_eq!(caps[2].as_str(), "prim.alloc");
}
