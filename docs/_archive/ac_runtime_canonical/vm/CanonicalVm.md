**ac_runtime_canonical > vm > CanonicalVm**

# Module: vm::CanonicalVm

## Contents

**Functions**

- [`eval`](#eval) - Evaluates an expression in the given context.
- [`eval_async`](#eval_async) - Evaluates an expression asynchronously.

---

## ac_runtime_canonical::vm::CanonicalVm::eval

*Function*

Evaluates an expression in the given context.

This is the core evaluation loop. It recursively evaluates
sub-expressions, dispatches calls to host functions, and
returns the resulting [`NonFunctionValue`].

For `Call` expressions, the VM checks [`Host::lazy_function`]
first. If the name resolves to a lazy function, the VM passes
the unevaluated argument expressions and an evaluate closure.
The function decides which arguments to evaluate (short-circuit).
Otherwise, the VM eagerly evaluates all arguments and dispatches
to [`Host::function`] or [`Host::operation`].

```rust
fn eval(self: &Self, expr: &crate::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>
```



## ac_runtime_canonical::vm::CanonicalVm::eval_async

*Function*

Evaluates an expression asynchronously.

Like [`eval`](CanonicalVm::eval), but supports async host functions.
The VM checks [`Host::function`] first (sync), then falls back
to [`Host::async_function`] (async). This means a host can
provide both sync and async functions, and the VM will use
whichever is available.

# Yielding

Async host functions can yield control to the executor by
awaiting a pending future. The entire evaluation suspends
until the future is ready.

To explicitly yield a value to the caller, a host function
returns `Err(Error::Yield(value))`. The caller receives
`Err(Error::Yield(value))` and can inspect the yielded value.

Like [`eval`](CanonicalVm::eval), this checks [`Host::lazy_function`]
first for short-circuit evaluation.

```rust
fn eval_async(self: &Self, expr: &crate::Expr<ac_runtime_values::NonFunctionValue>, ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>
```



