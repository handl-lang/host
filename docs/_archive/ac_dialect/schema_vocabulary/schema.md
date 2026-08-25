**handl_dialect > schema_vocabulary > schema**

# Module: schema_vocabulary::schema

## Contents

**Traits**

- [`Schema`](#schema) - Validates values of type `V`.

---

## handl_dialect::schema_vocabulary::schema::Schema

*Trait*

Validates values of type `V`.

This trait is the abstract schema interface. It knows nothing
about JSON Schema, `$ref`, `allOf`, `format`, or any other
concrete schema language. Those concepts belong to
implementations.

Generic over `V` so the abstraction works with any value type,
not just `NonFunctionValue`.

**Methods:**

- `validate`: Validates a value against this schema.



