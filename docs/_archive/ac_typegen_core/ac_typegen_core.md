**handl_typegen_core**

# Module: handl_typegen_core

## Contents

**Modules**

- [`plugin`](#plugin)

**Structs**

- [`TypegenContext`](#typegencontext) - Schema-system-agnostic generation context.

**Functions**

- [`strip_keys`](#strip_keys) - Recursively remove the given keys from a JSON value.
- [`strip_keys_in_place`](#strip_keys_in_place)

**Traits**

- [`TypegenBackend`](#typegenbackend) - A type definition generator. Format-specific.

---

## handl_typegen_core::TypegenBackend

*Trait*

A type definition generator. Format-specific.

The core produces clean `JsonValue` fragments. Each backend interprets
them and produces its target format.

- `handl-typegen-json` → JSON Schema (`JsonValue`)
- `handl-typegen-typescript` → TypeScript `.d.ts` (string)
- Future: Rust structs, Python TypedDict, etc.

**Methods:**

- `Output`
- `generate`: Generate a single document's type definition.
- `extension`: File extension for emitted files (e.g. "schema.json", "d.ts").
- `ide_hint`: IDE hint comment to inject into source files (e.g.



## handl_typegen_core::TypegenContext

*Struct*

Schema-system-agnostic generation context.

Holds field name → schema fragment pairs. Built from raw schemas
using [`TypegenContext::from_schemas`]. handl-key crates filter and strip
the context. Import resolution merges imported schemas. Backends
consume the final clean fragments.

**Fields:**
- `fields: indexmap::IndexMap<String, serde_json::Value>` - Field name → schema fragment.

**Methods:**

- `fn from_schemas(schemas: &[(String, JsonValue)]) -> Self` - Build from raw field schemas. No filtering, no stripping.
- `fn remove_fields(self: & mut Self, names: &[String])` - Remove fields by name.
- `fn strip_keys(self: & mut Self, keys: &[&str])` - Recursively strip the given keys from all field schemas, in place.
- `fn merge_fields(self: & mut Self, imported: &[(String, JsonValue)])` - Merge additional fields into the context.



## Module: plugin



## handl_typegen_core::strip_keys

*Function*

Recursively remove the given keys from a JSON value.

Walks objects and arrays, removing any key in `keys` from object maps.
Non-object values are returned unchanged.

Prefer [`strip_keys_in_place`] when you own the value — it avoids
a full deep clone.

```rust
fn strip_keys(value: &serde_json::Value, keys: &[&str]) -> serde_json::Value
```



## handl_typegen_core::strip_keys_in_place

*Function*


Walks objects and arrays, removing any key in `keys` from object maps.
Non-object values are left unchanged. This avoids the deep cloning
that the returning variant [`strip_keys`] performs.

```rust
fn strip_keys_in_place(value: & mut serde_json::Value, keys: &[&str])
```



