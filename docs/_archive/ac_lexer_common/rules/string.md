**handl_lexer_common > rules > string**

# Module: rules::string

## Contents

**Structs**

- [`StringRule`](#stringrule) - Matches string literals using the factory's [`StringFactory::is_string_start`]

---

## handl_lexer_common::rules::string::StringRule

*Struct*

Matches string literals using the factory's [`StringFactory::is_string_start`]
and [`StringFactory::lex_string`].

**Unit Struct**

**Trait Implementations:**

- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, config: &LexerConfig<K, E>) -> Option<Result<Token<K>, E>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`
- **Default**
  - `fn default() -> StringRule`
- **Clone**
  - `fn clone(self: &Self) -> StringRule`



