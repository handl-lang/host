**ac_typecheck_key_expr > check**

# Module: check

## Contents

**Functions**

- [`check`](#check) - Check all fields with `expr:` declarations.
- [`check_field`](#check_field) - Check a single field's `expr` against its declared type.
- [`check_field_cached`](#check_field_cached) - Check a single field's `expr` with an AST cache and a pre-interpreted
- [`get_expr`](#get_expr) - Get the expression string from a field schema, if present.

**Constants**

- [`KEY`](#key) - The handl key this crate handles.

---

## ac_typecheck_key_expr::check::KEY

*Constant*: `&str`

The handl key this crate handles.



## ac_typecheck_key_expr::check::check

*Function*

Check all fields with `expr:` declarations.

`schemas` is `(field_name, raw_schema)` pairs.
Returns a list of errors for all incompatible expressions.

```rust
fn check(schemas: &[(String, serde_json::Value)], env: &handl_typecheck_core::TypeEnv, functions: &handl_typecheck_infer::FunctionTable, interpreter: &dyn SchemaInterpreter) -> Vec<handl_typecheck_core::TypeError>
```



## ac_typecheck_key_expr::check::check_field

*Function*

Check a single field's `expr` against its declared type.

Returns a [`TypeError`] if the inferred type is incompatible with
the declared type. Returns `Ok(())` if the expression is absent or
the types are compatible.

```rust
fn check_field(field_name: &str, schema: &serde_json::Value, env: &handl_typecheck_core::TypeEnv, functions: &handl_typecheck_infer::FunctionTable, interpreter: &dyn SchemaInterpreter, cache: & mut handl_typecheck_infer::ParseCache) -> Result<(), handl_typecheck_core::TypeError>
```



## ac_typecheck_key_expr::check::check_field_cached

*Function*

Check a single field's `expr` with an AST cache and a pre-interpreted
declared type.

Like [`check_field`] but reuses a parse cache across multiple calls
and accepts the already-interpreted declared type, avoiding redundant
schema interpretation.

```rust
fn check_field_cached(field_name: &str, schema: &serde_json::Value, env: &handl_typecheck_core::TypeEnv, functions: &handl_typecheck_infer::FunctionTable, interpreter: &dyn SchemaInterpreter, cache: & mut handl_typecheck_infer::ParseCache) -> Result<(), handl_typecheck_core::TypeError>
```



## ac_typecheck_key_expr::check::get_expr

*Function*

Get the expression string from a field schema, if present.

```rust
fn get_expr(schema: &serde_json::Value) -> Option<String>
```



