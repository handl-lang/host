//! Regression: `.hnl` import statements are evaluated by the RUNTIME
//! (`prim_import` via the kit grammar), so a **named** import
//! (`from "./x.hnl" import f`) binds a bare callable `f`, while the existing
//! `from "x" import * as m` (`m.f`) form keeps working.
//!
//! The `TableImportResolver` turns each imported export into the string
//! `"<module>.<field>"` — the very callee form a qualified `mod.fn` already
//! produces — which the importer's module registry dispatches.

use std::sync::Arc;

use handl_platform::{EvalContext, Function, Module};
use handl_plugin_lib_handl::HandlModule;
use handl_plugin_lib_stdlib::packages::TableImportResolver;
use handl_plugin_lib_stdlib::DEFAULT_RESOLVER;

fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let f = module.function(name).expect("function exists");
    f.call(args, &mut EvalContext::new()).expect("call succeeds")
}

/// `from "len" import len` binds the bare name `len`, called directly.
#[test]
fn named_import_binds_a_bare_callable() {
    const SRC: &str = "from \"len\" import len\nexport length_of :: xs ~> len(xs)";
    let module = HandlModule::new("named_import_probe", SRC, &["length_of"])
        .with_resolver(&DEFAULT_RESOLVER)
        .with_import_resolver(Arc::new(TableImportResolver::new()));
    let out = call(&module, "length_of", &[serde_json::json!([1, 2, 3])]);
    assert_eq!(out, serde_json::json!(3));
}

/// `from "len" import * as l` still binds the module object; `l.len` works.
#[test]
fn qualified_import_still_binds_the_module_object() {
    const SRC: &str = "from \"len\" import * as l\nexport length_of :: xs ~> l.len(xs)";
    let module = HandlModule::new("qualified_import_probe", SRC, &["length_of"])
        .with_resolver(&DEFAULT_RESOLVER)
        .with_import_resolver(Arc::new(TableImportResolver::new()));
    let out = call(&module, "length_of", &[serde_json::json!([1, 2, 3, 4])]);
    assert_eq!(out, serde_json::json!(4));
}
