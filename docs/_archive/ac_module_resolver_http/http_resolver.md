**handl_module_resolver_http > http_resolver**

# Module: http_resolver

## Contents

**Structs**

- [`HttpResolver`](#httpresolver) - HTTP-based module resolver.

---

## handl_module_resolver_http::http_resolver::HttpResolver

*Struct*

HTTP-based module resolver.

Fetches documents via HTTP GET. Module paths must be valid URLs.
Relative paths are resolved against the base URL when provided.

By default, no module cache is used. Pass a [`ModuleCache`] to
[`HttpResolver::with_cache`] to memoize resolved modules by URL.

**Generic Parameters:**
- C

**Fields:**
- `cache: C`

**Methods:**

- `fn with_cache(cache: C) -> Self` - Create a resolver with a specific cache implementation.
- `fn new() -> Self` - Create a resolver with no caching.

**Trait Implementations:**

- **ModuleResolver**
  - `fn resolve(self: &Self, path: &str, base: Option<&Path>) -> Option<ResolvedModule>`
- **Default**
  - `fn default() -> Self`



