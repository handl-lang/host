//! # handl-lib-dedent
//!
//! Provides a `dedent` module for the handl runtime.
//!
//! Similar to the TypeScript `dedent` package, [`DedentModule`] exposes
//! `dedent.dedent(text)` which strips the common leading whitespace from
//! every line in the input string. This is especially useful with template
//! string literals for producing clean multi-line output.
//!
//! The dedent *logic* is handl-authored in
//! `handl-packages/kit-stdlib/dedent/dedent.hnl` (a record of arrows: `dedent`);
//! this crate loads and bridges that definition via the generic
//! [`handl_plugin_lib_handl`] loader. The per-library Rust bridge code has
//! been eliminated: [`HandlModule`] / [`HandlLibraryPlugin`] evaluate the
//! `.hnl` source directly, exposing `dedent.dedent` as a callable
//! [`Function`](handl_platform::Function).
//!
//! ## Example (handl)
//!
//! ```text
//! dedent.dedent(`    Hello,
//!       World!`)
//! // => "Hello,\n  World!"
//! ```

/// The handl source defining the dedent record — the single source of truth
/// for the dedent logic. Loaded at compile time from the kit-stdlib data dir.
const DEDENT_HNL: &str = include_str!("../../../handl-packages/kit-stdlib/dedent/dedent.hnl");

/// The function (record field) names exposed by the dedent module.
const DEDENT_FUNCS: &[&str] = &["dedent"];

use handl_platform::{EvalContext, EvalError, Function, Module, Plugin};
use handl_plugin_lib_handl::{HandlLibraryPlugin, HandlModule};
use handl_plugin_lib_utils::StrModule;
use serde_json::Value;
use std::sync::OnceLock;

/// Build the [`HandlModule`] for dedent, with `StrModule` (for `str.lines`,
/// `str.trim`, `str.trim_matches`, `str.char_at`, `str.slice`, `str.join`,
/// `str.length`, `str.is_empty`) as a dependency.
fn build_dedent_module() -> HandlModule {
    HandlModule::new("dedent", DEDENT_HNL, DEDENT_FUNCS).with_dependency(StrModule)
}

/// A thread-safe lazy-initialized [`HandlModule`] for dedent.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
static DEDENT_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for dedent, initializing it on first call.
fn dedent_module() -> &'static HandlModule {
    DEDENT_MODULE.get_or_init(build_dedent_module)
}

/// Require exactly one string argument, returning it as a [`Value`].
fn require_one_string(args: &[Value]) -> Result<&Value, EvalError> {
    if args.is_empty() {
        return Err(EvalError::TypeMismatch(format!(
            "expected {} args, got {}",
            1, 0,
        )));
    }
    let arg = &args[0];
    if !arg.is_string() {
        return Err(EvalError::TypeMismatch(format!(
            "expected {}, got {}",
            "string", "non-string",
        )));
    }
    Ok(arg)
}

/// `dedent.dedent(text)` — strips common leading whitespace from all lines.
///
/// The logic is handl-authored in `dedent.hnl`; this impl only validates
/// arguments and delegates to the generic [`HandlModule`] loader.
///
/// CRLF (`\r\n`) line endings are normalized to LF (`\n`) before passing the
/// text to the handl evaluator. The handl string-literal parser only
/// unescapes `\\`, `\"`, `\n`, and `\t` (not `\r`), so a raw `\r` would
/// survive as a literal backslash-r inside handl. Normalizing here preserves
/// the original Rust `.lines()` behaviour (which splits on both `\n` and
/// `\r\n`) and the `.join("\n")` output (which always emits LF).
#[derive(Clone, Copy)]
pub struct Dedent;
impl Function for Dedent {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        let input = require_one_string(args)?;
        let text = input.as_str().expect("verified string");
        let normalized = text.replace("\r\n", "\n");
        let func = dedent_module()
            .function("dedent")
            .expect("dedent should exist");
        func.call(
            &[Value::String(normalized)],
            &mut EvalContext::new(),
        )
    }
}

/// The `dedent` module — whitespace stripping utilities.
///
/// Provides `dedent.dedent`. The logic is handl-authored in `dedent.hnl` and
/// loaded via the generic [`HandlModule`](handl_plugin_lib_handl::HandlModule);
/// the [`Dedent`] function impl only validates arguments and delegates to the
/// loader.
#[derive(Clone, Copy)]
pub struct DedentModule;

impl Module for DedentModule {
    fn name(&self) -> &str {
        "dedent"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "dedent" => Some(&Dedent),
            _ => None,
        }
    }
}

/// A `Library` plugin that registers the `dedent` implementation.
///
/// This is a thin wrapper around the generic [`HandlLibraryPlugin`],
/// keeping the same public API (`DedentLibraryPlugin::new()`) as before.
/// The dedent logic is handl-authored in `dedent.hnl` and loaded via the
/// generic loader; no per-library Rust bridge code remains.
pub struct DedentLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl DedentLibraryPlugin {
    /// Create a new `DedentLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new(
                "dedent",
                "lib.dedent",
                "lib-dedent",
                DEDENT_HNL,
                DEDENT_FUNCS,
            )
            .with_dependency(StrModule),
        }
    }
}

