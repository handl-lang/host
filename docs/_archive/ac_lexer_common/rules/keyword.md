**handl_lexer_common > rules > keyword**

# Module: rules::keyword

## Contents

**Structs**

- [`KeywordRule`](#keywordrule) - Matches identifiers that the factory's [`KeywordFactory::keyword_kind`] maps

---

## handl_lexer_common::rules::keyword::KeywordRule

*Struct*

Matches identifiers that the factory's [`KeywordFactory::keyword_kind`] maps
to a keyword kind. Non-keyword identifiers are left for other rules.

**Unit Struct**

**Trait Implementations:**

- **Default**
  - `fn default() -> KeywordRule`
- **Clone**
  - `fn clone(self: &Self) -> KeywordRule`
- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, config: &LexerConfig<K, E>) -> Option<Result<Token<K>, E>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`



