//! # handl-lib-url-whatwg
//!
//! WHATWG URL Standard parsing for the handl runtime.
//!
//! Wraps the `url` crate to provide a focused, runtime-friendly API.

use std::fmt;

/// A parsed URL, aligned with the WHATWG URL Standard.
#[derive(Clone, Debug)]
pub struct WebUrl {
    inner: url::Url,
}

/// Error returned when a string cannot be parsed as a WHATWG URL.
#[derive(Debug)]
pub struct WebUrlError {
    message: String,
}

impl fmt::Display for WebUrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for WebUrlError {}

impl WebUrl {
    /// Parse a string using the WHATWG URL Standard.
    pub fn parse(s: &str) -> Result<Self, WebUrlError> {
        let inner = url::Url::parse(s).map_err(|e| WebUrlError {
            message: e.to_string(),
        })?;
        Ok(Self { inner })
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
        let u = WebUrl::parse("https://example.com:8080/path?a=1#frag").unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.host(), Some("example.com"));
        assert_eq!(u.port(), Some(8080));
        assert_eq!(u.path(), "/path");
        assert_eq!(u.query(), Some("a=1"));
        assert_eq!(u.fragment(), Some("frag"));
    }

    #[test]
    fn whatwg_normalizes_invalid() {
        let u = WebUrl::parse("HTtpS://EXAMPLE.com/../a/../b").unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.host(), Some("example.com"));
        assert_eq!(u.path(), "/b");
    }

    #[test]
    fn rejects_garbage() {
        assert!(WebUrl::parse("not a url").is_err());
    }
}
