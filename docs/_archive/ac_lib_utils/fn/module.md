**handl_lib_utils > fn > module**

# Module: fn::module

## Contents

**Structs**

- [`FnModule`](#fnmodule) - The `fn` module — function combinator utilities.

---

## handl_lib_utils::fn::module::FnModule

*Struct*

The `fn` module — function combinator utilities.

Provides functions callable as `fn.pipe(v, f, g)`, `fn.compose(f, g)`,
etc. in handl expressions.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> FnModule`
- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn lazy_function(self: &Self, name: &str) -> Option<&dyn LazyFunction>`



