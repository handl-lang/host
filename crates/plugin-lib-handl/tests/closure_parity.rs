//! Closure parity for `HandlModule` on tree-walk vs bytecode (ADR-0106:
//! arrows are self-contained `body_def` + `binds` data).
//!
//! Real `.hnl` modules (`transform.map` and a tiny `maker` module) that take
//! or return closures are built with both eval engines, then driven through a
//! generic `Runtime` using the same source. Results must be identical.

use std::rc::Rc;

use handl_kit_data_canonical;
use handl_language::{Source, Value};
use handl_plugin_engine_exec::EvalEngine;
use handl_plugin_engine_exec_bytecode::BytecodeEngine;
use handl_plugin_engine_exec_tree_walk::TreeWalkEngine;
use handl_plugin_engine_lex_pattern::DataLexerEngine;
use handl_plugin_engine_parse_pratt::PrattEngine;
use handl_plugin_lib_handl::HandlModule;
use handl_plugin_module::ModuleRegistry;
use handl_runtime::Runtime;
use serde_json::json;

const MAP_HNL: &str = include_str!("../../../handl-packages/transform/map.hnl");

const MAKER_HNL: &str = r#"
export make :: x ~> (y ~> x + y)
"#;

fn runtime_for(engine_name: &'static str) -> Runtime {
    let kit = handl_kit_data_canonical::assemble().unwrap();
    let lang = kit.lang().clone();

    let modules: Rc<dyn handl_platform::ModuleLookup> = Rc::new(
        ModuleRegistry::new()
            .with(HandlModule::new("transform", MAP_HNL, &["map"]).with_eval_engine(engine_name))
            .with(HandlModule::new("maker", MAKER_HNL, &["make"]).with_eval_engine(engine_name)),
    );

    let eval: Box<dyn EvalEngine> = if engine_name == "tree-walk" {
        Box::new(TreeWalkEngine::new().with_modules(Rc::clone(&modules)))
    } else {
        Box::new(BytecodeEngine::with_modules(Rc::clone(&modules)))
    };

    Runtime::new(lang)
        .with_lex_engine(Box::new(DataLexerEngine::new()))
        .with_parse_engine(PrattEngine::new())
        .with_eval_engine(eval)
}

fn assert_both_engines(src: &str, expected: Value) {
    for name in ["tree-walk", "bytecode"] {
        let rt = runtime_for(name);
        let got = rt
            .run(&Source::new(src))
            .unwrap_or_else(|e| panic!("{name}: `{src}` failed: {e}"));
        assert_eq!(got, expected, "{name}: `{src}` diverged");
    }
}

/// `transform.map` takes an arrow as an argument.
#[test]
fn transform_map_with_arrow_arg() {
    assert_both_engines("transform.map([1, 2, 3], x ~> x * 2)", json!([2, 4, 6]));
}

/// `maker.make` returns a closure that closes over `x`.
#[test]
fn module_returns_closure() {
    assert_both_engines("2 >>> maker.make(5)", json!(7));
}
