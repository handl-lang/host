**handl_store > canonical**

# Module: canonical

## Contents

**Functions**

- [`canonical_string`](#canonical_string) - Serialize a JSON value to a canonical string (sorted keys, no
- [`canonicalize_json`](#canonicalize_json) - Produce a canonical JSON value with sorted object keys.

---

## handl_store::canonical::canonical_string

*Function*

Serialize a JSON value to a canonical string (sorted keys, no
whitespace).

```rust
fn canonical_string(value: &serde_json::Value) -> String
```



## handl_store::canonical::canonicalize_json

*Function*

Produce a canonical JSON value with sorted object keys.

Canonicalization ensures that two semantically identical values
produce the same hash regardless of key order. Arrays preserve
their order (order is semantically significant for arrays).

```rust
fn canonicalize_json(value: &serde_json::Value) -> serde_json::Value
```



