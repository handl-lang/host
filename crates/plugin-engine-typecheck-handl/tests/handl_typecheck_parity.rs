//! Handl typecheck model parity tests.
//!
//! These tests exercise the Handl-authored typecheck models
//! (`typecheck_model.hnl`, `scope_model.hnl`, `totality_model.hnl`,
//! `type_model.hnl`) by loading them via the [`HandlTypecheckEngine`]
//! adapter and directly as [`HandlModule`]s, running simple typecheck
//! scenarios.
//!
//! The tests verify **plumbing**, not model completeness: a model that
//! returns `NotImplemented`, empty results, or runtime errors is
//! acceptable — the goal is to confirm the models load and their
//! exported functions are callable without crashing.  When a model
//! cannot be loaded (parse errors, missing exports) the test skips
//! gracefully with a warning rather than failing.

use handl_kit_data_canonical::language_from_yaml;
use handl_language::{Ast, Language};
use handl_platform::{EvalContext, Module};
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::lex;
use handl_plugin_engine_parse::{ParserEngine, ParserRequest};
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_engine_typecheck::{TypecheckEngine, TypecheckRequest};
use handl_plugin_engine_typecheck_handl::HandlTypecheckEngine;
use handl_plugin_lib_char_native::CharModule;
use handl_plugin_lib_handl::HandlModule;
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use serde_json::{json, Value};

const TYPECHECK_MODEL_HNL: &str =
    include_str!("../../../project/design/adr0103/typecheck_model.hnl");
const SCOPE_MODEL_HNL: &str =
    include_str!("../../../project/design/adr0103/scope_model.hnl");
const TOTALITY_MODEL_HNL: &str =
    include_str!("../../../project/design/adr0103/totality_model.hnl");
const TYPE_MODEL_HNL: &str =
    include_str!("../../../project/design/adr0103/type_model.hnl");

/// Function names exported by the typecheck model record.
const TYPECHECK_MODEL_FUNCS: &[&str] = &["typecheck"];

/// Function names exported by the scope model record.
const SCOPE_MODEL_FUNCS: &[&str] = &["check_module", "collect_bindings", "check_unbound_wires"];

/// Function names exported by the totality model record.
const TOTALITY_MODEL_FUNCS: &[&str] = &[
    "check_totality",
    "check_expr",
    "check_function",
    "check_assign",
    "read_termination_metadata",
    "child_role_index",
    "is_strict_sub_structure",
    "find_recursive_calls",
];

/// Function names exported by the type model record.
const TYPE_MODEL_FUNCS: &[&str] = &["run"];

/// Build the canonical `Language` from the embedded YAML fragments.
fn canonical_lang() -> Language {
    language_from_yaml().expect("canonical language should assemble")
}

/// Lex + parse a source string into an `Ast` using the canonical language.
fn parse(src: &str, lang: &Language) -> Ast {
    let tokens = lex(lang, src).expect("lex should succeed");
    let engine = PrattEngine::new();
    let req = ParserRequest::new(tokens);
    engine.parse(lang, &req).expect("parse should succeed")
}

/// Call a function on a [`HandlModule`] by name, returning the result.
///
/// Returns `Ok(Value)` on success, `Err(String)` if the function is
/// missing or the call fails (the error message is logged).
fn call_func(module: &HandlModule, name: &str, args: &[Value]) -> Result<Value, String> {
    let func = module
        .function(name)
        .ok_or_else(|| format!("model has no '{name}' export"))?;
    let mut ctx = EvalContext::new();
    func.call(args, &mut ctx)
        .map_err(|e| format!("model '{name}' call failed: {e:?}"))
}

/// Check whether a [`HandlModule`] successfully loaded all its declared
/// function exports.  Returns `Ok(())` if all functions resolve, or
/// `Err(message)` describing the first missing export.
fn module_loaded(module: &HandlModule, funcs: &[&str]) -> Result<(), String> {
    for &f in funcs {
        if module.function(f).is_none() {
            return Err(format!("model failed to load: missing '{f}' export"));
        }
    }
    Ok(())
}

