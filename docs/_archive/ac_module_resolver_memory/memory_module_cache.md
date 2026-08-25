**handl_module_resolver_memory > memory_module_cache**

# Module: memory_module_cache

## Contents

**Structs**

- [`MemoryModuleCache`](#memorymodulecache) - In-memory cache backed by a `Mutex<HashMap>`.

---

## handl_module_resolver_memory::memory_module_cache::MemoryModuleCache

*Struct*

In-memory cache backed by a `Mutex<HashMap>`.

Thread-safe — suitable for both single-threaded and multi-threaded
pipeline usage (e.g. parallel type checking with rayon).

**Fields:**
- `map: std::sync::Mutex<std::collections::HashMap<String, handl_module_resolver_core::ResolvedModule>>`

**Methods:**

- `fn new() -> Self`

**Trait Implementations:**

- **ModuleCache**
  - `fn get(self: &Self, key: &str) -> Option<ResolvedModule>`
  - `fn insert(self: &Self, key: &str, module: ResolvedModule)`
  - `fn clear(self: &Self)`
  - `fn len(self: &Self) -> usize`
- **Default**
  - `fn default() -> Self`



