//! # handl-lib-url-whatwg
//!
//! WHATWG URL Standard parser for `handl-lib-parse`.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The url-whatwg interface is declared in
//! `kit-stdlib/data/url_whatwg/url_whatwg.hnl` via `op` declarations, and
//! the Rust parse logic is registered as handls (handlers) in an
//! [`OpRegistry`]. The [`UrlWhatwgLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the
//! per-library `Module`/`Plugin` trait scaffolding.
//!
//! The [`Whatwg`] `Parser`/`DynParser` trait impls and [`UrlWhatwgModule`]
//! re-export are preserved for backward compatibility with
//! [`plugin-lib-utils`], which uses them in its `ParseModule` and
//! plugin registration.

use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};
use handl_plugin_lib_parse::{DynParser, ParseError, Parser};
use serde_json::{Value, json};

/// The handl source declaring the url_whatwg `op` interface — the single
/// source of truth for the url_whatwg API surface. Loaded at compile time
/// from the kit-stdlib data dir.
const URL_WHATWG_HNL: &str = include_str!("../../../kit-stdlib/data/url_whatwg/url_whatwg.hnl");

/// The op names declared in `url_whatwg.hnl`.
const URL_WHATWG_OPS: &[&str] = &["url_whatwg.parse", "url_whatwg.url_whatwg"];

/// Parser for the WHATWG URL Standard.
#[derive(Debug, Clone, Copy, Default)]
pub struct Whatwg;

/// A parsed URL, aligned with the WHATWG URL Standard.
#[derive(Clone, Debug)]
pub struct WebUrl {
    inner: url::Url,
}

impl Parser<str, WebUrl> for Whatwg {
    fn parse(&self, input: &str) -> Result<WebUrl, ParseError> {
        let inner = url::Url::parse(input).map_err(|e| ParseError::new(e.to_string()))?;
        Ok(WebUrl { inner })
    }
}

impl DynParser for Whatwg {
    fn parse(&self, input: &str) -> Result<Value, ParseError> {
        let u = <Whatwg as Parser<str, WebUrl>>::parse(self, input)?;
        Ok(json!({
            "scheme": u.scheme(),
            "host": u.host(),
            "port": u.port(),
            "path": u.path(),
            "query": u.query(),
            "fragment": u.fragment(),
            "href": u.as_str(),
        }))
    }
}

impl WebUrl {
    /// Parse a string using the WHATWG URL Standard.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        handl_plugin_lib_parse::parse(input, Whatwg)
    }

    /// Return the normalized URL as a string.
    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    /// Return the scheme, e.g. `"https"`.
    pub fn scheme(&self) -> &str {
        self.inner.scheme()
    }

    /// Return the host, if any.
    pub fn host(&self) -> Option<&str> {
        self.inner.host_str()
    }

    /// Return the explicit port, if any.
    pub fn port(&self) -> Option<u16> {
        self.inner.port()
    }

    /// Return the path.
    pub fn path(&self) -> &str {
        self.inner.path()
    }

    /// Return the query string, if any, without the leading `?`.
    pub fn query(&self) -> Option<&str> {
        self.inner.query()
    }

    /// Return the fragment, if any, without the leading `#`.
    pub fn fragment(&self) -> Option<&str> {
        self.inner.fragment()
    }
}

// ---------------------------------------------------------------------------
// Function impls (the handls — Rust bodies for the `op` declarations)
// ---------------------------------------------------------------------------

/// The Rust handl for `op "url_whatwg.parse" : String -> Value`.
///
/// Parses a WHATWG URL Standard string into a `Value` object with
/// `scheme`, `host`, `port`, `path`, `query`, `fragment`, and `href` fields.
#[derive(Debug, Clone, Copy)]
pub struct UrlWhatwgParse;

impl Function for UrlWhatwgParse {
    fn call(&self, args: &[Value], _ctx: &mut EvalContext) -> Result<Value, EvalError> {
        if args.len() != 1 {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                1,
                args.len()
            )));
        }
        let s = args[0]
            .as_str()
            .ok_or_else(|| EvalError::Other("url_whatwg.parse expects a string".to_string()))?;
        DynParser::parse(&Whatwg, s)
            .map_err(|e| EvalError::Other(format!("url_whatwg parse error: {e}")))
    }
}

/// The Rust handl for `op "url_whatwg.url_whatwg" : () -> Value`.
///
/// Returns the parser token `{"type": "parser", "name": "url_whatwg"}`.
#[derive(Debug, Clone, Copy)]
pub struct UrlWhatwgName;

