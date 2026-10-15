//! Regression: `.hnl` import statements are evaluated by the RUNTIME
//! (`prim_import` via the kit grammar), so a **named** import
//! (`from "./x.hnl" import f`) binds a bare callable `f`, while the existing
//! `from "x" import * as m` (`m.f`) form keeps working.
//!
//! The `TableImportResolver` turns each imported export into the string
//! `"<module>.<field>"` — the very callee form a qualified `mod.fn` already
//! produces — which the importer's module registry dispatches.

use std::sync::Arc;

use handl_platform::{EvalContext, Module};
use handl_plugin_lib_handl::HandlModule;
use handl_plugin_lib_stdlib::DEFAULT_RESOLVER;
use handl_plugin_lib_stdlib::TableModuleLookup;
use handl_plugin_lib_stdlib::packages::TableImportResolver;


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

fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let f = module.function(name).expect("function exists");
    f.call(args, &mut EvalContext::new())
        .expect("call succeeds")
}

/// `from "len" import len` binds the bare name `len`, called directly.
#[test]
fn named_import_binds_a_bare_callable() {
        ensure_lang();
    const SRC: &str = "from \"len\" import len\nexport length_of :: xs ~> len(xs)";
    let module = HandlModule::new("named_import_probe", SRC, &["length_of"])
        .with_import_resolver(Arc::new(TableImportResolver::new()))
        .with_global_lookup(Arc::new(TableModuleLookup::new(Arc::new(
            DEFAULT_RESOLVER.clone(),
        ))));
    let out = call(&module, "length_of", &[serde_json::json!([1, 2, 3])]);
    assert_eq!(out, serde_json::json!(3));
}

/// `from "len" import * as l` still binds the module object; `l.len` works.
#[test]
fn qualified_import_still_binds_the_module_object() {
        ensure_lang();
    const SRC: &str = "from \"len\" import * as l\nexport length_of :: xs ~> l.len(xs)";
    let module = HandlModule::new("qualified_import_probe", SRC, &["length_of"])
        .with_import_resolver(Arc::new(TableImportResolver::new()))
        .with_global_lookup(Arc::new(TableModuleLookup::new(Arc::new(
            DEFAULT_RESOLVER.clone(),
        ))));
    let out = call(&module, "length_of", &[serde_json::json!([1, 2, 3, 4])]);
    assert_eq!(out, serde_json::json!(4));
}

/// `from "bool" import { true, false }` binds real Bool values — `true`/`false`
/// are *value exports* on the bool module (`values: &["true", "false"]`),
/// not callable wire strings.
#[test]
fn named_import_binds_value_exports() {
        ensure_lang();
    const SRC: &str =
        "from \"bool\" import { true, false, xor }\nexport check :: x ~> xor(true, false)";
    let module = HandlModule::new("named_import_values_probe", SRC, &["check"])
        .with_import_resolver(Arc::new(TableImportResolver::new()))
        .with_global_lookup(Arc::new(TableModuleLookup::new(Arc::new(
            DEFAULT_RESOLVER.clone(),
        ))));
    let out = call(&module, "check", &[serde_json::json!(7)]);
    assert_eq!(out, serde_json::json!(true));
}

/// `from "bool" import *` splices both callable exports (wire strings) and
/// value exports (real Bools) into scope.
#[test]
fn star_import_splices_value_exports() {
        ensure_lang();
    const SRC: &str = "from \"bool\" import *\nexport check :: x ~> not(true)";
    let module = HandlModule::new("star_import_values_probe", SRC, &["check"])
        .with_import_resolver(Arc::new(TableImportResolver::new()))
        .with_global_lookup(Arc::new(TableModuleLookup::new(Arc::new(
            DEFAULT_RESOLVER.clone(),
        ))));
    let out = call(&module, "check", &[serde_json::json!(0)]);
    assert_eq!(out, serde_json::json!(false));
}
