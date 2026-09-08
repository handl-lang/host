//! # handl-plugin-lib-result
//!
//! The pure-handl `result` library for error handling (ADR-0077).
//!
//! Provides `result.ok` and `result.err` — constructors for the `Result`
//! type, built on the nominal tag system (ADR-0055). An `Ok` result is a
//! tagged value `{"__tag__": "ok", "value": <T>}`; an `Err` result is
//! `{"__tag__": "err", "value": <E>}`.
//!
//! Predicates (`is_ok`, `is_err`) are not provided — use `mood.int(r, ok)`
//! and `mood.int(r, err)` directly, or pattern match with
//! `match r { ok(v) => ..., err(e) => ... }`.
//!
//! This crate is pure handl: the logic lives in
//! `handl-packages/result/result.hnl`, loaded via the generic
//! [`HandlModule`]. The only Rust is the plugin scaffolding. The
//! [`TagModule`] is registered as a dependency so `tag.tag` resolves
//! during evaluation.

#![warn(missing_docs)]

use handl_platform::Plugin;
use handl_plugin_lib_handl::{HandlLibraryPlugin, HandlModule};
use handl_plugin_lib_tag::TagModule;
#[cfg(test)]
use std::sync::OnceLock;

/// The handl source defining the result constructors — the single source
/// of truth for the result logic. Loaded at compile time from the
/// handl-packages data dir.
const RESULT_HNL: &str = include_str!("../../../handl-packages/result/result.hnl");

/// The function (record field) names exposed by the result module.
const RESULT_FUNCS: &[&str] = &["ok", "err"];

/// Build the [`HandlModule`] for result, with [`TagModule`] (for `tag.tag`)
/// as a dependency.
///
/// The `tag.tag` host function is needed because the `tag ok = Value` and
/// `tag err = Error` declarations expand to constructor arrows that call
/// `tag.tag("ok", {value: _0})` / `tag.tag("err", {value: _0})`.
pub fn result_module() -> HandlModule {
    HandlModule::new("result", RESULT_HNL, RESULT_FUNCS).with_dependency(TagModule)
}

/// A thread-safe lazy-initialized [`HandlModule`] for result.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
#[cfg(test)]
static RESULT_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for result, initializing it on first call.
#[cfg(test)]
fn shared_result_module() -> &'static HandlModule {
    RESULT_MODULE.get_or_init(result_module)
}

/// A `Library` plugin that contributes the result module into the
/// platform's `ModuleRuntime`.
///
/// This is a thin wrapper around the generic [`HandlLibraryPlugin`],
/// keeping the same public API pattern as other pure-handl library crates.
/// The result logic is handl-authored in `result.hnl` and loaded via the
/// generic loader; no per-library Rust bridge code remains.
pub struct ResultLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl ResultLibraryPlugin {
    /// Create a new `ResultLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new(
                "result",
                "lib.result",
                "lib-result",
                RESULT_HNL,
                RESULT_FUNCS,
            )
            .with_dependency(TagModule),
        }
    }
}

impl Default for ResultLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ResultLibraryPlugin {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn capabilities(&self) -> &[handl_platform::CapabilityId] {
        self.inner.capabilities()
    }

    fn metadata(
        &self,
        capability: &handl_platform::CapabilityId,
    ) -> Option<handl_platform::CapabilityMetadata> {
        self.inner.metadata(capability)
    }

    fn contribute(
        &self,
        ctx: &mut handl_platform::ContributionContext,
    ) -> Result<(), handl_platform::PlatformError> {
        self.inner.contribute(ctx)
    }

    fn run(
        &self,
        request: &handl_platform::CapabilityRequest,
        ctx: &mut handl_platform::PluginContext,
    ) -> Result<handl_platform::CapabilityResponse, handl_platform::PlatformError> {
        self.inner.run(request, ctx)
    }
}

/// Build a [`HandlLibraryPlugin`] for result, with [`TagModule`] as a
/// dependency.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`ResultLibraryPlugin`].
pub fn result_handl_plugin() -> HandlLibraryPlugin {
    HandlLibraryPlugin::new(
        "result",
        "lib.result",
        "lib-result",
        RESULT_HNL,
        RESULT_FUNCS,
    )
    .with_dependency(TagModule)
}

#[cfg(test)]
mod tests {
    use handl_platform::{EvalContext, Module, Plugin};
    use serde_json::json;

    use super::shared_result_module;

    /// Helper: call a result function by name with the given args.
    fn call(name: &str, args: &[serde_json::Value]) -> serde_json::Value {
        let func = shared_result_module()
            .function(name)
            .unwrap_or_else(|| panic!("result.{name} should exist"));
        func.call(args, &mut EvalContext::new())
            .unwrap_or_else(|e| panic!("result.{name} failed: {e}"))
    }

    #[test]
    fn ok_wraps_value() {
        let result = call("ok", &[json!(42.0)]);
        assert_eq!(result, json!({"__tag__": "ok", "value": 42.0}));
    }

    #[test]
    fn err_wraps_error() {
        let result = call("err", &[json!("nope")]);
        assert_eq!(result, json!({"__tag__": "err", "value": "nope"}));
    }

    #[test]
    fn ok_and_err_have_different_tags() {
        let ok_val = call("ok", &[json!(42.0)]);
        let err_val = call("err", &[json!(42.0)]);
        assert_ne!(ok_val, err_val);
        assert_eq!(ok_val.get("__tag__"), Some(&json!("ok")));
        assert_eq!(err_val.get("__tag__"), Some(&json!("err")));
    }

    #[test]
    fn ok_wraps_string() {
        let result = call("ok", &[json!("hello")]);
        assert_eq!(result, json!({"__tag__": "ok", "value": "hello"}));
    }

    #[test]
    fn err_wraps_number() {
        let result = call("err", &[json!(404.0)]);
        assert_eq!(result, json!({"__tag__": "err", "value": 404.0}));
    }

    #[test]
    fn module_name_is_result() {
        assert_eq!(shared_result_module().name(), "result");
    }

    #[test]
    fn module_exposes_ok_and_err() {
        let module = shared_result_module();
        assert!(module.function("ok").is_some());
        assert!(module.function("err").is_some());
        assert!(module.function("is_ok").is_none());
        assert!(module.function("is_err").is_none());
    }

    #[test]
    fn plugin_id_and_name_are_preserved() {
        let plugin = super::ResultLibraryPlugin::new();
        assert_eq!(plugin.id(), "lib.result");
        assert_eq!(plugin.name(), "lib-result");
    }

    #[test]
    fn handl_plugin_constructor_matches() {
        let plugin = super::result_handl_plugin();
        assert_eq!(plugin.id(), "lib.result");
        assert_eq!(plugin.name(), "lib-result");
    }
}
