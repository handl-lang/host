**ac_runtime > host > Function**

# Module: host::Function

## Contents

**Functions**

- [`call`](#call) - Call this function with pre-evaluated arguments.

---

## ac_runtime::host::Function::call

*Function*

Call this function with pre-evaluated arguments.

# Arguments

- `args` — the evaluated argument values, in order.
- `ctx` — the execution context, for variable access.

# Returns

The result value, or an [`Error`] if execution failed.

```rust
fn call(self: &Self, args: &[V], ctx: & mut dyn Context) -> Result<V, Error<V>>
```



