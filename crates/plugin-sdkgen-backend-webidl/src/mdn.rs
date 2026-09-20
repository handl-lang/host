//! MDN web-docs doc source for the WebIDL backend.

use handl_plugin_lib_http_client::{HttpClient, HttpMethod, HttpRequest};
use handl_plugin_lib_http_client_std::StdHttpClient;
use handl_plugin_sdkgen::DocSource;
use std::cell::RefCell;
use std::collections::HashMap;

/// Best-effort doc lookup via the MDN Web Docs API. Queries
/// `https://developer.mozilla.org/api/v1/en-US/docs/Web/API/<key>` and
/// extracts the `doc.summary` field. Results are cached per key. Returns
/// `None` on network errors, non-200 responses, or missing content.
pub struct MdnDocSource {
    client: StdHttpClient,
    cache: RefCell<HashMap<String, Option<String>>>,
}

impl MdnDocSource {
    pub fn new() -> Self {
        Self {
            client: StdHttpClient,
            cache: RefCell::new(HashMap::new()),
        }
    }

    fn lookup_uncached(&self, key: &str) -> Option<String> {
        let url = format!(
            "https://developer.mozilla.org/api/v1/en-US/docs/Web/API/{}",
            key
        );
        let req = HttpRequest::new(HttpMethod::Get, &url).with_timeout(10_000);
        let resp = self.client.request(&req).ok()?;
        if resp.status != 200 {
            return None;
        }
        let json: serde_json::Value = serde_json::from_str(&resp.body).ok()?;
        // The MDN API returns `doc.summary` as the short description.
        json["doc"]["summary"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }
}

impl Default for MdnDocSource {
    fn default() -> Self {
        Self::new()
    }
}

impl DocSource for MdnDocSource {
    fn lookup(&self, key: &str) -> Option<String> {
        if let Some(cached) = self.cache.borrow().get(key) {
            return cached.clone();
        }
        let result = self.lookup_uncached(key);
        self.cache
            .borrow_mut()
            .insert(key.to_string(), result.clone());
        result
    }
}
