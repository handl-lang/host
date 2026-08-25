**ac_runtime_common > mock**

# Module: mock

## Contents

**Structs**

- [`CaptureFn`](#capturefn) - A function that records every call it receives.
- [`MockHost`](#mockhost) - A host built from maps of named callables.
- [`NoopHost`](#noophost) - A host that resolves nothing.

---

## ac_runtime_common::mock::CaptureFn

*Struct*

A function that records every call it receives.

**Generic Parameters:**
- F

**Fields:**
- `inner: F`
- `calls: std::cell::RefCell<Vec<Vec<ac_runtime_values::NonFunctionValue>>>`

**Methods:**

- `fn new(inner: F) -> Self` - Wraps a function, capturing each set of arguments.
- `fn calls(self: &Self) -> Vec<Vec<NonFunctionValue>>` - Returns the captured argument lists.

**Trait Implementations:**

- **Function**
  - `fn call(self: &Self, args: &[NonFunctionValue], ctx: & mut dyn ac_runtime::Context) -> Result<NonFunctionValue, ac_runtime::Error<NonFunctionValue>>`



## ac_runtime_common::mock::MockHost

*Struct*

A host built from maps of named callables.

**Fields:**
- `functions: std::collections::HashMap<String, Box<dyn Function>>`
- `async_functions: std::collections::HashMap<String, Box<dyn AsyncFunction>>`
- `operations: std::collections::HashMap<String, Box<dyn Operation>>`
- `lazy_functions: std::collections::HashMap<String, Box<dyn LazyFunction>>`
- `generic_functions: std::collections::HashMap<String, Box<dyn GenericFunction>>`

**Methods:**

- `fn new() -> Self` - Creates an empty mock host.
- `fn with_function<impl Into<String>>(self: Self, name: impl Trait, f: Box<dyn Function>) -> Self` - Registers a synchronous function.
- `fn with_async<impl Into<String>>(self: Self, name: impl Trait, f: Box<dyn AsyncFunction>) -> Self` - Registers an async function.
- `fn with_operation<impl Into<String>>(self: Self, name: impl Trait, f: Box<dyn Operation>) -> Self` - Registers an operation.
- `fn with_lazy<impl Into<String>>(self: Self, name: impl Trait, f: Box<dyn LazyFunction>) -> Self` - Registers a lazy function.
- `fn with_generic<impl Into<String>>(self: Self, name: impl Trait, f: Box<dyn GenericFunction>) -> Self` - Registers a generic function.

**Trait Implementations:**

- **Default**
  - `fn default() -> MockHost`
- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn async_function(self: &Self, name: &str) -> Option<&dyn AsyncFunction>`
  - `fn operation(self: &Self, name: &str) -> Option<&dyn Operation>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`
  - `fn generic_function(self: &Self, name: &str) -> Option<&dyn GenericFunction>`



## ac_runtime_common::mock::NoopHost

*Struct*

A host that resolves nothing.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Host**
  - `fn function(self: &Self, _name: &str) -> Option<&dyn Function>`
  - `fn async_function(self: &Self, _name: &str) -> Option<&dyn AsyncFunction>`
  - `fn operation(self: &Self, _name: &str) -> Option<&dyn Operation>`
  - `fn lazy_function(self: &Self, _name: &str) -> Option<&dyn LazyFunction>`
  - `fn generic_function(self: &Self, _name: &str) -> Option<&dyn GenericFunction>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> NoopHost`
- **Clone**
  - `fn clone(self: &Self) -> NoopHost`



