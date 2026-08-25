**handl_lib_rand > error**

# Module: error

## Contents

**Enums**

- [`RandError`](#randerror)

---

## handl_lib_rand::error::RandError

*Enum*

**Variants:**
- `InvalidRange{ min: f64, max: f64 }`
- `EmptyArray`
- `InvalidSampleCount{ requested: usize, available: usize }`
- `WeightsLengthMismatch{ items: usize, weights: usize }`
- `NonPositiveWeights`
- `NonNumericWeight{ index: usize }`
- `InvalidStringLength{ length: i64 }`
- `InvalidByteLength{ length: i64 }`
- `InvalidCharset{ name: String }`

**Methods:**

- `fn into_vm_error(self: Self) -> VmError`

**Traits:** Error

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> RandError`



