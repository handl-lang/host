**handl_typecheck_keys > inherit > walker**

# Module: inherit::walker

## Contents

**Functions**

- [`check_in_children`](#check_in_children) - Recursively check that `field` overrides in nested objects are
- [`infer_value_type`](#infer_value_type) - Infer the type of a concrete JSON value.

---

## handl_typecheck_keys::inherit::walker::check_in_children

*Function*

Recursively check that `field` overrides in nested objects are
type-compatible with `root_type`.

```rust
fn check_in_children(value: &serde_json::Value, field: &str, root_type: &handl_typecheck_core::Type, _root_val: &serde_json::Value, interpreter: &dyn SchemaInterpreter, errors: & mut Vec<handl_typecheck_core::TypeError>, path: &str)
```



## handl_typecheck_keys::inherit::walker::infer_value_type

*Function*

Infer the type of a concrete JSON value.

```rust
fn infer_value_type(value: &serde_json::Value, interpreter: &dyn SchemaInterpreter) -> handl_typecheck_core::Type
```



