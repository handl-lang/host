**handl_dialect > document_vocabulary > document_vocabulary**

# Module: document_vocabulary::document_vocabulary

## Contents

**Traits**

- [`DocumentVocabulary`](#documentvocabulary) - Document vocabulary — the reserved key names used in handl documents.

---

## handl_dialect::document_vocabulary::document_vocabulary::DocumentVocabulary

*Trait*

Document vocabulary — the reserved key names used in handl documents.

These keys have special meaning during field extraction, type checking,
and type generation. A custom dialect may use different key names
(e.g. `$` → `properties`, `expr` → `compute`).

This trait defines the interface only. Canonical key values are
implemented in the `handl-dialect-canonical` crate.

**Methods:**

- `fields_key`: The top-level key for field declarations.
- `expr_key`: The field-level key for computed field expressions.
- `arg_key`: The field-level key for parameterized computed field argument type.
- `merge_key`: The field-level key for merge field flag.
- `inheritable_key`: The field-level key for scoped inheritance flag.
- `mutable_key`: The field-level key for immutability flag.
- `params_key`: The function-level key for parameters.
- `return_key`: The function-level key for return schema.
- `body_key`: The function-level key for user-defined function body.
- `reserved_field_keys`: All field-level reserved keys (used for stripping during typegen).
- `top_level_reserved_keys`: Reserved top-level keys that cannot be declared as field names.



