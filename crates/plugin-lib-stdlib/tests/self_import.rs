//! Regression: the sdkgen port/adapter `_c` split — the adapter spec
//! `uchar_c` (`icu-uc-c-linux-x86_64/uchar_c.hnl`) contains
//! `from "../icu-uc/uchar" import { UCharType78, … }`, which
//! basename-normalizes to `uchar` — now the *port* spec row, not the
//! adapter itself. Resolution must succeed; only the `ffi.call` leaf may
//! still fail (no `ffi` module under the default table resolver, and no
//! `libicuuc.so` on machines without ICU).

use handl_platform::{EvalContext, Module};
use handl_plugin_lib_stdlib::module;

fn ensure_lang() {
    if handl_plugin_lib_handl::default_language().is_none() {
        let _ = handl_plugin_lib_handl::install_default_language(
            handl_kit_data_prelude::language_from_yaml().expect("canonical language assembles"),
        );
    }
}

/// The port row carries the bare name `uchar`; the C adapter is `uchar_c`.
#[test]
fn uchar_port_and_adapter_rows_both_resolve() {
    ensure_lang();
    assert!(module("uchar").is_some(), "port spec `uchar` in table");
    assert!(
        module("uchar_c").is_some(),
        "adapter spec `uchar_c` in table"
    );
}

/// Evaluating a `uchar_c` function resolves its imports — `result` plus
/// `from "../icu-uc/uchar" import { UCharType78 }` onto the port row —
/// instead of self-importing. Any remaining error must come from the
/// `ffi.call` leaf, not from import resolution.
#[test]
fn uchar_c_resolves_port_imports_instead_of_self_importing() {
    ensure_lang();
    let module = module("uchar_c").expect("uchar_c spec");
    let f = module
        .function("u_charType_78")
        .expect("uchar_c exports u_charType_78");
    let mut ctx = EvalContext::new();
    match f.call(&[serde_json::json!("a")], &mut ctx) {
        Ok(v) => {
            // Only reachable when `ffi` resolves and `libicuuc.so`
            // answers: the call itself succeeded.
            let _ = v;
        }
        Err(e) => {
            let err = e.to_string();
            assert!(
                !err.contains("cannot import itself"),
                "uchar_c must not self-import: {err}"
            );
            assert!(
                !err.contains("not in the package table"),
                "uchar_c imports must resolve through the table: {err}"
            );
            assert!(
                !err.contains("has no export"),
                "port `uchar` must project the imported signature names: {err}"
            );
        }
    }
}

/// `Module::value` on the adapter's const exports evaluates the module
/// record — the port import must resolve for that projection too, and
/// `UCHAR_XID_START` binds the adapter's concrete constant (33).
#[test]
fn uchar_c_value_exports_evaluate() {
    ensure_lang();
    let module = module("uchar_c").expect("uchar_c spec");
    let v = module
        .value("UCHAR_XID_START")
        .expect("UCHAR_XID_START projects");
    assert_eq!(v, serde_json::json!(33));
}

/// The port row's `funcs` entries are binding placeholders: calling one
/// evaluates the port source, whose `from "ffi" import { Errno }` has no
/// table answer — the failure is about `ffi`, never a missing export of
/// `uchar` itself.
#[test]
fn uchar_port_exports_bind_but_port_eval_reaches_ffi() {
    ensure_lang();
    let module = module("uchar").expect("uchar port spec");
    let f = module
        .function("UCharType78")
        .expect("port exports UCharType78");
    let mut ctx = EvalContext::new();
    let err = f
        .call(&[serde_json::json!(0)], &mut ctx)
        .expect_err("port signature is not callable; eval reaches its ffi import")
        .to_string();
    assert!(
        err.contains("ffi") || err.contains("import"),
        "expected the port's `from \"ffi\" import` to be the failing leaf, got: {err}"
    );
    assert!(
        !err.contains("has no export") && !err.contains("cannot import itself"),
        "port `uchar` must resolve and export its names: {err}"
    );
}
