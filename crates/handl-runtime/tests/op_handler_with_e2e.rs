//! End-to-end `op` / `handle ... with ...` runtime test.
//!
//! The `op` declaration is parsed, evaluated, and registered on the
//! `Language`. A concrete `Handler` value with an `add` implementation is
//! bound as an ordinary name — a handler is just a normal arrow/record
//! value, no dedicated declaration keyword. The
//! `handle add(1, 2) with NumberHandler` surface expression is parsed and
//! run through the tree-walk engine, exercising grade-based op dispatch
//! from source.

use std::collections::HashMap;
use std::rc::Rc;

use handl_kit_data_canonical;
use handl_language::Source;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::{EngineRegistry, Runtime};
use plugin_engine::TermData;
use serde_json::json;

#[test]
fn op_handler_with_runs_end_to_end() {
    let kit = handl_kit_data_canonical::assemble().expect("canonical kit assembles");

    let mut reg = EngineRegistry::new();
    reg.register_lex("data", Box::new(DataLexerEngine::new()));
    reg.register_parse("pratt", PrattEngine::new());
    reg.register_eval("tree-walk", Box::new(TreeWalkEngine::new()));
    let registry = Rc::new(reg);

    let mut config = kit.runtime_config().clone();
    config.typecheck.clear();
    let mut runtime =
        Runtime::from_registry(kit.lang().clone(), &registry, &config).expect("runtime binds");

    // Parse and evaluate the `op` declaration, then register it.
    let op_src = "op add : (Number, Number) ~[Addable]~> Number";
    let op_toks = runtime.lex(&Source::new(op_src)).expect("lex op");
    let op_ast = runtime.parse(&op_toks).expect("parse op");
    let op_value = runtime.eval(&op_ast).expect("eval op");
    runtime.lang_mut().register_op("add", op_value);

    // Build a concrete Handler value: `add` returns a constant `Number`.
    // A handler is an ordinary value bound with `:=` — here we inject the
    // map directly for the end-to-end `handle` test.
    let add_impl = serde_json::to_value(
        &TermData::arr(json!(3.0))
            .to_term()
            .expect("TermData -> Term"),
    )
    .expect("Term serializes");
    let handler_value = json!({
        "__tag__": "Handler",
        "__registry__": "handl.canonical",
        "grades": ["Addable"],
        "impls": {
            "add": add_impl
        }
    });

    // Parse and run `handle add(1, 2) with NumberHandler`.
    let handle_src = "handle add(1, 2) with NumberHandler";
    let handle_toks = runtime.lex(&Source::new(handle_src)).expect("lex handle");
    let handle_ast = runtime.parse(&handle_toks).expect("parse handle");

    let mut globals = HashMap::new();
    globals.insert("NumberHandler".to_string(), handler_value);
    let result = runtime
        .eval_with_globals(&handle_ast, &globals)
        .expect("handle expression evaluates");

    assert_eq!(result["__tag__"], "Number");
    assert_eq!(result["value"], 3.0);
}
