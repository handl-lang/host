**ac_typecheck_key_merge**

# Module: ac_typecheck_key_merge

## Contents

**Functions**

- [`check`](#check) - Check all merge fields.
- [`check_field`](#check_field) - Check a single merge field.
- [`is_merge_field`](#is_merge_field) - Check if a field schema has `merge: true`.

**Constants**

- [`KEY`](#key) - The handl key this crate handles.

---

## ac_typecheck_key_merge::KEY

*Constant*: `&str`

The handl key this crate handles.



## ac_typecheck_key_merge::check

*Function*

Check all merge fields.

Returns a list of errors for all incompatible merge fields.

```rust
fn check(schemas: &[(String, serde_json::Value)], interpreter: &dyn SchemaInterpreter, parent_env: &handl_typecheck_core::TypeEnv) -> Vec<handl_typecheck_core::TypeError>
```



## ac_typecheck_key_merge::check_field

*Function*

Check a single merge field.

Returns a [`TypeError`] if:
- The field is marked `merge: true` but is not declared as `object`.
- The field is an object but its properties conflict with parent types.

`parent_env` provides the parent scope's types for compatibility checks.

```rust
fn check_field(field_name: &str, schema: &serde_json::Value, interpreter: &dyn SchemaInterpreter, parent_env: &handl_typecheck_core::TypeEnv) -> Result<(), handl_typecheck_core::TypeError>
```



## ac_typecheck_key_merge::is_merge_field

*Function*

Check if a field schema has `merge: true`.

```rust
fn is_merge_field(schema: &serde_json::Value) -> bool
```



