//! ADR-0155 Phase 0 — scope-bound env payload: wrapper arrows over
//! `callv` bodies, applied through the *applying* runtime's module chain.
//!
//! The Phase-0 proof has two halves:
//!
//! - a [`wrap_dep_fn`]-minted arrow applies through a `HandlModule`
//!   runtime whose dep chain contains the stubbed module — the minted
//!   `callv` body lands on the same `call_host` leaf an unbound
//!   `dep.fn(…)` callee does; and
//! - `callv` splats *variably*: the wrapper's `rest`-bound args array
//!   forwards to the leaf as the whole argument list, so a multi-arg dep
//!   function receives separate arguments (a `call`-node body would
//!   deliver one packed array instead — the splat subtlety of ADR §1).
//!
//! The wrapper crosses the `Function::call` boundary as its
//! `{"__arrow__": …}` wire form, so each test also proves the marshal
//! story: the arrow is ordinary serializable data that re-decodes to a
//! callable value inside the runtime.

use std::rc::Rc;
use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function, Module, ModuleLookup};
use handl_plugin_lib_handl::{HandlModule, ModuleEnv, wrap_dep_fn, wrap_dep_module};
use serde_json::{Value, json};

/// Install the canonical default language once per test binary (see
/// `tests/multi_env.rs`).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

/// A `dep` stub with a nullary-tolerant marker (`who`) and a strict
/// two-arg function (`cat`): `cat`'s arity check is what makes a failed
/// splat observable — a packed-array call would reach it as one arg.
struct StubDep {
    who: StubWho,
    cat: StubCat,
}

