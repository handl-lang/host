**handl_lib_collections > error**

# Module: error

## Contents

**Enums**

- [`CollectionsError`](#collectionserror)

---

## handl_lib_collections::error::CollectionsError

*Enum*

**Variants:**
- `Empty`
- `OutOfBounds{ index: i64, length: usize }`

**Traits:** Error

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> CollectionsError`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



