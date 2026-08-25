//! # handl-lib-url-rfc
//!
//! Strict RFC 3986 URI parser for `handl-lib-parse`.

use handl_lib_parse::{ParseError, Parser};
use iri_string::types::UriString;

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

impl Uri {
    /// Parse a string using strict RFC 3986 URI rules.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        handl_lib_parse::parse(input, Rfc)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_http_uri() {
        let u = handl_lib_parse::parse("https://example.com:8080/path?a=1#frag", Rfc).unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.authority(), Some("example.com:8080"));
        assert_eq!(u.path(), "/path");
        assert_eq!(u.query(), Some("a=1"));
        assert_eq!(u.fragment(), Some("frag"));
    }

    #[test]
    fn rejects_non_ascii() {
        assert!(handl_lib_parse::parse("https://example.com/🦀", Rfc).is_err());
    }

    #[test]
    fn rejects_invalid_percent_encoding() {
        assert!(handl_lib_parse::parse("https://example.com/%ZZ", Rfc).is_err());
    }

    #[test]
    fn does_not_normalize_path() {
        let u = handl_lib_parse::parse("https://example.com/../a/../b", Rfc).unwrap();
        assert_eq!(u.path(), "/../a/../b");
    }
}
