//! ADR-0103 Phase 9: meta-model tests.
//!
//! The meta-model (`meta_model.hnl`) is the model that interprets models
//! themselves — it loads, validates, composes, and runs models treating
//! them as *data* (records of handlers). This file exercises the contract
//! of every exported arrow of the meta-model:
//!
//!   - `identity_model` — the identity model (run = identity).
//!   - `run_model` — dispatch to a model's `run` handler.
//!   - `compose` — pipe one model's output into another's input.
//!   - `validate_model` — check a model has a `run` function handler.
//!   - `available_tags` — list the tags a model handles.
//!   - `model_name` — get a model's `name` field, if any.
//!   - `register_model` / `lookup_model` — a name-keyed model registry.
//!   - `pipeline` — left-to-right fold-compose of a model list.
//!   - `parallel` — run two models on the same input, return a pair.
//!   - `trace_model` — run a model recording the boundary dispatch.
//!   - `model_handler` — look up a model's handler for a tag.
//!
//! `load_model` is not exercised here because it requires the host `fs`
//! and kernel `parse`/`ffi` capabilities, which are not wired into the
//! test runtime. All other functions are pure over the runtime `hash_map`,
//! `array`, and `typeof` modules and the host `tag` capability.

use handl_platform::{EvalContext, Module};
use handl_plugin_lib_handl::HandlModule;
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use serde_json::{json, Value};

const META_MODEL_HNL: &str = include_str!("../../../project/design/adr0103/meta_model.hnl");

/// The exported function names of the meta-model.
const META_FUNCS: &[&str] = &[
    "load_model",
    "compose",
    "run_model",
    "validate_model",
    "identity_model",
    "model_handler",
    "available_tags",
    "model_name",
    "register_model",
    "lookup_model",
    "pipeline",
    "parallel",
    "trace_model",
];

/// Build the meta-model as a `HandlModule`.
fn meta_module() -> HandlModule {
    HandlModule::new("meta_model", META_MODEL_HNL, META_FUNCS)
        .with_dependency(stdlib::module("array").expect("array module"))
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(TagModule)
}

/// Call a meta-model function by name with the given arguments.
fn call_meta(name: &str, args: &[Value]) -> Value {
    let module = meta_module();
    let func = module
        .function(name)
        .unwrap_or_else(|| panic!("meta_model has no '{name}' export"));
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx)
        .unwrap_or_else(|e| panic!("meta_model '{name}' failed: {e:?}"))
}

/// A sample term value (the ADR-0103 tagged-record shape) used as the
/// `term` argument to `run_model` / `trace_model`.
fn sample_term() -> Value {
    json!({ "__tag__": "Arr", "value": 42 })
}

// -------------------------------------------------------------------------
// identity_model
// -------------------------------------------------------------------------

#[test]
fn identity_model_returns_a_record_with_run() {
    let model = call_meta("identity_model", &[]);
    assert!(model.is_object(), "identity_model should be an object");
    assert!(
        model.get("run").is_some(),
        "identity_model should have a 'run' handler"
    );
}

#[test]
fn identity_model_run_returns_term_unchanged() {
    let model = call_meta("identity_model", &[]);
    let term = sample_term();
    let result = call_meta("run_model", &[model, term.clone(), Value::Null, json!({})]);
    assert_eq!(result, term);
}

#[test]
fn identity_model_run_preserves_arbitrary_term() {
    let model = call_meta("identity_model", &[]);
    let term = json!({ "__tag__": "Op", "name": "add", "args": [], "source": "Native", "lit": null });
    let result = call_meta("run_model", &[model, term.clone(), Value::Null, json!({})]);
    assert_eq!(result, term);
}

// -------------------------------------------------------------------------
// run_model
// -------------------------------------------------------------------------

#[test]
fn run_model_dispatches_to_run_handler() {
    let model = call_meta("identity_model", &[]);
    let term = json!({ "__tag__": "Arr", "value": 7 });
    let result = call_meta("run_model", &[model, term.clone(), Value::Null, json!({})]);
    assert_eq!(result, term);
}

// -------------------------------------------------------------------------
// validate_model
// -------------------------------------------------------------------------

#[test]
fn validate_model_accepts_identity_model() {
    let model = call_meta("identity_model", &[]);
    let result = call_meta("validate_model", &[model]);
    assert_eq!(result, json!(true));
}

