//! ADR-0156: `HandlModule::instantiate` runs the same prepared source
//! under multiple dependency environments — one file, several dep
//! bindings (stub/mock/A-B dep surfaces).

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::{HandlModule, ModuleEnv};

/// Install the canonical default language once per test binary.
///
/// `HandlModule` bridge runtimes resolve [`default_language`] lazily
/// (ADR-0128 §"The 0.8 seam"); tests assemble it from the dev-dep data
/// crate (dev-deps may link `lang/` data; the library must not).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

const SRC: &str = r#"from "dep" import * as dep
export ask := (x) ~> dep.who() + ":" + x
"#;

/// A `dep` stub whose `who` answers a marker string.
struct StubDep {
    who: StubWho,
}

struct StubWho(&'static str);

impl StubDep {
    fn new(marker: &'static str) -> Self {
        Self {
            who: StubWho(marker),
        }
    }
}

impl Function for StubWho {
    fn call(
        &self,
        _args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        Ok(serde_json::json!(self.0))
    }
}

impl Module for StubDep {
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

fn call_ask(m: &HandlModule, arg: &str) -> serde_json::Value {
    m.function("ask")
        .expect("ask bound")
        .call(&[serde_json::json!(arg)], &mut EvalContext::new())
        .expect("ask evaluates")
}

/// The same prepared source instantiated under two `dep` bindings yields
/// each env's answer, and the original module keeps its own env.
#[test]
fn instantiate_binds_each_env_independently() {
    ensure_lang();
    let module = HandlModule::new("m", SRC, &["ask"]).with_dependency(StubDep::new("orig"));

    let a = module.instantiate(ModuleEnv::new().with_dependency(StubDep::new("alpha")));
    let b = module.instantiate(ModuleEnv::new().with_dependency(StubDep::new("beta")));

    // Each instance re-keys the per-thread runtime/AST caches.
    assert_ne!(a.module_id(), b.module_id());
    assert_ne!(a.module_id(), module.module_id());

    assert_eq!(call_ask(&a, "x"), serde_json::json!("alpha:x"));
    assert_eq!(call_ask(&b, "x"), serde_json::json!("beta:x"));
    // Re-dispatch through `a` after `b` ran: the instances hold distinct
    // module_ids, so neither cache entry was clobbered.
    assert_eq!(call_ask(&a, "y"), serde_json::json!("alpha:y"));
    // The original module's env is untouched by `instantiate`.
    assert_eq!(call_ask(&module, "x"), serde_json::json!("orig:x"));
}

/// `instantiate` replaces the env wholesale: the source module's deps do
/// not leak into the instance.
#[test]
fn instantiate_does_not_carry_over_source_env() {
    ensure_lang();
    let module = HandlModule::new("m", SRC, &["ask"]).with_dependency(StubDep::new("orig"));

    let only = module.instantiate(ModuleEnv::new());
    assert!(
        only.function("ask")
            .expect("ask bound")
            .call(&[serde_json::json!("x")], &mut EvalContext::new())
            .is_err(),
        "empty env must not resolve `dep`"
    );
}

/// `env()` round-trips: instantiating under the source module's own env
/// yields an equivalent sibling, and a dep can be layered on top.
#[test]
fn env_round_trip_clones_the_env() {
    ensure_lang();
    let module = HandlModule::new("m", SRC, &["ask"]).with_dependency(StubDep::new("orig"));

    let same = module.instantiate(module.env());
    assert_ne!(same.module_id(), module.module_id());
    assert_eq!(call_ask(&same, "x"), serde_json::json!("orig:x"));
}

/// `instantiate` shares name, exports and value names with the source
/// module.
#[test]
fn instantiate_shares_module_surface() {
    ensure_lang();
    let module = HandlModule::new("m", SRC, &["ask"])
        .with_dependency(StubDep::new("orig"))
        .with_value_names(&["ask"]);
    let a = module.instantiate(ModuleEnv::new().with_dependency(StubDep::new("alpha")));

    assert_eq!(a.name(), "m");
    assert_eq!(a.value_names(), vec!["ask".to_string()]);
    let names: Vec<&str> = a.functions().iter().map(|(n, _)| *n).collect();
    assert_eq!(names, vec!["ask"]);
}

/// Env identity (ADR-0156 follow-up): `env_id` is what env-aware caches
/// above `HandlModule` key on, so one module name can mean several envs.
/// Fresh for `new`/`default`, preserved by `clone`, re-minted by `fork`
/// and by every `with_*` reconfiguration.
#[test]
fn env_id_is_identity_not_content() {
    let a = ModuleEnv::new();
    let b = ModuleEnv::new();
    // Independently created envs are distinct even with identical
    // contents — and `default` allocates fresh too (no shared id-0
    // bucket the env-keyed caches could collapse into).
    assert_ne!(a.env_id(), b.env_id());
    assert_ne!(ModuleEnv::default().env_id(), a.env_id());
    assert_ne!(ModuleEnv::default().env_id(), b.env_id());
    // Clone preserves identity: clones are the same env, so env-keyed
    // caches share one entry across them.
    assert_eq!(a.clone().env_id(), a.env_id());
    // Fork: same deps, deliberately separate identity.
    assert_ne!(a.fork().env_id(), a.env_id());
    // Reconfiguring re-mints: the contents changed, so caches keyed by
    // the old id must not serve entries for the new configuration.
    assert_ne!(
        a.clone().with_dependency(StubDep::new("x")).env_id(),
        a.env_id()
    );
    assert_ne!(
        a.clone()
            .with_dependency_arc(std::sync::Arc::new(StubDep::new("x")))
            .env_id(),
        a.env_id()
    );
}

/// `fork()` keeps the deps but splits the identity: instantiating under a
/// fork yields the same answers through a distinct env id.
#[test]
fn fork_reuses_deps_with_fresh_identity() {
    ensure_lang();
    let module = HandlModule::new("m", SRC, &["ask"]);
    let env = ModuleEnv::new().with_dependency(StubDep::new("alpha"));

    let a = module.instantiate(env.clone());
    let f = module.instantiate(env.fork());

    // Same dep binding → same answers; fresh module per instance.
    assert_eq!(call_ask(&a, "x"), serde_json::json!("alpha:x"));
    assert_eq!(call_ask(&f, "x"), serde_json::json!("alpha:x"));
    assert_ne!(a.module_id(), f.module_id());
    // The fork is a distinct env from the original, a clone is not.
    assert_ne!(env.fork().env_id(), env.env_id());
    assert_eq!(env.clone().env_id(), env.env_id());
}
