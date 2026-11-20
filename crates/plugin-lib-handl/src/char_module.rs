//! The `char` module — the host-shaped [`Module`] wrapper over
//! `handl-plugin-lib-char-native`'s kernel-ABI ops.
//!
//! `lib-char-native` registers its 18 `char.*` ops on the kernel
//! [`handl_ops::OpRegistry`] (via `char_op_registry()`); this module
//! imports that registry into the host [`OpRegistry`] through
//! [`OpRegistry::import_kernel`] and exposes the ops through
//! [`HandlModule::with_ops`], preserving the `char.<fn>` module/name keys
//! [`ModuleLookup`] resolves (`char` module + local function name).
//!
//! This is the `char` backend the embedding host provides for the `char`
//! axiom contract (`handl-packages/char/char.hnl`): `str-handl`, the
//! desugar overlay, and kit prelude all resolve `char.*` through it.

use std::sync::{Arc, OnceLock};

use handl_platform::{Function, Module};

use crate::{HandlModule, OpRegistry};

/// The qualified op names backing the `char` module — re-exported from
/// `lib-char-native` ([`handl_plugin_lib_char_native::CHAR_OPS`]).
pub const CHAR_OPS: &[&str] = handl_plugin_lib_char_native::CHAR_OPS;

static CHAR_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// The lazily-built `char` [`HandlModule`] — op dispatch only (no `.hnl`
/// record functions; every `char.*` export is an irreducible atom).
fn inner() -> &'static HandlModule {
    CHAR_MODULE.get_or_init(|| {
        let mut registry = OpRegistry::new();
        registry.import_kernel(&handl_plugin_lib_char_native::char_op_registry());
        // `with_loader_disabled`: `HandlModule::new`'s `analyze_source`
        // would otherwise route `char`'s prep through the installed
        // `.hnl` loader — a nested eval whose runtime build materializes
        // dep env bindings (`dep_env_globals` → `CharModule::name()` →
        // `inner()`) and self-deadlocks this `OnceLock`. Built-in module
        // construction uses the bootstrap transforms regardless of
        // loader state (same guard `plugin-lib-stdlib::module_ref` uses).
        crate::with_loader_disabled(|| {
            HandlModule::new("char", handl_plugin_lib_char_native::CHAR_NATIVE_HNL, &[])
                .with_ops(CHAR_OPS, Arc::new(registry))
        })
    })
}

/// The `char` module — per-character atoms for the `str` floor.
///
/// Provides `char.is_whitespace`, `is_alpha`, `is_numeric`, `is_alnum`,
/// `is_uppercase`, `is_lowercase`, `to_upper`, `to_lower`, `to_title`,
/// `code_point`, `from_code_point`, `at`, `slice`, `length`, `concat`,
/// `code_point_at`, `chars`, `from_chars`. All indices are character
/// indices; semantics follow Rust `std::char` (Unicode-aware).
///
/// A zero-sized handle: the underlying [`HandlModule`] is built once and
/// shared process-wide, so `Arc::new(CharModule)` stays cheap.
#[derive(Clone, Copy)]
pub struct CharModule;

impl Module for CharModule {
    fn name(&self) -> &str {
        inner().name()
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        inner().function(name)
    }

    fn functions(&self) -> Vec<(&str, &dyn Function)> {
        inner().functions()
    }

    fn value_names(&self) -> Vec<String> {
        inner().value_names()
    }

    fn value(&self, name: &str) -> Option<serde_json::Value> {
        inner().value(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::EvalContext;
    use serde_json::json;

    #[test]
    fn char_module_dispatches_all_ops_under_local_names() {
        let module = CharModule;
        assert_eq!(module.name(), "char");
        let names: Vec<&str> = module.functions().iter().map(|(n, _)| *n).collect();
        for local in [
            "is_whitespace",
            "is_alpha",
            "is_numeric",
            "is_alnum",
            "is_uppercase",
            "is_lowercase",
            "to_upper",
            "to_lower",
            "to_title",
            "code_point",
            "from_code_point",
            "at",
            "slice",
            "length",
            "concat",
            "code_point_at",
            "chars",
            "from_chars",
        ] {
            assert!(names.contains(&local), "missing char.{local}");
            assert!(
                module.function(local).is_some(),
                "char.{local} not callable"
            );
        }
        assert_eq!(names.len(), CHAR_OPS.len());
    }

    #[test]
    fn char_module_ops_match_kernel_semantics() {
        let module = CharModule;
        let mut ctx = EvalContext::new();
        let at = module.function("at").expect("char.at");
        let out = at
            .call(&[json!("héllo"), json!(1.0)], &mut ctx)
            .expect("char.at call");
        assert_eq!(out, json!("é"));
        let length = module.function("length").expect("char.length");
        let out = length
            .call(&[json!("héllo")], &mut ctx)
            .expect("char.length");
        assert_eq!(out, json!(5.0));
    }
}
