use std::sync::Arc;

use handl_platform::{EvalContext, EvalError, Function};
use handl_plugin_lib_handl::OpRegistry;
use handl_plugin_lib_parse::{DynParser, ParseError, Parser};
use serde_json::{Value, json};

/// The handl source declaring the url_whatwg `op` interface — the single
/// source of truth for the url_whatwg API surface. Loaded at compile time
/// from the kit-stdlib data dir.

/// The op names declared in `url_whatwg.hnl`.
pub const URL_WHATWG_OPS: &[&str] = &["url_whatwg.parse", "url_whatwg.url_whatwg"];

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
