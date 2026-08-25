**handl_module_resolver_core > module_chain_resolver**

# Module: module_chain_resolver

## Contents

**Structs**

- [`ChainResolver`](#chainresolver) - A resolver that tries multiple resolvers in order, returning the

---

## handl_module_resolver_core::module_chain_resolver::ChainResolver

*Struct*

A resolver that tries multiple resolvers in order, returning the
first non-`None` result.

This enables mixed import sources in a single document — e.g.,
local files, HTTP URLs, and git URLs — without the caller needing
to know which resolver handles which path.

Each resolver returns `None` for paths it doesn't handle, so the
chain falls through to the next. Resolvers that can quickly
determine a path is not theirs (e.g., `FileResolver` sees `git+`
prefix) should short-circuit early.

# Example

```ignore
use handl_module_resolver_core::ChainResolver;

let resolver = ChainResolver::new(vec![
    Box::new(handl_module_resolver_file::FileResolver::with_cache(
    handl_module_resolver_memory::MemoryModuleCache::new(),
    )),
    Box::new(handl_module_resolver_git::GitResolver::temp_with_cache(
        handl_module_resolver_memory::MemoryModuleCache::new(),
    )),
    Box::new(handl_module_resolver_http::HttpResolver::with_cache(
        handl_module_resolver_memory::MemoryModuleCache::new(),
    )),
]);
```

**Fields:**
- `resolvers: Vec<Box<dyn ModuleResolver>>`

**Methods:**

- `fn new(resolvers: Vec<Box<dyn ModuleResolver>>) -> Self` - Create a chain from an ordered list of resolvers.

**Trait Implementations:**

- **ModuleResolver**
  - `fn resolve(self: &Self, path: &str, base: Option<&Path>) -> Option<ResolvedModule>`