#[test]
fn validate_model_rejects_non_record() {
    let result = call_meta("validate_model", &[json!(42)]);
    assert_eq!(result.get("__tag__"), Some(&json!("ModelError")));
}

#[test]
fn validate_model_rejects_record_without_run() {
    let result = call_meta("validate_model", &[json!({ "foo": 1 })]);
    assert_eq!(result.get("__tag__"), Some(&json!("ModelError")));
}

#[test]
fn validate_model_rejects_non_function_run() {
    let result = call_meta("validate_model", &[json!({ "run": 5 })]);
    assert_eq!(result.get("__tag__"), Some(&json!("ModelError")));
}

// -------------------------------------------------------------------------
// available_tags
// -------------------------------------------------------------------------

#[test]
fn available_tags_lists_run_for_identity_model() {
    let model = call_meta("identity_model", &[]);
    let tags = call_meta("available_tags", &[model]);
    // `hash_map.keys` returns an array containing "run".
    assert!(tags.is_array(), "available_tags should return an array");
    let arr = tags.as_array().unwrap();
    assert!(arr.contains(&json!("run")), "tags should contain 'run': {tags}");
}

#[test]
fn available_tags_empty_for_non_record() {
    let tags = call_meta("available_tags", &[json!(5)]);
    assert_eq!(tags, json!([]));
}

// -------------------------------------------------------------------------
// model_name
// -------------------------------------------------------------------------

#[test]
fn model_name_is_null_for_anonymous_model() {
    let model = call_meta("identity_model", &[]);
    let name = call_meta("model_name", &[model]);
    assert_eq!(name, Value::Null);
}

#[test]
fn model_name_null_for_non_record() {
    let name = call_meta("model_name", &[json!(7)]);
    assert_eq!(name, Value::Null);
}

// -------------------------------------------------------------------------
// register_model / lookup_model
// -------------------------------------------------------------------------

#[test]
fn register_and_lookup_model_roundtrip() {
    let model = call_meta("identity_model", &[]);
    let registry = call_meta("register_model", &[json!("id"), model.clone(), json!({})]);
    assert!(registry.is_object());
    let looked_up = call_meta("lookup_model", &[json!("id"), registry]);
    assert!(
        looked_up.is_object(),
        "looked-up model should be an object: {looked_up}"
    );
    assert!(looked_up.get("run").is_some());
}

#[test]
fn register_model_stamps_name() {
    let model = call_meta("identity_model", &[]);
    let registry = call_meta("register_model", &[json!("my_model"), model, json!({})]);
    let looked_up = call_meta("lookup_model", &[json!("my_model"), registry]);
    assert_eq!(looked_up.get("name"), Some(&json!("my_model")));
}

#[test]
fn lookup_model_returns_null_for_missing() {
    let looked_up = call_meta("lookup_model", &[json!("nope"), json!({})]);
    assert_eq!(looked_up, Value::Null);
}

#[test]
fn lookup_model_null_for_non_record_registry() {
    let looked_up = call_meta("lookup_model", &[json!("x"), json!(3)]);
    assert_eq!(looked_up, Value::Null);
}

// -------------------------------------------------------------------------
// compose
// -------------------------------------------------------------------------

#[test]
fn compose_identity_with_identity_is_identity() {
    let id = call_meta("identity_model", &[]);
    let composed = call_meta("compose", &[id.clone(), id.clone()]);
    assert!(composed.is_object());
    assert!(composed.get("run").is_some());
    let term = sample_term();
    let result = call_meta("run_model", &[composed, term.clone(), Value::Null, json!({})]);
    assert_eq!(result, term);
}

#[test]
fn compose_left_identity_preserves_model() {
    // compose(identity, m).run(t) = m.run(identity.run(t)) = m.run(t).
    // With m = identity, this is identity.
    let id = call_meta("identity_model", &[]);
    let composed = call_meta("compose", &[id.clone(), id]);
    let term = json!({ "__tag__": "Arr", "value": 9 });
    let result = call_meta("run_model", &[composed, term.clone(), Value::Null, json!({})]);
    assert_eq!(result, term);
}

// -------------------------------------------------------------------------
// pipeline
// -------------------------------------------------------------------------

#[test]
fn pipeline_of_identity_list_is_identity() {
    let id = call_meta("identity_model", &[]);
    let piped = call_meta("pipeline", &[json!([id.clone(), id.clone(), id])]);
    assert!(piped.is_object());
    assert!(piped.get("run").is_some());
    let term = sample_term();
    let result = call_meta("run_model", &[piped, term.clone(), Value::Null, json!({})]);
    assert_eq!(result, term);
}