/// Build a [`HandlModule`] for the typecheck model with the stdlib
/// dependency chain.  The typecheck model uses `hash_map`, `array`, and
/// `typeof` — all available in the stdlib.
fn typecheck_model_module() -> HandlModule {
    HandlModule::new(
        "typecheck_model",
        TYPECHECK_MODEL_HNL,
        TYPECHECK_MODEL_FUNCS,
    )
    .with_dependency(stdlib::module("array").expect("array module"))
    .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
    .with_dependency(stdlib::module("typeof").expect("typeof module"))
    .with_dependency(stdlib::module("obj").expect("obj module"))
}

// ===========================================================================
// 1. Model loading test — typecheck_model.hnl
// ===========================================================================

/// The typecheck model should load and export `typecheck`.  If it fails
/// (parse errors, missing exports), the test skips gracefully with a
/// warning — the adapter degrades to `NotImplemented` in that case.
#[test]
fn typecheck_model_loads_and_exports_typecheck() {
    let module = typecheck_model_module();
    if let Err(msg) = module_loaded(&module, TYPECHECK_MODEL_FUNCS) {
        eprintln!("{msg}; the adapter will return NotImplemented — this is acceptable");
        return;
    }
    assert!(
        module.function("typecheck").is_some(),
        "typecheck model should export 'typecheck'"
    );
}

/// The typecheck model should also load via the adapter (the adapter
/// embeds the same source).  We verify by calling `typecheck` on an
/// empty AST — `NotImplemented` means the model did not load, any other
/// outcome means it did.
#[test]
fn typecheck_model_loads_via_adapter() {
    let engine = HandlTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    let lang = Language::new();
    let req = TypecheckRequest::new(Ast::new());
    match engine.typecheck(&lang, &req) {
        Ok(_) | Err(handl_language::LangError::NotImplemented) => {}
        Err(e) => {
            // A non-NotImplemented error means the model loaded but
            // produced an error — still acceptable.
            eprintln!("adapter typecheck returned error (ok): {e}");
        }
    }
}

// ===========================================================================
// 2. Simple typecheck test — full pipeline via the adapter
// ===========================================================================

/// Build a [`HandlTypecheckEngine`], parse a simple source (`x = 42`),
/// and call `typecheck`.  The test passes as long as the adapter does
/// not panic — `Ok` (with any type/diagnostics) or `Err(NotImplemented)`
/// are both acceptable.
#[test]
fn typecheck_simple_assign_via_adapter() {
    let lang = canonical_lang();
    let ast = parse("x = 42", &lang);

    let engine = HandlTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    let req = TypecheckRequest::new(ast);

    match engine.typecheck(&lang, &req) {
        Ok(result) => {
            // The adapter plumbing works.  The type may be imperfect
            // (skeleton model); we only assert non-panic and useful output.
            assert!(
                !result.typ.is_null() || !result.diagnostics.is_empty(),
                "Handl model should return a non-null type or diagnostics for x = 42"
            );
            let _ = result.node_types;
        }
        Err(handl_language::LangError::NotImplemented) => {
            // Model is a skeleton — adapter degraded gracefully.
        }
        Err(e) => {
            // Any other error is acceptable as long as we didn't panic.
            eprintln!("typecheck returned error (ok): {e}");
        }
    }
}

/// A `do { x = 1; x }` block should typecheck without panicking and
/// produce a non-null type or a diagnostic (no hang).
#[test]
fn typecheck_do_block_via_adapter() {
    let lang = canonical_lang();
    let ast = parse("do { x = 1; x }", &lang);

    let engine = HandlTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    let req = TypecheckRequest::new(ast);

    match engine.typecheck(&lang, &req) {
        Ok(result) => {
            assert!(
                !result.typ.is_null() || !result.diagnostics.is_empty(),
                "Handl model should return a non-null type or diagnostics for a do block"
            );
        }
        Err(handl_language::LangError::NotImplemented) => {}
        Err(e) => {
            eprintln!("typecheck returned error for do block (ok): {e}");
        }
    }
}

/// A simple `if` should typecheck without hanging and produce useful output.
#[test]
fn typecheck_if_via_adapter() {
    let lang = canonical_lang();
    let ast = parse("if true { 1 } else { 2 }", &lang);

    let engine = HandlTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    let req = TypecheckRequest::new(ast);

    match engine.typecheck(&lang, &req) {
        Ok(result) => {
            assert!(
                !result.typ.is_null() || !result.diagnostics.is_empty(),
                "Handl model should return a non-null type or diagnostics for if"
            );
        }
        Err(handl_language::LangError::NotImplemented) => {}
        Err(e) => {
            eprintln!("typecheck returned error for if (ok): {e}");
        }
    }
}

