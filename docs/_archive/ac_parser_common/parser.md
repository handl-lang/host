**handl_parser_common > parser**

# Module: parser

## Contents

**Structs**

- [`Ast`](#ast)
- [`Parser`](#parser)
- [`ParserContext`](#parsercontext)

---

## handl_parser_common::parser::Ast

*Struct*

**Generic Parameters:**
- N

**Fields:**
- `source: String`
- `root: N`

**Trait Implementations:**

- **Debug**
  - `fn fmt(self: &Self, f: & mut $crate::fmt::Formatter) -> $crate::fmt::Result`
- **Clone**
  - `fn clone(self: &Self) -> Ast<N>`



## handl_parser_common::parser::Parser

*Struct*

**Generic Parameters:**
- K
- N
- E

**Fields:**
- `grammar: std::sync::Arc<dyn Grammar>`

**Methods:**

- `fn new<G>(grammar: G) -> Self`
- `fn parse(self: &Self, tokens: Vec<Token<K>>, source: &str) -> Result<Ast<N>, E>`



## handl_parser_common::parser::ParserContext

*Struct*

**Generic Parameters:**
- K
- N
- E

**Fields:**
- `tokens: Vec<handl_lexer::Token<K>>`
- `source: String`
- `pos: usize`
- `grammar: std::sync::Arc<dyn Grammar>`

**Methods:**

- `fn tokens(self: &Self) -> &[Token<K>]`
- `fn source(self: &Self) -> &str`
- `fn pos(self: &Self) -> usize`
- `fn is_at_end(self: &Self) -> bool`
- `fn peek(self: &Self) -> Option<&Token<K>>`
- `fn peek_kind(self: &Self) -> Option<K>`
- `fn peek_span(self: &Self) -> Span`
- `fn advance(self: & mut Self) -> Result<&Token<K>, E>`
- `fn match_kind(self: & mut Self, kind: K) -> Option<&Token<K>>`
- `fn expect(self: & mut Self, kind: K, label: &str) -> Result<&Token<K>, E>`
- `fn expression(self: & mut Self, min_bp: u32) -> Result<N, E>`



