**handl_lexer_canonical > lexer**

# Module: lexer

## Contents

**Structs**

- [`CanonicalLexer`](#canonicallexer) - The canonical handl expression lexer.
- [`TokenIter`](#tokeniter) - Streaming token iterator backed by the canonical lexing engine.

---

## handl_lexer_canonical::lexer::CanonicalLexer

*Struct*

The canonical handl expression lexer.

Stateless — safe to reuse across inputs. Uses [`canonical_config()`]
(the canonical handl syntax) unless configured otherwise.

**Fields:**
- `config: handl_lexer::LexerConfig<crate::kind::CanonicalKind, handl_lexer::DefaultLexError>`

**Methods:**

- `fn with_config(config: LexerConfig<CanonicalKind, DefaultLexError>) -> Self` - Creates a lexer with a custom configuration.

**Trait Implementations:**

- **Lexer**
  - `fn lex(self: &Self, input: &str) -> Result<Vec<Token<CanonicalKind>>, DefaultLexError>`
  - `fn lex_iter<'a>(self: &'a Self, input: &'a str) -> Box<dyn Iterator>`
- **Default**
  - `fn default() -> Self`
- **Clone**
  - `fn clone(self: &Self) -> CanonicalLexer`



## handl_lexer_canonical::lexer::TokenIter

*Struct*

Streaming token iterator backed by the canonical lexing engine.

Yields `Result<Token<CanonicalKind>, DefaultLexError>` one at a time. Always
emits a final EOF token before returning `None`. On error, yields `Err` and
stops.

**Generic Parameters:**
- 'a

**Fields:**
- `chars: std::iter::Peekable<std::str::CharIndices<'a>>`
- `config: &'a handl_lexer::LexerConfig<crate::kind::CanonicalKind, handl_lexer::DefaultLexError>`
- `input_len: usize`
- `done: bool`

**Trait Implementations:**

- **Iterator**
  - `fn next(self: & mut Self) -> Option<<Self as >::Item>`