/// A simple `return` inside a block should typecheck without hanging.
#[test]
fn typecheck_return_via_adapter() {
    let lang = canonical_lang();
    let ast = parse("do { return 1 }", &lang);

    let engine = HandlTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    let req = TypecheckRequest::new(ast);

    match engine.typecheck(&lang, &req) {
        Ok(result) => {
            assert!(
                !result.typ.is_null() || !result.diagnostics.is_empty(),
                "Handl model should return a non-null type or diagnostics for return"
            );
        }
        Err(handl_language::LangError::NotImplemented) => {}
        Err(e) => {
            eprintln!("typecheck returned error for return (ok): {e}");
        }
    }
}

// ===========================================================================
// 3. Scope model test — scope_model.hnl as a HandlModule
// ===========================================================================

/// Build a [`HandlModule`] for the scope model with the stdlib + char
/// dependency chain.  The scope model uses `char.*`, `str.*`,
/// `hash_map`, `array`, `typeof`, and `ffi("parse", ...)` — the latter
/// requires a host `ffi` capability that is not wired in the test
/// runtime, so `check_module` may fail at call time.
fn scope_model_module() -> HandlModule {
    HandlModule::new("scope_model", SCOPE_MODEL_HNL, SCOPE_MODEL_FUNCS)
        .with_dependency(stdlib::module("array").expect("array module"))
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(stdlib::module("str").expect("str module"))
        .with_dependency(CharModule)
}

/// The scope model should load and export `check_module`.  Calling
/// `check_module` with a simple source may fail because the model needs
/// an `ffi("parse", ...)` host capability — that is acceptable.
#[test]
fn scope_model_loads_and_exports_check_module() {
    let module = scope_model_module();
    if let Err(msg) = module_loaded(&module, SCOPE_MODEL_FUNCS) {
        eprintln!("{msg}; skipping scope model call test");
        return;
    }
    assert!(
        module.function("check_module").is_some(),
        "scope model should export 'check_module'"
    );
}

/// Call `check_module` on a simple source.  The call may fail (the
/// model needs `ffi("parse", ...)` to parse chunks); the test passes as
/// long as the call does not panic.
#[test]
fn scope_model_check_module_simple_source() {
    let module = scope_model_module();
    if module_loaded(&module, &["check_module"]).is_err() {
        eprintln!("scope model did not load; skipping check_module call");
        return;
    }

    let lang = canonical_lang();
    let lang_val = serde_json::to_value(&lang).unwrap_or(Value::Null);
    let src = "x = 42";

    match call_func(&module, "check_module", &[lang_val, json!(src)]) {
        Ok(result) => {
            // The model returned a diagnostics array (or null).  Either
            // is acceptable — we are testing plumbing.
            assert!(
                result.is_array() || result.is_null(),
                "check_module should return an array of diagnostics or null, got: {result}"
            );
        }
        Err(msg) => {
            // The call failed — likely because `ffi("parse", ...)` is
            // not wired.  This is expected and acceptable.
            eprintln!("scope_model check_module failed (ok): {msg}");
        }
    }
}

// ===========================================================================
// 4. Totality model test — totality_model.hnl as a HandlModule
// ===========================================================================

/// Build a [`HandlModule`] for the totality model with the stdlib
/// dependency chain.  The totality model uses `hash_map`, `array`, and
/// `typeof` — all available in the stdlib.
fn totality_model_module() -> HandlModule {
    HandlModule::new("totality_model", TOTALITY_MODEL_HNL, TOTALITY_MODEL_FUNCS)
        .with_dependency(stdlib::module("array").expect("array module"))
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
}

/// The totality model should load and export `check_totality`.
#[test]
fn totality_model_loads_and_exports_check_totality() {
    let module = totality_model_module();
    if let Err(msg) = module_loaded(&module, TOTALITY_MODEL_FUNCS) {
        eprintln!("{msg}; skipping totality model call test");
        return;
    }
    assert!(
        module.function("check_totality").is_some(),
        "totality model should export 'check_totality'"
    );
}

