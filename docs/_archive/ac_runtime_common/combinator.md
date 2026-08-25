**ac_runtime_common > combinator**

# Module: combinator

## Contents

**Structs**

- [`ChainHost`](#chainhost) - A host that queries a list of child hosts in order and returns the
- [`FilterHost`](#filterhost) - A host that wraps another host and filters out names matching a
- [`SpyHost`](#spyhost) - A host that wraps another host and logs every lookup.

---

## ac_runtime_common::combinator::ChainHost

*Struct*

A host that queries a list of child hosts in order and returns the
first match found.

**Tuple Struct**: `(Vec<Box<dyn Host>>)`

**Methods:**

- `fn new() -> Self` - Creates an empty chain host.
- `fn push(self: Self, host: Box<dyn Host>) -> Self` - Adds a child host to the end of the chain.
- `fn with<H>(self: Self, host: H) -> Self` - Adds a child host by value.

**Trait Implementations:**

- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn async_function(self: &Self, name: &str) -> Option<&dyn AsyncFunction>`
  - `fn operation(self: &Self, name: &str) -> Option<&dyn Operation>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`
  - `fn generic_function(self: &Self, name: &str) -> Option<&dyn GenericFunction>`
- **Default**
  - `fn default() -> Self`



## ac_runtime_common::combinator::FilterHost

*Struct*

A host that wraps another host and filters out names matching a
predicate. Useful for forbidding specific functions in a sandbox.

**Fields:**
- `inner: Box<dyn Host>`
- `predicate: Box<dyn Fn>`

**Methods:**

- `fn new<impl Fn(&str) -> bool + 'static>(inner: Box<dyn Host>, predicate: impl Trait) -> Self` - Creates a host that forwards only names allowed by `predicate`.
- `fn block(inner: Box<dyn Host>, names: &[&str]) -> Self` - Creates a host that blocks the given names.

**Trait Implementations:**

- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn async_function(self: &Self, name: &str) -> Option<&dyn AsyncFunction>`
  - `fn operation(self: &Self, name: &str) -> Option<&dyn Operation>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`
  - `fn generic_function(self: &Self, name: &str) -> Option<&dyn GenericFunction>`



## ac_runtime_common::combinator::SpyHost

*Struct*

A host that wraps another host and logs every lookup.

**Fields:**
- `inner: Box<dyn Host>`
- `lookups: std::cell::RefCell<Vec<String>>`

**Methods:**

- `fn new(inner: Box<dyn Host>) -> Self` - Creates a host that records every name it is asked to resolve.
- `fn lookups(self: &Self) -> Vec<String>` - Returns the recorded lookups.

**Trait Implementations:**

- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn async_function(self: &Self, name: &str) -> Option<&dyn AsyncFunction>`
  - `fn operation(self: &Self, name: &str) -> Option<&dyn Operation>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`
  - `fn generic_function(self: &Self, name: &str) -> Option<&dyn GenericFunction>`



