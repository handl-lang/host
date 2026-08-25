**handl_lib_serialize > error**

# Module: error

## Contents

**Structs**

- [`SerializeError`](#serializeerror) - The shared error type for all serialization failures.

---

## handl_lib_serialize::error::SerializeError

*Struct*

The shared error type for all serialization failures.

**Fields:**
- `reason: String` - Human-readable description of the failure.

**Methods:**

- `fn new<impl Into<String>>(reason: impl Trait) -> Self` - Create a new serialization error.

**Traits:** Error

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, __formatter: & mut ::core::fmt::Formatter) -> ::core::fmt::Result`



