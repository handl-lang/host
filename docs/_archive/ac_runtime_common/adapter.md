**ac_runtime_common > adapter**

# Module: adapter

## Contents

**Structs**

- [`Catch`](#catch) - Catches errors from a function and runs a handler to recover.
- [`CatchOp`](#catchop) - Catches operation errors and recovers with a handler.
- [`Const`](#const) - A function that always returns a constant value.
- [`Identity`](#identity) - A function that returns its first argument unchanged.
- [`If`](#if) - A function that conditionally evaluates one of two lazy branches.
- [`Logged`](#logged) - Logs arguments and results of a function.
- [`MapArgs`](#mapargs) - Transforms arguments before passing them to the inner function.
- [`MapResult`](#mapresult) - Transforms the result of a function.
- [`Memoize`](#memoize) - Caches results of a function by the JSON representation of its args.
- [`Partial`](#partial) - A function that partially applies the first `n` arguments to `inner`.
- [`Pipe`](#pipe) - Pipes the result of one function into a second function.
- [`Timed`](#timed) - Times a function and reports the duration.

**Functions**

- [`args_to_key`](#args_to_key)
- [`catch`](#catch) - Creates a function that catches errors and applies a handler.
- [`catch_operation`](#catch_operation) - Creates an operation that catches errors and applies a handler.
- [`const`](#const) - Creates a function that always returns `value`.
- [`identity`](#identity) - Creates the identity function.
- [`if`](#if) - Creates a conditional function.
- [`logged`](#logged) - Creates a function that logs its calls and results.
- [`map_args`](#map_args) - Creates a function that transforms its arguments before evaluation.
- [`map_result`](#map_result) - Creates a function that transforms its result.
- [`memoize`](#memoize) - Creates a memoized function from another function.
- [`partial`](#partial) - Creates a function with `bound` arguments prepended to every call.
- [`pipe`](#pipe) - Creates a function that pipes `first` into `second`.
- [`timed`](#timed) - Creates a function that times its calls.

---

## ac_runtime_common::adapter::Catch

*Struct*

Catches errors from a function and runs a handler to recover.

**Generic Parameters:**
- F
- H

**Fields:**
- `inner: F`
- `handler: H`

**Methods:**

- `fn new(inner: F, handler: H) -> Self` - Wraps a function with an error handler.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::adapter::CatchOp

*Struct*

Catches operation errors and recovers with a handler.

**Generic Parameters:**
- O
- H

**Fields:**
- `inner: O`
- `handler: H`

**Methods:**

- `fn new(inner: O, handler: H) -> Self` - Wraps an operation with an error handler.

**Trait Implementations:**

- **Operation**
  - `fn signature(self: &Self) -> ac_runtime::OperationSig<NonFunctionValue>`
  - `fn perform(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::adapter::Const

*Struct*

A function that always returns a constant value.

**Tuple Struct**: `(ac_runtime_values::NonFunctionValue)`

**Methods:**

- `fn new(value: NonFunctionValue) -> Self` - Creates a constant function returning `value`.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, _args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::adapter::Identity

*Struct*

A function that returns its first argument unchanged.

**Unit Struct**

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], _ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::adapter::If

*Struct*

A function that conditionally evaluates one of two lazy branches.

**Generic Parameters:**
- C
- T
- E

**Fields:**
- `condition: C`
- `then_branch: T`
- `else_branch: E`

**Methods:**

- `fn new(condition: C, then_branch: T, else_branch: E) -> Self` - Creates a conditional function.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::adapter::Logged

*Struct*

Logs arguments and results of a function.

**Generic Parameters:**
- F
- L

**Fields:**
- `inner: F`
- `logger: L`

**Methods:**

- `fn new(inner: F, logger: L) -> Self` - Wraps a function with a logger.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::adapter::MapArgs

*Struct*

Transforms arguments before passing them to the inner function.

**Generic Parameters:**
- F
- T

**Fields:**
- `inner: F`
- `transform: T`

**Methods:**

- `fn new(inner: F, transform: T) -> Self` - Wraps a function, applying a transform to its arguments.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::adapter::MapResult

*Struct*

Transforms the result of a function.

**Generic Parameters:**
- F
- T

**Fields:**
- `inner: F`
- `transform: T`

**Methods:**

- `fn new(inner: F, transform: T) -> Self` - Wraps a function, applying a transform to its result.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::adapter::Memoize

*Struct*

Caches results of a function by the JSON representation of its args.

**Generic Parameters:**
- F

**Fields:**
- `inner: F`
- `cache: std::cell::RefCell<std::collections::HashMap<String, ac_runtime_values::NonFunctionValue>>`

**Methods:**

- `fn new(inner: F) -> Self` - Wraps a function with a per-args cache.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::adapter::Partial

*Struct*

A function that partially applies the first `n` arguments to `inner`.

**Generic Parameters:**
- F

**Fields:**
- `inner: F`
- `bound: Vec<ac_runtime_values::NonFunctionValue>`

**Methods:**

- `fn new(inner: F, bound: Vec<NonFunctionValue>) -> Self` - Binds `bound` to the front of every call to `inner`.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::adapter::Pipe

*Struct*

Pipes the result of one function into a second function.

**Generic Parameters:**
- F
- G

**Fields:**
- `first: F`
- `second: G`

**Methods:**

- `fn new(first: F, second: G) -> Self` - Pipes `first` into `second`.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::adapter::Timed

*Struct*

Times a function and reports the duration.

**Generic Parameters:**
- F
- L

**Fields:**
- `inner: F`
- `label: String`
- `logger: L`

**Methods:**

- `fn new<impl Into<String>>(inner: F, label: impl Trait, logger: L) -> Self` - Wraps a function with a timer.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::adapter::args_to_key

*Function*

```rust
fn args_to_key(args: &[ac_runtime_values::NonFunctionValue]) -> String
```



## ac_runtime_common::adapter::catch

*Function*

Creates a function that catches errors and applies a handler.

```rust
fn catch<F, H>(inner: F, handler: H) -> Box<dyn Function>
```



## ac_runtime_common::adapter::catch_operation

*Function*

Creates an operation that catches errors and applies a handler.

```rust
fn catch_operation<O, H>(inner: O, handler: H) -> Box<dyn Operation>
```



## ac_runtime_common::adapter::const

*Function*

Creates a function that always returns `value`.

```rust
fn const(value: ac_runtime_values::NonFunctionValue) -> Box<dyn Function>
```



## ac_runtime_common::adapter::identity

*Function*

Creates the identity function.

```rust
fn identity() -> Box<dyn Function>
```



## ac_runtime_common::adapter::if

*Function*

Creates a conditional function.

```rust
fn if<C, T, E>(condition: C, then_branch: T, else_branch: E) -> Box<dyn Function>
```



## ac_runtime_common::adapter::logged

*Function*

Creates a function that logs its calls and results.

```rust
fn logged<F, L>(inner: F, logger: L) -> Box<dyn Function>
```



## ac_runtime_common::adapter::map_args

*Function*

Creates a function that transforms its arguments before evaluation.

```rust
fn map_args<F, T>(inner: F, transform: T) -> Box<dyn Function>
```



## ac_runtime_common::adapter::map_result

*Function*

Creates a function that transforms its result.

```rust
fn map_result<F, T>(inner: F, transform: T) -> Box<dyn Function>
```



## ac_runtime_common::adapter::memoize

*Function*

Creates a memoized function from another function.

```rust
fn memoize<F>(inner: F) -> Box<dyn Function>
```



## ac_runtime_common::adapter::partial

*Function*

Creates a function with `bound` arguments prepended to every call.

```rust
fn partial<F>(inner: F, bound: Vec<ac_runtime_values::NonFunctionValue>) -> Box<dyn Function>
```



## ac_runtime_common::adapter::pipe

*Function*

Creates a function that pipes `first` into `second`.

```rust
fn pipe<F, G>(first: F, second: G) -> Box<dyn Function>
```



## ac_runtime_common::adapter::timed

*Function*

Creates a function that times its calls.

```rust
fn timed<F, L, impl Into<String>>(inner: F, label: impl Trait, logger: L) -> Box<dyn Function>
```



