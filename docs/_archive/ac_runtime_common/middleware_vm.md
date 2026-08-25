**ac_runtime_common > middleware_vm**

# Module: middleware_vm

## Contents

**Structs**

- [`Chain`](#chain) - A chain of middlewares applied in order.
- [`Counter`](#counter) - A middleware that counts completed evaluations.
- [`MiddlewareVm`](#middlewarevm) - A `VirtualMachine` wrapper that applies one or more `VmMiddleware`s.
- [`NoopMiddleware`](#noopmiddleware) - A middleware that does nothing.
- [`Trace`](#trace) - A middleware that records every evaluation.
- [`TraceEvent`](#traceevent) - A recorded VM evaluation.

**Enums**

- [`EvalKind`](#evalkind) - Whether an evaluation is synchronous or asynchronous.

**Functions**

- [`summarize`](#summarize)

**Traits**

- [`VmMiddleware`](#vmmiddleware) - A hook that can observe, short-circuit, or transform VM evaluations.

---

## ac_runtime_common::middleware_vm::Chain

*Struct*

A chain of middlewares applied in order.

`before_eval` runs left-to-right; the first `Some` short-circuits the rest.
`after_eval` runs right-to-left so nested middlewares close like a stack
and each can transform the result produced by the inner/earlier layers.

**Fields:**
- `middlewares: Vec<Box<dyn VmMiddleware>>`

**Methods:**

- `fn new() -> Self` - Creates an empty chain.
- `fn with(self: Self, middleware: Box<dyn VmMiddleware>) -> Self` - Adds a middleware to the chain.
- `fn len(self: &Self) -> usize` - Returns the number of middlewares in the chain.
- `fn is_empty(self: &Self) -> bool` - Returns `true` if the chain is empty.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **VmMiddleware**
  - `fn before_eval(self: &Self, kind: EvalKind, expr: &Expr<NonFunctionValue>, ctx: & mut dyn Context) -> Option<Result<NonFunctionValue, Error<NonFunctionValue>>>`
  - `fn after_eval(self: &Self, kind: EvalKind, expr: &Expr<NonFunctionValue>, result: Result<NonFunctionValue, Error<NonFunctionValue>>, ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut std::fmt::Formatter) -> std::fmt::Result`



## ac_runtime_common::middleware_vm::Counter

*Struct*

A middleware that counts completed evaluations.

**Fields:**
- `count: std::sync::Arc<std::sync::atomic::AtomicUsize>`

**Methods:**

- `fn new() -> Self` - Creates a new counter.
- `fn count(self: &Self) -> usize` - Returns the number of evaluations that have completed.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Counter`
- **Default**
  - `fn default() -> Counter`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **VmMiddleware**
  - `fn after_eval(self: &Self, _kind: EvalKind, _expr: &Expr<NonFunctionValue>, result: Result<NonFunctionValue, Error<NonFunctionValue>>, _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::middleware_vm::EvalKind

*Enum*

Whether an evaluation is synchronous or asynchronous.

**Variants:**
- `Sync` - A synchronous evaluation.
- `Async` - An asynchronous evaluation.

**Traits:** Eq, Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &EvalKind) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> EvalKind`



## ac_runtime_common::middleware_vm::MiddlewareVm

*Struct*

A `VirtualMachine` wrapper that applies one or more `VmMiddleware`s.

`MiddlewareVm` is generic over the inner VM and the middleware, so any
combination can be assembled without modifying the VM trait.

**Generic Parameters:**
- V
- M

**Fields:**
- `inner: V`
- `middleware: M`

**Methods:**

- `fn new(inner: V, middleware: M) -> Self` - Creates a new wrapper around `inner` with the given `middleware`.
- `fn inner(self: &Self) -> &V` - Returns a reference to the inner VM.
- `fn middleware(self: &Self) -> &M` - Returns a reference to the middleware.

**Trait Implementations:**

- **VirtualMachine**
  - `fn eval(self: &Self, expr: &Expr<NonFunctionValue>, ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
  - `fn eval_async(self: &Self, expr: &Expr<NonFunctionValue>, ctx: & mut dyn Context) -> impl Trait`



## ac_runtime_common::middleware_vm::NoopMiddleware

*Struct*

A middleware that does nothing.

**Unit Struct**

**Traits:** VmMiddleware, Copy

**Trait Implementations:**

- **Default**
  - `fn default() -> NoopMiddleware`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> NoopMiddleware`



## ac_runtime_common::middleware_vm::Trace

*Struct*

A middleware that records every evaluation.

**Fields:**
- `events: std::sync::Arc<std::sync::Mutex<Vec<TraceEvent>>>`

**Methods:**

- `fn new() -> Self` - Creates a new trace recorder.
- `fn events(self: &Self) -> Vec<TraceEvent>` - Returns the recorded trace events.
- `fn clear(self: &Self)` - Clears the recorded trace events.

**Trait Implementations:**

- **Default**
  - `fn default() -> Trace`
- **VmMiddleware**
  - `fn after_eval(self: &Self, kind: EvalKind, expr: &Expr<NonFunctionValue>, result: Result<NonFunctionValue, Error<NonFunctionValue>>, _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut std::fmt::Formatter) -> std::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> Trace`



## ac_runtime_common::middleware_vm::TraceEvent

*Struct*

A recorded VM evaluation.

**Fields:**
- `kind: EvalKind` - Whether the evaluation was sync or async.
- `summary: String` - A short summary of the evaluated expression.
- `result: Result<ac_runtime_values::NonFunctionValue, ac_runtime::Error<ac_runtime_values::NonFunctionValue>>` - The result produced by the wrapped VM.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> TraceEvent`
- **PartialEq**
  - `fn eq(self: &Self, other: &TraceEvent) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## ac_runtime_common::middleware_vm::VmMiddleware

*Trait*

A hook that can observe, short-circuit, or transform VM evaluations.

Middlewares are `Send + Sync + 'static` so they can travel across
threads and be held in `Chain` and `Box<dyn VmMiddleware>`.

**Methods:**

- `before_eval`: Called before the inner VM. Returning `Some(result)` short-circuits
- `after_eval`: Called after the inner VM (or a short-circuited `before_eval`).



## ac_runtime_common::middleware_vm::summarize

*Function*

```rust
fn summarize(expr: &ac_runtime::Expr<ac_runtime_values::NonFunctionValue>) -> String
```



