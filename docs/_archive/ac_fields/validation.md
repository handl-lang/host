**handl_fields > validation**

# Module: validation

## Contents

**Functions**

- [`validate_args`](#validate_args) - Validates function call arguments against declared param schemas.
- [`validate_return`](#validate_return) - Validates a function return value against a declared return schema.

---

## handl_fields::validation::validate_args

*Function*

Validates function call arguments against declared param schemas.

Returns `Ok(())` if all args validate, or an error message.
The caller provides a `Schema<V>` implementation for each param.

```rust
fn validate_args(args: &[ac_runtime_values::NonFunctionValue], param_schemas: &[Box<dyn Schema>]) -> Result<(), String>
```



## handl_fields::validation::validate_return

*Function*

Validates a function return value against a declared return schema.

Returns `Ok(())` if the value validates, or an error message.

```rust
fn validate_return(value: &ac_runtime_values::NonFunctionValue, return_schema: &dyn Schema) -> Result<(), String>
```



