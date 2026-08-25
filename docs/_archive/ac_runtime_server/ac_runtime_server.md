**ac_runtime_server**

# Module: ac_runtime_server

## Contents

**Structs**

- [`ServerHost`](#serverhost) - A host with effects: sandbox modules plus HTTP, OS, and random.
- [`ServerLimits`](#serverlimits) - Resource limits for server-side evaluation.

---

## ac_runtime_server::ServerHost

*Struct*

A host with effects: sandbox modules plus HTTP, OS, and random.

Function lookup checks the built-in [`DefaultHost`] first, then the
module host (pure libs, HTTP, OS, random, KV, etc.).

**Fields:**
- `default: ac_runtime_example_1::DefaultHost`
- `modules: ac_runtime_modules::ModuleHost`
- `limits: ServerLimits`

**Methods:**

- `fn new(limits: ServerLimits) -> Self` - Creates a new server host with the given limits and all enabled modules.
- `fn new_context(self: &Self) -> CanonicalContext` - Creates a [`Context`] with the server limits applied.
- `fn limits(self: &Self) -> &ServerLimits` - Returns the configured limits.
- `fn modules(self: &Self) -> &ModuleHost` - Returns the inner [`ModuleHost`], allowing callers to register
- `fn into_modules(self: Self) -> ModuleHost` - Consumes the host and returns the inner [`ModuleHost`].

**Trait Implementations:**

- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`
  - `fn generic_function(self: &Self, name: &str) -> Option<&dyn GenericFunction>`
- **Default**
  - `fn default() -> Self`
- **Clone**
  - `fn clone(self: &Self) -> ServerHost`



## ac_runtime_server::ServerLimits

*Struct*

Resource limits for server-side evaluation.

Mirrors [`ac_runtime_sandbox::SandboxLimits`] so the same depth/call
guards can be enforced even when effects are enabled.

**Fields:**
- `max_depth: Option<usize>` - Maximum scope depth (function call nesting).
- `max_calls: Option<usize>` - Maximum number of function calls during a single evaluation.

**Trait Implementations:**

- **Default**
  - `fn default() -> Self`
- **Clone**
  - `fn clone(self: &Self) -> ServerLimits`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



