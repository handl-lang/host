**handl_lexer_common > rules > ident**

# Module: rules::ident

## Contents

**Structs**

- [`IdentRule`](#identrule) - Matches identifiers and keywords (identifiers in the keyword table

---

## handl_lexer_common::rules::ident::IdentRule

*Struct*

Matches identifiers and keywords (identifiers in the keyword table
are remapped to their token kind).

**Unit Struct**

**Trait Implementations:**

- **LexRule**
  - `fn try_lex(self: &Self, chars: & mut std::iter::Peekable<std::str::CharIndices>, config: &LexerConfig<K, E>) -> Option<Result<Token<K>, E>>`
  - `fn name(self: &Self) -> &'static str`
  - `fn clone_box(self: &Self) -> Box<dyn LexRule>`
- **Default**
  - `fn default() -> IdentRule`
- **Clone**
  - `fn clone(self: &Self) -> IdentRule`



