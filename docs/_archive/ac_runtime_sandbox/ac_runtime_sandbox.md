**ac_runtime_sandbox**

# Module: ac_runtime_sandbox

## Contents

**Structs**

- [`SandboxHost`](#sandboxhost) - A sandboxed host with only pure library modules and resource limits.
- [`SandboxLimits`](#sandboxlimits) - Resource limits for sandboxed evaluation.

---

## ac_runtime_sandbox::SandboxHost

*Struct*

A sandboxed host with only pure library modules and resource limits.

Wraps [`DefaultHost`] (built-in functions) and a [`ModuleHost`] pre-loaded
with every pure library module. Function lookup checks built-ins first,
then library modules. Use [`new_context`](Self::new_context) to obtain a
[`Context`] with the configured [`SandboxLimits`] applied.

**Fields:**
- `default: ac_runtime_example_1::DefaultHost`
- `modules: ac_runtime_modules::ModuleHost`
- `limits: SandboxLimits`

**Methods:**

- `fn new(limits: SandboxLimits) -> Self` - Creates a new sandbox host with the given limits and all pure
- `fn new_context(self: &Self) -> CanonicalContext` - Creates a [`Context`] with the sandbox limits applied.
- `fn limits(self: &Self) -> &SandboxLimits` - Returns the configured limits.
- `fn modules(self: &Self) -> &ModuleHost` - Returns the inner [`ModuleHost`], allowing callers to register
- `fn into_modules(self: Self) -> ModuleHost` - Consumes the host and returns the inner [`ModuleHost`],

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> SandboxHost`
- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`
  - `fn generic_function(self: &Self, name: &str) -> Option<&dyn GenericFunction>`
  - `fn is_reserved(self: &Self, name: &str) -> bool`
- **Default**
  - `fn default() -> Self`



## ac_runtime_sandbox::SandboxLimits

*Struct*

Resource limits for sandboxed evaluation.

Configured on the [`SandboxHost`] and applied to every [`Context`]
created via [`SandboxHost::new_context`]. The VM enforces these
limits during evaluation — see [`Error::ResourceLimit`](ac_runtime::Error::ResourceLimit).

**Fields:**
- `max_depth: Option<usize>` - Maximum scope depth (function call nesting).
- `max_calls: Option<usize>` - Maximum number of function calls during a single evaluation.

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> Self`
- **Clone**
  - `fn clone(self: &Self) -> SandboxLimits`



