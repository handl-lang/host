//! # handl-lib-url-whatwg
//!
//! WHATWG URL Standard parser for `handl-lib-parse`.

use handl_lib_parse::{ParseError, Parser};

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

impl WebUrl {
    /// Parse a string using the WHATWG URL Standard.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        handl_lib_parse::parse(input, Whatwg)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_http_url() {
        let u = handl_lib_parse::parse("https://example.com:8080/path?a=1#frag", Whatwg).unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.host(), Some("example.com"));
        assert_eq!(u.port(), Some(8080));
        assert_eq!(u.path(), "/path");
        assert_eq!(u.query(), Some("a=1"));
        assert_eq!(u.fragment(), Some("frag"));
    }

    #[test]
    fn whatwg_normalizes_invalid() {
        let u = handl_lib_parse::parse("HTtpS://EXAMPLE.com/../a/../b", Whatwg).unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.host(), Some("example.com"));
        assert_eq!(u.path(), "/b");
    }

    #[test]
    fn rejects_garbage() {
        assert!(handl_lib_parse::parse("not a url", Whatwg).is_err());
    }
}
