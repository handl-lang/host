**handl_lexer > error**

# Module: error

## Contents

**Enums**

- [`DefaultLexError`](#defaultlexerror) - The default, concrete lexing error type.

**Traits**

- [`LexError`](#lexerror) - Trait for unrecoverable lexing errors.

---

## handl_lexer::error::DefaultLexError

*Enum*

The default, concrete lexing error type.

Recoverable errors produce `TokenKind::Error` tokens instead.
This type is for errors where no meaningful token stream can
be produced (e.g. unterminated string literal).

**Variants:**
- `UnterminatedString{ span: crate::Span }` - An unterminated string literal starting at the given span.
- `InvalidNumber{ span: crate::Span }` - An invalid number literal at the given span.
- `UnexpectedChar{ span: crate::Span, char: char }` - An unexpected character at the given span.

**Traits:** Error

**Trait Implementations:**

- **PartialEq**
  - `fn eq(self: &Self, other: &DefaultLexError) -> bool`
- **Clone**
  - `fn clone(self: &Self) -> DefaultLexError`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Display**
  - `fn fmt(self: &Self, __formatter: & mut ::core::fmt::Formatter) -> ::core::fmt::Result`



## handl_lexer::error::LexError

*Trait*

Trait for unrecoverable lexing errors.

Dialects can use the built-in [`DefaultLexError`] or define their own
error type. Rule implementations that use the default primitives need
`E: From<DefaultLexError>` so the primitive errors can be converted.



