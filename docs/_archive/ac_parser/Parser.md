**handl_parser > Parser**

# Module: Parser

## Contents

**Functions**

- [`parse`](#parse) - Parse a token stream into an expression.

---

## handl_parser::Parser::parse

*Function*

Parse a token stream into an expression.

Returns `Err(ParseError)` on syntactic errors (unexpected token,
missing delimiter, etc.).

```rust
fn parse(self: &Self, tokens: Vec<Token<K>>) -> Result<Expr<NonFunctionValue>, ParseError>
```



