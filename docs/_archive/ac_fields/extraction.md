**handl_fields > extraction**

# Module: extraction

## Contents

**Functions**

- [`extract_fields`](#extract_fields) - Extracts the `$` key from a value, returning
- [`undeclared_fields`](#undeclared_fields) - Returns the names of fields present in `data` but not declared in

---

## handl_fields::extraction::extract_fields

*Function*

Extracts the `$` key from a value, returning
`(field_schemas, data_without_fields)`.

Each entry in `field_schemas` is `(name, raw_schema_value)`.
The caller is responsible for constructing `Schema<V>`
implementations from the raw values.

Key names are read from the dialect — a custom dialect may use
different key names for `$`, `expr`, etc.

Returns an error if a declared field name is reserved.

```rust
fn extract_fields(value: ac_runtime_values::NonFunctionValue, dialect: &dyn Dialect) -> Result<crate::types::ExtractedFields, String>
```



## handl_fields::extraction::undeclared_fields

*Function*

Returns the names of fields present in `data` but not declared in
`declared_names`.

```rust
fn undeclared_fields<'a>(data: &'a ac_runtime_values::NonFunctionValue, declared: &[&str]) -> Vec<&'a str>
```



