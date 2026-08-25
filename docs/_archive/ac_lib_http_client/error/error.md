**handl_lib_http_client > error > error**

# Module: error::error

## Contents

**Enums**

- [`HttpError`](#httperror) - Errors produced by HTTP service implementations.

---

## handl_lib_http_client::error::error::HttpError

*Enum*

Errors produced by HTTP service implementations.

Converts to [`ac_runtime::Error`] via `From`, so service methods can return
`Result<T, HttpError>` and the adapter handles the conversion.

**Variants:**
- `Network{ message: String }`
- `InvalidArgument{ message: String }`

**Methods:**

- `fn into_vm_error(self: Self) -> VmError` - Converts into a [`VmError`].

**Traits:** Error

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> HttpError`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



