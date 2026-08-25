**ac_runtime > operation > Operation**

# Module: operation::Operation

## Contents

**Functions**

- [`perform`](#perform) - Perform the operation with pre-evaluated arguments.

---

## ac_runtime::operation::Operation::perform

*Function*

Perform the operation with pre-evaluated arguments.

Return `Err(Error::Operation { name, args })` to signal.
Return `Ok(value)` to complete without signaling (e.g. cached).

```rust
fn perform(self: &Self, args: &[V], ctx: & mut dyn Context) -> Result<V, Error<V>>
```



