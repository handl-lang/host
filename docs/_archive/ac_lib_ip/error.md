**handl_lib_ip > error**

# Module: error

## Contents

**Enums**

- [`IpError`](#iperror) - Errors that can occur when parsing or operating on IP values.

---

## handl_lib_ip::error::IpError

*Enum*

Errors that can occur when parsing or operating on IP values.

**Variants:**
- `InvalidAddress`
- `InvalidCidr`
- `InvalidPort`

**Traits:** Error, Eq, Copy

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &IpError) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> IpError`



