**handl_ast_common > metadata**

# Module: metadata

## Contents

**Structs**

- [`Attrs`](#attrs) - Arbitrary attributes attached to a node.
- [`Both`](#both) - Composed metadata: two metadata values side by side.
- [`Span`](#span) - A source span: byte offsets `[start, end)`.

---

## handl_ast_common::metadata::Attrs

*Struct*

Arbitrary attributes attached to a node.

**Generic Parameters:**
- A

**Tuple Struct**: `(A)`

**Traits:** NodeMetadata, Eq

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &Attrs<A>) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> Attrs<A>`
- **Clone**
  - `fn clone(self: &Self) -> Attrs<A>`



## handl_ast_common::metadata::Both

*Struct*

Composed metadata: two metadata values side by side.

Use `Both` to layer `Span` with `Attrs`, two spans, or any other
combination without writing a new struct.

**Generic Parameters:**
- M1
- M2

**Fields:**
- `first: M1`
- `second: M2`

**Traits:** NodeMetadata, Eq

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Both<M1, M2>`
- **PartialEq**
  - `fn eq(self: &Self, other: &Both<M1, M2>) -> bool`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Default**
  - `fn default() -> Both<M1, M2>`



## handl_ast_common::metadata::Span

*Struct*

A source span: byte offsets `[start, end)`.

**Fields:**
- `start: usize`
- `end: usize`

**Traits:** Copy, NodeMetadata, Eq

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &Span) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> Span`
- **Default**
  - `fn default() -> Span`



