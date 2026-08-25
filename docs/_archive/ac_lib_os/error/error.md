**handl_lib_os > error > error**

# Module: error::error

## Contents

**Enums**

- [`OsError`](#oserror) - Errors produced by OS service implementations.

---

## handl_lib_os::error::error::OsError

*Enum*

Errors produced by OS service implementations.

Converts to [`ac_runtime::Error`] via `From`, so service methods can return
`Result<T, OsError>` and the adapter handles the conversion.

**Variants:**
- `NotFound{ path: String }`
- `PermissionDenied{ path: String }`
- `AlreadyExists{ path: String }`
- `InvalidArgument{ message: String }`
- `Io{ message: String }`

**Methods:**

- `fn into_vm_error(self: Self) -> VmError` - Converts into a [`VmError`].

**Traits:** Error

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> OsError`



