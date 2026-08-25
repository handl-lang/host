**ac_runtime_modules > module > module**

# Module: module::module

## Contents

**Traits**

- [`Module`](#module) - A named group of functions.

---

## ac_runtime_modules::module::module::Module

*Trait*

A named group of functions.

A module provides a namespace for host functions. The module name
becomes the first segment of a qualified function name — e.g. a
module named `"math"` providing a function `"abs"` is callable as
`math.abs(x)`.

The `Module` trait is the runtime-side analogue of the language's
namespace concept: named scopes for path lookups. Which modules
exist is a runtime host decision, not a language-level feature.

## Implementing a module

```ignore
struct MathModule;

impl Module for MathModule {
    fn name(&self) -> &str { "math" }
    fn function(&self, name: &str) -> Option<&dyn Function<NonFunctionValue>> {
        match name {
            "abs" => Some(&Abs),
            _ => None,
        }
    }
}
```

**Methods:**

- `name`: The module name — the namespace prefix for qualified calls.
- `function`: Resolve a function name (without the module prefix) to a callable.
- `lazy_function`: Resolve a function name to a lazy callable.
- `generic_function`: Resolve a function name to a [`GenericFunction`] that receives
- `async_function`: Resolve a function name to an [`AsyncFunction`].
- `operation`: Resolve a function name to a typed [`Operation`].