struct StubWho(&'static str);
struct StubCat;

impl StubDep {
    fn new(marker: &'static str) -> Self {
        Self {
            who: StubWho(marker),
            cat: StubCat,
        }
    }
}

impl Function for StubWho {
    fn call(
        &self,
        args: &[Value],
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        if args.is_empty() {
            Ok(json!(self.0))
        } else {
            let joined: Vec<String> = args
                .iter()
                .map(|a| a.as_str().unwrap_or("?").to_string())
                .collect();
            Ok(json!(format!("{}:{}", self.0, joined.join(","))))
        }
    }
}

impl Function for StubCat {
    fn call(
        &self,
        args: &[Value],
        _ctx: &mut EvalContext,
    ) -> Result<Value, EvalError> {
        if args.len() != 2 {
            return Err(EvalError::Other(format!(
                "dep.cat expects 2 args, got {}",
                args.len()
            )));
        }
        Ok(json!(format!(
            "{}|{}",
            args[0].as_str().unwrap_or("?"),
            args[1].as_str().unwrap_or("?")
        )))
    }
}

impl Module for StubDep {
    fn name(&self) -> &str {
        "dep"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "who" => Some(&self.who),
            "cat" => Some(&self.cat),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![("who", &self.who), ("cat", &self.cat)]
    }
}

/// The importer module: each export applies a *value* passed in on the
/// call frame — `f`/`m` are bound to wrapper arrows / an env record that
/// crossed the `serde_json` boundary as ordinary data.
const SRC: &str = r#"
export ping := (f) ~> f()
export apply := (f, x) ~> f(x)
export apply2 := (f, a, b) ~> f(a, b)
export ask := (m, x) ~> m.who() + ":" + x
"#;

const FUNCS: &[&str] = &["ping", "apply", "apply2", "ask"];

fn module() -> HandlModule {
    HandlModule::new("m", SRC, FUNCS).with_dependency(StubDep::new("orig"))
}

fn call(m: &HandlModule, f: &str, args: &[Value]) -> Value {
    m.function(f)
        .unwrap_or_else(|| panic!("{f} bound"))
        .call(args, &mut EvalContext::new())
        .unwrap_or_else(|e| panic!("{f} evaluates: {e}"))
}

/// `wrap_dep_fn` mints the `{"__arrow__": …}` record the ADR specifies:
/// `params: []`, `rest: "args"`, a two-child `callv` `body_def`.
#[test]
fn wrap_dep_fn_mints_the_wrapper_shape() {
    let wire = Value::from(wrap_dep_fn("dep.who"));
    assert_eq!(wire["__arrow__"], json!(true));
    assert_eq!(wire["params"], json!([]));
    assert_eq!(wire["rest"], json!("args"));
    // The name lives inside `body_def` as a `lit` payload — ambient data,
    // not a captured binding.
    assert!(
        wire["body_def"]["__ast__"]["nodes"].is_array(),
        "body_def is a tagged Ast arena: {}",
        wire["body_def"]
    );
    let body_src = serde_json::to_string(&wire["body_def"]).unwrap();
    assert!(body_src.contains("\"callv\""), "body_def: {body_src}");
    assert!(body_src.contains("\"dep.who\""), "body_def: {body_src}");
    // No captures: the name stays ambient and re-resolves at apply time.
    assert!(wire.get("binds").is_none() || wire["binds"] == json!({}));
}

/// A minted wrapper applied inside a runtime whose module chain holds
/// `dep` re-resolves `"dep.who"` through the chain and returns the dep's
/// result — for zero and one forwarded arguments.
#[test]
fn wrapper_arrow_applies_through_the_module_chain() {
    ensure_lang();
    let m = module();
    let wrapper = Value::from(wrap_dep_fn("dep.who"));

    // f() — the rest-bound args array is empty.
    assert_eq!(call(&m, "ping", &[wrapper.clone()]), json!("orig"));
    // f("x") — one forwarded arg.
    assert_eq!(
        call(&m, "apply", &[wrapper, json!("x")]),
        json!("orig:x")
    );
}

/// `callv` forwards the *whole* args array: a two-arg dep function sees
/// two arguments, not one packed array.
#[test]
fn callv_splat_forwards_multi_arg_calls() {
    ensure_lang();
    let m = module();
    let wrapper = Value::from(wrap_dep_fn("dep.cat"));
    assert_eq!(
        call(&m, "apply2", &[wrapper, json!("a"), json!("b")]),
        json!("a|b")
    );
}

/// A wrapper for a name the chain cannot resolve fails at *call time*
/// with the `call_host` leaf error — the same failure mode an unbound
/// dotted callee produces (ADR-0155 §Consequences).
#[test]
fn wrapper_for_an_undeclared_name_fails_at_call_time() {
    ensure_lang();
    let m = module();
    let wrapper = Value::from(wrap_dep_fn("ghost.missing"));
    let err = m
        .function("apply")
        .unwrap()
        .call(&[wrapper, json!("x")], &mut EvalContext::new())
        .expect_err("unresolvable name must fail at call time");
    assert!(
        err.to_string().contains("ghost.missing"),
        "error names the unresolved callee: {err}"
    );
}

/// `wrap_dep_module` produces the env record (`{fn: <arrow>, …}`) and a
/// `record.field(args)` call — the end-state call shape of §Decision —
/// applies through the ordinary `!App` path.
#[test]
fn env_record_field_applies_through_field_access() {
    ensure_lang();
    let m = module();
    let record = wrap_dep_module(&StubDep::new("orig"));
    // Record shape: one arrow per function export.
    let wire = Value::from(record.clone());
    assert_eq!(wire["who"]["__arrow__"], json!(true));
    assert_eq!(wire["cat"]["__arrow__"], json!(true));
    // `m.who() + ":" + x` — record field access yields the wrapper arrow,
    // `call` routes the non-string callee to `!App`, `callv` lands on
    // `call_host` with zero args.
    assert_eq!(call(&m, "ask", &[Value::from(record), json!("x")]), json!("orig:x"));
}

/// The bytecode engine compiles the `callv` scheme like any other
/// `Seq`/`Op` composition — the same smoke test passes with the VM
/// eval engine.
#[test]
fn wrapper_arrow_applies_on_bytecode() {
    ensure_lang();
    let m = module().with_eval_engine("bytecode");
    let wrapper = Value::from(wrap_dep_fn("dep.who"));
    assert_eq!(call(&m, "apply", &[wrapper, json!("x")]), json!("orig:x"));
    let cat = Value::from(wrap_dep_fn("dep.cat"));
    assert_eq!(
        call(&m, "apply2", &[cat, json!("a"), json!("b")]),
        json!("a|b")
    );
}

// ---------------------------------------------------------------------------
// Phase 1 — resolvers emit the callable record (ADR-0155 §3)
// ---------------------------------------------------------------------------

/// The resolver-emitted record's function fields are `{"__arrow__":…}`
/// wire values, while the legacy `module_bindings_object` fields stay
/// `"mod.fn"` strings — both payload encodings coexist, per ADR-0155 §2.
#[test]
fn module_bindings_tagged_emits_arrows_not_strings() {
    let dep = StubDep::new("orig");
    let tagged = handl_plugin_lib_handl::module_bindings_tagged(&dep);
    let wire = Value::from(tagged.as_ref().clone());
    assert_eq!(wire["who"]["__arrow__"], json!(true));
    assert_eq!(wire["cat"]["__arrow__"], json!(true));
    // The string encoding is unchanged for hosts that pin it.
    let legacy = handl_plugin_lib_handl::module_bindings_object(&dep);
    assert_eq!(legacy["who"], json!("dep.who"));
    assert_eq!(legacy["cat"], json!("dep.cat"));
}

/// `from "dep" import cat` binds a **wrapper arrow** (not the string
/// `"dep.cat"`): `cat(a, b)` routes `route_on_type` → `!App`, and the
/// `callv` body lands on `call_host` with the two forwarded args. The
/// `* as d` form binds the whole callable record, so `d.who()` applies
/// through ordinary field access.
#[test]
fn dependency_resolver_binds_callable_arrows() {
    ensure_lang();
    const IMPORT_SRC: &str = r#"
from "dep" import cat
from "dep" import * as d
export pair := (a, b) ~> cat(a, b)
export ask2 := (x) ~> d.who() + ":" + x
"#;
    let m = HandlModule::new("imp", IMPORT_SRC, &["pair", "ask2"])
        .with_dependency(StubDep::new("orig"));
    assert_eq!(call(&m, "pair", &[json!("a"), json!("b")]), json!("a|b"));
    assert_eq!(call(&m, "ask2", &[json!("x")]), json!("orig:x"));
}

/// The two encodings are dispatch-equivalent: a bare `"dep.who"` string
/// bound as `f` still applies through `call`'s string arm (`call_host`),
/// and a `wrap_dep_fn` arrow for the same name applies through `!App` —
/// same leaf, same result (ADR-0155 §2 value-first/string-fallback).
#[test]
fn both_binding_encodings_apply_through_call() {
    ensure_lang();
    let m = module();
    // Legacy string callee — the `host_import_lines` lowering's encoding.
    assert_eq!(
        call(&m, "apply", &[json!("dep.who"), json!("x")]),
        json!("orig:x")
    );
    // Callable-record field — the resolver's Phase-1 payload.
    assert_eq!(
        call(
            &m,
            "apply",
            &[Value::from(wrap_dep_fn("dep.who")), json!("x")]
        ),
        json!("orig:x")
    );
}

// ---------------------------------------------------------------------------
// Phase 2 — dep names bound scope-first in the module runtime (ADR-0155 §3-4)
// ---------------------------------------------------------------------------

/// Mint an arrow that ignores its arguments and returns `result` — a
/// `lit`-body counterpart of `wrap_dep_fn`, used to give a dep a *bespoke*
/// scope record that answers differently than its chain dispatch, so a
/// `dep.f(x)` call reports which resolution path it took.
fn const_arrow(result: &str) -> handl_value::TaggedValue {
    let mut body = handl_language::Ast::new();
    body.push(handl_language::AstNode::new(handl_language::LIT_KIND).with_payload(json!(result)));
    handl_value::TaggedValue::Arrow {
        params: Rc::from(Vec::<String>::new()),
        rest: Some(Rc::from("args")),
        array_pattern: false,
        body_def: Rc::new(handl_value::TaggedValue::from(
            handl_language::ast_value(&body),
        )),
        binds: Default::default(),
        handlers: Default::default(),
    }
}

/// A `dep` module whose **scope record** deliberately disagrees with its
/// **chain dispatch**: `bindings_tagged` hands the env a record whose
/// `who` field is a const arrow answering `"scope"`, while the module
/// registry's `dep.who` (reached through the `call_host` leaf) still
/// answers `"chain"`/`"chain:…"`. The `cat` field stays the standard
/// `wrap_dep_fn` wrapper, so it still leaf-dispatches to the chain.
///
/// The record is kept as its `serde_json::Value` wire form — `Rc`-shared
/// `TaggedValue`s are not `Send`, and dep modules must be `Send + Sync`.
struct ShadowedDep(StubDep, Value);

fn shadowed_dep() -> ShadowedDep {
    let record = json!({
        "who": Value::from(const_arrow("scope")),
        "cat": Value::from(wrap_dep_fn("dep.cat")),
    });
    ShadowedDep(StubDep::new("chain"), record)
}

impl Module for ShadowedDep {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        self.0.function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        self.0.functions()
    }

    fn bindings_tagged(&self) -> Option<Rc<handl_value::TaggedValue>> {
        Some(Rc::new(handl_value::TaggedValue::from(self.1.clone())))
    }
}

