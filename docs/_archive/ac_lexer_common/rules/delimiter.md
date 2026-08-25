**handl_lexer_common > rules > delimiter**

# Module: rules::delimiter

## Contents

**Structs**

- [`DelimiterRule`](#delimiterrule) - Matches single-character delimiters from the config's delimiter set.

---

## handl_lexer_common::rules::delimiter::DelimiterRule

*Struct*

Matches single-character delimiters from the config's delimiter set.

**Unit Struct**

**Trait Implementations:**

- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, config: &LexerConfig<K, E>) -> Option<Result<Token<K>, E>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`
- **Default**
  - `fn default() -> DelimiterRule`
- **Clone**
  - `fn clone(self: &Self) -> DelimiterRule`



