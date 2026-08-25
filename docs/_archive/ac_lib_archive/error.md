**handl_lib_archive > error**

# Module: error

## Contents

**Enums**

- [`ArchiveError`](#archiveerror)

---

## handl_lib_archive::error::ArchiveError

*Enum*

**Variants:**
- `PackFailed{ message: String }`
- `UnpackFailed{ message: String }`
- `InvalidPath{ message: String }`

**Methods:**

- `fn pack<impl Into<String>>(message: impl Trait) -> Self`
- `fn unpack<impl Into<String>>(message: impl Trait) -> Self`
- `fn invalid_path<impl Into<String>>(message: impl Trait) -> Self`

**Traits:** Error

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, __formatter: & mut ::core::fmt::Formatter) -> ::core::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &ArchiveError) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> ArchiveError`



