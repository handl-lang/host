**handl_dialect_canonical > schema_vocabulary > json_schema**

# Module: schema_vocabulary::json_schema

## Contents

**Structs**

- [`JsonSchema`](#jsonschema) - A JSON Schema document that can validate `NonFunctionValue`s.

---

## handl_dialect_canonical::schema_vocabulary::json_schema::JsonSchema

*Struct*

A JSON Schema document that can validate `NonFunctionValue`s.

Stores the raw schema as a `serde_json::Value` and implements
[`Schema<NonFunctionValue>`] using the [`jsonschema`] crate for full
JSON Schema draft 2020-12 validation.

**Fields:**
- `raw: serde_json::Value` - The raw schema document.

**Methods:**

- `fn new(raw: JsonValue) -> Self` - Creates a JSON schema from a `serde_json::Value`.
- `fn parse(s: &str) -> Result<Self, serde_json::Error>` - Creates a JSON schema by parsing a JSON string.
- `fn type_name(self: &Self) -> Option<&str>` - Returns the schema's declared type, if any.

**Trait Implementations:**

- **Schema**
  - `fn validate(self: &Self, value: &NonFunctionValue) -> Result<(), ValidationError>`
- **Clone**
  - `fn clone(self: &Self) -> JsonSchema`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



