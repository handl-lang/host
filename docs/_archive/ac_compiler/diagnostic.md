**handl_compiler > diagnostic**

# Module: diagnostic

## Contents

**Structs**

- [`Diagnostic`](#diagnostic) - A structured diagnostic message.
- [`DiagnosticSink`](#diagnosticsink) - Collects diagnostics emitted during compilation.

**Enums**

- [`Severity`](#severity) - Severity of a diagnostic.

---

## handl_compiler::diagnostic::Diagnostic

*Struct*

A structured diagnostic message.

**Fields:**
- `severity: Severity`
- `code: Option<String>`
- `message: String`
- `location: Option<handl_source::SourceLocation>`
- `notes: Vec<String>`

**Methods:**

- `fn error<impl Into<String>>(message: impl Trait) -> Self`
- `fn warning<impl Into<String>>(message: impl Trait) -> Self`
- `fn info<impl Into<String>>(message: impl Trait) -> Self`
- `fn hint<impl Into<String>>(message: impl Trait) -> Self`
- `fn with_code<impl Into<String>>(self: Self, code: impl Trait) -> Self`
- `fn with_location(self: Self, location: SourceLocation) -> Self`
- `fn with_note<impl Into<String>>(self: Self, note: impl Trait) -> Self`

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> Diagnostic`



## handl_compiler::diagnostic::DiagnosticSink

*Struct*

Collects diagnostics emitted during compilation.

**Fields:**
- `diagnostics: Vec<Diagnostic>`

**Methods:**

- `fn new() -> Self`
- `fn push(self: & mut Self, diagnostic: Diagnostic)`
- `fn extend<impl IntoIterator<Item = Diagnostic>>(self: & mut Self, other: impl Trait)`
- `fn iter(self: &Self) -> impl Trait`
- `fn has_errors(self: &Self) -> bool`
- `fn errors(self: &Self) -> impl Trait`
- `fn warnings(self: &Self) -> impl Trait`
- `fn len(self: &Self) -> usize`
- `fn is_empty(self: &Self) -> bool`
- `fn into_iter(self: Self) -> impl Trait`

**Trait Implementations:**

- **IntoIterator**
  - `fn into_iter(self: Self) -> <Self as >::IntoIter`
- **Default**
  - `fn default() -> DiagnosticSink`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> DiagnosticSink`



## handl_compiler::diagnostic::Severity

*Enum*

Severity of a diagnostic.

**Variants:**
- `Error`
- `Warning`
- `Info`
- `Hint`

**Traits:** Eq, Copy

**Trait Implementations:**

- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &Severity) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> Severity`



