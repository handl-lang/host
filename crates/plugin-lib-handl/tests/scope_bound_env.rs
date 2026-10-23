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

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::{HandlModule, wrap_dep_fn, wrap_dep_module};
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
