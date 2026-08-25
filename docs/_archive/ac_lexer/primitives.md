**handl_lexer > primitives**

# Module: primitives

## Contents

**Functions**

- [`is_block_comment_start`](#is_block_comment_start) - Returns `true` if the iterator is positioned at the start of a block comment.
- [`is_ident_continue`](#is_ident_continue) - Returns `true` if `ch` may appear after the first character of an identifier.
- [`is_ident_start`](#is_ident_start)
- [`is_line_comment_start`](#is_line_comment_start) - Returns `true` if the iterator is positioned at the start of a line comment.
- [`is_number_start`](#is_number_start)
- [`is_string_start`](#is_string_start)
- [`lex_block_comment`](#lex_block_comment) - Lexes a `/* block comment */` (assumes `is_block_comment_start` is true).
- [`lex_ident`](#lex_ident) - Lexes an identifier (may include `.` for namespace paths).
- [`lex_line_comment`](#lex_line_comment) - Lexes a `// line comment` (assumes `is_line_comment_start` is true).
- [`lex_number`](#lex_number) - Lexes a number literal (integer or decimal, optionally negative).
- [`lex_string`](#lex_string) - Lexes a `"..."` or `'...'` string literal with backslash escapes.

**Constants**

- [`DEFAULT_OPERATOR_CHARS`](#default_operator_chars) - Canonical single-character operator set.

---

## handl_lexer::primitives::DEFAULT_OPERATOR_CHARS

*Constant*: `&str`

Canonical single-character operator set.



## handl_lexer::primitives::is_block_comment_start

*Function*

Returns `true` if the iterator is positioned at the start of a block comment.

```rust
fn is_block_comment_start(chars: & mut std::iter::Peekable<std::str::CharIndices>) -> bool
```



## handl_lexer::primitives::is_ident_continue

*Function*

Returns `true` if `ch` may appear after the first character of an identifier.

```rust
fn is_ident_continue(ch: char) -> bool
```



## handl_lexer::primitives::is_ident_start

*Function*

```rust
fn is_ident_start(ch: char) -> bool
```



## handl_lexer::primitives::is_line_comment_start

*Function*

Returns `true` if the iterator is positioned at the start of a line comment.

```rust
fn is_line_comment_start(chars: & mut std::iter::Peekable<std::str::CharIndices>) -> bool
```



## handl_lexer::primitives::is_number_start

*Function*

```rust
fn is_number_start(ch: char) -> bool
```



## handl_lexer::primitives::is_string_start

*Function*

```rust
fn is_string_start(ch: char) -> bool
```



## handl_lexer::primitives::lex_block_comment

*Function*

Lexes a `/* block comment */` (assumes `is_block_comment_start` is true).
Returns the comment text and its span, or an error if unterminated.

```rust
fn lex_block_comment(chars: & mut std::iter::Peekable<std::str::CharIndices>) -> Result<(String, crate::Span), crate::DefaultLexError>
```



## handl_lexer::primitives::lex_ident

*Function*

Lexes an identifier (may include `.` for namespace paths).
Returns the raw identifier text and its span.

```rust
fn lex_ident(chars: & mut std::iter::Peekable<std::str::CharIndices>) -> Result<(String, crate::Span), crate::DefaultLexError>
```



## handl_lexer::primitives::lex_line_comment

*Function*

Lexes a `// line comment` (assumes `is_line_comment_start` is true).
Returns the comment text and its span.

```rust
fn lex_line_comment(chars: & mut std::iter::Peekable<std::str::CharIndices>) -> (String, crate::Span)
```



## handl_lexer::primitives::lex_number

*Function*

Lexes a number literal (integer or decimal, optionally negative).
Returns the parsed f64 value and its span.

```rust
fn lex_number(chars: & mut std::iter::Peekable<std::str::CharIndices>) -> Result<(f64, crate::Span), crate::DefaultLexError>
```



## handl_lexer::primitives::lex_string

*Function*

Lexes a `"..."` or `'...'` string literal with backslash escapes.
Returns the unescaped value and its span.

```rust
fn lex_string(chars: & mut std::iter::Peekable<std::str::CharIndices>) -> Result<(String, crate::Span), crate::DefaultLexError>
```



