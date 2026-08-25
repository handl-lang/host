**ac_runtime_example_1 > host**

# Module: host

## Contents

**Structs**

- [`DefaultHost`](#defaulthost)

---

## ac_runtime_example_1::host::DefaultHost

*Struct*

**Fields:**
- `var: crate::functions::Var`
- `resolve: crate::functions::Resolve`
- `apply: crate::functions::Apply`

**Trait Implementations:**

- **Host**
  - `fn function(self: &Self, name: &str) -> Option<&dyn Function>`
  - `fn is_reserved(self: &Self, name: &str) -> bool`
- **Default**
  - `fn default() -> Self`
- **Clone**
  - `fn clone(self: &Self) -> DefaultHost`



