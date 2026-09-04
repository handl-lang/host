//! # handl-lib-url-rfc
//!
//! Strict RFC 3986 URI parser for `handl-lib-parse`.
//!
//! This crate has been migrated to the `op`/`handl` declaration form
//! (ADR-0077 Phase 4). The url-rfc interface is declared in
//! `handl-packages/url_rfc/url_rfc.hnl` via `op` declarations, and the
//! Rust parse logic is registered as handls (handlers) in an
//! [`OpRegistry`]. The [`UrlRfcLibraryPlugin`] wraps the generic
//! [`HandlLibraryPlugin`] with the op registry, eliminating the
//! per-library `Module`/`Plugin` trait scaffolding.
//!
//! The [`Rfc`] `Parser`/`DynParser` trait impls and [`UrlRfcModule`]
//! re-export are preserved for backward compatibility with
//! [`plugin-lib-utils`], which uses them in its `ParseModule` and
//! plugin registration.

use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function, Module};
use handl_plugin_lib_handl::{HandlLibraryPlugin, OpRegistry};
use handl_plugin_lib_parse::{DynParser, ParseError, Parser};
use iri_string::types::UriString;
use serde_json::{Value, json};

/// The handl source declaring the url_rfc `op` interface — the single
/// source of truth for the url_rfc API surface. Loaded at compile time
/// from the kit-stdlib data dir.
const URL_RFC_HNL: &str = include_str!("../../../handl-packages/url_rfc/url_rfc.hnl");

/// The op names declared in `url_rfc.hnl`.
const URL_RFC_OPS: &[&str] = &["url_rfc.parse", "url_rfc.url_rfc"];

/// Parser for strict RFC 3986 URIs.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rfc;

/// A parsed RFC 3986 URI. Non-ASCII characters and malformed percent-encoding are rejected.
#[derive(Clone, Debug)]
pub struct Uri {
    inner: UriString,
}

impl Parser<str, Uri> for Rfc {
    fn parse(&self, input: &str) -> Result<Uri, ParseError> {
        let inner = input
            .parse::<UriString>()
            .map_err(|e| ParseError::new(e.to_string()))?;
        Ok(Uri { inner })
    }
}

impl DynParser for Rfc {
    fn parse(&self, input: &str) -> Result<Value, ParseError> {
        let u = <Rfc as Parser<str, Uri>>::parse(self, input)?;
        Ok(json!({
            "scheme": u.scheme(),
            "authority": u.authority(),
            "path": u.path(),
            "query": u.query(),
            "fragment": u.fragment(),
            "href": u.as_str(),
        }))
    }
}

impl Uri {
    /// Parse a string using strict RFC 3986 URI rules.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        handl_plugin_lib_parse::parse(input, Rfc)
    }

    /// Return the URI as a string.
    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    /// Return the scheme, e.g. `"https"`.
    pub fn scheme(&self) -> &str {
        self.inner.scheme_str()
    }

    /// Return the authority, if any, e.g. `"example.com:8080"`.
    pub fn authority(&self) -> Option<&str> {
        self.inner.authority_str()
    }

    /// Return the path.
    pub fn path(&self) -> &str {
        self.inner.path_str()
    }

    /// Return the query string, if any, without the leading `?`.
    pub fn query(&self) -> Option<&str> {
        self.inner.query_str()
    }

    /// Return the fragment, if any, without the leading `#`.
    pub fn fragment(&self) -> Option<&str> {
        self.inner.fragment_str()
    }
}

// ---------------------------------------------------------------------------
// Function impls (the handls — Rust bodies for the `op` declarations)
// ---------------------------------------------------------------------------

/// The Rust handl for `op "url_rfc.parse" : String -> Value`.
///
/// Parses a strict RFC 3986 URI string into a `Value` object with
/// `scheme`, `authority`, `path`, `query`, `fragment`, and `href` fields.
#[derive(Debug, Clone, Copy)]
pub struct UrlRfcParse;

impl Function for UrlRfcParse {
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
            .ok_or_else(|| EvalError::Other("url_rfc.parse expects a string".to_string()))?;
        DynParser::parse(&Rfc, s)
            .map_err(|e| EvalError::Other(format!("url_rfc parse error: {e}")))
    }
}

/// The Rust handl for `op "url_rfc.url_rfc" : () -> Value`.
///
/// Returns the parser token `{"type": "parser", "name": "url_rfc"}`.
#[derive(Debug, Clone, Copy)]
pub struct UrlRfcName;

impl Function for UrlRfcName {
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
            "name": "url_rfc",
        })))
    }
}

// ---------------------------------------------------------------------------
// Op Registry
// ---------------------------------------------------------------------------

/// Build the [`OpRegistry`] for url_rfc, registering the Rust handls under
/// their op names.
///
/// This registry maps:
/// - `"url_rfc.parse"` → [`UrlRfcParse`] (no runtime checker; axioms trusted)
/// - `"url_rfc.url_rfc"` → [`UrlRfcName`] (no runtime checker; axioms trusted)
pub fn url_rfc_op_registry() -> OpRegistry {
    let mut registry = OpRegistry::new();
    registry.register("url_rfc.parse", Arc::new(UrlRfcParse));
    registry.register("url_rfc.url_rfc", Arc::new(UrlRfcName));
    registry
}

// ---------------------------------------------------------------------------
// UrlRfcModule (backward compat — delegates to op functions)
// ---------------------------------------------------------------------------

