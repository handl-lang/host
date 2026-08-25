**ac_typecheck_key_termination**

# Module: ac_typecheck_key_termination

## Contents

**Functions**

- [`check`](#check) - Check all document-declared functions for infinite recursion cycles.
- [`extract_function_bodies`](#extract_function_bodies) - Extract function bodies from the document data.

---

## ac_typecheck_key_termination::check

*Function*

Check all document-declared functions for infinite recursion cycles.

Returns a list of [`TypeError`]s, one per detected cycle.

```rust
fn check(function_bodies: &std::collections::HashMap<String, String>, dialect: &dyn Dialect) -> Vec<handl_typecheck_core::TypeError>
```



## ac_typecheck_key_termination::extract_function_bodies

*Function*

Extract function bodies from the document data.

Functions are stored as JSON objects with a `body` string field,
typically under `doc.functions.<name>` or directly as field values
with `type: "function"`.

```rust
fn extract_function_bodies(data: &serde_json::Value) -> std::collections::HashMap<String, String>
```



