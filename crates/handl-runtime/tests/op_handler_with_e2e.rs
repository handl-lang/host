//! End-to-end `op` / `handler` / `with ... do ...` runtime test.
//!
//! The `op` declaration is parsed, evaluated, and registered on the
//! `Language`. The `handler` declaration is parsed; a concrete `Handler`
//! value with an `add` implementation is then injected manually. The
//! `with NumberHandler do add(1, 2)` surface expression is parsed and run
//! through the tree-walk engine, exercising grade-based op dispatch from
//! source.

use std::collections::HashMap;
use std::rc::Rc;

use handl_kit_data_canonical;
use handl_language::{Source, Term};
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_runtime::{EngineRegistry, Runtime};
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
    let op_src = "op add : (Number, Number) ~> Number grade Addable";
    let op_toks = runtime.lex(&Source::new(op_src)).expect("lex op");
    let op_ast = runtime.parse(&op_toks).expect("parse op");
    let op_value = runtime.eval(&op_ast).expect("eval op");
    runtime.lang_mut().register_op("add", op_value);

    // Parse the `handler` declaration to exercise the surface syntax.
    let handler_src = "handler NumberHandler = () ~> { 1 + 2 }";
    let handler_toks = runtime.lex(&Source::new(handler_src)).expect("lex handler");
    let _handler_ast = runtime.parse(&handler_toks).expect("parse handler");

    // Build a concrete Handler value: `add` returns a constant `Number`.
    // In a full implementation the source `handler` would produce this map;
    // for the end-to-end `with` test we insert it directly.
    let add_impl = serde_json::to_value(Term::Arr { value: json!(3.0) })
        .expect("Term serializes");
    let handler_value = json!({
        "__tag__": "Handler",
        "__registry__": "handl.canonical",
        "grades": ["Addable"],
        "impls": {
            "add": add_impl
        }
    });

    // Parse and run `with NumberHandler do add(1, 2)`.
    let with_src = "with NumberHandler do add(1, 2)";
    let with_toks = runtime.lex(&Source::new(with_src)).expect("lex with");
    let with_ast = runtime.parse(&with_toks).expect("parse with");

    let mut globals = HashMap::new();
    globals.insert("NumberHandler".to_string(), handler_value);
    let result = runtime
        .eval_with_globals(&with_ast, &globals)
        .expect("with expression evaluates");

    assert_eq!(result["__tag__"], "Number");
    assert_eq!(result["value"], 3.0);
}
