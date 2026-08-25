**handl_lexer > token**

# Module: token

## Contents

**Structs**

- [`Token`](#token) - A lexed token — its kind and source span.

**Traits**

- [`CommentFactory`](#commentfactory) - A factory for constructing and lexing comment token kinds.
- [`CommentTokenKind`](#commenttokenkind) - A token kind that can represent comments.
- [`EofFactory`](#eoffactory) - Factory for constructing end-of-input token kinds.
- [`IdentFactory`](#identfactory) - Factory for constructing and lexing identifier token kinds.
- [`KeywordFactory`](#keywordfactory) - Factory for constructing keyword token kinds.
- [`KindFactory`](#kindfactory) - A convenience trait for factories that implement all common token-kind
- [`NumberFactory`](#numberfactory) - Factory for constructing and lexing number-literal token kinds.
- [`OperatorFactory`](#operatorfactory) - Factory for constructing and lexing operator token kinds.
- [`StringFactory`](#stringfactory) - Factory for constructing and lexing string-literal token kinds.
- [`TokenKind`](#tokenkind) - A token kind — the type of a lexed token.

---

## handl_lexer::token::CommentFactory

*Trait*

A factory for constructing and lexing comment token kinds.

**Methods:**

- `comment_kind`: Construct a comment token kind from the comment text.
- `is_line_comment_start`: Returns `true` if the iterator is positioned at the start of a line
- `lex_line_comment`: Lex a line comment from the current position.
- `is_block_comment_start`: Returns `true` if the iterator is positioned at the start of a block
- `lex_block_comment`: Lex a block comment from the current position.



## handl_lexer::token::CommentTokenKind

*Trait*

A token kind that can represent comments.

Dialects with comment tokens implement this trait. Rules that lex comments
(e.g. `LineCommentRule`) require the token kind to implement this.

**Methods:**

- `is_comment`: Returns `true` if this is a comment token (to be filtered by the lexer).



## handl_lexer::token::EofFactory

*Trait*

Factory for constructing end-of-input token kinds.

**Methods:**

- `eof_kind`: Construct the end-of-input sentinel token kind.



## handl_lexer::token::IdentFactory

*Trait*

Factory for constructing and lexing identifier token kinds.

**Methods:**

- `ident_kind`: Construct an identifier token kind from the raw text.
- `is_ident_start`: Returns `true` if `ch` may start an identifier.
- `is_ident_continue`: Returns `true` if `ch` may follow the first character of an identifier.
- `lex_ident`: Lex an identifier from the current position.



## handl_lexer::token::KeywordFactory

*Trait*

Factory for constructing keyword token kinds.

**Methods:**

- `keyword_kind`: Construct a keyword token kind from the keyword text, if recognized.



## handl_lexer::token::KindFactory

*Trait*

A convenience trait for factories that implement all common token-kind
constructors. Rules can instead require the smaller factory traits they
actually need, so dialects only implement the categories they support.

**Methods:**

- `string_kind`: Construct a string-literal token kind from the unescaped value.
- `number_kind`: Construct a number-literal token kind from the parsed value.
- `ident_kind`: Construct an identifier token kind from the raw text.
- `eof_kind`: Construct the end-of-input sentinel token kind.
- `keyword_kind`: Construct a keyword token kind from the keyword text, if recognized.
- `operator_kind`: Construct an operator token kind from the operator text, if recognized.



## handl_lexer::token::NumberFactory

*Trait*

Factory for constructing and lexing number-literal token kinds.

**Methods:**

- `number_kind`: Construct a number-literal token kind from the parsed value.
- `is_number_start`: Returns `true` if `ch` may start a number literal.
- `lex_number`: Lex a number literal from the current position.



## handl_lexer::token::OperatorFactory

*Trait*

Factory for constructing and lexing operator token kinds.

**Methods:**

- `operator_kind`: Construct an operator token kind from the operator text, if recognized.
- `operator_chars`: The set of characters that may begin an operator.
- `lex_operator`: Lex an operator from the current position.



## handl_lexer::token::StringFactory

*Trait*

Factory for constructing and lexing string-literal token kinds.

**Methods:**

- `string_kind`: Construct a string-literal token kind from the unescaped value.
- `is_string_start`: Returns `true` if `ch` may open a string literal.
- `lex_string`: Lex a string literal from the current position.



## handl_lexer::token::Token

*Struct*

A lexed token — its kind and source span.

**Generic Parameters:**
- K

**Fields:**
- `kind: K` - What kind of token this is.
- `span: crate::Span` - Where this token appears in the source text.

**Methods:**

- `fn new(kind: K, span: Span) -> Self` - Creates a token at the given span.

**Trait Implementations:**

- **Clone**
  - `fn clone(self: &Self) -> Token<K>`
- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **PartialEq**
  - `fn eq(self: &Self, other: &Token<K>) -> bool`



## handl_lexer::token::TokenKind

*Trait*

A token kind — the type of a lexed token.

Each dialect defines its own enum implementing this trait. This allows
compile-time type safety and pattern matching on token kinds without
a closed central enum.

Only `EofFactory` is required; the other category factories are opt-in
and are brought into scope by the rules that need them.

# Implementing

```
use handl_lexer::{TokenKind, EofFactory};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
enum MyKind {
    Identifier(String),
    Number(f64),
    Eof,
}

impl fmt::Display for MyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MyKind::Identifier(s) => write!(f, "identifier({s})"),
            MyKind::Number(n) => write!(f, "number({n})"),
            MyKind::Eof => write!(f, "eof"),
        }
    }
}

#[derive(Clone)]
struct MyFactory;

impl EofFactory<MyKind> for MyFactory {
    fn eof_kind(&self) -> MyKind { MyKind::Eof }
}

impl TokenKind for MyKind {
    type Factory = MyFactory;
    fn is_eof(&self) -> bool { matches!(self, MyKind::Eof) }
    fn is_error(&self) -> bool { false }
}
```

**Methods:**

- `Factory`: The factory type that constructs this token kind from raw lexed content.
- `is_eof`: Returns `true` if this is the end-of-input sentinel token.
- `is_error`: Returns `true` if this is a recoverable error token.