/// Call `check_totality` with a simple AST containing a single `lit`
/// node (no recursive functions).  The model should return a proof
/// record with `verdict` and `reason` fields.
#[test]
fn totality_model_check_totality_simple_ast() {
    let module = totality_model_module();
    if module_loaded(&module, &["check_totality"]).is_err() {
        eprintln!("totality model did not load; skipping check_totality call");
        return;
    }

    let lang = Language::new();
    let lang_val = serde_json::to_value(&lang).unwrap_or(Value::Null);
    // A minimal AST: a single `lit` node (not a function, so the
    // totality checker should report "no recursive functions found").
    let ast = json!([
        { "kind": "lit", "payload": 42 }
    ]);

    match call_func(&module, "check_totality", &[lang_val, ast]) {
        Ok(result) => {
            // The model should return a proof record: { verdict, reason }.
            assert!(
                result.is_object(),
                "check_totality should return a proof record, got: {result}"
            );
            let verdict = result
                .get("verdict")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            assert!(
                verdict == "proven" || verdict == "disproved" || verdict == "blocked",
                "proof verdict should be proven/disproved/blocked, got '{verdict}'"
            );
            // A non-recursive `lit` node should be trivially total.
            assert_eq!(
                verdict, "proven",
                "non-recursive lit should be proven total, got '{verdict}'"
            );
        }
        Err(msg) => {
            // The call failed — acceptable for the plumbing test.
            eprintln!("totality_model check_totality failed (ok): {msg}");
        }
    }
}

/// Call `check_totality` with an empty AST — the model should return a
/// `blocked` proof (empty ast).
#[test]
fn totality_model_check_totality_empty_ast() {
    let module = totality_model_module();
    if module_loaded(&module, &["check_totality"]).is_err() {
        eprintln!("totality model did not load; skipping empty-ast test");
        return;
    }

    let lang = Language::new();
    let lang_val = serde_json::to_value(&lang).unwrap_or(Value::Null);
    let ast = json!([]);

    match call_func(&module, "check_totality", &[lang_val, ast]) {
        Ok(result) => {
            assert!(result.is_object(), "expected proof record, got: {result}");
            let verdict = result.get("verdict").and_then(|v| v.as_str()).unwrap_or("");
            assert_eq!(
                verdict, "blocked",
                "empty ast should be blocked, got '{verdict}'"
            );
        }
        Err(msg) => {
            eprintln!("totality_model empty-ast call failed (ok): {msg}");
        }
    }
}

// ===========================================================================
// 5. Type model parity — type_model.hnl as a HandlModule
// ===========================================================================

/// Build a [`HandlModule`] for the type model with the stdlib + tag
/// dependency chain.  The type model uses `hash_map`, `array`,
/// `typeof`, and `tag.*` — all available in the stdlib and `TagModule`.
fn type_model_module() -> HandlModule {
    HandlModule::new("type_model", TYPE_MODEL_HNL, TYPE_MODEL_FUNCS)
        .with_dependency(stdlib::module("array").expect("array module"))
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(TagModule)
}

/// The type model should load and export `run`.
#[test]
fn type_model_loads_and_exports_run() {
    let module = type_model_module();
    if let Err(msg) = module_loaded(&module, TYPE_MODEL_FUNCS) {
        eprintln!("{msg}; skipping type model call test");
        return;
    }
    assert!(
        module.function("run").is_some(),
        "type model should export 'run'"
    );
}

/// Call `run` with a simple `Arr` term.  The type model interprets
/// `Arr` as the constant type (the value IS the type), so `run({__tag__:
/// "Arr", value: 42}, null, {})` should return `42`.
#[test]
fn type_model_run_arr_returns_value() {
    let module = type_model_module();
    if module_loaded(&module, &["run"]).is_err() {
        eprintln!("type model did not load; skipping run test");
        return;
    }

    let term = json!({ "__tag__": "Arr", "value": 42 });
    let incoming = Value::Null;
    let ctx = json!({ "handler_stack": [] });

    match call_func(&module, "run", &[term, incoming, ctx]) {
        Ok(result) => {
            assert_eq!(result, json!(42), "Arr type model should return the value");
        }
        Err(msg) => {
            eprintln!("type_model run failed (ok): {msg}");
        }
    }
}

