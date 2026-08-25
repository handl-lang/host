**handl_parser > error**

# Module: error

## Contents

**Enums**

- [`ParseError`](#parseerror) - A parsing error — the AST could not be constructed.

---

## handl_parser::error::ParseError

*Enum*

A parsing error — the AST could not be constructed.

Carries a span and message for diagnostic reporting. The span
may be zero-length for errors at a specific position (e.g.
unexpected end of input).

**Variants:**
- `UnexpectedToken{ span: handl_lexer::Span, expected: String }` - An unexpected token was encountered.
- `UnexpectedEof{ span: handl_lexer::Span, expected: String }` - An unexpected end of input.
- `LexError{ message: String }` - A lexing error occurred during `parse_text`.

**Traits:** Error

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &ParseError) -> bool`
- **Display**
  - `fn fmt(self: &Self, __formatter: & mut ::core::fmt::Formatter) -> ::core::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> ParseError`



