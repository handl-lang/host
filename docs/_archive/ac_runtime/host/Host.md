**ac_runtime > host > Host**

# Module: host::Host

## Contents

**Functions**

- [`async_function`](#async_function) - Resolve a function name to an async callable [`AsyncFunction`].
- [`function`](#function) - Resolve a function name to a callable [`Function`].
- [`generic_function`](#generic_function) - Resolve a function name to a [`GenericFunction`] that receives
- [`lazy_function`](#lazy_function) - Resolve a function name to a [`LazyFunction`] that receives
- [`operation`](#operation) - Resolve a name to a typed [`Operation`].

---

## ac_runtime::host::Host::async_function

*Function*

Resolve a function name to an async callable [`AsyncFunction`].

Returns `None` by default. Override to provide async functions
(network I/O, timers, cooperative yielding). The async VM
checks this after [`Host::function`] returns `None`.

```rust
fn async_function(self: &Self, _name: &str) -> Option<&dyn AsyncFunction>
```



## ac_runtime::host::Host::function

*Function*

Resolve a function name to a callable [`Function`].

Returns `None` if the name is not recognized.

```rust
fn function(self: &Self, name: &str) -> Option<&dyn Function>
```



## ac_runtime::host::Host::generic_function

*Function*

Resolve a function name to a [`GenericFunction`] that receives
resolved type arguments.

Returns `None` by default. The VM checks this when a call carries
non-empty `type_args`. If a generic function is registered, it
receives the type arguments via [`GenericFunction::call_with_types`].
Otherwise, the VM falls back to [`Host::function`].

```rust
fn generic_function(self: &Self, _name: &str) -> Option<&dyn GenericFunction>
```



## ac_runtime::host::Host::lazy_function

*Function*

Resolve a function name to a [`LazyFunction`] that receives
unevaluated arguments.

Returns `None` by default. Override to provide functions that
need lazy (short-circuit) argument evaluation — e.g. `if`,
`and`, `or`. The VM checks this before [`Host::function`] and
[`Host::operation`]. If a name resolves to a lazy function,
the VM passes the raw expression tree instead of pre-evaluated
values, and the function decides which arguments to evaluate.

```rust
fn lazy_function(self: &Self, _name: &str) -> Option<&dyn LazyFunction>
```



## ac_runtime::host::Host::operation

*Function*

Resolve a name to a typed [`Operation`].

Returns `None` by default. Override to provide operations that
signal (yield) instead of completing. The VM checks this before
[`Host::function`] — if a name resolves to an operation, the
operation's `perform` method is called instead of a function's
`call` method.

Operations enable algebraic effect handling: the VM captures
the continuation when an operation signals, and a handler can
intercept, inspect the arguments, and resume.

```rust
fn operation(self: &Self, _name: &str) -> Option<&dyn Operation>
```



