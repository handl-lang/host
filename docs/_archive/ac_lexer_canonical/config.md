**handl_lexer_canonical > config**

# Module: config

## Contents

**Structs**

- [`ByteRule`](#byterule)
- [`OperatorRule`](#operatorrule)

**Functions**

- [`canonical_config`](#canonical_config) - Returns the canonical handl expression syntax configuration.

---

## handl_lexer_canonical::config::ByteRule

*Struct*

**Unit Struct**

**Trait Implementations:**

- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, _config: &LexerConfig<CanonicalKind, DefaultLexError>) -> Option<Result<Token<CanonicalKind>, DefaultLexError>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`
- **Default**
  - `fn default() -> ByteRule`
- **Clone**
  - `fn clone(self: &Self) -> ByteRule`



## handl_lexer_canonical::config::OperatorRule

*Struct*

**Unit Struct**

**Methods:**

- `fn two_or_three(chars: & mut std::iter::Peekable<std::str::CharIndices>) -> Option<(usize, usize, &'static str)>`

**Trait Implementations:**

- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, _config: &LexerConfig<CanonicalKind, DefaultLexError>) -> Option<Result<Token<CanonicalKind>, DefaultLexError>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`
- **Default**
  - `fn default() -> OperatorRule`
- **Clone**
  - `fn clone(self: &Self) -> OperatorRule`



## handl_lexer_canonical::config::canonical_config

*Function*

Returns the canonical handl expression syntax configuration.

This is the preset used by [`CanonicalLexer::default`](crate::CanonicalLexer::default).
Custom dialects can start from this and modify it:

```
use handl_lexer_canonical::{canonical_config, CanonicalKind};

let config = canonical_config()
    .with_keyword("await", CanonicalKind::Handle)
    .with_delimiter(';', CanonicalKind::Comma);
```

```rust
fn canonical_config() -> handl_lexer::LexerConfig<crate::kind::CanonicalKind, handl_lexer::DefaultLexError>
```



