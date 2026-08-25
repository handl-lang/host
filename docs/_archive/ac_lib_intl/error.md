**handl_lib_intl > error**

# Module: error

## Contents

**Enums**

- [`IntlError`](#intlerror)

---

## handl_lib_intl::error::IntlError

*Enum*

**Variants:**
- `InvalidLocale{ input: String }`
- `InvalidNumber{ input: String }`
- `InvalidArgument{ message: String }`
- `InvalidDate{ input: String }`
- `InvalidCurrency{ code: String }`
- `InvalidUnit{ unit: String }`
- `InvalidType{ type_name: String, valid: String }`
- `Formatting{ message: String }`
- `DataProvider{ message: String }`

**Methods:**

- `fn into_vm_error(self: Self) -> VmError`

**Traits:** Error

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> IntlError`



