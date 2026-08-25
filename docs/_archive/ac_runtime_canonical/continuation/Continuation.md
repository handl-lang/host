**ac_runtime_canonical > continuation > Continuation**

# Module: continuation::Continuation

## Contents

**Functions**

- [`to_json`](#to_json) - Serializes the continuation to a JSON value.

---

## ac_runtime_canonical::continuation::Continuation::to_json

*Function*

Serializes the continuation to a JSON value.

The format is an array of frame objects, each with:
- `name` — the function name
- `evaluated` — array of already-evaluated argument values
- `pending` — array of remaining expression trees

```rust
fn to_json(self: &Self) -> serde_json::Value
```



