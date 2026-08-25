**handl_lexer > lexer > Lexer**

# Module: lexer::Lexer

## Contents

**Functions**

- [`lex`](#lex) - Lex `input` into a flat list of tokens.
- [`lex_iter`](#lex_iter) - Lex `input` into a streaming iterator of tokens.

---

## handl_lexer::lexer::Lexer::lex

*Function*

Lex `input` into a flat list of tokens.

Returns `Err(E)` on unrecoverable input (e.g. unterminated
string literal). Implementations may also emit error tokens
for recoverable errors, allowing the parser to attempt
recovery with diagnostic context.

```rust
fn lex(self: &Self, input: &str) -> Result<Vec<Token<K>>, E>
```



## handl_lexer::lexer::Lexer::lex_iter

*Function*

Lex `input` into a streaming iterator of tokens.

Each call to `next` yields one `Result<Token<K>, E>`. The
iterator always emits a final EOF token before exhausting.
On unrecoverable input (e.g. unterminated string), an `Err` is
yielded and the iterator stops.

The default implementation calls [`lex`](Self::lex) and wraps the
result in an iterator — not truly streaming. Concrete
implementations should override this for real incremental
tokenization.

```rust
fn lex_iter<'a>(self: &'a Self, input: &'a str) -> Box<dyn Iterator>
```