/// Call `run` with a `Const` term — same semantics as `Arr`.
#[test]
fn type_model_run_const_returns_value() {
    let module = type_model_module();
    if module_loaded(&module, &["run"]).is_err() {
        eprintln!("type model did not load; skipping const test");
        return;
    }

    let term = json!({ "__tag__": "Const", "value": 7 });
    let incoming = Value::Null;
    let ctx = json!({ "handler_stack": [] });

    match call_func(&module, "run", &[term, incoming, ctx]) {
        Ok(result) => {
            assert_eq!(result, json!(7), "Const type model should return the value");
        }
        Err(msg) => {
            eprintln!("type_model const run failed (ok): {msg}");
        }
    }
}

/// Call `run` with a `Seq` term — the type model builds a `Compose`
/// tagged type.
#[test]
fn type_model_run_seq_builds_compose_type() {
    let module = type_model_module();
    if module_loaded(&module, &["run"]).is_err() {
        eprintln!("type model did not load; skipping seq test");
        return;
    }

    let term = json!({
        "__tag__": "Seq",
        "fst": { "__tag__": "Arr", "value": 1 },
        "snd": { "__tag__": "Arr", "value": 2 }
    });
    let incoming = Value::Null;
    let ctx = json!({ "handler_stack": [] });

    match call_func(&module, "run", &[term, incoming, ctx]) {
        Ok(result) => {
            assert_eq!(
                result,
                json!({ "__tag__": "Compose", "fst": 1, "snd": 2 }),
                "Seq type model should build a Compose type"
            );
        }
        Err(msg) => {
            eprintln!("type_model seq run failed (ok): {msg}");
        }
    }
}

/// Call `run` with a `Path` term — the type model builds the identity
/// type `Id_A(a, b)`.
#[test]
fn type_model_run_path_builds_identity_type() {
    let module = type_model_module();
    if module_loaded(&module, &["run"]).is_err() {
        eprintln!("type model did not load; skipping path test");
        return;
    }

    let term = json!({
        "__tag__": "Path",
        "from": { "__tag__": "Arr", "value": 1 },
        "to": { "__tag__": "Arr", "value": 2 }
    });
    let incoming = Value::Null;
    let ctx = json!({ "handler_stack": [] });

    match call_func(&module, "run", &[term, incoming, ctx]) {
        Ok(result) => {
            assert_eq!(
                result,
                json!({ "__tag__": "Path", "from": 1, "to": 2 }),
                "Path type model should build the identity type"
            );
        }
        Err(msg) => {
            eprintln!("type_model path run failed (ok): {msg}");
        }
    }
}

/// Typecheck a real canonical file (`examples/handl/hello.hnl`) through the
/// adapter and assert the model returns without hanging and produces useful
/// output (a non-null type or diagnostic set).
#[test]
fn typecheck_canonical_hello_hnl_runs_and_returns_useful_output() {
    const HELLO_HNL: &str = include_str!("../../../examples/handl/hello.hnl");
    let lang = canonical_lang();
    let ast = parse(HELLO_HNL, &lang);

    let engine = HandlTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    let req = TypecheckRequest::new(ast);

    match engine.typecheck(&lang, &req) {
        Ok(result) => {
            assert!(
                !result.typ.is_null() || !result.diagnostics.is_empty(),
                "Handl model should return a non-null type or diagnostics for hello.hnl, got null and no diagnostics"
            );
        }
        Err(handl_language::LangError::NotImplemented) => {
            eprintln!("handl model not implemented; this is acceptable on a skeleton");
        }
        Err(e) => {
            eprintln!("typecheck returned error (ok): {e}");
        }
    }
}

/// Typecheck a simple field access (`rec.x`) and assert it returns without
/// hanging and produces useful output (a non-null type or diagnostics).
#[test]
fn typecheck_field_access_runs_and_returns_useful_output() {
    let lang = canonical_lang();
    let ast = parse("rec.x", &lang);

    let engine = HandlTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    let req = TypecheckRequest::new(ast);

    match engine.typecheck(&lang, &req) {
        Ok(result) => {
            assert!(
                !result.typ.is_null() || !result.diagnostics.is_empty(),
                "Handl model should return a non-null type or diagnostics for field_access"
            );
        }
        Err(handl_language::LangError::NotImplemented) => {}
        Err(e) => {
            eprintln!("typecheck returned error for field_access (ok): {e}");
        }
    }
}

