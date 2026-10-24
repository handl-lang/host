//! Regression: a spec whose source basename-normalizes to its own name
//! must fail loudly, not stack-overflow (the `uchar` self-import class).
//! `icu-uc-c-linux-x86_64/uchar.hnl` contains `from "../icu-uc/uchar" import`
//! — basename `uchar` resolves back to the `uchar` spec row.

use handl_platform::{EvalContext, Module};
use handl_plugin_lib_stdlib::module;

fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml()
                .expect("canonical language assembles"),
        );
    }
}

/// Calling a `uchar` function evaluates its source, whose
/// `from "../icu-uc/uchar" import` resolves `uchar` → the same spec →
/// previously recursed forever. It must now terminate with a loud eval
/// error (currently: the self-resolved module lacks the requested
/// `UHasBinaryProperty78` export; a `TABLE_IMPORT_IN_PROGRESS` guard
/// additionally rejects any re-entrant same-name resolution).
#[test]
fn uchar_self_import_errors_instead_of_recursing() {
    ensure_lang();
    let module = module("uchar").expect("uchar spec");
    let f = module.function("u_charType_78").expect("uchar fn");
    let mut ctx = EvalContext::new();
    let err = f
        .call(&[serde_json::json!("a")], &mut ctx)
        .expect_err("self-import is an eval error, not a hang")
        .to_string();
    assert!(
        err.contains("uchar") || err.contains("import"),
        "expected a loud eval error about the self-importing module, got: {err}"
    );

    // The `Module::value` path recursed historically — it must terminate too.
    let _ = module.value("UCHAR_XID_START");
}
