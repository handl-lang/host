**handl_typegen_backend_json_schema**

# Module: handl_typegen_backend_json_schema

## Contents

**Modules**

- [`compose`](#compose)
- [`emit`](#emit)

**Structs**

- [`JsonSchemaBackend`](#jsonschemabackend) - JSON Schema backend. Produces `JsonValue` output.

---

## handl_typegen_backend_json_schema::JsonSchemaBackend

*Struct*

JSON Schema backend. Produces `JsonValue` output.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> JsonSchemaBackend`
- **TypegenBackend**
  - `fn generate(self: &Self, ctx: &TypegenContext) -> JsonValue`
  - `fn extension(self: &Self) -> &str`
  - `fn ide_hint(self: &Self, schema_rel_path: &str) -> String`
- **Default**
  - `fn default() -> JsonSchemaBackend`



## Module: compose



## Module: emit



