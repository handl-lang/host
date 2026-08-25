**ac_typecheck_key_inherit > check**

# Module: check

## Contents

**Functions**

- [`check`](#check) - Check that inherited field overrides in nested data are type-compatible.
- [`is_inheritable`](#is_inheritable) - Check if a field schema has `inheritable: true`.

**Constants**

- [`KEY`](#key) - The handl key this crate handles.

---

## ac_typecheck_key_inherit::check::KEY

*Constant*: `&str`

The handl key this crate handles.



## ac_typecheck_key_inherit::check::check

*Function*

Check that inherited field overrides in nested data are type-compatible.

Walks the data tree. For each inheritable field, compares every nested
occurrence's type against the root declared type. Returns errors for
any mismatches.

- `inheritable_schemas` — `(field_name, raw_schema)` pairs for fields
  marked `inheritable: true`.
- `data` — the document data (after field extraction).
- `interpreter` — converts raw schemas to types.

```rust
fn check(inheritable_schemas: &[(String, serde_json::Value)], data: &serde_json::Value, interpreter: &dyn SchemaInterpreter) -> Vec<handl_typecheck_core::TypeError>
```



## ac_typecheck_key_inherit::check::is_inheritable

*Function*

Check if a field schema has `inheritable: true`.

```rust
fn is_inheritable(schema: &serde_json::Value) -> bool
```



