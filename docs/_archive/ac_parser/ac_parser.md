**handl_parser**

# Module: handl_parser

## Contents

**Modules**

- [`error`](#error)

**Traits**

- [`Parser`](#parser) - A parser that converts a token stream into an expression AST.

---

## handl_parser::Parser

*Trait*

A parser that converts a token stream into an expression AST.

The parser consumes the token stream produced by a
[`Lexer`](handl_lexer::Lexer) and builds an [`Expr`] tree. Dialect-specific
grammar rules (operator precedence, keyword handling, call syntax) are
implemented by the concrete type.

The parser does not perform type checking or resolution — it produces
a syntactically valid AST only.

**Methods:**

- `parse`: Parse a token stream into an expression.
- `parse_text`: Convenience: lex and parse in one call.



## Module: error



