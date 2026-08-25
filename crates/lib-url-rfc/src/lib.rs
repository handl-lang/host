//! # handl-lib-url-rfc
//!
//! Strict RFC 3986 URI parsing for the handl runtime.
//!
//! Uses `iri-string` to enforce URI-only (ASCII) syntax.

use std::fmt;

use iri_string::types::UriString;

/// A parsed RFC 3986 URI. Non-ASCII characters and malformed percent-encoding are rejected.
#[derive(Clone, Debug)]
pub struct Uri {
    inner: UriString,
}

/// Error returned when a string cannot be parsed as a strict RFC 3986 URI.
#[derive(Debug)]
pub struct UriError {
    message: String,
}

impl fmt::Display for UriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for UriError {}

impl Uri {
    /// Parse a string using strict RFC 3986 URI rules.
    pub fn parse(s: &str) -> Result<Self, UriError> {
        let inner = s.parse::<UriString>().map_err(|e| UriError {
            message: e.to_string(),
        })?;
        Ok(Self { inner })
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
        let u = Uri::parse("https://example.com:8080/path?a=1#frag").unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.authority(), Some("example.com:8080"));
        assert_eq!(u.path(), "/path");
        assert_eq!(u.query(), Some("a=1"));
        assert_eq!(u.fragment(), Some("frag"));
    }

    #[test]
    fn rejects_non_ascii() {
        assert!(Uri::parse("https://example.com/🦀").is_err());
    }

    #[test]
    fn rejects_invalid_percent_encoding() {
        assert!(Uri::parse("https://example.com/%ZZ").is_err());
    }

    #[test]
    fn does_not_normalize_path() {
        let u = Uri::parse("https://example.com/../a/../b").unwrap();
        assert_eq!(u.path(), "/../a/../b");
    }
}