#[test]
fn pipeline_empty_is_identity() {
    let piped = call_meta("pipeline", &[json!([])]);
    assert!(piped.is_object());
    assert!(piped.get("run").is_some());
    let term = sample_term();
    let result = call_meta("run_model", &[piped, term.clone(), Value::Null, json!({})]);
    assert_eq!(result, term);
}

// -------------------------------------------------------------------------
// parallel
// -------------------------------------------------------------------------

#[test]
fn parallel_returns_a_pair_of_results() {
    let id = call_meta("identity_model", &[]);
    let par = call_meta("parallel", &[id.clone(), id]);
    assert!(par.is_object());
    assert!(par.get("run").is_some());
    let term = sample_term();
    let result = call_meta("run_model", &[par, term.clone(), Value::Null, json!({})]);
    // parallel(a, b).run(t) = (a.run(t), b.run(t)) = (t, t)
    assert_eq!(result, json!({ "__pair__": [term.clone(), term] }));
}

// -------------------------------------------------------------------------
// trace_model
// -------------------------------------------------------------------------

#[test]
fn trace_model_returns_result_and_trace() {
    let id = call_meta("identity_model", &[]);
    let term = sample_term();
    let traced = call_meta("trace_model", &[id, term.clone(), Value::Null, json!({})]);
    assert!(traced.is_object(), "trace_model should return a record");
    assert_eq!(traced.get("result"), Some(&term));
    let trace = traced.get("trace").expect("trace_model should have a 'trace' field");
    assert!(trace.is_array(), "trace should be an array: {trace}");
}

#[test]
fn trace_model_records_tag_dispatch() {
    let id = call_meta("identity_model", &[]);
    let term = json!({ "__tag__": "Arr", "value": 1 });
    let traced = call_meta("trace_model", &[id, term, Value::Null, json!({})]);
    let trace = traced.get("trace").and_then(|v| v.as_array()).unwrap();
    assert!(!trace.is_empty(), "trace should have at least one entry");
    let entry = &trace[0];
    assert_eq!(entry.get("tag"), Some(&json!("Arr")));
    assert_eq!(entry.get("ok"), Some(&json!(true)));
}

// -------------------------------------------------------------------------
// model_handler
// -------------------------------------------------------------------------

#[test]
fn model_handler_returns_run_for_identity_model() {
    let model = call_meta("identity_model", &[]);
    let handler = call_meta("model_handler", &[model, json!("run")]);
    assert!(
        handler.is_object() || !handler.is_null(),
        "model_handler('run') should return the run handler: {handler}"
    );
}

#[test]
fn model_handler_returns_null_for_missing_tag() {
    let model = call_meta("identity_model", &[]);
    let handler = call_meta("model_handler", &[model, json!("does_not_exist")]);
    assert_eq!(handler, Value::Null);
}

// =========================================================================
// load_model — requires the kernel `ffi` (host capability call) and `parse`
// (source -> model value) primitives, which are not wired into the default
// test runtime. A mock kernel module providing both is registered as a
// meta-model dependency below. Per the ADR-0103 spec, `load_model` reads a
// model's source via `ffi("fs", "read", [path])` then parses it via `parse`.
// =========================================================================

use std::collections::HashMap as StdHashMap;
use std::rc::Rc;

use handl_language::{Ast, LangError, Source};
use handl_platform::{EvalError, Function};
use handl_plugin_engine_exec_tree_walk::register_home_frame;
use handl_runtime::{EngineRegistry, Runtime, RuntimeConfig};

/// A small valid model source: a record with a `run` handler (identity) and a
/// `name` field. This is what the mock `fs.read` returns for a known path,
/// and what the mock `parse` evaluates into a model value.
const MINI_MODEL_SOURCE: &str = r#"{
  name: "mini_arrow",
  run: [term, incoming, ctx] ~> term
}"#;

