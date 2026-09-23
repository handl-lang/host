//! MDN web-docs doc source for the WebIDL backend.

use handl_plugin_sdkgen::{http_get, DocSource};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

/// Best-effort doc lookup via MDN. Fetches
/// `https://developer.mozilla.org/en-US/docs/Web/API/<path>/index.json`
/// and extracts the `doc.summary` field. Results are cached per key.
/// Returns `None` on network errors, non-200 responses, or missing
/// content.
pub struct MdnDocSource {
    cache: RefCell<HashMap<String, Option<String>>>,
    cache_path: Option<PathBuf>,
}

impl MdnDocSource {
    pub fn new() -> Self {
        Self {
            cache: RefCell::new(HashMap::new()),
            cache_path: None,
        }
    }

    /// Like [`MdnDocSource::new`] but persists the lookup cache as a JSON
    /// map at `path` — loaded on construction, written back on drop.
    /// Repeat sdkgen runs then skip the network entirely for known keys
    /// (misses are cached too, so 404s don't refetch).
    pub fn with_disk_cache(path: PathBuf) -> Self {
        let cache = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            cache: RefCell::new(cache),
            cache_path: Some(path),
        }
    }

    fn lookup_uncached(&self, key: &str) -> Option<String> {
        for path in mdn_paths(key) {
            let url = format!("https://developer.mozilla.org/en-US/docs/Web/API/{path}/index.json");
            let Ok(body) = http_get(&url, 10_000) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) else {
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
/// instance in the key. Constructors arrive as `URL.constructor` but
/// MDN hosts them at `URL/URL`.
fn mdn_paths(key: &str) -> Vec<String> {
    match key.split_once('.') {
        None => vec![key.to_string()],
        Some((interface, "constructor")) => vec![format!("{interface}/{interface}")],
        Some((interface, member)) => vec![
            format!("{interface}/{member}"),
            format!("{interface}/{member}_static"),
        ],
    }
}

impl Drop for MdnDocSource {
    fn drop(&mut self) {
        if let Some(path) = &self.cache_path {
            if let Ok(json) = serde_json::to_string(&*self.cache.borrow()) {
                let _ = std::fs::write(path, json);
            }
        }
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
        // Constructor keys resolve to the `Interface/Interface` page.
        assert_eq!(mdn_paths("URL.constructor"), vec!["URL/URL".to_string()]);
    }
}
