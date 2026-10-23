//! ADR-0155 Phase 3 — the ambient module table as explicit default-env
//! globals (see [`packages::std_default_env`]).
//!
//! `build_module` now wires every table-built `HandlModule` with a
//! `DefaultEnv` provider binding each `MODULES` name to a record of
//! `wrap_dep_fn` wrapper arrows (plus `values` projections for
//! value-bearing specs): `math.add(2, 3)` in a `.hnl` source resolves
//! **scope-first** — record field + `!App` — and the wrapper's `callv`
//! body lands on the same `call_host` leaf the unbound `"math.add"`
//! callee dispatched to. Declared deps shadow the defaults; a name bound
//! by no layer fails loudly at the leaf.

use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::{HandlModule, ModuleResolver};
use handl_plugin_lib_stdlib::{
    ModuleSpec, PackageResolver, TableModuleLookup, build_module, module_with, std_default_env,
};
use serde_json::{Value, json};

/// Install the canonical default language once per test binary (see
/// `named_import.rs` — the `HandlModule` bridge runtimes resolve it lazily).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml().expect("canonical language assembles"),
        );
    }
}

fn resolver() -> Arc<dyn ModuleResolver> {
    Arc::new(PackageResolver { fallback: None })
}

/// A probe spec: a table row for a module whose `.hnl` calls table names
/// ambiently — no `deps`, no imports, exactly the surface the ambient
/// table lookup used to cover.
const PROBE_SPEC: ModuleSpec = ModuleSpec {
    name: "default_env_probe",
    hnl: "export add := (a, b) ~> math.add(a, b)\nexport bad := (x) ~> nosuchmod.f(x)",
    funcs: &["add", "bad"],
    ops: &[],
    values: &[],
    registry: None,
};

/// Same, with a value export probing what a table name *evaluates to*.
const VALUE_PROBE_SPEC: ModuleSpec = ModuleSpec {
    name: "default_env_value_probe",
    hnl: "export mod := math\nexport t := bool.true",
    funcs: &[],
    ops: &[],
    values: &["mod", "t"],
    registry: None,
};

fn probe_module(spec: &ModuleSpec) -> HandlModule {
    ensure_lang();
    build_module(spec, resolver())
}

fn call(module: &HandlModule, name: &str, args: &[Value]) -> Value {
    module
        .function(name)
        .expect("function exists")
        .call(args, &mut EvalContext::new())
        .expect("call succeeds")
}

/// `math` is never declared anywhere near the probe's source — it is a
/// default-env binding: `math.add` resolves scope-first through the
/// env record's wrapper arrow, then re-dispatches `"math.add"` through
/// the table lookup leaf.
#[test]
fn ambient_table_names_bind_via_default_env() {
    let m = probe_module(&PROBE_SPEC);
    assert_eq!(call(&m, "add", &[json!(2), json!(3)]), json!(5.0));
}

/// The binding is a **record value**, not the old unbound→string
/// placeholder: `math` in value position evaluates to the env record
/// (fields are `{"__arrow__":…}` wrappers). Value exports are *not*
/// projected into the default env — `bool.true` reads `Null` via the
/// record (the ambient path used to yield the string `"bool.true"`;
/// `from "bool" import true` remains the real-value binding path).
#[test]
fn default_env_names_are_values_not_name_strings() {
    let m = probe_module(&VALUE_PROBE_SPEC);
    let math = m.value("mod").expect("mod value projects");
    assert_eq!(math["add"]["__arrow__"], json!(true), "math: {math}");
    assert_eq!(m.value("t"), Some(json!(null)));
}

/// A declared dep named `math` shadows the table default: the probe's
/// `math.add` call resolves through the dep's env record — the dep's own
/// function, not the table's op.
#[test]
fn declared_deps_shadow_table_defaults() {
    struct StubAdd;
    impl Function for StubAdd {
        fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            Ok(json!("stubbed"))
        }
    }
    struct StubMath(StubAdd);
    impl Module for StubMath {
        fn name(&self) -> &str {
            "math"
        }
        fn function(&self, name: &str) -> Option<&dyn Function> {
            (name == "add").then_some(&self.0 as &dyn Function)
        }
        fn functions(&self) -> Vec<(&str, &dyn Function)> {
            vec![("add", &self.0)]
        }
    }
    let m = probe_module(&PROBE_SPEC).with_dependency(StubMath(StubAdd));
    assert_eq!(call(&m, "add", &[json!(2), json!(3)]), json!("stubbed"));
}

/// `nosuchmod` is bound by no layer — not a dep, not the default env, not
/// the lookup's resolver fallback — so the dotted callee fails loudly at
/// `call_host` rather than silently dispatching.
#[test]
fn unlisted_dotted_callee_fails_loudly() {
    let m = probe_module(&PROBE_SPEC);
    let err = m
        .function("bad")
        .unwrap()
        .call(&[json!("x")], &mut EvalContext::new())
        .expect_err("unlisted callee must fail");
    assert!(
        err.to_string().contains("nosuchmod.f"),
        "error names the unresolved callee: {err}"
    );
}

/// The provider is opt-in, not forced: a module wired the old way —
/// `global_lookup` only, no default env — still resolves ambient table
/// names as string callees through the leaf (the residual ambient hosts
/// may deliberately keep).
#[test]
fn residual_ambient_without_default_env() {
    ensure_lang();
    let m = HandlModule::new(
        "residual_probe",
        "export add := (a, b) ~> math.add(a, b)",
        &["add"],
    )
    .with_global_lookup(Arc::new(TableModuleLookup::new(resolver())));
    assert_eq!(call(&m, "add", &[json!(2), json!(3)]), json!(5.0));
}

/// A real table module through `module_with` (the kit-prelude path) gets
/// the env too — `bool.eq` resolves scope-first via the bound record —
/// and the provider materializes a wrapper-arrow record per table name.
#[test]
fn table_modules_and_provider_records() {
    ensure_lang();
    let m = module_with("bool", resolver()).expect("bool in the table");
    assert_eq!(
        call(&m, "eq", &[json!(true), json!(true)]),
        json!(true)
    );
    let provider = std_default_env();
    let globals = provider();
    let bool_record = globals.get("bool").expect("bool bound by default env");
    assert_eq!(bool_record["eq"]["__arrow__"], json!(true), "{bool_record}");
    // Every table name is bound — spot-check the op-backed and
    // funcs-only extremes.
    assert!(globals["math"]["add"]["__arrow__"].as_bool().unwrap());
    assert!(globals.contains_key("uchar"));
}
