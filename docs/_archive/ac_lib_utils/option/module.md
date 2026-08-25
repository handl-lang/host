**handl_lib_utils > option > module**

# Module: option::module

## Contents

**Structs**

- [`OptionModule`](#optionmodule) - The `option` module — constructors and operations for `Option` values

---

## handl_lib_utils::option::module::OptionModule

*Struct*

The `option` module — constructors and operations for `Option` values
in handl expressions.

An `Option` is represented as `{"_tag": "Some", "value": <T>}` or
`{"_tag": "None"}`.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> OptionModule`
- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`



