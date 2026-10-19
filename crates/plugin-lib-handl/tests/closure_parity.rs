//! Closure parity for `HandlModule` on tree-walk vs bytecode (ADR-0106:
//! arrows are self-contained `body_def` + `binds` data).
//!
//! Real `.hnl` modules (`transform.map` and a tiny `maker` module) that take
//! or return closures are built with both eval engines, then driven through a
//! generic `Runtime` using the same source. Results must be identical.

use std::rc::Rc;

use handl_kit_data_prelude;
use handl_language::{Source, Value};
use hnl_engine_exec::EvalEngine;
use hnl_engine_exec_bytecode::BytecodeEngine;
use hnl_engine_exec_tree_walk::TreeWalkEngine;
use hnl_engine_lex_pattern::DataLexerEngine;
use hnl_engine_parse_pratt::PrattEngine;
use handl_plugin_lib_handl::HandlModule;
use handl_plugin_module::ModuleRegistry;
use handl_runtime::Runtime;
use serde_json::json;


/// Install the canonical default language once per test binary.
///
/// `HandlModule` bridge runtimes resolve [`default_language`] lazily
/// (ADR-0128 §"The 0.8 seam"); tests assemble it from the dev-dep data
/// crate (dev-deps may link `lang/` data; the library must not).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

// `transform/map.hnl` is owned by `handl-plugin-lib-stdlib` (ADR-0128
// §".hnl ownership"); consumers take the `pub const` re-export.
const MAP_HNL: &str = handl_plugin_lib_stdlib::TRANSFORM_MAP_HNL;

const MAKER_HNL: &str = r#"
export make :: x ~> (y ~> x + y)
"#;

fn runtime_for(engine_name: &'static str) -> Runtime {
    let kit = handl_kit_data_prelude::assemble().unwrap();
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
        ensure_lang();
    assert_both_engines("transform.map([1, 2, 3], x ~> x * 2)", json!([2, 4, 6]));
}

/// `maker.make` returns a closure that closes over `x`.
#[test]
fn module_returns_closure() {
        ensure_lang();
    assert_both_engines("2 >>> maker.make(5)", json!(7));
}
