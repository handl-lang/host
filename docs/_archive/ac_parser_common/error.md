**handl_parser_common > error**

# Module: error

## Contents

**Enums**

- [`DefaultParseError`](#defaultparseerror)

**Traits**

- [`ParseError`](#parseerror) - An extensible parser error.

---

## handl_parser_common::error::DefaultParseError

*Enum*

**Variants:**
- `UnexpectedEof{ span: handl_lexer::Span, expected: String }`
- `UnexpectedToken{ span: handl_lexer::Span, expected: String }`
- `Message{ span: handl_lexer::Span, message: String }`

**Traits:** Error

**Trait Implementations:**

- **Display**
  - `fn fmt(self: &Self, __formatter: & mut ::core::fmt::Formatter) -> ::core::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> DefaultParseError`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **ParseError**
  - `fn unexpected_eof<S>(span: Span, expected: S) -> Self`
  - `fn unexpected_token<S>(span: Span, expected: S) -> Self`
  - `fn message<S>(span: Span, message: S) -> Self`



## handl_parser_common::error::ParseError

*Trait*

An extensible parser error.

Dialects can implement this trait with their own error type and pass it
as `E` to `Parser`, `Grammar`, and `Rule`. The common engine calls the
constructor methods when it encounters unexpected tokens or EOF.

**Methods:**

- `unexpected_eof`
- `unexpected_token`
- `message`



