**handl_module_resolver_file > file_resolver**

# Module: file_resolver

## Contents

**Structs**

- [`FileResolver`](#fileresolver) - Filesystem-based module resolver.

---

## handl_module_resolver_file::file_resolver::FileResolver

*Struct*

Filesystem-based module resolver.

Reads files from disk, parses by extension, extracts fields.
This is the default resolver for CLI usage.

By default, no module cache is used. Pass a [`ModuleCache`] to
[`FileResolver::with_cache`] to memoize resolved modules by
canonical path.

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



