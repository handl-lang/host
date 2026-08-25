**handl_module_resolver_virtual > virtual_resolver**

# Module: virtual_resolver

## Contents

**Structs**

- [`VirtualResolver`](#virtualresolver) - In-memory module resolver.

---

## handl_module_resolver_virtual::virtual_resolver::VirtualResolver

*Struct*

In-memory module resolver.

Maps module paths to pre-parsed JSON document data.
No filesystem I/O — all modules live in memory.

**Fields:**
- `modules: std::collections::HashMap<String, serde_json::Value>`

**Methods:**

- `fn new() -> Self` - Create an empty resolver.
- `fn add(self: & mut Self, path: &str, data: JsonValue)` - Register a module by path with its parsed document data.
- `fn add_raw(self: & mut Self, path: &str, content: &str)` - Register a module by path with raw content (parsed as YAML).

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **ModuleResolver**
  - `fn resolve(self: &Self, path: &str, _base: Option<&Path>) -> Option<ResolvedModule>`



