//! Integration test: the `import`/`from ... import` binding modes run
//! identically through all three eval engines (ADR-0096 unified import
//! model; the `import` op semantics live in the shared
//! `plugin-engine-exec::native_ops` table behind `OpService::Import`).
//!
//! Each engine is wired with a stub [`ImportResolver`] returning a fixed
//! module record; the same sources are evaluated through the tree-walk,
//! bytecode, and cranelift backends covering: bare/`*` splice, `* as`
//! module-record binding, named projection (`import name [as alias]`),
//! braced import lists, the missing-export error, and the no-resolver
//! error.

use handl_language::{LangError, Source, Value};
use handl_plugin_engine_exec::{EvalEngine, ImportResolver};
use handl_plugin_engine_exec_bytecode::BytecodeEngine;
use handl_plugin_engine_exec_cranelift::CraneliftEngine;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::Runtime;
use serde_json::json;
use std::rc::Rc;

struct StubResolver;

impl ImportResolver for StubResolver {
    fn resolve_import(&self, _path: &str) -> Result<Value, LangError> {
        Ok(json!({"x": 42, "y": "hi"}))
    }
}

fn make_runtime(engine: Box<dyn EvalEngine>) -> Runtime {
    let kit = handl_kit_data_canonical::assemble().expect("canonical kit assembles");
    Runtime::new(kit.lang().clone())
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(engine)
}

/// Every eval engine, with the stub resolver wired in.
fn engines_with_resolver() -> Vec<(&'static str, Box<dyn EvalEngine>)> {
    vec![
        (
            "tree-walk",
            Box::new(TreeWalkEngine::new().with_import_resolver(Rc::new(StubResolver)))
                as Box<dyn EvalEngine>,
        ),
        (
            "bytecode",
            BytecodeEngine::new().with_import_resolver(Rc::new(StubResolver)),
        ),
        (
            "cranelift",
            CraneliftEngine::new().with_import_resolver(Rc::new(StubResolver)),
        ),
    ]
}

/// Every eval engine with no resolver configured.
fn engines_without_resolver() -> Vec<(&'static str, Box<dyn EvalEngine>)> {
    vec![
        ("tree-walk", Box::new(TreeWalkEngine::new())),
        ("bytecode", Box::new(BytecodeEngine::new())),
        ("cranelift", Box::new(CraneliftEngine::new())),
    ]
}

/// Run `src` through every resolver-wired engine; assert each produces
/// `expected`.
fn assert_all_engines(src: &str, expected: Value) {
    for (name, engine) in engines_with_resolver() {
        let value = make_runtime(engine)
            .run(&Source::new(src))
            .unwrap_or_else(|e| panic!("{name} engine failed on `{src}`: {e}"));
        assert_eq!(value, expected, "{name} engine on `{src}`");
    }
}

/// Run `src` through every resolver-wired engine; assert each fails with
/// an error containing `needle`.
fn assert_all_engines_err(src: &str, needle: &str) {
    for (name, engine) in engines_with_resolver() {
        let err = match make_runtime(engine).run(&Source::new(src)) {
            Err(e) => e,
            Ok(v) => panic!("{name} engine should fail on `{src}`, got {v}"),
        };
        assert!(
            err.to_string().contains(needle),
            "{name} engine on `{src}`: expected error containing `{needle}`, got: {err}"
        );
    }
}

#[test]
fn import_splices_module_bindings_all_engines() {
    assert_all_engines(r#"do { import "m"; x }"#, json!(42));
}

#[test]
fn from_import_star_splices_module_bindings_all_engines() {
    assert_all_engines(r#"do { from "m" import *; y }"#, json!("hi"));
}

#[test]
fn from_import_star_as_binds_module_record_all_engines() {
    assert_all_engines(r#"do { from "m" import * as m; m.x }"#, json!(42));
}

#[test]
fn from_import_named_projects_export_all_engines() {
    assert_all_engines(r#"do { from "m" import y; y }"#, json!("hi"));
}

#[test]
fn from_import_named_as_binds_alias_all_engines() {
    assert_all_engines(r#"do { from "m" import x as z; z }"#, json!(42));
}

#[test]
fn from_import_braced_list_binds_each_entry_all_engines() {
    assert_all_engines(r#"do { from "m" import { x, y as w }; w }"#, json!("hi"));
}

// Note: the import statement is the *last* statement in these `do`
// blocks — `Runtime::eval`'s `Term`-lowerer fast path discards non-binding
// mid-block statements, so a trailing `; 0` would never reach the eval
// engines (pre-existing lowerer behavior, unrelated to the op itself).
#[test]
fn import_missing_export_errors_all_engines() {
    assert_all_engines_err(
        r#"do { from "m" import zzz }"#,
        "module has no export `zzz`",
    );
}

#[test]
fn import_without_resolver_errors_all_engines() {
    for (name, engine) in engines_without_resolver() {
        let err = match make_runtime(engine).run(&Source::new(r#"do { import "m" }"#)) {
            Err(e) => e,
            Ok(v) => panic!("{name} engine should fail without a resolver, got {v}"),
        };
        assert!(
            err.to_string().contains("no import resolver configured"),
            "{name} engine: expected no-resolver error, got: {err}"
        );
    }
}
