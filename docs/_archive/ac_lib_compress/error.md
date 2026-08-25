**handl_lib_compress > error**

# Module: error

## Contents

**Enums**

- [`CompressError`](#compresserror)

---

## handl_lib_compress::error::CompressError

*Enum*

**Variants:**
- `CompressFailed{ message: String }`
- `DecompressFailed{ message: String }`
- `InvalidInput{ message: String }`

**Methods:**

- `fn compress<impl Into<String>>(message: impl Trait) -> Self`
- `fn decompress<impl Into<String>>(message: impl Trait) -> Self`
- `fn invalid_input<impl Into<String>>(message: impl Trait) -> Self`

**Traits:** Error

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, __formatter: & mut ::core::fmt::Formatter) -> ::core::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &CompressError) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> CompressError`



