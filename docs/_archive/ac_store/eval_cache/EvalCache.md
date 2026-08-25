**handl_store > eval_cache > EvalCache**

# Module: eval_cache::EvalCache

## Contents

**Functions**

- [`to_json`](#to_json) - Serializes the cache to a JSON value.

---

## handl_store::eval_cache::EvalCache::to_json

*Function*

Serializes the cache to a JSON value.

Format: `{ "hash|args_json": result, ... }`

```rust
fn to_json(self: &Self) -> serde_json::Value
```



