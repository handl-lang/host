//! Integration test for ADR-0103: a Handl-authored `arrow_model` registered
//! as an eval engine and selected through `RuntimeConfig::eval`.

use std::rc::Rc;

use handl_kit_data_canonical::assemble;
use handl_language::{Ast, AstNode, EvalSchemeDef, Value};
use handl_plugin_lib_handl::{HandlModelEngine, HandlModule};
use handl_plugin_lib_stdlib as stdlib;
use handl_plugin_lib_tag::TagModule;
use handl_runtime::{EngineRegistry, Runtime, RuntimeConfig};
use hnl_engine::TermData;


/// Install the canonical default language once per test binary.
///
/// `HandlModule` bridge runtimes resolve [`default_language`] lazily
/// (ADR-0128 §"The 0.8 seam"); tests assemble it from the dev-dep data
/// crate (dev-deps may link `lang/` data; the library must not).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_canonical::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

const ARROW_MODEL_HNL: &str = include_str!("fixtures/arrow_model.hnl");

#[test]
fn handl_arrow_model_evaluates_arr_const() {
        ensure_lang();
    // Build the canonical language and add a tiny eval scheme that exposes
    // a concrete Term to the engine.
    let mut lang = assemble().unwrap().lang().clone();
    lang.eval
        .merge(EvalSchemeDef::new(
            "e.test",
            "test.root",
            TermData::arr(Value::from(5))
                .to_term()
                .expect("TermData -> Term"),
        ))
        .unwrap();

    // Load the Handl arrow model and wire the runtime modules it uses.
    let module = HandlModule::new("arrow_model", ARROW_MODEL_HNL, &["run"])
        .with_dependency_arc(
            stdlib::load_hnl_file(format!(
                "{}/src/array/index.hnl",
                handl_lib_collections::ROOT
            ))
            .expect("array module"),
        )
        .with_dependency(stdlib::module("hash_map").expect("hash_map module"))
        .with_dependency(stdlib::module("typeof").expect("typeof module"))
        .with_dependency(TagModule);

    let mut reg = EngineRegistry::new();
    reg.register_eval("handl-arrow", Box::new(HandlModelEngine::new(module)));

    let rt = Runtime::from_registry(
        lang,
        &Rc::new(reg),
        &RuntimeConfig::new().with_eval("handl-arrow"),
    )
    .expect("runtime should build with handl-arrow eval engine");

    let mut ast = Ast::new();
    ast.push(AstNode::new("test.root"));

    let result = rt.eval(&ast).expect("eval should return 5");
    assert_eq!(result, Value::from(5));
}
