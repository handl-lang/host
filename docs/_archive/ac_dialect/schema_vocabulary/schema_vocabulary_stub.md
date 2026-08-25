**handl_dialect > schema_vocabulary > schema_vocabulary_stub**

# Module: schema_vocabulary::schema_vocabulary_stub

## Contents

**Structs**

- [`StubVocabulary`](#stubvocabulary) - A minimal schema vocabulary for testing.

---

## handl_dialect::schema_vocabulary::schema_vocabulary_stub::StubVocabulary

*Struct*

A minimal schema vocabulary for testing.

**Unit Struct**

**Trait Implementations:**

- **SchemaVocabulary**
  - `fn id(self: &Self) -> &'static str`
  - `fn compile(self: &Self, _raw: &serde_json::Value) -> Result<Box<dyn Schema>, crate::schema_vocabulary::ValidationError>`



