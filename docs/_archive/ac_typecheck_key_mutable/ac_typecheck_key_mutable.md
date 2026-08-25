**ac_typecheck_key_mutable**

# Module: ac_typecheck_key_mutable

## Contents

**Functions**

- [`check`](#check) - Check that fields declared `mutable: false` are not overridden in
- [`is_immutable`](#is_immutable) - Check if a field schema has `mutable: false`.

**Constants**

- [`KEY`](#key) - The handl key this crate handles.

---

## ac_typecheck_key_mutable::KEY

*Constant*: `&str`

The handl key this crate handles.



## ac_typecheck_key_mutable::check

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



## ac_typecheck_key_mutable::is_immutable

*Function*

Check if a field schema has `mutable: false`.

```rust
fn is_immutable(schema: &serde_json::Value) -> bool
```



