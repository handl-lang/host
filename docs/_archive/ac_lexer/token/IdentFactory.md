**handl_lexer > token > IdentFactory**

# Module: token::IdentFactory

## Contents

**Functions**

- [`is_ident_continue`](#is_ident_continue) - Returns `true` if `ch` may follow the first character of an identifier.

---

## handl_lexer::token::IdentFactory::is_ident_continue

*Function*

Returns `true` if `ch` may follow the first character of an identifier.

The default implementation allows alphanumeric characters, `_`, and `.`.

```rust
fn is_ident_continue(self: &Self, ch: char) -> bool
```



