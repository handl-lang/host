**handl_fields > scope**

# Module: scope

## Contents

**Functions**

- [`check_mutable_in_children`](#check_mutable_in_children) - Recursively checks that `field` is not overridden in nested objects.
- [`validate_mutable`](#validate_mutable) - Validates that fields declared `mutable: false` are not overridden in

---

## handl_fields::scope::check_mutable_in_children

*Function*

Recursively checks that `field` is not overridden in nested objects.

```rust
fn check_mutable_in_children(value: &serde_json::Value, field: &str, root_val: &serde_json::Value, errors: & mut Vec<String>, path: &str)
```



## handl_fields::scope::validate_mutable

*Function*

Validates that fields declared `mutable: false` are not overridden in
nested scopes.

Walks the data tree. For each immutable field, compares every nested
occurrence against the root value. Returns a list of error messages
for any mismatches.

```rust
fn validate_mutable(data: &serde_json::Value, immutable: &[String]) -> Vec<String>
```



