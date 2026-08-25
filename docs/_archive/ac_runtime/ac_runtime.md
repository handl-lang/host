**ac_runtime**

# Module: ac_runtime

## Contents

**Modules**

- [`error`](#error)
- [`eval`](#eval)
- [`function_repr`](#function_repr)
- [`host`](#host)
- [`operation`](#operation) - Typed operations for algebraic effect handling.
- [`runtime`](#runtime)
- [`value`](#value)

---

## Module: error



## Module: eval



## Module: function_repr



## Module: host



## Module: operation

Typed operations for algebraic effect handling.

An [`Operation`] is like a [`Function`](crate::Function) but it
**signals** instead of completing. When an operation is performed,
the VM captures the current continuation and yields it to the
caller. A handler can intercept the operation, inspect the
arguments, and decide whether to resume the continuation.

This is the foundation for Sanada-style arrow handlers adapted to
handl's tree-walking VM. Operations are typed, inspectable, and
serializable — the continuation machinery in
[`continuation`](crate::continuation) captures the frame stack.

## Operation vs Function

- A [`Function`] completes: it returns `Ok(value)` or `Err(error)`.
- An [`Operation`] signals: it returns `Err(Error::Operation { ... })`
  carrying its name and arguments. The VM captures the continuation
  and yields `EvalResult::Yielded`.

The VM checks `Host::operation` before `Host::function`. If a name
resolves to an operation, the operation's `perform` method is called.
If `perform` returns `Err(Error::Operation { ... })`, the VM captures
the continuation and yields. If `perform` returns `Ok(value)`, the
operation completed without signaling (e.g. a no-op or cached result).



## Module: runtime



## Module: value



