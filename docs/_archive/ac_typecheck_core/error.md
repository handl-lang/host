**handl_typecheck_core > error**

# Module: error

## Contents

**Structs**

- [`CallCheckError`](#callcheckerror) - A type error detected during function call checking.
- [`TypeError`](#typeerror) - A type error detected during static checking.

**Enums**

- [`CallErrorKind`](#callerrorkind) - The kind of call checking error.

**Type Aliases**

- [`TypeResult`](#typeresult) - Result type for typechecking — collects multiple errors.

---

## handl_typecheck_core::error::CallCheckError

*Struct*

A type error detected during function call checking.

Produced when argument types don't match a function's parameter types,
including generic unification failures.

**Fields:**
- `function: String` - The function name being called.
- `kind: CallErrorKind` - What went wrong.

**Methods:**

- `fn to_type_error(self: &Self) -> TypeError` - Build a `TypeError` from this call error.

**Traits:** Eq

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &CallCheckError) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> CallCheckError`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_typecheck_core::error::CallErrorKind

*Enum*

The kind of call checking error.

**Variants:**
- `ArgCount{ expected: usize, got: usize }` - Wrong number of arguments.
- `ArgType{ param: String, index: usize, expected: crate::Type, actual: crate::Type }` - Argument type incompatible with parameter type (monomorphic).
- `UnificationFailed{ param: String, index: usize, expected: crate::Type, actual: crate::Type }` - Generic unification failed — a type variable couldn't be bound

**Traits:** Eq

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &CallErrorKind) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> CallErrorKind`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



## handl_typecheck_core::error::TypeError

*Struct*

A type error detected during static checking.

**Fields:**
- `field: String` - The field name where the error was detected.
- `expected: Option<crate::Type>` - The expected type, if known.
- `actual: Option<crate::Type>` - The actual (inferred) type, if known.
- `message: String` - Human-readable explanation.
- `path: String` - Dotted path to the location within the document (e.g. `config.server`).
- `location: Option<handl_source::SourceLocation>` - Source location (line:column) of the offending value, if known.

**Methods:**

- `fn mismatch<impl Into<String>, impl Into<String>>(field: impl Trait, expected: Type, actual: Type, path: impl Trait) -> Self` - Create a type mismatch error.
- `fn custom<impl Into<String>, impl Into<String>, impl Into<String>>(field: impl Trait, message: impl Trait, path: impl Trait) -> Self` - Create a generic error with a custom message.
- `fn display(self: &Self) -> String` - Format the error for display.

**Traits:** Eq, Error

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &TypeError) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> TypeError`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, f: & mut std::fmt::Formatter) -> std::fmt::Result`



## handl_typecheck_core::error::TypeResult

*Type Alias*: `Result<T, Vec<TypeError>>`

Result type for typechecking — collects multiple errors.



