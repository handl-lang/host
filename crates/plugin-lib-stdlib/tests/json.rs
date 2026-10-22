//! `json` library row — the module loads the flattened single-source
//! `.hnl` (`handl_lib_json::JSON_FLAT_HNL`, generated from lib-json's
//! `src/index_flat.hnl`), so these tests pin that the flat artifact stays
//! in sync with the split sources: `parse`/`stringify`/`json` resolve
//! through `module_with("json", …)`, named imports
//! (`from "json" import { parse }`) dispatch through the table, and
//! parse∘stringify round-trips.
//!
//! The flat `.hnl` calls `char.*`/`str.*`/`obj.*`/`typeof.*`. `char` is a
//! contract-only floor module (no table row — a table `char` would shadow
//! the host's native module), so the resolver must carry it as a fallback,
//! exactly like production's `module_with(name, StdlibResolver)` path.

use std::sync::Arc;

use handl_platform::{EvalContext, Module, ModuleLookup, Plugin};
use handl_plugin_lib_char_native::CharModule;
use handl_plugin_lib_handl::{HandlModule, ModuleResolver};
use handl_plugin_lib_stdlib::packages::TableImportResolver;
use handl_plugin_lib_stdlib::{
    PackageResolver, TableModuleLookup, library, module_with,
};
use handl_plugin_module::ModuleRegistry;
use serde_json::json;

/// Install the canonical default language once per test binary (see
/// `named_import.rs` — the `HandlModule` bridge runtimes resolve it lazily).
fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml().expect("canonical language assembles"),
        );
    }
}

/// Floor-module resolver: native `char` plus the package table — the same
/// resolver shape production passes to `module_with`.
fn floor_resolver() -> Arc<dyn ModuleResolver> {
    struct CharFloor;
    impl ModuleResolver for CharFloor {
        fn resolve(&self, name: &str) -> Option<Arc<dyn Module + Send + Sync>> {
            (name == "char").then(|| Arc::new(CharModule) as Arc<dyn Module + Send + Sync>)
        }
    }
    Arc::new(PackageResolver {
        fallback: Some(Arc::new(CharFloor)),
    })
}

fn table_module(name: &str) -> HandlModule {
    module_with(name, floor_resolver()).expect("module in the package table")
}

fn call(module: &dyn Module, name: &str, args: &[serde_json::Value]) -> serde_json::Value {
    let f = module.function(name).expect("function exists");
    f.call(args, &mut EvalContext::new())
        .expect("call succeeds")
}

/// A user-source probe wired like the `hnl` runtime: imports resolve
/// through the module table, bare globals through the table lookup.
fn probe(src: &str, funcs: &[&str]) -> HandlModule {
    ensure_lang();
    HandlModule::new("json_librow_probe", src, funcs)
        .with_import_resolver(Arc::new(TableImportResolver::with_resolver(
            floor_resolver(),
        )))
        .with_global_lookup(Arc::new(TableModuleLookup::new(floor_resolver())))
}

#[test]
fn json_module_parse_and_stringify() {
    ensure_lang();
    let m = table_module("json");
    assert_eq!(m.name(), "json");

    let parsed = call(&m, "parse", &[json!("{\"a\":1}")]);
    assert_eq!(parsed, json!({"a": 1.0}));

    let text = call(&m, "stringify", &[json!({"a": [1.0, 2.0]})]);
    assert_eq!(text, json!("{\"a\":[1,2]}"));
}

#[test]
fn json_module_parse_stringify_round_trip() {
    ensure_lang();
    let m = table_module("json");
    let original = json!({"a": [1.0, 2.0], "b": {"c": "x"}, "n": null, "t": true});
    let text = call(&m, "stringify", &[original.clone()]);
    let back = call(&m, "parse", &[text]);
    assert_eq!(back, original);
}

/// `from "json" import { parse }` — the librow path: the named import is
/// evaluated by `prim_import`, which resolves module `"json"` through the
/// table and binds `parse` to the flat module's export.
#[test]
fn named_import_from_json_librow() {
    const SRC: &str = "from \"json\" import { parse }\nexport go := (s) ~> parse(s)";
    let m = probe(SRC, &["go"]);
    let out = call(&m, "go", &[json!("{\"a\":[1,2]}")]);
    assert_eq!(out, json!({"a": [1.0, 2.0]}));
}

/// `from "json" import * as json` — the qualified form keeps working and
/// the codec record's `name` is readable inside `.hnl`.
#[test]
fn star_import_and_codec_name() {
    const SRC: &str = "from \"json\" import * as json\nexport codec_name := () ~> json.json().name\nexport rt := (s) ~> json.parse(json.stringify(json.parse(s)))";
    let m = probe(SRC, &["codec_name", "rt"]);
    assert_eq!(call(&m, "codec_name", &[]), json!("json"));
    let out = call(&m, "rt", &[json!("{\"k\":[1,{\"n\":null}]}")]);
    assert_eq!(out, json!({"k": [1.0, {"n": null}]}));
}

#[test]
fn json_module_works_in_module_registry() {
    ensure_lang();
    let m = table_module("json");
    let registry = ModuleRegistry::new().with(m);
    let lookup: &dyn ModuleLookup = &registry;

    let func = lookup
        .lookup("json.parse")
        .expect("json.parse should resolve");
    let mut ctx = EvalContext::new();
    let result = func.call(&[json!("42")], &mut ctx).unwrap();
    assert_eq!(result, json!(42.0));
}

// -----------------------------------------------------------------------
// Neighboring utils/serialize rows (kept from the pre-.hnl suite).
// -----------------------------------------------------------------------

#[test]
fn utils_plugin_id_and_name() {
    let plugin = library("utils").unwrap();
    assert_eq!(plugin.id(), "lib.utils");
    assert_eq!(plugin.name(), "lib-utils");
}

#[test]
fn serialize_module_works_in_module_registry() {
    ensure_lang();
    let module = table_module("serialize");
    let registry = ModuleRegistry::new().with(module);
    let lookup: &dyn ModuleLookup = &registry;

    assert!(lookup.lookup("serialize.serialize").is_some());
    let func = lookup
        .lookup("serialize.deserialize")
        .expect("serialize.deserialize should resolve");
    let mut ctx = EvalContext::new();
    let result = func.call(&[json!("42"), json!("json")], &mut ctx).unwrap();
    assert_eq!(result, json!(42.0));
}

#[test]
fn utils_plugin_declares_serialize_capabilities() {
    let plugin = library("utils").unwrap();
    let caps = plugin.capabilities();
    assert!(
        caps.iter().any(|c| c.as_str() == "serialize.serialize"),
        "missing serialize.serialize cap"
    );
    assert!(
        caps.iter().any(|c| c.as_str() == "serialize.deserialize"),
        "missing serialize.deserialize cap"
    );
}
