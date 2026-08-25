**handl_source > location**

# Module: location

## Contents

**Structs**

- [`SourceLocation`](#sourcelocation) - A location in source text — 1-indexed line and column.

---

## handl_source::location::SourceLocation

*Struct*

A location in source text — 1-indexed line and column.

**Fields:**
- `line: usize` - 1-indexed line number.
- `column: usize` - 1-indexed column number (character offset within the line).

**Methods:**

- `fn new(line: usize, column: usize) -> Self`

**Traits:** Eq, Copy

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &SourceLocation) -> bool`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **Clone**
  - `fn clone(self: &Self) -> SourceLocation`
- **Default**
  - `fn default() -> SourceLocation`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`



