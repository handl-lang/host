**handl_dialect > schema_vocabulary > schema_stub**

# Module: schema_vocabulary::schema_stub

## Contents

**Structs**

- [`StubSchema`](#stubschema) - A minimal schema validator for testing.

---

## handl_dialect::schema_vocabulary::schema_stub::StubSchema

*Struct*

A minimal schema validator for testing.

Always succeeds (accepts any value).

**Unit Struct**

**Trait Implementations:**

- **Schema**
  - `fn validate(self: &Self, _value: &NonFunctionValue) -> Result<(), ValidationError>`