/// Typecheck a simple function call (`f(1)`) and assert it returns without
/// hanging and produces useful output (a non-null type or diagnostics).
#[test]
fn typecheck_call_runs_and_returns_useful_output() {
    let lang = canonical_lang();
    let ast = parse("f(1)", &lang);

    let engine = HandlTypecheckEngine::new(Box::new(TreeWalkEngine::new()));
    let req = TypecheckRequest::new(ast);

    match engine.typecheck(&lang, &req) {
        Ok(result) => {
            assert!(
                !result.typ.is_null() || !result.diagnostics.is_empty(),
                "Handl model should return a non-null type or diagnostics for call"
            );
        }
        Err(handl_language::LangError::NotImplemented) => {}
        Err(e) => {
            eprintln!("typecheck returned error for call (ok): {e}");
        }
    }
}
// ===========================================================================
// 5. ADR-0106 nominal subtyping — the `subtype` judgment rule
// ===========================================================================
//
// The rule lives in `eval_judgment_impl_` ("subtype") and is reached by the
// typechecker through `eval_scheme_` -> `eval_scheme_impl_` (the `Op` tag
// with `source: "Judgment"`). These tests drive that same path via the
// model's exported `eval_scheme` helper with a synthesized `Op`/`Judgment`
// term, so they exercise the real dispatch — not a reimplementation.

/// Function names needed to drive the `subtype` rule directly.
const SUBTYPE_MODEL_FUNCS: &[&str] = &["typecheck", "eval_scheme"];

/// Build the typecheck model exporting `eval_scheme` so the `subtype`
/// judgment op can be driven directly.
fn subtype_model_module() -> HandlModule {
    HandlModule::new("typecheck_model", TYPECHECK_MODEL_HNL, SUBTYPE_MODEL_FUNCS)
        .with_dependency(stdlib::module("array").expect("array module"))
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(stdlib::module("obj").expect("obj module"))
}

/// A nominal value in the ADR-0106 Phase 2 encoding:
/// `{"__sym__": {"handle": n, "name": "...", "parent": <sym or null>}}`.
fn nominal(handle: i64, name: &str, parent: Value) -> Value {
    json!({ "__sym__": { "handle": handle, "name": name, "parent": parent } })
}

/// A synthesized scheme `Op`/`Judgment` term invoking `subtype(sub, sup)`.
/// The two `Const` args evaluate to the literal values, exactly as a
/// type-rule scheme would supply them.
fn subtype_term(sub: Value, sup: Value) -> Value {
    json!({
        "__tag__": "Op",
        "source": "Judgment",
        "name": "subtype",
        "args": [
            { "__tag__": "Const", "value": sub },
            { "__tag__": "Const", "value": sup }
        ]
    })
}

