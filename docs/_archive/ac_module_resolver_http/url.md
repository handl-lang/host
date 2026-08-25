**handl_module_resolver_http > url**

# Module: url

## Contents

**Functions**

- [`resolve_url`](#resolve_url) - Resolve a path to a full URL.

---

## handl_module_resolver_http::url::resolve_url

*Function*

Resolve a path to a full URL.

If `path` is already a full URL, use it directly.
If `base` is provided and is a URL, resolve `path` relative to it.

```rust
fn resolve_url(path: &str, base: Option<&std::path::Path>) -> Option<url::Url>
```



