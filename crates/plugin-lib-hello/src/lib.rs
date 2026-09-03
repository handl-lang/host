//! # handl-lib-hello
//!
//! Minimal example standard library for the handl runtime.
//!
//! Provides the `hello` library as a `Library` plugin.
//!
//! The greet *logic* is handl-authored in `kit-stdlib/data/hello/greet.hnl`;
//! this crate loads and bridges that definition via the generic
//! [`handl_plugin_lib_handl`] loader. The per-library Rust bridge code has
//! been eliminated: [`HandlModule`] / [`HandlLibraryPlugin`] evaluate the
//! `.hnl` source directly, exposing `hello.greet` as a callable
//! [`Function`](handl_platform::Function).

/// The handl source defining `greet` — the single source of truth for the
/// greeting logic. Loaded at compile time from the kit-stdlib data dir.
///
/// `greet.hnl` is a bare arrow (`name ~> "Hello, " + name + "!"`), not a
/// record of arrows. The generic [`HandlModule`] evaluates
/// `(source).func_name(args)`, which requires a record, so the bare arrow is
/// wrapped into a single-field record `{ greet: (<greet.hnl>) }` before being
/// handed to the loader. Comment lines (`--- ...`) are stripped by the
/// loader's `strip_comment_lines`.
const GREET_HNL_RECORD: &str = concat!(
    "{ greet: (\n",
    include_str!("../../../kit-stdlib/data/hello/greet.hnl"),
    "\n) }"
);

/// The function (record field) names exposed by the hello module.
const HELLO_FUNCS: &[&str] = &["greet"];

use handl_platform::Plugin;
use handl_plugin_lib_handl::HandlLibraryPlugin;
#[cfg(test)]
use handl_plugin_lib_handl::HandlModule;
#[cfg(test)]
use std::sync::OnceLock;

/// Build the [`HandlModule`] for hello.
///
/// `greet` uses only the native `+` string-concatenation op, so no
/// dependency modules are required.
#[cfg(test)]
fn build_hello_module() -> HandlModule {
    HandlModule::new("hello", GREET_HNL_RECORD, HELLO_FUNCS)
}

/// A thread-safe lazy-initialized [`HandlModule`] for hello.
///
/// The module is built once and shared across all calls. The per-thread
/// `Runtime` caching is handled internally by [`HandlModule`].
#[cfg(test)]
static HELLO_MODULE: OnceLock<HandlModule> = OnceLock::new();

/// Get the shared [`HandlModule`] for hello, initializing it on first call.
#[cfg(test)]
fn hello_module() -> &'static HandlModule {
    HELLO_MODULE.get_or_init(build_hello_module)
}

/// A `Library` plugin that registers the `hello` implementation.
///
/// This is a thin wrapper around the generic [`HandlLibraryPlugin`],
/// keeping the same public API (`HelloLibraryPlugin::new()`) as before.
/// The greet logic is handl-authored in `greet.hnl` and loaded via the
/// generic loader; no per-library Rust bridge code remains.
pub struct HelloLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl HelloLibraryPlugin {
    /// Create a new `HelloLibraryPlugin`.
    pub fn new() -> Self {
        Self {
            inner: HandlLibraryPlugin::new(
                "hello",
                "lib.hello",
                "lib-hello",
                GREET_HNL_RECORD,
                HELLO_FUNCS,
            ),
        }
    }
}

impl Default for HelloLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for HelloLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for hello.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`HelloLibraryPlugin`].
pub fn hello_handl_plugin() -> HandlLibraryPlugin {
    HandlLibraryPlugin::new("hello", "lib.hello", "lib-hello", GREET_HNL_RECORD, HELLO_FUNCS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{EvalContext, Module};
    use serde_json::json;

    /// Helper: call the `greet` function on a JSON string arg and compare.
    fn check_greet(input: &str, expected: &str) {
        let func = hello_module().function("greet").expect("greet should exist");
        let result = func
            .call(&[json!(input)], &mut EvalContext::new())
            .unwrap_or_else(|e| panic!("greet failed: {e}"));
        assert_eq!(result, json!(expected));
    }

    #[test]
    fn greet_returns_canonical_greeting() {
        // The greet logic is handl-authored in greet.hnl; this confirms the
        // generic HandlModule loader loads + evaluates it.
        check_greet("world", "Hello, world!");
    }

    #[test]
    fn greet_escapes_special_chars() {
        check_greet("a\"b\\c", "Hello, a\"b\\c!");
    }

    #[test]
    fn greet_with_empty_name() {
        check_greet("", "Hello, !");
    }

    #[test]
    fn plugin_id_and_name_are_preserved() {
        let plugin = HelloLibraryPlugin::new();
        assert_eq!(plugin.id(), "lib.hello");
        assert_eq!(plugin.name(), "lib-hello");
    }

    #[test]
    fn handl_plugin_constructor_matches() {
        let plugin = hello_handl_plugin();
        assert_eq!(plugin.id(), "lib.hello");
        assert_eq!(plugin.name(), "lib-hello");
    }
}
