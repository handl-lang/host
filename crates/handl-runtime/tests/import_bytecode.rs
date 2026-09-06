//! Integration test: `import` works end-to-end through the bytecode eval engine.
//!
//! Builds a generic `Runtime` from the canonical data-driven language, wires a
//! stub `ImportResolver`, and evaluates `do { import "m"; x }` with the
//! bytecode engine. Verifies that `eval_block` and the `import` native op
//! cooperate to bind module exports and return the last expression.

use handl_language::{LangError, Source, Value};
use handl_plugin_engine_eval::{EvalEngine, ImportResolver};
use handl_plugin_engine_eval_bytecode::BytecodeEngine;
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

fn make_runtime() -> Runtime {
    let kit = handl_kit_data_canonical::assemble().expect("canonical kit assembles");
    let lang = kit.lang().clone();

    let eval_engine: Box<dyn handl_plugin_engine_eval::EvalEngine> =
        BytecodeEngine::new().with_import_resolver(Rc::new(StubResolver));

    Runtime::new(lang)
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(eval_engine)
}

#[test]
fn bytecode_import_binds_module_bindings() {
    let runtime = make_runtime();
    let value = runtime
        .run(&Source::new(r#"do { import "m"; x }"#))
        .expect("run should succeed");
    assert_eq!(value, json!(42));
}

#[test]
fn bytecode_import_is_statement_value() {
    let runtime = make_runtime();
    let value = runtime
        .run(&Source::new(r#"do { import "m"; 0 }"#))
        .expect("run should succeed");
    assert_eq!(value, json!(0));
}
