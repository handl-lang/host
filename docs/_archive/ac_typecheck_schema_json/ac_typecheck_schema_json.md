**handl_typecheck_schema_json**

# Module: handl_typecheck_schema_json

## Contents

**Modules**

- [`interpret`](#interpret)

**Structs**

- [`JsonSchemaInterpreter`](#jsonschemainterpreter) - JSON Schema interpreter — converts JSON Schema fragments to [`Type`]s.

---

## handl_typecheck_schema_json::JsonSchemaInterpreter

*Struct*

JSON Schema interpreter — converts JSON Schema fragments to [`Type`]s.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> JsonSchemaInterpreter`
- **Default**
  - `fn default() -> JsonSchemaInterpreter`
- **SchemaInterpreter**
  - `fn interpret(self: &Self, schema: &JsonValue) -> Type`



## Module: interpret



