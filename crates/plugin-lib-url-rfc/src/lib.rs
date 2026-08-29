//! # handl-plugin-lib-url-rfc
//!
//! Strict RFC 3986 URI parser for `handl-plugin-lib-parse`.

use handl_plugin_lib_parse::{DynParser, ParseError, Parser};
use handl_plugin_vm::{Context, Function};
use handl_plugin_vm_common::Module;
use handl_plugin_vm_values_json::NonFunctionValue;
use iri_string::types::UriString;
use serde_json::{Value, json};

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

/// The `url_rfc` runtime module.
pub struct UrlRfcModule;

impl Module for UrlRfcModule {
    fn name(&self) -> &str {
        "url_rfc"
    }

    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "url_rfc" => Some(&UrlRfcName),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct UrlRfcName;

impl Function<NonFunctionValue> for UrlRfcName {
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
            "name": "url_rfc",
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use handl_plugin_vm_values_json::CanonicalContext;

    #[test]
    fn parses_http_uri() {
        let u = handl_plugin_lib_parse::parse("https://example.com:8080/path?a=1#frag", Rfc).unwrap();
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
        let mut ctx = CanonicalContext::new();
        let obj = UrlRfcName.call(&[], &mut ctx).unwrap().into_inner();
        assert_eq!(obj["type"], "parser");
        assert_eq!(obj["name"], "url_rfc");
        assert!(obj.get("parse").is_none());
    }
}
