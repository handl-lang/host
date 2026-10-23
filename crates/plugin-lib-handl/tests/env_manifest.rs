//! ADR-0156: `ModuleEnv::from_manifest` — declarative env construction
//! from a `pkg.hnl` `deps` record, resolved through a `ModuleResolver`
//! (the data-driven counterpart of hand-rolled
//! `ModuleEnv::new().with_dependency_arc(..)` chains).

use std::collections::HashMap;
use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::{HandlModule, ModuleEnv, ModuleResolver};

/// Install the canonical default language once per test binary.
///
/// `HandlModule` bridge runtimes resolve [`default_language`] lazily
/// (ADR-0128 §"The 0.8 seam"); tests assemble it from the dev-dep data
/// crate (dev-deps may link `lang/` data; the library must not).
///
/// [`default_language`]: handl_plugin_lib_handl::default_language
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

const SRC: &str = r#"from "alpha" import * as alpha
from "beta" import * as beta
export ask := (x) ~> alpha.who() + "/" + beta.who() + ":" + x
"#;

/// A named dep stub whose `who` answers a marker string.
struct StubDep {
    name: String,
    who: StubWho,
}

struct StubWho(&'static str);

impl StubDep {
    fn arc(name: &str, marker: &'static str) -> Arc<dyn Module + Send + Sync> {
        Arc::new(Self {
            name: name.to_string(),
            who: StubWho(marker),
        })
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
        &self.name
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        (name == "who").then_some(&self.who as &dyn Function)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![("who", &self.who)]
    }
}

/// Maps declared dep names to stub modules — the `ModuleResolver` a
/// manifest resolves through.
struct StubResolver {
    modules: HashMap<String, Arc<dyn Module + Send + Sync>>,
}

impl StubResolver {
    fn new(deps: &[(&str, &'static str)]) -> Self {
        Self {
            modules: deps
                .iter()
                .map(|(name, marker)| ((*name).to_string(), StubDep::arc(name, marker)))
                .collect(),
        }
    }
}

impl ModuleResolver for StubResolver {
    fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
        self.modules.get(name).cloned()
    }
}

fn call_ask(m: &HandlModule, arg: &str) -> serde_json::Value {
    m.function("ask")
        .expect("ask bound")
        .call(&[serde_json::json!(arg)], &mut EvalContext::new())
        .expect("ask evaluates")
}

fn deps(spec: &str) -> Vec<(String, String)> {
    vec![
        ("alpha".to_string(), spec.to_string()),
        ("beta".to_string(), spec.to_string()),
    ]
}

/// Both declared names resolve: the env binds each manifest dep, and the
/// same prepared source instantiated under it reaches every one.
#[test]
fn from_manifest_resolves_every_declared_dep() {
    ensure_lang();
    let resolver = StubResolver::new(&[("alpha", "A"), ("beta", "B")]);
    let env =
        ModuleEnv::from_manifest(&deps("branch:main"), &resolver).expect("both deps resolve");

    let m = HandlModule::new("m", SRC, &["ask"]).instantiate(env);
    assert_eq!(call_ask(&m, "x"), serde_json::json!("A/B:x"));
}

/// A name the resolver does not know is an error listing *every* missing
/// name — nothing is skipped silently, and resolved names are not
/// reported.
#[test]
fn from_manifest_names_every_missing_dep() {
    let resolver = StubResolver::new(&[("alpha", "A")]);
    let declared = vec![
        ("alpha".to_string(), "branch:main".to_string()),
        ("beta".to_string(), "branch:main".to_string()),
        ("gamma".to_string(), "rev:abc123".to_string()),
    ];
    let err = match ModuleEnv::from_manifest(&declared, &resolver) {
        Ok(_) => panic!("beta and gamma are unresolvable"),
        Err(e) => e,
    };
    let msg = err.to_string();
    assert!(msg.contains("beta"), "error names beta: {msg}");
    assert!(msg.contains("gamma"), "error names gamma: {msg}");
    assert!(msg.contains("rev:abc123"), "error carries the spec: {msg}");
    assert!(!msg.contains("alpha"), "alpha resolved, not missing: {msg}");
}

/// Each `from_manifest` call is a new env: one fresh `env_id` per
/// construction, distinct from every other env — and the `with_*`
/// builders still re-mint on top for overrides.
#[test]
fn from_manifest_mints_one_fresh_env_id() {
    let resolver = StubResolver::new(&[("alpha", "A"), ("beta", "B")]);
    let a = ModuleEnv::from_manifest(&deps("branch:main"), &resolver).expect("deps resolve");
    let b = ModuleEnv::from_manifest(&deps("branch:main"), &resolver).expect("deps resolve");
    assert_ne!(a.env_id(), b.env_id());
    assert_ne!(a.env_id(), ModuleEnv::new().env_id());
    // Overrides still build: layering a dep reconfigures the env and
    // re-mints its id (the contents changed).
    assert_ne!(
        a.clone().with_dependency_arc(StubDep::arc("extra", "E")).env_id(),
        a.env_id()
    );
    // An empty manifest is a valid (empty) env, still fresh.
    let empty = ModuleEnv::from_manifest(&[], &resolver).expect("empty manifest is valid");
    assert_ne!(empty.env_id(), a.env_id());
}

/// `from_pkg_hnl` reads the `deps` record out of a `pkg.hnl` source —
/// the manifest form `from_manifest` consumes — and is lenient where
/// `resolve_pkg_dependencies` is: no (or malformed) `deps` table means
/// an empty env, not an error.
#[test]
fn from_pkg_hnl_builds_the_env_from_a_pkg_manifest() {
    ensure_lang();
    let resolver = StubResolver::new(&[("alpha", "A"), ("beta", "B")]);
    // A lib-tensor-style manifest: comment header + deps record.
    let pkg = "--- deps of this package\n{ deps: { alpha: \"branch:main\", beta: \"branch:main\" } }\n";
    let env = ModuleEnv::from_pkg_hnl(pkg, &resolver).expect("manifest deps resolve");
    let m = HandlModule::new("m", SRC, &["ask"]).instantiate(env);
    assert_eq!(call_ask(&m, "x"), serde_json::json!("A/B:x"));

    // No `deps` table → an empty env: the source's deps do not resolve
    // under it (nothing is skipped silently — there is nothing declared).
    let modules_only = ModuleEnv::from_pkg_hnl("{ modules: [\"cli\"] }", &resolver)
        .expect("a manifest without deps is valid");
    let bare = HandlModule::new("m", SRC, &["ask"]).instantiate(modules_only);
    assert!(
        bare.function("ask")
            .expect("ask bound")
            .call(&[serde_json::json!("x")], &mut EvalContext::new())
            .is_err(),
        "empty env must not resolve `alpha`/`beta`"
    );

    // A malformed manifest is likewise lenient (empty env, no error).
    assert!(ModuleEnv::from_pkg_hnl("{ deps: { alpha: ", &resolver).is_ok());
}
