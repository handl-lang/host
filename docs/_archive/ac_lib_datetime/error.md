**handl_lib_datetime > error**

# Module: error

## Contents

**Enums**

- [`DatetimeError`](#datetimeerror)

---

## handl_lib_datetime::error::DatetimeError

*Enum*

**Variants:**
- `InvalidDatetime{ input: String }`
- `InvalidUnixTimestamp{ value: f64 }`
- `InvalidTimezoneOffset{ offset: String }`
- `InvalidDateComponents{ year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32 }`
- `YearOverflow{ year: i64 }`
- `InvalidDateAfterArithmetic{ year: i64, month: i64, day: u32 }`
- `ParseFailed{ input: String, format: String }`

**Methods:**

- `fn into_vm_error(self: Self) -> VmError`

**Traits:** Error

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> DatetimeError`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`