impl Default for DedentLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for DedentLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for dedent, with `StrModule` as a
/// dependency.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`DedentLibraryPlugin`].
pub fn dedent_handl_plugin() -> HandlLibraryPlugin {
    HandlLibraryPlugin::new("dedent", "lib.dedent", "lib-dedent", DEDENT_HNL, DEDENT_FUNCS)
        .with_dependency(StrModule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{EvalContext, Function, Module};
    use serde_json::json;

    /// Helper: call `dedent.dedent(text)` via the [`Dedent`] function struct
    /// and compare the result.
    fn check_dedent(input: &str, expected: &str) {
        let result = Dedent
            .call(&[json!(input)], &mut EvalContext::new())
            .unwrap_or_else(|e| panic!("dedent failed: {e}"));
        assert_eq!(result, json!(expected));
    }

    // --- Ported from dedent_tests.rs ---

    #[test]
    fn dedent_simple() {
        check_dedent("    Hello,\n    World!", "Hello,\nWorld!");
    }

    #[test]
    fn dedent_preserves_relative_indent() {
        check_dedent("    Hello,\n      World!", "Hello,\n  World!");
    }

    #[test]
    fn dedent_no_indent() {
        check_dedent("Hello,\nWorld!", "Hello,\nWorld!");
    }

    #[test]
    fn dedent_empty() {
        check_dedent("", "");
    }

    #[test]
    fn dedent_blank_lines_ignored() {
        check_dedent("    Hello,\n\n    World!", "Hello,\n\nWorld!");
    }

    #[test]
    fn dedent_trims_leading_trailing_newlines() {
        check_dedent("\n\n  Hello,\n  World!\n\n", "Hello,\nWorld!");
    }

    #[test]
    fn dedent_tabs() {
        check_dedent("\tHello,\n\tWorld!", "Hello,\nWorld!");
    }

    #[test]
    fn dedent_mixed_indent() {
        check_dedent("  \tHello,\n  \tWorld!", "Hello,\nWorld!");
    }

    #[test]
    fn dedent_only_whitespace_lines() {
        check_dedent("   \n  \n   ", "   \n  \n   ");
    }

    #[test]
    fn dedent_single_line() {
        check_dedent("    Hello!", "Hello!");
    }

    #[test]
    fn dedent_trailing_whitespace_line() {
        check_dedent("  Hello,\n  World!\n  ", "Hello,\nWorld!\n");
    }

    #[test]
    fn dedent_crlf_input() {
        // str.lines splits on \r\n and str.join normalizes to LF
        check_dedent("  Hello,\r\n  World!", "Hello,\nWorld!");
    }

    #[test]
    fn dedent_uneven_indent() {
        check_dedent("    Hello,\n  World!", "  Hello,\nWorld!");
    }

    #[test]
    fn dedent_single_trailing_newline() {
        check_dedent("  Hello,\n  World!\n", "Hello,\nWorld!");
    }

    // --- Ported from module_tests.rs ---

    #[test]
    fn dedent_module_name() {
        assert_eq!(DedentModule.name(), "dedent");
    }

    #[test]
    fn dedent_module_function_lookup() {
        let module = DedentModule;
        assert!(module.function("dedent").is_some());
        assert!(module.function("nonexistent").is_none());
    }

    #[test]
    fn dedent_call_with_string() {
        let result = Dedent
            .call(&[json!("  hello\n  world")], &mut EvalContext::new())
            .unwrap();
        assert_eq!(result.as_str().unwrap(), "hello\nworld");
    }

    #[test]
    fn dedent_call_with_zero_args() {
        let result = Dedent.call(&[], &mut EvalContext::new());
        assert!(matches!(result, Err(EvalError::TypeMismatch(_))));
    }

    #[test]
    fn dedent_call_with_non_string_arg() {
        let result = Dedent.call(&[json!(42.0)], &mut EvalContext::new());
        assert!(matches!(result, Err(EvalError::TypeMismatch(msg)) if msg.contains("string")));
    }

    #[test]
    fn dedent_call_with_extra_args() {
        // Extra args are ignored — first arg is used
        let result = Dedent
            .call(&[json!("  hi"), json!("extra")], &mut EvalContext::new())
            .unwrap();
        assert_eq!(result.as_str().unwrap(), "hi");
    }

    #[test]
    fn dedent_call_with_bool_arg() {
        let result = Dedent.call(&[json!(true)], &mut EvalContext::new());
        assert!(matches!(result, Err(EvalError::TypeMismatch(msg)) if msg.contains("string")));
    }

    #[test]
    fn dedent_call_with_null_arg() {
        let result = Dedent.call(&[json!(())], &mut EvalContext::new());
        assert!(matches!(result, Err(EvalError::TypeMismatch(msg)) if msg.contains("string")));
    }

    #[test]
    fn dedent_call_empty_string() {
        let result = Dedent
            .call(&[json!("")], &mut EvalContext::new())
            .unwrap();
        assert_eq!(result.as_str().unwrap(), "");
    }

    // --- Plugin API tests ---

    #[test]
    fn plugin_id_and_name_are_preserved() {
        let plugin = DedentLibraryPlugin::new();
        assert_eq!(plugin.id(), "lib.dedent");
        assert_eq!(plugin.name(), "lib-dedent");
    }

    #[test]
    fn handl_plugin_constructor_matches() {
        let plugin = dedent_handl_plugin();
        assert_eq!(plugin.id(), "lib.dedent");
        assert_eq!(plugin.name(), "lib-dedent");
    }
}
