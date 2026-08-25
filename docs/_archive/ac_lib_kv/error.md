**handl_lib_kv > error**

# Module: error

## Contents

**Enums**

- [`KvError`](#kverror) - Errors produced by key-value store implementations.

---

## handl_lib_kv::error::KvError

*Enum*

Errors produced by key-value store implementations.

**Variants:**
- `KeyNotFound{ key: String }`
- `Backend{ message: String }`

**Traits:** Error

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> KvError`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



