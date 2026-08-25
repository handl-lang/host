**handl_frontend_toml > error**

# Module: error

## Contents

**Enums**

- [`TomlError`](#tomlerror) - Errors that can occur during TOML parsing.

---

## handl_frontend_toml::error::TomlError

*Enum*

Errors that can occur during TOML parsing.

**Variants:**
- `Parse(toml::de::Error)` - The underlying TOML parser returned an error.
- `Serialize(toml::ser::Error)` - The underlying TOML serializer returned an error.

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, __formatter: & mut ::core::fmt::Formatter) -> ::core::fmt::Result`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Error**
  - `fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private20::Error>`
- **From**
  - `fn from(source: toml::ser::Error) -> Self`
- **From**
  - `fn from(source: toml::de::Error) -> Self`