/// A module whose exports exercise the dep-name paths Phase 2 rewires:
/// `who`/`cat` are first-hop dep calls, `probe` reaches a name that is
/// *not* a declared dep, `shadow` takes a param that collides with the
/// dep name.
const DEP_CALL_SRC: &str = r#"
export who := (x) ~> dep.who() + ":" + x
export cat := (a, b) ~> dep.cat(a, b)
export probe := (x) ~> ghost.f(x)
export shadow := (dep) ~> dep
"#;

const DEP_CALL_FUNCS: &[&str] = &["who", "cat", "probe", "shadow"];

/// `dep.f(x)` resolves **scope-first**: the env record bound in the
/// runtime's eval globals wins over the `ModuleLookup` chain the
/// unbound-identifier fallback would have consulted (ADR-0155 §3). The
/// record's `who` field is the env's bespoke arrow — `"scope"`, not
/// `"chain"` — while `cat` stays a standard wrapper and still
/// leaf-dispatches `dep.cat` to the chain's real answer.
#[test]
fn dep_calls_resolve_scope_first_via_env_record() {
    ensure_lang();
    let m = HandlModule::new("m", DEP_CALL_SRC, DEP_CALL_FUNCS).with_dependency(shadowed_dep());
    assert_eq!(call(&m, "who", &[json!("x")]), json!("scope:x"));
    assert_eq!(call(&m, "cat", &[json!("a"), json!("b")]), json!("a|b"));
}

