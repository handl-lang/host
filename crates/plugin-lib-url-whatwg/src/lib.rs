//! # handl-lib-url-whatwg
//!
//! WHATWG URL Standard parser for `handl-lib-parse`.

use handl_plugin_lib_parse::{DynParser, ParseError, Parser};
use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::NonFunctionValue;
use serde_json::{Value, json};

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

/// The `url_whatwg` runtime module.
pub struct UrlWhatwgModule;

impl Module for UrlWhatwgModule {
    fn name(&self) -> &str {
        "url_whatwg"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "url_whatwg" => Some(&UrlWhatwgName),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct UrlWhatwgName;

impl Function<NonFunctionValue> for UrlWhatwgName {
    fn call(
        &self,
        args: &[NonFunctionValue],
        _ctx: &mut dyn Context<NonFunctionValue>,
    ) -> Result<NonFunctionValue, handl_plugin_vm::Error<NonFunctionValue>> {
        if !args.is_empty() {
            return Err(handl_plugin_vm::Error::ArgumentCount {
                expected: 0,
                got: args.len(),
            });
        }

        Ok(NonFunctionValue::new(json!({
            "type": "parser",
            "name": "url_whatwg",
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_vm_values_json::CanonicalContext;

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
        let mut ctx = CanonicalContext::new();
        let obj = UrlWhatwgName.call(&[], &mut ctx).unwrap().into_inner();
        assert_eq!(obj["type"], "parser");
        assert_eq!(obj["name"], "url_whatwg");
        assert!(obj.get("parse").is_none());
    }
}

pub mod plugin;
pub use plugin::LibraryPlugin;
