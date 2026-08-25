**handl_dialect_canonical > schema_vocabulary > json_schema_vocabulary**

# Module: schema_vocabulary::json_schema_vocabulary

## Contents

**Structs**

- [`JsonSchemaVocabulary`](#jsonschemavocabulary) - The canonical schema vocabulary: JSON Schema draft 2020-12.

**Statics**

- [`JSON_SCHEMA_VOCABULARY`](#json_schema_vocabulary) - The canonical schema vocabulary as a static singleton.

---

## handl_dialect_canonical::schema_vocabulary::json_schema_vocabulary::JSON_SCHEMA_VOCABULARY

*Static*

The canonical schema vocabulary as a static singleton.

```rust
static JSON_SCHEMA_VOCABULARY: JsonSchemaVocabulary
```



## handl_dialect_canonical::schema_vocabulary::json_schema_vocabulary::JsonSchemaVocabulary

*Struct*

The canonical schema vocabulary: JSON Schema draft 2020-12.

Implements [`SchemaVocabulary<NonFunctionValue>`] by compiling raw
schema documents into [`JsonSchema`] validators via the
[`jsonschema`] crate.

**Unit Struct**

**Trait Implementations:**

- **SchemaVocabulary**
  - `fn id(self: &Self) -> &'static str`
  - `fn compile(self: &Self, raw: &serde_json::Value) -> Result<Box<dyn Schema>, ValidationError>`