/// The env record is bound during the module-record eval itself: a
/// field computed from a dep at record-construction time resolves
/// scope-first through the same globals channel (`Module::value`
/// projections ride it too — the record is built inside the module's
/// own runtime).
#[test]
fn env_binds_during_module_record_eval() {
    ensure_lang();
    const EAGER_SRC: &str = "export eager := dep.who(\"E\")";
    let m = HandlModule::new("m", EAGER_SRC, &[])
        .with_dependency(shadowed_dep())
        .with_value_names(&["eager"]);
    assert_eq!(m.value("eager"), Some(json!("scope")));
}

/// A local binding shadows the dep env record: `shadow`'s `dep` param
/// binds in a scope *above* the engine-globals layer, so the record is
/// invisible and the arg value passes through unchanged.
#[test]
fn local_bindings_shadow_the_env_record() {
    ensure_lang();
    let m = HandlModule::new("m", DEP_CALL_SRC, DEP_CALL_FUNCS).with_dependency(shadowed_dep());
    assert_eq!(call(&m, "shadow", &[json!("x")]), json!("x"));
}

/// A name that is *not* a declared dep never enters the env record — it
/// still resolves through the `global_lookup` fallback layer as a
/// string callee, so the ambient surface stays intact (ADR-0155 §2:
/// `proj_env`'s unbound→string fallback is unchanged).
#[test]
fn undeclared_names_still_fall_back_through_the_chain() {
    ensure_lang();
    struct GhostFn;
    impl Function for GhostFn {
        fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
            Ok(json!(format!(
                "table:{}",
                args[0].as_str().unwrap_or("?")
            )))
        }
    }
    struct GhostLookup {
        f: GhostFn,
    }
    impl ModuleLookup for GhostLookup {
        fn lookup(&self, name: &str) -> Option<&dyn Function> {
            (name == "ghost.f").then_some(&self.f as &dyn Function)
        }
    }
    let m = HandlModule::new("m", DEP_CALL_SRC, DEP_CALL_FUNCS)
        .with_dependency(StubDep::new("orig"))
        .with_global_lookup(Arc::new(GhostLookup { f: GhostFn }));
    // `dep.*` still works through its env record → wrapper → chain leaf;
    // `ghost.f` was never bound — it falls back to the global lookup.
    assert_eq!(call(&m, "who", &[json!("x")]), json!("orig:x"));
    assert_eq!(call(&m, "probe", &[json!("x")]), json!("table:x"));
}

