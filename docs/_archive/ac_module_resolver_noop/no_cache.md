**handl_module_resolver_noop > no_cache**

# Module: no_cache

## Contents

**Structs**

- [`NoCache`](#nocache) - No-op cache — every `get` returns `None`.

---

## handl_module_resolver_noop::no_cache::NoCache

*Struct*

No-op cache — every `get` returns `None`.

Use when caching is not desired (e.g., tests that need fresh I/O
on every resolve, or memory-constrained environments).

**Unit Struct**

**Trait Implementations:**

- **ModuleCache**
  - `fn get(self: &Self, _key: &str) -> Option<ResolvedModule>`
  - `fn insert(self: &Self, _key: &str, _module: ResolvedModule)`
  - `fn clear(self: &Self)`
  - `fn len(self: &Self) -> usize`