impl Function for UrlWhatwgName {
    fn call(
        &self,
        args: &[serde_json::Value],
        _ctx: &mut EvalContext,
    ) -> Result<serde_json::Value, EvalError> {
        if !args.is_empty() {
            return Err(EvalError::TypeMismatch(format!(
                "expected {} args, got {}",
                0,
                args.len(),
            )));
        }

        Ok(serde_json::Value::from(json!({
            "type": "parser",
            "name": "url_whatwg",
        })))
    }
}

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for url_whatwg, registering the Rust handls
/// under their op names.
///
/// This registry maps:
/// - `"url_whatwg.parse"` → [`UrlWhatwgParse`] (no runtime checker; axioms trusted)
/// - `"url_whatwg.url_whatwg"` → [`UrlWhatwgName`] (no runtime checker; axioms trusted)
pub fn url_whatwg_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("url_whatwg.parse", Arc::new(UrlWhatwgParse));
    registry.register("url_whatwg.url_whatwg", Arc::new(UrlWhatwgName));
    registry
}

// ---------------------------------------------------------------------------
// UrlWhatwgModule (backward compat — delegates to op functions)
// ---------------------------------------------------------------------------

/// The `url_whatwg` module — a thin wrapper that delegates to the op-backed
/// functions.
///
/// This is preserved for backward compatibility with downstream crates
/// (e.g. [`plugin-lib-utils`]) that register `UrlWhatwgModule` directly.
#[derive(Debug, Clone, Copy, Default)]
pub struct UrlWhatwgModule;

impl Module for UrlWhatwgModule {
    fn name(&self) -> &str {
        "url_whatwg"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "url_whatwg" => Some(&UrlWhatwgName),
            "parse" => Some(&UrlWhatwgParse),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// UrlWhatwgLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `url_whatwg` implementation via
/// the generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The url_whatwg interface is declared in `url_whatwg.hnl` via
/// `op`/`handl` declarations; the Rust parse handls are registered in the
/// op registry. The generic loader handles module registration.
pub struct UrlWhatwgLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl UrlWhatwgLibraryPlugin {
    /// Create a new `UrlWhatwgLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(url_whatwg_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "url_whatwg",
                "lib.url-whatwg",
                "lib-url-whatwg",
                URL_WHATWG_HNL,
                &[],
            )
            .with_ops(URL_WHATWG_OPS, registry),
        }
    }
}

impl Default for UrlWhatwgLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for UrlWhatwgLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for url_whatwg with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`UrlWhatwgLibraryPlugin`].
pub fn url_whatwg_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(url_whatwg_op_registry());
    HandlLibraryPlugin::new_with_caps(
        "url_whatwg",
        "lib.url-whatwg",
        "lib-url-whatwg",
        URL_WHATWG_HNL,
        &[],
    )
    .with_ops(URL_WHATWG_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`UrlWhatwgLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_url_whatwg::LibraryPlugin`.
pub use UrlWhatwgLibraryPlugin as LibraryPlugin;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{EvalContext, Plugin};

    #[test]
    fn parses_http_url() {
        let u = handl_plugin_lib_parse::parse("https://example.com:8080/path?a=1#frag", Whatwg)
            .unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.host(), Some("example.com"));
        assert_eq!(u.port(), Some(8080));
        assert_eq!(u.path(), "/path");
        assert_eq!(u.query(), Some("a=1"));
        assert_eq!(u.fragment(), Some("frag"));
    }

    #[test]
    fn whatwg_normalizes_invalid() {
        let u = handl_plugin_lib_parse::parse("HTtpS://EXAMPLE.com/../a/../b", Whatwg).unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.host(), Some("example.com"));
        assert_eq!(u.path(), "/b");
    }

    #[test]
    fn rejects_garbage() {
        assert!(handl_plugin_lib_parse::parse("not a url", Whatwg).is_err());
    }

    #[test]
    fn parses_to_value() {
        let v = DynParser::parse(&Whatwg, "https://example.com:8080/path?a=1#frag").unwrap();
        assert_eq!(v["scheme"], "https");
        assert_eq!(v["host"], "example.com");
        assert_eq!(v["path"], "/path");
    }

    #[test]
    fn url_whatwg_object_is_parser_token() {
        let mut ctx = EvalContext::new();
        let obj = UrlWhatwgName.call(&[], &mut ctx).unwrap();
        assert_eq!(obj["type"], "parser");
        assert_eq!(obj["name"], "url_whatwg");
        assert!(obj.get("parse").is_none());
    }

    #[test]
    fn op_registry_registers_both_ops() {
        let registry = url_whatwg_op_registry();
        assert!(registry.contains("url_whatwg.parse"));
        assert!(registry.contains("url_whatwg.url_whatwg"));
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn url_whatwg_parse_op_works() {
        let registry = Arc::new(url_whatwg_op_registry());
        let op_func = handl_plugin_lib_handl::OpFunction::new("url_whatwg.parse", registry);
        let mut ctx = EvalContext::new();
        let result = op_func
            .call(&[json!("https://example.com:8080/path?a=1#frag")], &mut ctx)
            .unwrap();
        assert_eq!(result["scheme"], "https");
        assert_eq!(result["host"], "example.com");
        assert_eq!(result["path"], "/path");
    }

    #[test]
    fn plugin_id_and_name() {
        let plugin = UrlWhatwgLibraryPlugin::new();
        assert_eq!(plugin.id(), "lib.url-whatwg");
        assert_eq!(plugin.name(), "lib-url-whatwg");
    }
}