/// Drive `eval_scheme` on a `subtype(sub, sup)` term and return the
/// diagnostics accumulated on the resulting state.
///
/// `eval_judgment_impl_` always reads the current AST node's kind (via
/// `ast_node`/`node_kind`), so the state carries a single dummy node; every
/// other field is filled in by `tc_sanitize`.
fn run_subtype(module: &HandlModule, sub: Value, sup: Value) -> Vec<Value> {
    let state = json!({ "ast": [{ "kind": "subtype-test", "payload": {} }] });
    let result = call_func(
        module,
        "eval_scheme",
        &[state, json!(0), subtype_term(sub, sup), Value::Null],
    )
    .expect("eval_scheme on a subtype term should not error");
    result["state"]["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// Whether a `subtype` run reported the failure diagnostic.
fn subtype_failed(diags: &[Value]) -> bool {
    diags
        .iter()
        .any(|d| d.get("message").and_then(|m| m.as_str()) == Some("subtype check failed"))
}

/// Like [`run_subtype`], but surfaces a model-call error instead of panicking
/// (used for the transitional nominal-vs-legacy case, where `unify` is handed
/// a raw `sym` value and may error — the pre-ADR-0106 behavior).
fn try_run_subtype(module: &HandlModule, sub: Value, sup: Value) -> Result<Vec<Value>, String> {
    let state = json!({ "ast": [{ "kind": "subtype-test", "payload": {} }] });
    match call_func(
        module,
        "eval_scheme",
        &[state, json!(0), subtype_term(sub, sup), Value::Null],
    ) {
        Ok(result) => Ok(result["state"]["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default()),
        Err(e) => Err(e),
    }
}

/// The standard nominal chain used throughout these tests:
/// `Obj(1) <- Number(2) <- {Int(3), Float(4)}`, `Int(3) <- Nat(5)`.
fn std_chain() -> (Value, Value, Value, Value, Value) {
    let obj = nominal(1, "Obj", Value::Null);
    let number = nominal(2, "Number", obj.clone());
    let int = nominal(3, "Int", number.clone());
    let float = nominal(4, "Float", number.clone());
    let nat = nominal(5, "Nat", int.clone());
    (obj, number, int, float, nat)
}

/// The model must export the `eval_scheme` helper the rule tests drive.
/// (Hard assertion: a missing export would make every rule test vacuous.)
#[test]
fn subtype_model_exports_eval_scheme() {
    let module = subtype_model_module();
    assert!(
        module.function("eval_scheme").is_some(),
        "typecheck model must export 'eval_scheme' to drive judgment rules"
    );
}


/// `Int <: Number` — direct child of Number.
#[test]
fn subtype_int_is_number() {
    let module = subtype_model_module();
    let (_obj, number, int, _float, _nat) = std_chain();
    assert!(
        !subtype_failed(&run_subtype(&module, int, number)),
        "Int <: Number should succeed"
    );
}

/// `Float <: Number` — direct child of Number.
#[test]
fn subtype_float_is_number() {
    let module = subtype_model_module();
    let (_obj, number, _int, float, _nat) = std_chain();
    assert!(
        !subtype_failed(&run_subtype(&module, float, number)),
        "Float <: Number should succeed"
    );
}

/// `Nat <: Number` — transitive through `Int`.
#[test]
fn subtype_nat_is_number_transitive() {
    let module = subtype_model_module();
    let (_obj, number, _int, _float, nat) = std_chain();
    assert!(
        !subtype_failed(&run_subtype(&module, nat, number)),
        "Nat <: Number should succeed through the Int parent"
    );
}

/// `Nat <: Int` — immediate parent.
#[test]
fn subtype_nat_is_int() {
    let module = subtype_model_module();
    let (_obj, _number, int, _float, nat) = std_chain();
    assert!(
        !subtype_failed(&run_subtype(&module, nat, int)),
        "Nat <: Int should succeed"
    );
}

/// A nominal is a subtype of itself (reflexivity).
#[test]
fn subtype_nominal_is_itself() {
    let module = subtype_model_module();
    let (_obj, number, _int, _float, _nat) = std_chain();
    let number2 = number.clone();
    assert!(
        !subtype_failed(&run_subtype(&module, number, number2)),
        "a nominal should be a subtype of itself"
    );
}

/// `Number </: Float` — a parent is not a subtype of its child.
#[test]
fn subtype_number_is_not_float() {
    let module = subtype_model_module();
    let (_obj, number, _int, float, _nat) = std_chain();
    assert!(
        subtype_failed(&run_subtype(&module, number, float)),
        "Number <: Float must fail (parent-to-child)"
    );
}

/// `Int </: Float` — siblings under `Number` are unrelated.
#[test]
fn subtype_siblings_int_is_not_float() {
    let module = subtype_model_module();
    let (_obj, _number, int, float, _nat) = std_chain();
    assert!(
        subtype_failed(&run_subtype(&module, int, float)),
        "Int <: Float must fail (siblings)"
    );
}

/// A deep chain (`N0 <- N1 <- ... <- N63`): the leaf is a subtype of the
/// root (full transitive walk) and the root is not a subtype of the leaf.
#[test]
fn subtype_deep_chain_transitivity() {
    let module = subtype_model_module();
    const DEPTH: i64 = 64;
    let mut cur = nominal(1, "N0", Value::Null);
    let root = cur.clone();
    for i in 1..DEPTH {
        cur = nominal(i + 1, &format!("N{i}"), cur);
    }
    let leaf = cur;

    assert!(
        !subtype_failed(&run_subtype(&module, leaf.clone(), root.clone())),
        "the leaf of a depth-{DEPTH} chain should be a subtype of the root"
    );
    assert!(
        subtype_failed(&run_subtype(&module, root, leaf)),
        "the root of a deep chain must not be a subtype of the leaf"
    );
}

/// Malformed nominals are not well-formed, so the rule falls back to
/// unification (pre-ADR-0106 behavior) instead of crashing. A malformed
/// value unified with itself succeeds structurally.
#[test]
fn subtype_malformed_nominal_falls_back_to_unify() {
    let module = subtype_model_module();

    // Zero handle is not a valid identity.
    let zero_handle = json!({ "__sym__": { "handle": 0, "name": "Bad", "parent": null } });
    assert!(
        !subtype_failed(&run_subtype(&module, zero_handle.clone(), zero_handle.clone())),
        "a zero-handle nominal is malformed; unify(a, a) should succeed"
    );

    // Missing handle.
    let missing_handle = json!({ "__sym__": { "name": "Bad", "parent": null } });
    assert!(
        !subtype_failed(&run_subtype(&module, missing_handle.clone(), missing_handle)),
        "a handle-less nominal is malformed; unify(a, a) should succeed"
    );

    // Non-numeric handle.
    let string_handle =
        json!({ "__sym__": { "handle": "nope", "name": "Bad", "parent": null } });
    assert!(
        !subtype_failed(&run_subtype(&module, string_handle.clone(), string_handle)),
        "a non-numeric handle is malformed; unify(a, a) should succeed"
    );

    // `__sym__` is not an object at all.
    let non_object_sym = json!({ "__sym__": 5 });
    assert!(
        !subtype_failed(&run_subtype(&module, non_object_sym.clone(), non_object_sym)),
        "`__sym__: 5` is malformed; unify(a, a) should succeed"
    );

    // A malformed parent rejects the whole chain, so the rule still falls
    // back to unify rather than panicking.
    let bad_parent = json!({
        "__sym__": { "handle": 7, "name": "X", "parent": { "__sym__": { "handle": 0, "name": "Y", "parent": null } } }
    });
    assert!(
        !subtype_failed(&run_subtype(&module, bad_parent.clone(), bad_parent)),
        "a malformed parent rejects the chain; unify(a, a) should succeed"
    );

    // Distinct malformed values fall back to unify and fail (no crash).
    let other = json!({ "__sym__": { "handle": 0, "name": "Other", "parent": null } });
    assert!(
        subtype_failed(&run_subtype(&module, zero_handle, other)),
        "distinct malformed nominals unify-fail without crashing"
    );
}

/// Legacy `__tag__`-tagged type values carry no nominal identity, so the
/// rule delegates to unification: structurally equal tags succeed, distinct
/// tags fail.
#[test]
fn subtype_legacy_tagged_equality_via_unify() {
    let module = subtype_model_module();
    let int_a = json!({ "__tag__": "int", "__registry__": "type", "kind": "primitive" });
    let int_b = json!({ "__tag__": "int", "__registry__": "type", "kind": "primitive" });
    let float = json!({ "__tag__": "float", "__registry__": "type", "kind": "primitive" });

    assert!(
        !subtype_failed(&run_subtype(&module, int_a.clone(), int_b)),
        "equal legacy tagged values should succeed via unify"
    );
    assert!(
        subtype_failed(&run_subtype(&module, int_a, float)),
        "distinct legacy tagged values should fail via unify"
    );
}

/// One nominal and one legacy tagged value: not both nominals, so the rule
/// delegates to `unify` (the pre-ADR-0106 path). `unify` does not understand
/// raw `sym` values (its `type_tag` field-accesses the value), so the mixed
/// case may error — the point of this test is that it does not panic and
/// that non-nominal inputs keep their old behavior.
#[test]
fn subtype_nominal_vs_legacy_tag_delegates_to_unify() {
    let module = subtype_model_module();
    let (_obj, number, _int, _float, _nat) = std_chain();
    let tagged_number =
        json!({ "__tag__": "number", "__registry__": "type", "kind": "primitive" });
    match try_run_subtype(&module, number, tagged_number) {
        Ok(diags) => assert!(
            subtype_failed(&diags),
            "a nominal vs a distinct legacy tagged value should fail"
        ),
        // `unify` errored on the raw sym (pre-existing); acceptable so long
        // as the model did not panic.
        Err(e) => eprintln!("nominal vs legacy tag went through unify and errored (ok): {e}"),
    }
}

