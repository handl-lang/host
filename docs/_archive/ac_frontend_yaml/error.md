**handl_frontend_yaml > error**

# Module: error

## Contents

**Enums**

- [`YamlError`](#yamlerror) - Errors that can occur during YAML parsing.

---

## handl_frontend_yaml::error::YamlError

*Enum*

Errors that can occur during YAML parsing.

**Variants:**
- `Parse(serde_yaml::Error)` - The underlying YAML parser returned an error.
- `Convert(String)` - Conversion to the core value model failed.

**Trait Implementations:**

- **From**
  - `fn from(source: serde_yaml::Error) -> Self`
- **Display**
  - `fn fmt(self: &Self, __formatter: & mut ::core::fmt::Formatter) -> ::core::fmt::Result`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Error**
  - `fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private20::Error>`



