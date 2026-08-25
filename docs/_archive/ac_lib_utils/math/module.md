**handl_lib_utils > math > module**

# Module: math::module

## Contents

**Structs**

- [`MathModule`](#mathmodule) - The `math` module — numeric utility functions.

---

## handl_lib_utils::math::module::MathModule

*Struct*

The `math` module — numeric utility functions.

Provides functions callable as `math.abs(x)`, `math.floor(x)`, etc.
in handl expressions.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> MathModule`



