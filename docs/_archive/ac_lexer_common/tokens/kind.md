**handl_lexer_common > tokens > kind**

# Module: tokens::kind

## Contents

**Enums**

- [`MockKind`](#mockkind) - A mock token kind covering all common lexer token categories.

---

## handl_lexer_common::tokens::kind::MockKind

*Enum*

A mock token kind covering all common lexer token categories.

**Variants:**
- `Ident(String)`
- `Keyword(String)`
- `Num(f64)`
- `Str(String)`
- `Bool(bool)`
- `Null`
- `Comma`
- `Colon`
- `Comment(String)`
- `Delim(char)`
- `Operator(String)`
- `Custom(String)`
- `Eof`

**Trait Implementations:**

- **CommentTokenKind**
  - `fn is_comment(self: &Self) -> bool`
- **TokenKind**
  - `fn is_eof(self: &Self) -> bool`
  - `fn is_error(self: &Self) -> bool`
- **Display**
  - `fn fmt(self: &Self, f: & mut fmt::Formatter) -> fmt::Result`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &MockKind) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> MockKind`



