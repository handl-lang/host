**handl_typecheck_import > detect**

# Module: detect

## Contents

**Functions**

- [`detect_import`](#detect_import) - Detect import path from a merge field's `expr`.

---

## handl_typecheck_import::detect::detect_import

*Function*

Detect import path from a merge field's `expr`.

Reuses the same patterns as `handl-typegen-import`:
- `import('path')` or `import("path")` → static path
- `import(arg)` → data-driven (path from the field's data value)

```rust
fn detect_import(schema: &serde_json::Value, data: &serde_json::Value, field_name: &str) -> Option<String>
```



