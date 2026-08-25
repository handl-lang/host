**ac_typecheck_key_arg**

# Module: ac_typecheck_key_arg

## Contents

**Functions**

- [`check`](#check) - Check all fields with `arg:` declarations.
- [`check_field`](#check_field) - Check a single field's `arg` against its declared type.
- [`get_arg_schema`](#get_arg_schema) - Get the raw `arg` schema from a field schema, if present.
- [`has_arg`](#has_arg) - Check if a field schema has an `arg` key.

**Constants**

- [`KEY`](#key) - The handl key this crate handles.

---

## ac_typecheck_key_arg::KEY

*Constant*: `&str`

The handl key this crate handles.



## ac_typecheck_key_arg::check

*Function*

Check all fields with `arg:` declarations.

Returns a list of errors for all incompatible arg types.

```rust
fn check(schemas: &[(String, serde_json::Value)], interpreter: &dyn SchemaInterpreter) -> Vec<handl_typecheck_core::TypeError>
```



## ac_typecheck_key_arg::check_field

*Function*

Check a single field's `arg` against its declared type.

Returns a [`TypeError`] if the arg type is incompatible with the
declared type. Returns `Ok(())` if the arg is absent or compatible.

```rust
fn check_field(field_name: &str, schema: &serde_json::Value, interpreter: &dyn SchemaInterpreter) -> Result<(), handl_typecheck_core::TypeError>
```



## ac_typecheck_key_arg::get_arg_schema

*Function*

Get the raw `arg` schema from a field schema, if present.

```rust
fn get_arg_schema(schema: &serde_json::Value) -> Option<&serde_json::Value>
```



## ac_typecheck_key_arg::has_arg

*Function*

Check if a field schema has an `arg` key.

```rust
fn has_arg(schema: &serde_json::Value) -> bool
```



