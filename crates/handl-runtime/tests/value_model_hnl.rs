//! Integration test for ADR-0103: a Handl-authored `ValueModel` registered
//! as an eval engine and selected through `RuntimeConfig::eval`.

use std::rc::Rc;

use handl_kit_data_canonical::assemble;
use handl_language::{Ast, AstNode, EvalSchemeDef, Term, Value};
use handl_plugin_lib_handl::{HandlModelEngine, HandlModule};
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use handl_runtime::{EngineRegistry, Runtime, RuntimeConfig};

const VALUE_MODEL_HNL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../project/design/adr0103/value_model.hnl"
));

#[test]
fn handl_value_model_evaluates_arr_const() {
    // Build the canonical language and add a tiny eval scheme that exposes
    // a concrete Term to the engine.
    let mut lang = assemble().unwrap().lang().clone();
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.test",
            "test.root",
            Term::Arr {
                value: Value::from(5),
            },
        ))
        .unwrap();

    // Load the Handl value model and wire the runtime modules it uses.
    let module = HandlModule::new("value_model", VALUE_MODEL_HNL, &["run"])
        .with_dependency(stdlib::module("array").expect("array module"))
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(TagModule);

    let mut reg = EngineRegistry::new();
    reg.register_eval("handl-value", Box::new(HandlModelEngine::new(module)));

    let rt = Runtime::from_registry(
        lang,
        &Rc::new(reg),
        &RuntimeConfig::new().with_eval("handl-value"),
    )
    .expect("runtime should build with handl-value eval engine");

    let mut ast = Ast::new();
    ast.push(AstNode::new("test.root"));

    let result = rt.eval(&ast).expect("eval should return 5");
    assert_eq!(result, Value::from(5));
}
