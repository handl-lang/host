**handl_lexer_common > rules > number**

# Module: rules::number

## Contents

**Structs**

- [`NumberRule`](#numberrule) - Matches number literals using the factory's [`NumberFactory::is_number_start`]

---

## handl_lexer_common::rules::number::NumberRule

*Struct*

Matches number literals using the factory's [`NumberFactory::is_number_start`]
and [`NumberFactory::lex_number`].

**Unit Struct**

**Trait Implementations:**

- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, config: &LexerConfig<K, E>) -> Option<Result<Token<K>, E>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`
- **Default**
  - `fn default() -> NumberRule`
- **Clone**
  - `fn clone(self: &Self) -> NumberRule`



