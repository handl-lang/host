**handl_lib_utils > type > module**

# Module: type::module

## Contents

**Structs**

- [`TypeModule`](#typemodule) - The `type` module — type coercion and predicate functions.

---

## handl_lib_utils::type::module::TypeModule

*Struct*

The `type` module — type coercion and predicate functions.

Provides functions callable as `type.to_string(v)`, `type.to_number(v)`,
`type.to_bool(v)`, `type.is_null(v)`, `type.is_bool(v)`, etc. in handl
expressions.

# NaN and Infinity

`serde_json::Value` cannot represent NaN or Infinity — they serialize as
`null`. Values that would be NaN or Infinity are therefore handled by the
`null` branch in each function, yielding `"null"`, `0.0`, and `false`
respectively. This is a deliberate graceful-degradation choice, not a bug.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> TypeModule`