/// Two envs, one source: `instantiate` re-keys the per-thread caches, so
/// the same `dep.who` call scope-resolves against each instance's own
/// env — the leaf answer differs per env exactly as it does on the
/// chain path (ADR-0155 §5: env-keyed caching by `module_id`).
#[test]
fn instantiated_modules_scope_resolve_against_their_own_env() {
    ensure_lang();
    const WHO_SRC: &str = "export who := (x) ~> dep.who() + \":\" + x";
    let m = HandlModule::new("m", WHO_SRC, &["who"]);
    let a = m.instantiate(ModuleEnv::new().with_dependency(StubDep::new("A")));
    let b = m.instantiate(ModuleEnv::new().with_dependency(StubDep::new("B")));
    assert_eq!(call(&a, "who", &[json!("x")]), json!("A:x"));
    assert_eq!(call(&b, "who", &[json!("x")]), json!("B:x"));
}

/// The bytecode engine merges engine globals into its `ScopeStack`
/// globals the same way — scope-first dep resolution is
/// engine-agnostic (the loader's eval engine).
#[test]
fn dep_calls_resolve_scope_first_on_bytecode() {
    ensure_lang();
    let m = HandlModule::new("m", DEP_CALL_SRC, DEP_CALL_FUNCS)
        .with_dependency(shadowed_dep())
        .with_eval_engine("bytecode");
    assert_eq!(call(&m, "who", &[json!("x")]), json!("scope:x"));
    assert_eq!(call(&m, "cat", &[json!("a"), json!("b")]), json!("a|b"));
}

// ---------------------------------------------------------------------------
// Phase 3 — ambient names as explicit default-env globals (ADR-0155 §2.4/§6)
// ---------------------------------------------------------------------------

/// A module source where `ghost` is a *default-env* name (not a declared
/// dep) and `dep`/`param` probe the layering.
const DEFAULT_ENV_SRC: &str = r#"
export who := (x) ~> ghost.who() + ":" + x
export depw := (x) ~> dep.who() + ":" + x
export flag := () ~> cfg.enabled
export probe := (x) ~> nomod.f(x)
"#;

const DEFAULT_ENV_FUNCS: &[&str] = &["who", "depw", "flag", "probe"];

/// A default env binding `ghost` (a bespoke record — `who` answers
/// `"env"` rather than the chain's `"chain:…"`, so scope-first vs leaf
/// resolution is observable) and `cfg` (a plain value field — env
/// records carry non-callable fields too, parity with dep envs).
fn test_default_env() -> handl_plugin_lib_handl::DefaultEnv {
    let record = json!({
        "who": Value::from(const_arrow("env")),
        "cat": Value::from(wrap_dep_fn("ghost.cat")),
    });
    Arc::new(move || {
        let mut globals = std::collections::HashMap::new();
        globals.insert("ghost".to_string(), record.clone());
        globals.insert("cfg".to_string(), json!({"enabled": true}));
        globals
    })
}

/// The leaf `ghost.*` re-resolves through — the default env's `who`
/// bypasses it (const arrow), while `cat` is a standard wrapper landing
/// on `call_host("ghost.cat")`.
struct GhostModule {
    who: GhostWho,
    cat: GhostCat,
}
struct GhostWho;
struct GhostCat;

impl Function for GhostWho {
    fn call(&self, _args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        Ok(json!("chain"))
    }
}
impl Function for GhostCat {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        Ok(json!(format!("chain:{}", args[0].as_str().unwrap_or("?"))))
    }
}
impl Module for GhostModule {
    fn name(&self) -> &str {
        "ghost"
    }
    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "who" => Some(&self.who),
            "cat" => Some(&self.cat),
            _ => None,
        }
    }
    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![("who", &self.who), ("cat", &self.cat)]
    }
}

