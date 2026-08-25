**ac_runtime_modules > host > host**

# Module: host::host

## Contents

**Structs**

- [`ModuleHost`](#modulehost) - A composite host that dispatches function calls to registered modules.

---

## ac_runtime_modules::host::host::ModuleHost

*Struct*

A composite host that dispatches function calls to registered modules.

`ModuleHost` implements [`Host`] by splitting the function name on the
first `.`. The left segment is the module name; the right segment is
the function name within that module. For example, `math.abs(x)` is
split into module `"math"` and function `"abs"`.

If the name contains no `.`, the host searches all modules in order
and returns the first match. This allows flat names (e.g. `add`) to
coexist with qualified names (e.g. `math.abs`).

# Architecture

```text
           ModuleHost
               │
               │ Host::function("math.abs")
               ▼
         split on "."
          ┌────┴────┐
          ▼         ▼
     "math"     "abs"
          │         │
          ▼         ▼
      Module    Function
```

The VM sees only the `Host` interface — it does not know about modules.

**Fields:**
- `modules: Vec<std::rc::Rc<dyn Module>>`

**Methods:**

- `fn new() -> Self` - Creates a new empty host.
- `fn with<impl Module + 'static>(self: Self, module: impl Trait) -> Self` - Registers a module.
- `fn module_count(self: &Self) -> usize` - Returns the number of registered modules.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ModuleHost`
- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`
  - `fn generic_function(self: &Self, name: &str) -> Option<&dyn GenericFunction>`
  - `fn async_function(self: &Self, name: &str) -> Option<&dyn AsyncFunction>`
  - `fn operation(self: &Self, name: &str) -> Option<&dyn Operation>`
  - `fn is_reserved(self: &Self, name: &str) -> bool`
- **Default**
  - `fn default() -> Self`



