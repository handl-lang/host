use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;
use handl_plugin_lib_parse::{DynParser, ParseError, Parser};
use iri_string::types::UriString;
use serde_json::{Value, json};

/// The handl source declaring the url_rfc `op` interface — the single
/// source of truth for the url_rfc API surface. Loaded at compile time
/// from the kit-stdlib data dir.

/// The op names declared in `url_rfc.hnl`.
pub const URL_RFC_OPS: &[&str] = &["url_rfc.parse", "url_rfc.url_rfc"];

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
        DynParser::parse(&Rfc, s).map_err(|e| EvalError::Other(format!("url_rfc parse error: {e}")))
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
