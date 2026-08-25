**handl_typegen_core > TypegenContext**

# Module: TypegenContext

## Contents

**Functions**

- [`from_schemas`](#from_schemas) - Build from raw field schemas. No filtering, no stripping.

---

## handl_typegen_core::TypegenContext::from_schemas

*Function*

Build from raw field schemas. No filtering, no stripping.

The orchestrator applies handl-key crates after this.

```rust
fn from_schemas(schemas: &[(String, JsonValue)]) -> Self
```



