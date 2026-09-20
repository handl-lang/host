//! MDN web-docs doc source for the WebIDL backend.

use handl_plugin_lib_http_client::{HttpClient, HttpMethod, HttpRequest};
use handl_plugin_lib_http_client_std::StdHttpClient;
use handl_plugin_sdkgen::DocSource;
use std::cell::RefCell;
use std::collections::HashMap;

/// Best-effort doc lookup via MDN. Fetches
/// `https://developer.mozilla.org/en-US/docs/Web/API/<path>/index.json`
/// and extracts the `doc.summary` field. Results are cached per key.
/// Returns `None` on network errors, non-200 responses, or missing
/// content.
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
        for path in mdn_paths(key) {
            let url = format!("https://developer.mozilla.org/en-US/docs/Web/API/{path}/index.json");
            let req = HttpRequest::new(HttpMethod::Get, &url).with_timeout(10_000);
            let Ok(resp) = self.client.request(&req) else {
                continue;
            };
            if resp.status != 200 {
                continue;
            }
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body) else {
                continue;
            };
            // The MDN index.json holds `doc.summary` as the short description.
            if let Some(doc) = json["doc"]["summary"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
            {
                return Some(doc);
            }
        }
        None
    }
}

/// Candidate MDN page paths for a doc key, tried in order. Interface
/// keys (`URL`) map to `URL`; member keys (`URL.href`) map to
/// `URL/href`, with the `_static` suffix as fallback — MDN puts static
/// members at `member_static` and there is no way to tell static from
/// instance in the key. Constructors arrive as `URL.URL`, which resolves
/// through the plain member path.
fn mdn_paths(key: &str) -> Vec<String> {
    match key.split_once('.') {
        None => vec![key.to_string()],
        Some((interface, member)) => vec![
            format!("{interface}/{member}"),
            format!("{interface}/{member}_static"),
        ],
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

#[cfg(test)]
mod tests {
    use super::mdn_paths;

    #[test]
    fn paths_map_member_keys_to_slugs() {
        assert_eq!(mdn_paths("URL"), vec!["URL".to_string()]);
        assert_eq!(
            mdn_paths("URL.href"),
            vec!["URL/href".to_string(), "URL/href_static".to_string()]
        );
        // Constructor keys resolve through the plain member path.
        assert_eq!(
            mdn_paths("URL.URL"),
            vec!["URL/URL".to_string(), "URL/URL_static".to_string()]
        );
    }
}