/// A name the default env binds resolves **scope-first** — the env
/// record's `who` answers `"env"`, not the chain's `"chain"`. The env
/// carries no `Module` for `ghost` at all: only the record.
#[test]
fn default_env_binds_ambient_names_scope_first() {
    ensure_lang();
    let m = HandlModule::new("m", DEFAULT_ENV_SRC, DEFAULT_ENV_FUNCS)
        .with_default_env(test_default_env());
    assert_eq!(call(&m, "who", &[json!("x")]), json!("env:x"));
    assert_eq!(call(&m, "flag", &[]), json!(true));
}

/// A declared dep shadows a default-env binding of the same name:
/// `build_runtime` merges dep globals over the default layer
/// (ADR-0155 §2: "the ambient becomes shadowed by declared deps and
/// imports"). `dep` is bound by both — the dep's record answers.
#[test]
fn declared_deps_shadow_the_default_env() {
    ensure_lang();
    let default_env: handl_plugin_lib_handl::DefaultEnv = {
        let record = json!({
            "who": Value::from(const_arrow("default")),
        });
        Arc::new(move || {
            let mut globals = std::collections::HashMap::new();
            globals.insert("dep".to_string(), record.clone());
            globals.insert("ghost".to_string(), {
                json!({"who": Value::from(const_arrow("env"))})
            });
            globals
        })
    };
    let m = HandlModule::new("m", DEFAULT_ENV_SRC, DEFAULT_ENV_FUNCS)
        .with_default_env(default_env)
        .with_dependency(shadowed_dep());
    // The dep's bespoke record wins over the default layer.
    assert_eq!(call(&m, "depw", &[json!("x")]), json!("scope:x"));
    // …while non-shadowed default-env names still bind scope-first.
    assert_eq!(call(&m, "who", &[json!("x")]), json!("env:x"));
}

/// A name bound by **no** layer — not a dep, not a default env, not a
/// `global_lookup` hit — fails loudly at the `call_host` leaf
/// (`function 'nomod.f' not found`): the ambient is explicit now.
#[test]
fn unlisted_names_fail_at_the_leaf() {
    ensure_lang();
    let m = HandlModule::new("m", DEFAULT_ENV_SRC, DEFAULT_ENV_FUNCS)
        .with_default_env(test_default_env());
    let err = m
        .function("probe")
        .unwrap()
        .call(&[json!("x")], &mut EvalContext::new())
        .expect_err("unlisted callee must fail");
    assert!(
        err.to_string().contains("nomod.f"),
        "error names the unresolved callee: {err}"
    );
}

/// A default-env wrapper field that does resolve through the leaf lands
/// on the same `call_host` dispatch as a declared dep's — the env only
/// changes *where the name lives*, not where it dispatches.
#[test]
fn default_env_wrappers_re_resolve_through_the_leaf() {
    ensure_lang();
    let ghost: Arc<dyn handl_platform::ModuleLookup + Send + Sync> = Arc::new({
        struct L(GhostModule);
        impl ModuleLookup for L {
            fn lookup(&self, name: &str) -> Option<&dyn Function> {
                let (m, f) = name.split_once('.')?;
                (m == "ghost").then(|| self.0.function(f)).flatten()
            }
        }
        L(GhostModule {
            who: GhostWho,
            cat: GhostCat,
        })
    });
    // Env record for `ghost` is a *plain* wrapper record (no bespoke
    // arrows): `ghost.cat` applies through the chain like any dep.
    let default_env: handl_plugin_lib_handl::DefaultEnv = Arc::new(move || {
        let mut globals = std::collections::HashMap::new();
        globals.insert(
            "ghost".to_string(),
            json!({"cat": Value::from(wrap_dep_fn("ghost.cat"))}),
        );
        globals
    });
    const SRC: &str = "export cat := (x) ~> ghost.cat(x)";
    let m = HandlModule::new("m", SRC, &["cat"])
        .with_default_env(default_env)
        .with_global_lookup(ghost);
    assert_eq!(call(&m, "cat", &[json!("x")]), json!("chain:x"));
}
