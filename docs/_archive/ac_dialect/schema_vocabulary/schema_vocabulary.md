**handl_dialect > schema_vocabulary > schema_vocabulary**

# Module: schema_vocabulary::schema_vocabulary

## Contents

**Traits**

- [`SchemaVocabulary`](#schemavocabulary) - A schema vocabulary — compiles raw schema documents into validators.

---

## handl_dialect::schema_vocabulary::schema_vocabulary::SchemaVocabulary

*Trait*

A schema vocabulary — compiles raw schema documents into validators.

This trait abstracts which schema language a dialect uses. The
canonical implementation is JSON Schema draft 2020-12, provided by
`handl-dialect-canonical` as `JsonSchemaVocabulary`.

Generic over `V` so the abstraction works with any value type,
not just `NonFunctionValue`.

**Methods:**

- `id`: The vocabulary identifier (e.g. `"json-schema-draft-2020-12"`).
- `compile`: Compile a raw schema document into a validator.