/// Build a minimal lex/parse/eval `Runtime` (canonical language, tree-walk
/// engine, no dependency modules) for the mock `parse` primitive to evaluate
/// a model source string into a value.
fn build_parse_runtime() -> Result<Runtime, LangError> {
    let kit = handl_kit_data_canonical::assemble()?;
    let lang = kit.lang().clone();
    let config = RuntimeConfig::new()
        .with_lex("data")
        .with_parse("pratt")
        .with_eval("tree-walk");
    let mut registry = EngineRegistry::new();
    registry.register_lex(
        "data",
        Box::new(handl_plugin_engine_lex_pattern::DataLexerEngine::new()),
    );
    registry.register_parse(
        "pratt",
        handl_plugin_engine_parse_pratt::PrattEngine::new(),
    );
    registry.register_eval(
        "tree-walk",
        Box::new(handl_plugin_engine_exec_tree_walk::TreeWalkEngine::new()),
    );
    let registry = Rc::new(registry);
    let globals = handl_runtime::fp::canonical_fp_host_functions(None);
    Runtime::from_registry_with_globals(lang, &registry, &config, Some(globals), None)
}

/// True iff `value` contains an arrow (`{"__arrow__": true}`) that lacks a
/// `__home__` frame id — i.e. an arrow minted by an eval whose AST the caller
/// cannot route back to. Mirrors `plugin-lib-handl`'s private helper.
fn contains_unhomed_arrow(value: &Value) -> bool {
    match value {
        Value::Array(a) => a.iter().any(contains_unhomed_arrow),
        Value::Object(o) => {
            if o.get("__arrow__").and_then(|v| v.as_bool()) == Some(true) {
                return o.get("__home__").and_then(|v| v.as_u64()).is_none()
                    || o.values().any(contains_unhomed_arrow);
            }
            o.values().any(contains_unhomed_arrow)
        }
        _ => false,
    }
}

/// Stamp every unhomed arrow inside `value` with `frame_id` (recursively,
/// including arrows nested in a closure's captured `env`). Mirrors
/// `plugin-lib-handl`'s private helper.
fn stamp_home(value: &mut Value, frame_id: u64) {
    match value {
        Value::Array(a) => {
            for v in a.iter_mut() {
                stamp_home(v, frame_id);
            }
        }
        Value::Object(o) => {
            if o.get("__arrow__").and_then(|v| v.as_bool()) == Some(true)
                && o.get("__home__").and_then(|v| v.as_u64()).is_none()
            {
                o.insert("__home__".to_string(), Value::from(frame_id));
            }
            for v in o.values_mut() {
                stamp_home(v, frame_id);
            }
        }
        _ => {}
    }
}

/// Mock `ffi` host function: dispatches `ffi("fs", "read", [path])` against an
/// in-memory file map. Returns the source string for a known path, or `Null`
/// for an unknown path (mirroring a missing-file failure that `load_model`
/// surfaces as a null model).
struct MockFfiFn {
    files: StdHashMap<String, String>,
}

impl Function for MockFfiFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let module = args.get(0).and_then(|v| v.as_str()).unwrap_or("");
        let name = args.get(1).and_then(|v| v.as_str()).unwrap_or("");
        if module == "fs" && name == "read" {
            if let Some(arr) = args.get(2).and_then(|v| v.as_array()) {
                if let Some(path) = arr.get(0).and_then(|v| v.as_str()) {
                    if let Some(src) = self.files.get(path) {
                        return Ok(Value::String(src.clone()));
                    }
                }
            }
            return Ok(Value::Null);
        }
        Ok(Value::Null)
    }
}

/// Mock kernel `parse` primitive: evaluates a Handl source string into a
/// value (source -> model value). Arrows in the result are stamped with a
/// home frame so the parsed model is runnable from the caller's eval.
struct MockParseFn;

impl Function for MockParseFn {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let source = match args.get(0) {
            Some(Value::String(s)) => s.clone(),
            _ => return Ok(Value::Null),
        };
        let runtime = build_parse_runtime().map_err(|e| EvalError::Other(e.to_string()))?;
        let toks = runtime
            .lex(&Source::new(&source))
            .map_err(|e| EvalError::Other(e.to_string()))?;
        let ast: Rc<Ast> = Rc::new(
            runtime
                .parse(&toks)
                .map_err(|e| EvalError::Other(e.to_string()))?,
        );
        let result = runtime
            .eval_shared_with_globals(&ast, &StdHashMap::new())
            .map_err(|e| EvalError::Other(e.to_string()))?;
        if contains_unhomed_arrow(&result) {
            let frame_id = register_home_frame(
                Rc::clone(&ast),
                Rc::new(runtime.lang().clone()),
                runtime.modules().cloned(),
                None,
                runtime.import_resolver().cloned(),
            );
            let mut result = result;
            stamp_home(&mut result, frame_id);
            return Ok(result);
        }
        Ok(result)
    }
}

