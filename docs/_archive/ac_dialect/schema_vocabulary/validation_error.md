**handl_dialect > schema_vocabulary > validation_error**

# Module: schema_vocabulary::validation_error

## Contents

**Structs**

- [`ValidationError`](#validationerror) - A validation error.

---

## handl_dialect::schema_vocabulary::validation_error::ValidationError

*Struct*

A validation error.

Carries a human-readable message, a dotted path to the
offending value within the document (empty string for root),
and an optional source location (line:column).

**Fields:**
- `message: String` - Human-readable description of the violation.
- `path: String` - Dotted path to the offending value within the document.
- `location: Option<handl_source::SourceLocation>` - Source location (line:column) of the offending value, if known.

**Traits:** Error

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, f: & mut std::fmt::Formatter) -> std::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &ValidationError) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> ValidationError`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



