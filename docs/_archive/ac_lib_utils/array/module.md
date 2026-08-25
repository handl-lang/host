**handl_lib_utils > array > module**

# Module: array::module

## Contents

**Structs**

- [`ArrayModule`](#arraymodule) - The `array` module — array utility functions.

---

## handl_lib_utils::array::module::ArrayModule

*Struct*

The `array` module — array utility functions.

Provides functions callable as `array.length(a)`, `array.reverse(a)`,
etc. in handl expressions.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> ArrayModule`
- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn generic_function(self: &Self, name: &str) -> Option<&dyn GenericFunction>`



