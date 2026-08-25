**handl_typecheck_keys > mutable**

# Module: mutable

## Contents

**Functions**

- [`check`](#check) - Check that fields declared `mutable: false` are not overridden in
- [`check_in_children`](#check_in_children) - Recursively check that `field` is not overridden in nested objects.
- [`is_immutable`](#is_immutable) - Check if a field schema has `mutable: false`.

**Constants**

- [`KEY`](#key) - The handl key this module handles.

---

## handl_typecheck_keys::mutable::KEY

*Constant*: `&str`

The handl key this module handles.



## handl_typecheck_keys::mutable::check

*Function*

Check that fields declared `mutable: false` are not overridden in
nested scopes.

Walks the data tree. For each immutable field, compares every nested
occurrence against the root value. Returns [`TypeError`]s for any
mismatches.

- `immutable_fields` — field names declared `mutable: false`.
- `data` — the document data (after field extraction).

```rust
fn check(immutable_fields: &[String], data: &serde_json::Value) -> Vec<handl_typecheck_core::TypeError>
```



## handl_typecheck_keys::mutable::check_in_children

*Function*

Recursively check that `field` is not overridden in nested objects.

```rust
fn check_in_children(value: &serde_json::Value, field: &str, root_val: &serde_json::Value, errors: & mut Vec<handl_typecheck_core::TypeError>, path: &str)
```



## handl_typecheck_keys::mutable::is_immutable

*Function*

Check if a field schema has `mutable: false`.

```rust
fn is_immutable(schema: &serde_json::Value) -> bool
```



