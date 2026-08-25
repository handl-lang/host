**handl_lexer > lexer**

# Module: lexer

## Contents

**Traits**

- [`Lexer`](#lexer) - A lexer that converts expression text into a token stream.

---

## handl_lexer::lexer::Lexer

*Trait*

A lexer that converts expression text into a token stream.

The batch method [`lex`](Lexer::lex) is the primary interface and is
object-safe (usable via `&dyn Lexer`). The streaming method
[`lex_iter`](Lexer::lex_iter) returns tokens one at a time via
`Box<dyn Iterator>` — it has a default implementation that collects
from `lex`, but concrete implementations can override it with a true
streaming iterator.

The lexer does not build structure — structural decisions (nesting,
precedence, argument grouping) are the parser's job.

Dialect-specific behavior (which characters open calls, what
keywords exist, literal syntax) is configured by the implementing
type, typically using a [`LexerConfig`](crate::LexerConfig) preset
from a dialect crate.

**Methods:**

- `lex`: Lex `input` into a flat list of tokens.
- `lex_iter`: Lex `input` into a streaming iterator of tokens.



