**ac_runtime_common > middleware**

# Module: middleware

## Contents

**Structs**

- [`AllowHost`](#allowhost) - A host that only forwards a fixed set of allowed names.
- [`DenyHost`](#denyhost) - A host that blocks a fixed set of names.
- [`HostEvent`](#hostevent) - A host-lookup event.
- [`LoggingHost`](#logginghost) - A host that logs every lookup it forwards.
- [`MapHost`](#maphost) - A host that rewrites lookup names before forwarding them.

**Enums**

- [`HostEventKind`](#hosteventkind) - What was looked up on a host.

---

## ac_runtime_common::middleware::AllowHost

*Struct*

A host that only forwards a fixed set of allowed names.

**Fields:**
- `inner: Box<dyn Host>`
- `allowed: std::collections::HashSet<String>`

**Methods:**

- `fn new<impl Into<String>, impl IntoIterator<Item = impl Into<String>>>(inner: Box<dyn Host>, allowed: impl Trait) -> Self` - Creates a host that only resolves names in `allowed`.

**Trait Implementations:**

- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn async_function(self: &Self, name: &str) -> Option<&dyn AsyncFunction>`
  - `fn operation(self: &Self, name: &str) -> Option<&dyn Operation>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`
  - `fn generic_function(self: &Self, name: &str) -> Option<&dyn GenericFunction>`



## ac_runtime_common::middleware::DenyHost

*Struct*

A host that blocks a fixed set of names.

**Fields:**
- `inner: Box<dyn Host>`
- `denied: std::collections::HashSet<String>`

**Methods:**

- `fn new<impl Into<String>, impl IntoIterator<Item = impl Into<String>>>(inner: Box<dyn Host>, denied: impl Trait) -> Self` - Creates a host that denies names in `denied`.

**Trait Implementations:**

- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn async_function(self: &Self, name: &str) -> Option<&dyn AsyncFunction>`
  - `fn operation(self: &Self, name: &str) -> Option<&dyn Operation>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`
  - `fn generic_function(self: &Self, name: &str) -> Option<&dyn GenericFunction>`



## ac_runtime_common::middleware::HostEvent

*Struct*

A host-lookup event.

**Generic Parameters:**
- 'a

**Fields:**
- `kind: HostEventKind`
- `name: &'a str`
- `found: bool`

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## ac_runtime_common::middleware::HostEventKind

*Enum*

What was looked up on a host.

**Variants:**
- `Function`
- `AsyncFunction`
- `Operation`
- `LazyFunction`
- `GenericFunction`

**Traits:** Copy, Eq

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &HostEventKind) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> HostEventKind`



## ac_runtime_common::middleware::LoggingHost

*Struct*

A host that logs every lookup it forwards.

**Fields:**
- `inner: Box<dyn Host>`
- `log: Box<dyn Fn>`

**Methods:**

- `fn new<impl Fn(&HostEvent<'_>) + 'static>(inner: Box<dyn Host>, logger: impl Trait) -> Self` - Creates a host that logs every lookup to `logger`.
- `fn eprint(inner: Box<dyn Host>) -> Self` - Creates a host that prints every lookup to stderr.

**Trait Implementations:**

- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn async_function(self: &Self, name: &str) -> Option<&dyn AsyncFunction>`
  - `fn operation(self: &Self, name: &str) -> Option<&dyn Operation>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`
  - `fn generic_function(self: &Self, name: &str) -> Option<&dyn GenericFunction>`



## ac_runtime_common::middleware::MapHost

*Struct*

A host that rewrites lookup names before forwarding them.

Useful for namespacing or versioning: `"math.add"` → `"add"`.

**Fields:**
- `inner: Box<dyn Host>`
- `mapper: Box<dyn Fn>`

**Methods:**

- `fn new<impl Fn(&str) -> String + 'static>(inner: Box<dyn Host>, mapper: impl Trait) -> Self` - Creates a host that maps names through `mapper`.
- `fn with_prefix<impl Into<String>>(inner: Box<dyn Host>, prefix: impl Trait) -> Self` - Convenience constructor that prepends a fixed prefix.
- `fn strip_prefix<impl Into<String>>(inner: Box<dyn Host>, prefix: impl Trait) -> Self` - Convenience constructor that strips a fixed prefix.
- `fn map(self: &Self, name: &str) -> String`

**Trait Implementations:**

- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn async_function(self: &Self, name: &str) -> Option<&dyn AsyncFunction>`
  - `fn operation(self: &Self, name: &str) -> Option<&dyn Operation>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`
  - `fn generic_function(self: &Self, name: &str) -> Option<&dyn GenericFunction>`



