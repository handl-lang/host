**ac_runtime > host**

# Module: host

## Contents

**Structs**

- [`HostRef`](#hostref) - A borrowed reference to a [`Host`] that itself implements [`Host`].

**Traits**

- [`AsyncFunction`](#asyncfunction) - An async callable function provided by the host.
- [`Function`](#function) - A callable function provided by the host.
- [`GenericFunction`](#genericfunction) - A generic function that receives resolved type arguments at runtime.
- [`Host`](#host) - Provides functions to the VM.
- [`LazyFunction`](#lazyfunction) - A lazy function that receives unevaluated arguments.

---

## ac_runtime::host::AsyncFunction

*Trait*

An async callable function provided by the host.

Like [`Function`], but returns a `Future` instead of a direct
result. This allows host functions to perform async operations
(network I/O, timers, cooperative yielding) without blocking the
calling thread.

The async VM ([`Vm::eval_async`](crate::VirtualMachine::eval_async)) checks
[`Host::async_function`] after [`Host::function`] returns `None`.
This means a host can provide both sync and async functions, and
the VM will use whichever is available for a given name.

# Implementing

```ignore
struct Fetch;
impl<V: Value> AsyncFunction<V> for Fetch {
    fn call<'a>(
        &'a self,
        args: &'a [V],
        _ctx: &'a mut (dyn Context<V> + 'a),
    ) -> Pin<Box<dyn Future<Output = Result<V, Error<V>>> + 'a>> {
        Box::pin(async {
            // async work here
            Ok(V::default())
        })
    }
}
```

# Yielding

An async function can yield control to the executor by awaiting
a pending future (e.g. `tokio::task::yield_now().await`). This
suspends the entire evaluation without producing a result — the
executor reschedules it later.

To yield a value to the caller (explicit yield), return
`Err(Error::Yield(value))`. The caller can inspect the yielded
value and decide whether to resume.

**Methods:**

- `call`: Call this function with pre-evaluated arguments.



## ac_runtime::host::Function

*Trait*

A callable function provided by the host.

Functions implement the execution semantics for built-in
operations. The VM evaluates arguments and passes the resulting
values to the function's [`call`](Function::call) method.

Functions receive a mutable reference to the [`Context`],
allowing them to read or modify variables in the current scope.

# Interior mutability

The `call` method takes `&self`, not `&mut self`. If a function
needs mutable internal state, use interior mutability (e.g.
`RefCell`, `AtomicUsize`).

**Methods:**

- `call`: Call this function with pre-evaluated arguments.



## ac_runtime::host::GenericFunction

*Trait*

A generic function that receives resolved type arguments at runtime.

Extends [`Function`] with the ability to inspect the concrete types
that a generic function was instantiated with. The typechecker
populates `type_args` during monomorphization — each entry is a
string representation of the concrete type (e.g. `"string"`,
`"array<number>"`).

The VM checks [`Host::generic_function`] for calls that carry
type arguments. If a generic function is registered, it receives
the type arguments. Otherwise, the VM falls back to [`Function`].

# Example

```ignore
struct TypedIdentity;

impl<V: Value> GenericFunction<V> for TypedIdentity {
    fn call_with_types(
        &self,
        args: &[V],
        type_args: &[String],
        _ctx: &mut dyn Context<V>,
    ) -> Result<V, Error<V>> {
        // type_args[0] might be "string" or "number"
        Ok(args[0].clone())
    }
}
```

**Methods:**

- `call_with_types`: Call this function with pre-evaluated arguments and resolved type arguments.



## ac_runtime::host::Host

*Trait*

Provides functions to the VM.

The host is the bridge between the language runtime and the
outside world. The VM asks the host to resolve function names;
the host returns a reference to a [`Function`] that knows how
to execute that function.

The VM has no knowledge of what any function does — it only
knows how to call functions that the host provides.

# Implementing a host

```ignore
struct MyHost<V: Value> {
    functions: Vec<(String, Box<dyn Function<V>>)>,
}

impl<V: Value> Host<V> for MyHost<V> {
    fn function(&self, name: &str) -> Option<&dyn Function<V>> {
        self.functions
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, f)| f.as_ref())
    }
}
```

**Methods:**

- `function`: Resolve a function name to a callable [`Function`].
- `async_function`: Resolve a function name to an async callable [`AsyncFunction`].
- `operation`: Resolve a name to a typed [`Operation`].
- `lazy_function`: Resolve a function name to a [`LazyFunction`] that receives
- `generic_function`: Resolve a function name to a [`GenericFunction`] that receives
- `is_reserved`: Check whether a name is reserved and may not be shadowed by assignment.



## ac_runtime::host::HostRef

*Struct*

A borrowed reference to a [`Host`] that itself implements [`Host`].

This lets the generic [`VirtualMachine`] run against a `&dyn Host` without
needing ownership of the host value.

**Generic Parameters:**
- 'a
- V

**Tuple Struct**: `(&'a dyn Host)`

**Methods:**

- `fn new(host: &'a dyn Host) -> Self` - Wraps a `&dyn Host` for use with the VM.

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> HostRef<'a, V>`
- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn async_function(self: &Self, name: &str) -> Option<&dyn AsyncFunction>`
  - `fn operation(self: &Self, name: &str) -> Option<&dyn Operation>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`
  - `fn generic_function(self: &Self, name: &str) -> Option<&dyn GenericFunction>`
  - `fn is_reserved(self: &Self, name: &str) -> bool`



## ac_runtime::host::LazyFunction

*Trait*

A lazy function that receives unevaluated arguments.

Unlike [`Function`], which receives pre-evaluated argument values,
`LazyFunction` receives the raw expression tree. The function
decides which arguments to evaluate and in what order by calling
the `evaluate` closure.

This enables short-circuit evaluation:
- `if(cond, then, else)` only evaluates the chosen branch.
- `and(a, b)` stops at the first `false`.
- `or(a, b)` stops at the first `true`.

The VM checks [`Host::lazy_function`] before [`Host::function`]
and [`Host::operation`]. If a name resolves to a lazy function,
the VM calls it with unevaluated args. Otherwise, the VM eagerly
evaluates args and dispatches to [`Host::function`].

# Implementing

A lazy function can also implement [`Function`] for direct callers
that pass pre-evaluated values. The `Function` impl evaluates all
args eagerly; the `LazyFunction` impl short-circuits.

```ignore
struct If;

impl<V: Value> LazyFunction<V> for If {
    fn call(
        &self,
        args: &[Expr<V>],
        ctx: &mut dyn Context<V>,
        evaluate: &mut dyn FnMut(&Expr<V>, &mut dyn Context<V>) -> Result<V, Error<V>>,
    ) -> Result<V, Error<V>> {
        let cond = evaluate(&args[0], ctx)?.as_bool().unwrap_or(false);
        if cond {
            evaluate(&args[1], ctx)
        } else {
            evaluate(&args[2], ctx)
        }
    }
}
```

**Methods:**

- `call`: Call this function with unevaluated arguments.



