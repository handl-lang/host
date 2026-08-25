**handl_typegen_backend_json_schema > compose**

# Module: compose

## Contents

**Functions**

- [`compose_schema`](#compose_schema) - Compose a root JSON Schema from clean field schema fragments.

---

## handl_typegen_backend_json_schema::compose::compose_schema

*Function*

Compose a root JSON Schema from clean field schema fragments.

Each fragment is `(field_name, schema_json)`. The output is a JSON
Schema draft-07 document with:
- `$schema` set to draft-07
- `type: object`
- `properties` built from the fragments
- `additionalProperties: false`

```rust
fn compose_schema(fields: &indexmap::IndexMap<String, serde_json::Value>) -> serde_json::Value
```