/// The `url_rfc` module — a thin wrapper that delegates to the op-backed
/// functions.
///
/// This is preserved for backward compatibility with downstream crates
/// (e.g. [`plugin-lib-utils`]) that register `UrlRfcModule` directly.
#[derive(Debug, Clone, Copy, Default)]
pub struct UrlRfcModule;

impl Module for UrlRfcModule {
    fn name(&self) -> &str {
        "url_rfc"
    }

    fn function(&self, name: &str) -> Option<&dyn Function> {
        match name {
            "url_rfc" => Some(&UrlRfcName),
            "parse" => Some(&UrlRfcParse),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// UrlRfcLibraryPlugin (wraps HandlLibraryPlugin with op registry)
// ---------------------------------------------------------------------------

/// A `Library` plugin that registers the `url_rfc` implementation via the
/// generic [`HandlLibraryPlugin`] with an [`OpRegistry`].
///
/// This is the Phase 4 replacement for the old hand-rolled `LibraryPlugin`.
/// The url_rfc interface is declared in `url_rfc.hnl` via `op`/`handl`
/// declarations; the Rust parse handls are registered in the op registry.
/// The generic loader handles module registration.
pub struct UrlRfcLibraryPlugin {
    inner: HandlLibraryPlugin,
}

impl UrlRfcLibraryPlugin {
    /// Create a new `UrlRfcLibraryPlugin`.
    pub fn new() -> Self {
        let registry = Arc::new(url_rfc_op_registry());
        Self {
            inner: HandlLibraryPlugin::new_with_caps(
                "url_rfc",
                "lib.url-rfc",
                "lib-url-rfc",
                URL_RFC_HNL,
                &[],
            )
            .with_ops(URL_RFC_OPS, registry),
        }
    }
}

impl Default for UrlRfcLibraryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl handl_platform::Plugin for UrlRfcLibraryPlugin {
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

/// Build a [`HandlLibraryPlugin`] for url_rfc with the op registry.
///
/// This is provided for consumers that prefer to use the generic plugin
/// directly instead of [`UrlRfcLibraryPlugin`].
pub fn url_rfc_handl_plugin() -> HandlLibraryPlugin {
    let registry = Arc::new(url_rfc_op_registry());
    HandlLibraryPlugin::new_with_caps(
        "url_rfc",
        "lib.url-rfc",
        "lib-url-rfc",
        URL_RFC_HNL,
        &[],
    )
    .with_ops(URL_RFC_OPS, registry)
}

// ---------------------------------------------------------------------------
// Legacy LibraryPlugin (re-exported for backward compat)
// ---------------------------------------------------------------------------

/// Legacy `LibraryPlugin` — delegates to [`UrlRfcLibraryPlugin`].
///
/// This preserves the old `plugin::LibraryPlugin` API for downstream crates
/// that import `handl_plugin_lib_url_rfc::LibraryPlugin`.
pub use UrlRfcLibraryPlugin as LibraryPlugin;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use handl_platform::{EvalContext, Plugin};

    #[test]
    fn parses_http_uri() {
        let u =
            handl_plugin_lib_parse::parse("https://example.com:8080/path?a=1#frag", Rfc).unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.authority(), Some("example.com:8080"));
        assert_eq!(u.path(), "/path");
        assert_eq!(u.query(), Some("a=1"));
        assert_eq!(u.fragment(), Some("frag"));
    }

    #[test]
    fn rejects_non_ascii() {
        assert!(handl_plugin_lib_parse::parse("https://example.com/🦀", Rfc).is_err());
    }

    #[test]
    fn rejects_invalid_percent_encoding() {
        assert!(handl_plugin_lib_parse::parse("https://example.com/%ZZ", Rfc).is_err());
    }

    #[test]
    fn does_not_normalize_path() {
        let u = handl_plugin_lib_parse::parse("https://example.com/../a/../b", Rfc).unwrap();
        assert_eq!(u.path(), "/../a/../b");
    }

    #[test]
    fn parses_to_value() {
        let v = DynParser::parse(&Rfc, "https://example.com:8080/path?a=1#frag").unwrap();
        assert_eq!(v["scheme"], "https");
        assert_eq!(v["authority"], "example.com:8080");
        assert_eq!(v["path"], "/path");
    }

    #[test]
    fn url_rfc_object_is_parser_token() {
        let mut ctx = EvalContext::new();
        let obj = UrlRfcName.call(&[], &mut ctx).unwrap();
        assert_eq!(obj["type"], "parser");
        assert_eq!(obj["name"], "url_rfc");
        assert!(obj.get("parse").is_none());
    }

    #[test]
    fn op_registry_registers_both_ops() {
        let registry = url_rfc_op_registry();
        assert!(registry.contains("url_rfc.parse"));
        assert!(registry.contains("url_rfc.url_rfc"));
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn url_rfc_parse_op_works() {
        let registry = Arc::new(url_rfc_op_registry());
        let op_func = handl_plugin_lib_handl::OpFunction::new("url_rfc.parse", registry);
        let mut ctx = EvalContext::new();
        let result = op_func
            .call(&[json!("https://example.com:8080/path?a=1#frag")], &mut ctx)
            .unwrap();
        assert_eq!(result["scheme"], "https");
        assert_eq!(result["authority"], "example.com:8080");
        assert_eq!(result["path"], "/path");
    }

    #[test]
    fn plugin_id_and_name() {
        let plugin = UrlRfcLibraryPlugin::new();
        assert_eq!(plugin.id(), "lib.url-rfc");
        assert_eq!(plugin.name(), "lib-url-rfc");
    }
}
