**handl_lib_utils > result > module**

# Module: result::module

## Contents

**Structs**

- [`ResultModule`](#resultmodule) - The `result` module — constructors and predicates for `Result` values

---

## handl_lib_utils::result::module::ResultModule

*Struct*

The `result` module — constructors and predicates for `Result` values
in handl expressions.

A `Result` is represented as `{"_tag": "Ok", "value": <T>}` or
`{"_tag": "Err", "error": <E>}`.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> ResultModule`



