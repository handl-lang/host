**handl_module_resolver_core > module_cache**

# Module: module_cache

## Contents

**Traits**

- [`ModuleCache`](#modulecache) - Cache for resolved modules, keyed by a resolver-specific string.

---

## handl_module_resolver_core::module_cache::ModuleCache

*Trait*

Cache for resolved modules, keyed by a resolver-specific string.

Each resolver decides what to cache and how to key it:
- **FileResolver** — key = canonical path
- **HttpResolver** — key = full URL
- **GitResolver** — key = `repo_url@ref/path` (module-level),
  plus an internal repo cache for clone-level dedup

Implementations must be thread-safe (`Send`) since resolvers may be
shared across passes (`ResolvePass` + `TypecheckPass`).

See `handl-module-resolver-memory` for the default in-memory implementation
and `handl-module-resolver-noop` for a no-op implementation.

**Methods:**

- `get`
- `insert`
- `clear`
- `len`
- `is_empty`



