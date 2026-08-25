**handl_module_resolver_core > module_caching_resolver**

# Module: module_caching_resolver

## Contents

**Structs**

- [`CachingResolver`](#cachingresolver) - A caching wrapper around any [`ModuleResolver`].

---

## handl_module_resolver_core::module_caching_resolver::CachingResolver

*Struct*

A caching wrapper around any [`ModuleResolver`].

Uses a [`ModuleCache`] to memoize `resolve` results. The cache key
is `path:base` (stringified). Subsequent calls with the same
arguments return a cached clone without re-invoking the inner
resolver.

For resolvers that need resolver-specific caching (e.g., git's
repo-level clone cache), prefer passing a `ModuleCache` directly
to the resolver instead of wrapping with `CachingResolver`.

# Example

```ignore
use handl_module_resolver_core::CachingResolver;
use handl_module_resolver_memory::MemoryModuleCache;
let resolver = CachingResolver::with_cache(
    handl_module_resolver_file::FileResolver::new(),
    MemoryModuleCache::new(),
);
// First call reads from disk; second call hits the cache.
resolver.resolve("base.yml", None);
resolver.resolve("base.yml", None);
```

**Generic Parameters:**
- R
- C

**Fields:**
- `inner: R`
- `cache: C`

**Methods:**

- `fn with_cache(inner: R, cache: C) -> Self` - Wrap a resolver with a specific cache implementation.
- `fn clear_cache(self: &Self)` - Clear the cache.
- `fn cache_len(self: &Self) -> usize` - Returns the number of cached entries.

**Trait Implementations:**

- **ModuleResolver**
  - `fn resolve(self: &Self, path: &str, base: Option<&Path>) -> Option<ResolvedModule>`



