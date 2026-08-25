**handl_module_resolver_core > resolved_module**

# Module: resolved_module

## Contents

**Structs**

- [`ResolvedModule`](#resolvedmodule) - A resolved module — the result of a `ModuleResolver::resolve` call.

---

## handl_module_resolver_core::resolved_module::ResolvedModule

*Struct*

A resolved module — the result of a `ModuleResolver::resolve` call.

Contains everything needed to merge imported schemas into a parent
context and recurse into nested imports.

**Fields:**
- `schemas: Vec<(String, serde_json::Value)>` - Field name → raw schema fragment (handl keys still present).
- `merge: Vec<String>` - Field names marked with `merge: true`.
- `data: serde_json::Value` - The document's data values (for data-driven import resolution).
- `base_path: Option<std::path::PathBuf>` - Base path for resolving nested imports from this module.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ResolvedModule`



