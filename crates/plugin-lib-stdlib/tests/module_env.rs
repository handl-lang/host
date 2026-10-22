//! ADR-0154 follow-up: env-aware sharing in the stdlib layers.
//!
//! The name-keyed singleton caches (`module_ref`, `top_module_cache`) map
//! one name/path to one env; their env-aware siblings —
//! [`module_ref_with_env`] (package table) and [`load_hnl_file_with_env`]
//! (on-disk loader) — key by `(name/path, env_id)`, so one module can run
//! under several dependency bindings: stub/mock / A-B dep surfaces.
//!
//! [`module_ref_with_env`]: handl_plugin_lib_stdlib::module_ref_with_env
//! [`load_hnl_file_with_env`]: handl_plugin_lib_stdlib::load_hnl_file_with_env

use std::sync::Arc;

use handl_platform::{EvalContext, Function, Module};
use handl_plugin_lib_handl::ModuleEnv;
use handl_plugin_lib_stdlib::{load_hnl_file, load_hnl_file_with_env, module_ref, module_ref_with_env};
use serde_json::json;

/// Install the canonical default language once per test binary (ADR-0128
/// §"The 0.8 seam"): `HandlModule` bridge runtimes resolve it lazily.
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

fn call(m: &dyn Module, func: &str, arg: serde_json::Value) -> serde_json::Value {
    m.function(func)
        .unwrap_or_else(|| panic!("{func} bound"))
        .call(&[arg], &mut EvalContext::new())
        .unwrap_or_else(|e| panic!("{func} evaluates: {e}"))
}

fn as_number(v: &serde_json::Value) -> f64 {
    v.as_f64()
        .unwrap_or_else(|| panic!("did not return a number: {v:?}"))
}

/// A `typeof` stub whose `type_of` answers a fixed kind — shadows the
/// table's `typeof` for the instance's `mod.fn` callees (the dependency
/// registry is consulted before the global lookup).
struct TypeofStub {
    type_of: TypeofStubFn,
}

struct TypeofStubFn(&'static str);

impl Function for TypeofStubFn {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, handl_platform::EvalError> {
        Ok(json!(self.0))
    }
}

impl TypeofStub {
    fn new(kind: &'static str) -> Self {
        Self {
            type_of: TypeofStubFn(kind),
        }
    }
}

impl Module for TypeofStub {
    fn name(&self) -> &str {
        "typeof"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        (name == "type_of").then_some(&self.type_of as &dyn Function)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![("type_of", &self.type_of)]
    }
}

/// `stats.sum` filters elements by `typeof.type_of(h) == "num"`, so a
/// `typeof` stub in the env changes the answer — one name, several envs,
/// one instance per env.
#[test]
fn module_ref_with_env_serves_one_instance_per_env() {
    ensure_lang();
    let base = module_ref("stats").expect("stats base");
    // The table wiring rides along in the snapshot; the layered stub
    // shadows `typeof` for the instance only.
    let env_num = base.env().with_dependency(TypeofStub::new("num"));
    let env_str = base.env().with_dependency(TypeofStub::new("str"));

    let nums = module_ref_with_env("stats", &env_num).expect("env num instance");
    let strs = module_ref_with_env("stats", &env_str).expect("env str instance");

    // Same name, two envs → two instances.
    assert_ne!(nums.module_id(), strs.module_id());
    assert_ne!(nums.module_id(), base.module_id());

    // Each env's answer: "num" keeps every element, "str" drops them all.
    let nums_sum = call(&*nums, "sum", json!([1, 2, 3]));
    let strs_sum = call(&*strs, "sum", json!([1, 2, 3]));
    assert_eq!(as_number(&nums_sum), 6.0);
    assert_eq!(as_number(&strs_sum), 0.0);

    // The single-env singleton is untouched: the base still answers
    // through the real `typeof`.
    let base_sum = call(base, "sum", json!([1, 2, 3]));
    assert_eq!(as_number(&base_sum), 6.0);

    // A clone is the same env → the same cached instance.
    let nums_again = module_ref_with_env("stats", &env_num.clone()).expect("clone instance");
    assert!(Arc::ptr_eq(&nums, &nums_again), "env clones share the instance");

    // Re-dispatch after the other env ran: distinct module_ids keep the
    // per-thread runtime caches apart.
    assert_eq!(as_number(&call(&*nums, "sum", json!([4]))), 4.0);
}

/// A `dep` stub whose `who` answers a marker string.
struct DepStubWho(&'static str);

impl Function for DepStubWho {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, handl_platform::EvalError> {
        Ok(json!(self.0))
    }
}

struct DepStub {
    who: DepStubWho,
}

impl DepStub {
    fn new(marker: &'static str) -> Self {
        Self {
            who: DepStubWho(marker),
        }
    }
}

impl Module for DepStub {
    fn name(&self) -> &str {
        "dep"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        (name == "who").then_some(&self.who as &dyn Function)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![("who", &self.who)]
    }
}

const MAIN_HNL: &str = "from \"./dep.hnl\" import * as dep\nexport ask := (x) ~> dep.who() + \":\" + x\n";
const DEP_HNL: &str = "export who := () ~> \"real\"\n";

/// One on-disk file, several envs: `load_hnl_file_with_env` instantiates
/// the shared path-keyed base under each env (the env replaces the file's
/// own dep binding), one instance per `(path, env_id)`.
#[test]
fn load_hnl_file_with_env_binds_each_env_independently() {
    ensure_lang();
    let dir = std::env::temp_dir().join(format!("hnl-stdlib-env-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    std::fs::write(dir.join("dep.hnl"), DEP_HNL).expect("write dep.hnl");
    std::fs::write(dir.join("main.hnl"), MAIN_HNL).expect("write main.hnl");
    let main = dir.join("main.hnl");

    // The path-keyed singleton resolves the file's own `./dep.hnl` import.
    let base = load_hnl_file(&main).expect("base loads");
    assert_eq!(call(&*base, "ask", json!("x")), json!("real:x"));

    let env_a = ModuleEnv::new().with_dependency(DepStub::new("alpha"));
    let a = load_hnl_file_with_env(&main, &env_a).expect("env a instance");
    let b = load_hnl_file_with_env(&main, &ModuleEnv::new().with_dependency(DepStub::new("beta")))
        .expect("env b instance");

    assert_ne!(a.module_id(), b.module_id());
    assert_ne!(a.module_id(), base.module_id());
    assert_eq!(call(&*a, "ask", json!("x")), json!("alpha:x"));
    assert_eq!(call(&*b, "ask", json!("x")), json!("beta:x"));
    // Re-dispatch through `a` after `b` ran: distinct module_ids keep the
    // per-thread runtime caches apart.
    assert_eq!(call(&*a, "ask", json!("y")), json!("alpha:y"));
    // The path-keyed singleton is untouched.
    assert_eq!(call(&*base, "ask", json!("x")), json!("real:x"));

    // A clone is the same env → the same cached instance.
    let a_again = load_hnl_file_with_env(&main, &env_a.clone()).expect("clone instance");
    assert!(Arc::ptr_eq(&a, &a_again), "env clones share the instance");

    std::fs::remove_dir_all(&dir).ok();
}