/// A mock kernel `Module` providing the two primitives `load_model` needs:
/// `ffi` (host capability call) and `parse` (source -> model value). Registered
/// as a meta-model dependency so `call_host` dispatch resolves both names.
struct MockKernelModule {
    ffi_fn: MockFfiFn,
    parse_fn: MockParseFn,
}

impl MockKernelModule {
    fn new(files: StdHashMap<String, String>) -> Self {
        Self {
            ffi_fn: MockFfiFn { files },
            parse_fn: MockParseFn,
        }
    }
}

impl handl_platform::Module for MockKernelModule {
    fn name(&self) -> &str {
        "mock_kernel"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "ffi" => Some(&self.ffi_fn as &dyn Function),
            "parse" => Some(&self.parse_fn as &dyn Function),
            _ => None,
        }
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        vec![
            ("ffi", &self.ffi_fn as &dyn Function),
            ("parse", &self.parse_fn as &dyn Function),
        ]
    }
}

/// Build a fresh mock kernel module with the mini model registered under
/// `"models/mini.hnl"`.
fn mock_kernel() -> MockKernelModule {
    let mut files = StdHashMap::new();
    files.insert("models/mini.hnl".to_string(), MINI_MODEL_SOURCE.to_string());
    MockKernelModule::new(files)
}

/// Build the meta-model with the mock kernel module wired in as a dependency.
fn meta_module_with_kernel(kernel: MockKernelModule) -> HandlModule {
    meta_module().with_dependency(kernel)
}

/// Call a meta-model function by name with the mock kernel wired in.
fn call_meta_kernel(name: &str, args: &[Value], kernel: MockKernelModule) -> Value {
    let module = meta_module_with_kernel(kernel);
    let func = module
        .function(name)
        .unwrap_or_else(|| panic!("meta_model has no '{name}' export"));
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx)
        .unwrap_or_else(|e| panic!("meta_model '{name}' failed: {e:?}"))
}

// -------------------------------------------------------------------------
// load_model tests
// -------------------------------------------------------------------------

#[test]
fn load_model_loads_and_parses_a_small_model() {
    let model = call_meta_kernel("load_model", &[json!("models/mini.hnl")], mock_kernel());
    assert!(
        model.is_object(),
        "load_model should return a record of handlers: {model}"
    );
    assert!(
        model.get("run").is_some(),
        "loaded model should have a 'run' handler"
    );
    let run = model.get("run").expect("run handler");
    assert_eq!(
        run.get("__arrow__"),
        Some(&json!(true)),
        "'run' should be a function (arrow) value"
    );
    assert_eq!(model.get("name"), Some(&json!("mini_arrow")));
}

#[test]
fn load_model_loaded_model_validates() {
    let model = call_meta_kernel("load_model", &[json!("models/mini.hnl")], mock_kernel());
    let result = call_meta_kernel("validate_model", &[model], mock_kernel());
    assert_eq!(result, json!(true));
}

#[test]
fn load_model_loaded_model_lists_run_tag() {
    let model = call_meta_kernel("load_model", &[json!("models/mini.hnl")], mock_kernel());
    let tags = call_meta_kernel("available_tags", &[model], mock_kernel());
    assert!(tags.is_array(), "available_tags should return an array: {tags}");
    assert!(
        tags.as_array().unwrap().contains(&json!("run")),
        "loaded model tags should contain 'run': {tags}"
    );
}

#[test]
fn load_model_loaded_model_is_runnable() {
    let model = call_meta_kernel("load_model", &[json!("models/mini.hnl")], mock_kernel());
    let term = json!({ "__tag__": "Arr", "value": 5 });
    let result = call_meta_kernel(
        "run_model",
        &[model, term.clone(), Value::Null, json!({})],
        mock_kernel(),
    );
    // The mini model's `run` is identity, so run_model returns the term.
    assert_eq!(result, term);
}

#[test]
fn load_model_returns_null_for_nonexistent_path() {
    let model = call_meta_kernel(
        "load_model",
        &[json!("does/not/exist.hnl")],
        mock_kernel(),
    );
    // The mock `fs.read` returns Null for an unknown path; `parse(Null)`
    // returns Null, so `load_model` surfaces a null model.
    assert_eq!(model, Value::Null);
}
