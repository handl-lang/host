**handl_lexer > span**

# Module: span

## Contents

**Structs**

- [`Span`](#span) - A byte range in source text — half-open `[start, end)`.

---

## handl_lexer::span::Span

*Struct*

A byte range in source text — half-open `[start, end)`.

Spans are byte offsets (0-indexed). Convert to [`SourceLocation`]
(1-indexed line:column) when reporting to users.

**Fields:**
- `start: usize` - Start byte offset (inclusive).
- `end: usize` - End byte offset (exclusive).

**Methods:**

- `fn new(start: usize, end: usize) -> Self` - Creates a span from `[start, end)`.
- `fn at(pos: usize) -> Self` - Creates a zero-length span at `pos`.
- `fn len(self: &Self) -> usize` - Returns the length of the span in bytes.
- `fn is_empty(self: &Self) -> bool` - Returns `true` if the span is zero-length.

**Traits:** Eq, Copy

**Trait Implementations:**

- **Default**
  - `fn default() -> Span`
- **Hash**
  - `fn hash<__H>(self: &Self, state: & mut __H)`
- **PartialEq**
  - `fn eq(self: &Self, other: &Span) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Span`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`



