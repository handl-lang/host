**handl_lib_otlp > error**

# Module: error

## Contents

**Enums**

- [`OtlpError`](#otlperror) - Errors that can occur inside the OTLP module.

**Type Aliases**

- [`OtlpResult`](#otlpresult) - Result type used inside the OTLP module.

---

## handl_lib_otlp::error::OtlpError

*Enum*

Errors that can occur inside the OTLP module.

**Variants:**
- `NotConfigured` - No exporter endpoint has been configured yet.
- `InvalidArgument{ message: String }` - An argument was malformed or missing.
- `UnknownSpan{ id: String }` - The referenced span id does not exist.
- `Network{ message: String }` - The network request failed.
- `Serialization{ message: String }` - Serialization of the OTLP payload failed.

**Methods:**

- `fn invalid_argument<impl Into<String>>(message: impl Trait) -> Self` - Convenience constructor for [`OtlpError::InvalidArgument`].
- `fn network<impl Into<String>>(message: impl Trait) -> Self` - Convenience constructor for [`OtlpError::Network`].
- `fn serialization<impl Into<String>>(message: impl Trait) -> Self` - Convenience constructor for [`OtlpError::Serialization`].

**Traits:** Error

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, __formatter: & mut ::core::fmt::Formatter) -> ::core::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> OtlpError`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_lib_otlp::error::OtlpResult

*Type Alias*: `Result<T, OtlpError>`

Result type used inside the OTLP module.



