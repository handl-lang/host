**ac_runtime > host > GenericFunction**

# Module: host::GenericFunction

## Contents

**Functions**

- [`call_with_types`](#call_with_types) - Call this function with pre-evaluated arguments and resolved type arguments.

---

## ac_runtime::host::GenericFunction::call_with_types

*Function*

Call this function with pre-evaluated arguments and resolved type arguments.

- `args` — the evaluated argument values, in order.
- `type_args` — resolved concrete type names from the typechecker.
- `ctx` — the execution context.

```rust
fn call_with_types(self: &Self, args: &[V], type_args: &[String], ctx: & mut dyn Context) -> Result<V, Error<V>>
```



