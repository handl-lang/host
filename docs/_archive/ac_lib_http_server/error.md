**handl_lib_http_server > error**

# Module: error

## Contents

**Enums**

- [`HttpServerError`](#httpservererror) - Errors produced by HTTP server implementations.

---

## handl_lib_http_server::error::HttpServerError

*Enum*

Errors produced by HTTP server implementations.

**Variants:**
- `Network{ message: String }`
- `InvalidArgument{ message: String }`
- `NotRunning`
- `AlreadyRunning`
- `Handler{ message: String }`
- `NotFound`
- `PayloadTooLarge{ limit: usize }`
- `Timeout{ message: String }`

**Methods:**

- `fn into_vm_error(self: Self) -> VmError`

**Traits:** Error

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> HttpServerError`



