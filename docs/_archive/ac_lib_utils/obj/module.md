**handl_lib_utils > obj > module**

# Module: obj::module

## Contents

**Structs**

- [`ObjModule`](#objmodule) - The `obj` module — object utility functions.

---

## handl_lib_utils::obj::module::ObjModule

*Struct*

The `obj` module — object utility functions.

Provides functions callable as `obj.keys(o)`, `obj.get(o, k)`,
`obj.set(o, k, v)`, `obj.has_key(o, k)`, `obj.map_values(fn, o)`,
`obj.deep_get(o, path)`, `obj.deep_merge(a, b)`, etc. in handl expressions.

**Unit Struct**

**Traits:** Copy

**Trait Implementations:**

- **Module**
  - `fn name(self: &Self) -> &str`
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
- **Clone**
  - `fn clone(self: &Self) -> ObjModule`



