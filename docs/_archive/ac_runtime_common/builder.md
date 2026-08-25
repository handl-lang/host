**ac_runtime_common > builder**

# Module: builder

## Contents

**Structs**

- [`FnAsync`](#fnasync) - An async function backed by a closure.
- [`FnFunction`](#fnfunction) - A function backed by a closure.
- [`FnGeneric`](#fngeneric) - A generic function backed by a closure.
- [`FnLazy`](#fnlazy) - A lazy function backed by a closure.
- [`FnOperation`](#fnoperation) - An operation backed by closures for its signature and behavior.

**Functions**

- [`fn_async`](#fn_async) - Creates an async function from a higher-ranked closure.
- [`fn_function`](#fn_function) - Creates a synchronous function from a closure.
- [`fn_generic`](#fn_generic) - Creates a generic function from a closure.
- [`fn_lazy`](#fn_lazy) - Creates a lazy function from a closure.
- [`fn_operation`](#fn_operation) - Creates an operation from a signature closure and a perform closure.

---

## ac_runtime_common::builder::FnAsync

*Struct*

An async function backed by a closure.

**Generic Parameters:**
- F

**Tuple Struct**: `(F)`

**Methods:**

- `fn new(f: F) -> Self` - Creates a new async function from a closure.

**Trait Implementations:**

- **AsyncFunction**
  - `fn call<'a>(self: &'a Self, args: &'a [NonFunctionValue], ctx: &'a  mut dyn Context) -> Pin<Box<dyn Future>>`



## ac_runtime_common::builder::FnFunction

*Struct*

A function backed by a closure.

**Generic Parameters:**
- F

**Tuple Struct**: `(F)`

**Methods:**

- `fn new(f: F) -> Self` - Creates a new function from a closure.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::builder::FnGeneric

*Struct*

A generic function backed by a closure.

**Generic Parameters:**
- F

**Tuple Struct**: `(F)`

**Methods:**

- `fn new(f: F) -> Self` - Creates a new generic function from a closure.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`
- **GenericFunction**
  - `fn call_with_types(self: &Self, args: &[NonFunctionValue], type_args: &[String], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::builder::FnLazy

*Struct*

A lazy function backed by a closure.

**Generic Parameters:**
- F

**Tuple Struct**: `(F)`

**Methods:**

- `fn new(f: F) -> Self` - Creates a new lazy function from a closure.

**Trait Implementations:**

- **LazyFunction**
  - `fn call(self: &Self, args: &[ac_runtime::Expr<NonFunctionValue>], ctx: & mut dyn Context, evaluate: & mut dyn FnMut) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::builder::FnOperation

*Struct*

An operation backed by closures for its signature and behavior.

**Generic Parameters:**
- S
- P

**Fields:**
- `signature: S`
- `perform: P`

**Methods:**

- `fn new(signature: S, perform: P) -> Self` - Creates a new operation from a signature closure and a perform closure.

**Trait Implementations:**

- **Operation**
  - `fn signature(self: &Self) -> OperationSig<NonFunctionValue>`
  - `fn perform(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn Context) -> Result<NonFunctionValue, Error<NonFunctionValue>>`



## ac_runtime_common::builder::fn_async

*Function*

Creates an async function from a higher-ranked closure.

```rust
fn fn_async<F>(f: F) -> Box<dyn AsyncFunction>
```



## ac_runtime_common::builder::fn_function

*Function*

Creates a synchronous function from a closure.

```rust
fn fn_function<F>(f: F) -> Box<dyn Function>
```



## ac_runtime_common::builder::fn_generic

*Function*

Creates a generic function from a closure.

```rust
fn fn_generic<F>(f: F) -> Box<dyn GenericFunction>
```



## ac_runtime_common::builder::fn_lazy

*Function*

Creates a lazy function from a closure.

```rust
fn fn_lazy<F>(f: F) -> Box<dyn LazyFunction>
```



## ac_runtime_common::builder::fn_operation

*Function*

Creates an operation from a signature closure and a perform closure.

```rust
fn fn_operation<S, P>(signature: S, perform: P) -> Box<dyn Operation>
```



