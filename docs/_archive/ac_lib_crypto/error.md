**handl_lib_crypto > error**

# Module: error

## Contents

**Enums**

- [`CryptoError`](#cryptoerror)

---

## handl_lib_crypto::error::CryptoError

*Enum*

**Variants:**
- `InvalidInput{ message: String }`
- `InvalidByteValue{ value: f64 }`
- `DecodeError{ message: String }`
- `InvalidArgument{ message: String }`

**Methods:**

- `fn into_vm_error(self: Self) -> VmError`

**Traits:** Error

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> CryptoError`



